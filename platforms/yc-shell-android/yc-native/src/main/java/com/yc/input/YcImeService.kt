package com.yc.input

import android.app.AlertDialog
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.util.Log
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import com.yc.input.handwriting.PaddleOcrEngine
import com.yc.input.native.ArenaCommand
import com.yc.input.native.YcNative
import com.yc.input.speech.SpeechHost
import com.yc.input.ui.CandidateItem
import com.yc.input.ui.HandwritingPad
import com.yc.input.ui.KeyAction
import com.yc.input.ui.KeyDef
import com.yc.input.ui.KeyboardSnapshot
import com.yc.input.ui.LangOption
import com.yc.input.ui.LayoutLoader
import com.yc.input.ui.ModeOption
import com.yc.input.ui.PhraseDeckPanel
import com.yc.input.ui.SkinRegistry
import com.yc.input.ui.ThemeTokens
import com.yc.input.ui.YcKeyboardPanel
import android.content.Intent
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
/**
 * Android IME shell.
 *
 * 退格优先级：
 * 1) 组字中（有拼音缓存）→ 引擎 Backspace 缩拼音
 * 2) 已上屏 / 无缓存 → deleteSurroundingText 删正文
 *
 * 选词后必须：结束 IC composing 区间 + 禁止再把引擎 composing 同步回 IC，
 * 否则退格会先 invisible 地删残留 span（表现为连按多次无反应）。
 */
class YcImeService : InputMethodService() {

    private var editorId: Long = 0
    private var clientSeq: Long = 0
    private var lastSeq: Long = -1
    private var lastComposing: String = ""
    private var lastCandidates: List<CandidateItem> = emptyList()
    private var lastCandPage: Int = 0
    private var lastTotalPages: Int = 0
    private var asciiMode: Boolean = false
    private var candExpanded: Boolean = false
    private var expandedCandidates: MutableList<CandidateItem> = mutableListOf()
    private var panel: YcKeyboardPanel? = null
    private var coreInited = false
    private var currentLayoutId: String = "layout_pinyin26"
    /** 当前输入语言：zh / en / vi / th（en 默认 en-v1 预测；密码框仍可 ascii 直通） */
    private var currentLangCode: String = "zh"
    /** 符号层返回目标：letters | handwriting */
    private var numberLayerSource: String = "letters"
    /** 进入符号层前的字母布局 id */
    private var letterLayoutId: String = "layout_pinyin26"
    private var currentModeId: String = "pinyin"

    /** 选词后 shell 已上屏，refreshUi 跳过 Commit/discard */
    private var skipEditorCommands = false

    /**
     * 选词/空格上屏后为 true：退格只删正文，且 refreshUi 不再 setComposingText。
     * 下次输入字母时清零。
     */
    private var preferEditorDelete = false

    /** 系统报告的 composing 区间 [start, end)，-1 表示无 */
    private var composingRegionStart = -1
    private var composingRegionEnd = -1
    private var handwritingActive = false

    /** AI 面板打开时：缓存宿主选区，用于剪切（剪贴板未变时 listener 不回调） */
    private var aiCaptureSelection: String? = null
    private var aiCaptureEditorLen: Int = -1
    private var hwSessionStrokeId = 0L
    private var hwPasswordBlocked = false
    private val hwStrokes = mutableListOf<HandwritingPad.StrokePayload>()
    private val hwHandler = Handler(Looper.getMainLooper())
    private var hwDebounce: Runnable? = null
    private val hwRecognizeGen = AtomicInteger(0)
    private val hwExecutor = Executors.newSingleThreadExecutor()
    private var hwEngine: PaddleOcrEngine? = null
    private val speechHost = SpeechHost(this)

    override fun onCreate() {
        super.onCreate()
        if (!coreInited) {
            val rc = YcNative.ycCoreInit(filesDir.absolutePath)
            coreInited = rc == YcNative.OK
            Log.i(TAG, "ycCoreInit -> $rc")
            if (coreInited) {
                ensureLangPacks()
            }
        }
        hwEngine = PaddleOcrEngine(applicationContext)
        hwExecutor.execute { hwEngine?.ensureLoaded() }
        applyPreferredLangFromPrefs()
    }

    /** 主 App「语言与布局」写入的默认语言，打开键盘时生效。 */
    private fun applyPreferredLangFromPrefs() {
        val code = getSharedPreferences("yc_lang", MODE_PRIVATE).getString("preferred_lang", null) ?: return
        if (code in listOf("zh", "en", "vi", "th") && code != currentLangCode) {
            currentLangCode = code
            asciiMode = code == "en"
            currentLayoutId = layoutIdForLang(code)
            letterLayoutId = currentLayoutId
        }
    }

    override fun onDestroy() {
        speechHost.stop()
        super.onDestroy()
    }

    override fun onCreateInputView(): View {
        val kb = YcKeyboardPanel(this)
        panel = kb
        kb.setKeyListener { key -> onKey(key) }
        kb.setCandidateListener { cand -> onCandidate(cand) }
        kb.setPageListener { delta -> onCandPage(delta) }
        kb.setExpandListener { onCandExpand() }
        kb.setNeedMoreListener { onCandNeedMore() }
        kb.setToolbarListener { item -> onToolbar(item) }
        wireEntertainmentPanels(kb)
        kb.setHandwritingStrokeListener { stroke -> onHwStroke(stroke) }
        kb.setHandwritingRecognizeListener { onHwRecognize() }
        kb.setHandwritingUndoListener { onHwUndo() }
        kb.setHandwritingClearListener { onHwClear() }
        kb.setHandwritingDismissListener { dismissHandwriting() }
        kb.setLangPickListener { opt -> onLangPicked(opt) }
        kb.setModePickListener { opt -> onModePicked(opt) }
        kb.setOnModeClick { onModeButtonClick() }
        kb.setOnCollapseClick { requestHideSelf(0) }
        kb.setOnAiChipClick { chip -> Log.i(TAG, "ai chip: $chip") }
        kb.setKeyDownListener { key -> onKeyDown(key) }
        kb.setKeyUpListener { key -> onKeyUp(key) }
        applyPreferredLangFromPrefs()
        reloadLayout(currentLayoutId)
        updateModeLabel()
        updateHandwritingToolbar()
        return kb
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        panel?.applyTheme(SkinRegistry.resolve(this))
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        panel?.applyTheme(SkinRegistry.resolve(this))
    }

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        if (!coreInited) return
        ensureLangPacks()
        if (editorId != 0L) {
            YcNative.ycSessionStop(editorId, 0)
        }
        val inputType = attribute?.inputType ?: 0
        editorId = YcNative.ycSessionBeginWithInput(1L, inputType)
        clientSeq = 0
        lastSeq = -1
        lastComposing = ""
        lastCandidates = emptyList()
        lastCandPage = 0
        lastTotalPages = 0
        asciiMode = false
        if (currentLangCode == "en") currentLangCode = "zh"
        handwritingActive = false
        hwSessionStrokeId = 0L
        hwStrokes.clear()
        cancelHwDebounce()
        hwPasswordBlocked = isPasswordInputType(inputType)
        updateHandwritingToolbar()
        clearCandScrollBuffer()
        skipEditorCommands = false
        preferEditorDelete = false
        composingRegionStart = -1
        composingRegionEnd = -1
        panel?.setHandwritingMode(false)
        panel?.hideLangPicker()
        submit(YcNative.ACTION_INIT)
        refreshUi()
    }

    override fun onFinishInput() {
        if (editorId != 0L) {
            YcNative.ycSessionStop(editorId, 0)
            editorId = 0
        }
        lastComposing = ""
        lastCandidates = emptyList()
        asciiMode = false
        handwritingActive = false
        clearCandScrollBuffer()
        preferEditorDelete = false
        panel?.setHandwritingMode(false)
        super.onFinishInput()
    }

    /**
     * 系统回调里的 candidatesStart/End 实际是 composing 区间。
     * 选词后若区间仍在，说明 IC 残留 span —— 这是「退格多次无反应」的根因。
     */
    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int,
    ) {
        super.onUpdateSelection(
            oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd,
        )
        composingRegionStart = candidatesStart
        composingRegionEnd = candidatesEnd
        val spanLen =
            if (candidatesStart >= 0 && candidatesEnd > candidatesStart) {
                candidatesEnd - candidatesStart
            } else {
                0
            }
        Log.i(
            TAG,
            "onUpdateSelection sel=$newSelStart..$newSelEnd composing=$candidatesStart..$candidatesEnd spanLen=$spanLen preferEditorDelete=$preferEditorDelete",
        )
        if (preferEditorDelete && spanLen > 0) {
            Log.w(TAG, "stale composing span after commit — resolve len=$spanLen")
            resolveStaleComposingSpan(spanLen)
        }
        captureAiAssistFromCutOrCopy(
            oldSelStart = oldSelStart,
            oldSelEnd = oldSelEnd,
            newSelStart = newSelStart,
            newSelEnd = newSelEnd,
        )
    }

    /**
     * AI 面板可见时：复制靠剪贴板 listener；剪切若内容与上次剪贴板相同可能不触发 listener，
     * 用「选区消失 + 正文变短 + 剪贴板==原选区」兜底填入。
     */
    private fun captureAiAssistFromCutOrCopy(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
    ) {
        if (panel?.isAiAssistShowing() != true) {
            aiCaptureSelection = null
            aiCaptureEditorLen = -1
            return
        }
        val ic = currentInputConnection ?: return
        val selected = ic.getSelectedText(0)?.toString()
        val beforeLen = ic.getTextBeforeCursor(10_000, 0)?.length ?: 0
        val afterLen = ic.getTextAfterCursor(10_000, 0)?.length ?: 0
        val selLen = if (!selected.isNullOrEmpty()) selected.length else 0
        val editorLen = beforeLen + afterLen + selLen

        if (!selected.isNullOrBlank()) {
            aiCaptureSelection = selected
            aiCaptureEditorLen = editorLen
            return
        }

        val hadSelection = oldSelStart != oldSelEnd
        val nowCollapsed = newSelStart == newSelEnd
        val captured = aiCaptureSelection
        if (hadSelection && nowCollapsed && !captured.isNullOrBlank()) {
            val shrunk = aiCaptureEditorLen >= 0 && editorLen < aiCaptureEditorLen
            if (shrunk) {
                fun tryIngest() {
                    val clip = readClipboardPlainText()
                    if (clip == captured) {
                        panel?.prefillAiAssist(captured)
                    }
                }
                tryIngest()
                // 部分 App 先删选区再写剪贴板，延迟再读一次
                android.os.Handler(mainLooper).postDelayed({ tryIngest() }, 50)
            }
            aiCaptureSelection = null
            aiCaptureEditorLen = -1
        } else {
            aiCaptureEditorLen = editorLen
        }
    }

    private fun readClipboardPlainText(): String? {
        return try {
            val cm = getSystemService(CLIPBOARD_SERVICE) as? android.content.ClipboardManager
                ?: return null
            val clip = cm.primaryClip ?: return null
            if (clip.itemCount <= 0) return null
            clip.getItemAt(0)?.coerceToText(this)?.toString()?.trim()?.takeIf { it.isNotEmpty() }
        } catch (_: Exception) {
            null
        }
    }

    /**
     * 上屏后系统仍报告 composing 区间时：
     * - 纯 a-z 拼音 → 丢弃（否则退格会先逐字啃拼音）
     * - 含汉字或其他内容 → finish 提交（切勿把 ?/NUL 当拼音整段清空）
     */
    private fun resolveStaleComposingSpan(spanLen: Int) {
        if (spanLen <= 0) return
        val ic = currentInputConnection ?: return
        val before = ic.getTextBeforeCursor(spanLen + 4, 0)?.toString().orEmpty()
        val spanText = if (before.length >= spanLen) before.takeLast(spanLen) else before
        val isPinyinOnly = spanText.isNotEmpty() && spanText.all { it in 'a'..'z' || it in 'A'..'Z' }
        Log.i(TAG, "resolveStaleComposingSpan spanText='$spanText' pinyinOnly=$isPinyinOnly")
        ic.beginBatchEdit()
        if (isPinyinOnly) {
            if (composingRegionStart >= 0 && composingRegionEnd > composingRegionStart) {
                ic.setComposingRegion(composingRegionStart, composingRegionEnd)
            }
            ic.commitText("", 1)
        } else {
            ic.finishComposingText()
        }
        ic.endBatchEdit()
        composingRegionStart = -1
        composingRegionEnd = -1
    }

    /** 安装并启用中/英/越/泰语言包。 */
    private fun ensureLangPacks() {
        for (id in listOf("zh-pack-v1", "en-v1", "vi-v1", "th-v1")) {
            installPackFromAssets(id)
        }
        val rc = YcNative.ycCoreSyncLangPacks()
        Log.i(TAG, "ycCoreSyncLangPacks -> $rc")
    }

    private fun installPackFromAssets(packId: String) {
        val packFile = File(filesDir, "$packId.imepack")
        try {
            assets.open("langpacks/$packId.imepack").use { input ->
                packFile.outputStream().use { output -> input.copyTo(output) }
            }
            val rc = YcNative.ycCoreInstallLangpack(packFile.absolutePath)
            Log.i(TAG, "ycCoreInstallLangpack $packId -> $rc size=${packFile.length()}")
        } catch (e: Exception) {
            Log.w(TAG, "$packId not in assets", e)
        }
    }

    /** 去掉 NUL/替换符/控制字符，避免脏候选上屏后再被误清。 */
    private fun sanitizeCommitText(raw: String): String =
        raw.filter { ch ->
            ch != '\u0000' && ch != '\uFFFD' && !ch.isISOControl()
        }.trim()

    private fun hasInputCache(): Boolean =
        lastComposing.isNotEmpty() || lastCandidates.isNotEmpty()


    private fun onKey(key: KeyDef) {
        panel?.hideModePanel()
        when (key.action) {
            KeyAction.Backspace -> {
                handleBackspace()
                return
            }
            KeyAction.Shift -> {
                onShiftKey()
                return
            }
            KeyAction.Search -> {
                if ((currentLangCode == "vi" || currentLangCode == "th" || currentLangCode == "en") && lastComposing.isNotEmpty()) {
                    // 有候选则选首选；否则上屏 composing
                    if (lastCandidates.isNotEmpty()) {
                        onCandidate(lastCandidates.first())
                    } else {
                        currentInputConnection?.finishComposingText()
                        lastComposing = ""
                        pushCandSnapshot()
                    }
                    return
                }
                if (lastComposing.isNotEmpty()) {
                    commitPinyinAsRawText()
                } else {
                    performEditorImeAction()
                }
                return
            }
            KeyAction.Globe -> {
                if (panel?.isLangPickerShowing() == true) {
                    panel?.hideLangPicker()
                } else {
                    panel?.showLangPicker(LANG_OPTIONS, currentLangCode)
                }
                return
            }
            KeyAction.Tone -> {
                applyViTone(key.output?.removePrefix("tone:") ?: return)
                return
            }
            KeyAction.Symbol -> {
                enterSymbolLayer(
                    if (handwritingActive || panel?.isHandwritingMode() == true) "handwriting" else "letters",
                )
                return
            }
            KeyAction.Letters -> {
                exitSymbolLayer()
                return
            }
            KeyAction.Mic -> {
                return
            }
            KeyAction.Space -> {
                // th / vi / zh / en：走引擎（latin 空格 = 确认首选或上屏 composing）
                submit(YcNative.ACTION_KEY_PRESS, key.keyCode ?: ' '.code)
                val committed = refreshUi()
                if (committed || !hasInputCache()) {
                    enterEditorDeleteMode(commitSucceeded = committed)
                } else {
                    Log.w(TAG, "space had no Commit and cache remains composing='$lastComposing'")
                }
                return
            }
            KeyAction.Letter -> {
                if (key.output == "half") {
                    Log.i(TAG, "symbol half page (stub)")
                    return
                }
                preferEditorDelete = false
                val text = key.output?.takeIf { it.isNotEmpty() && !it.startsWith("tone:") }
                    ?: key.label
                when (currentLangCode) {
                    "th", "vi" -> {
                        // 泰文 Kedmanee / 越语专用字母与 a–z 一律喂引擎（Unicode code point）
                        if (candExpanded) collapseCandExpand()
                        for (ch in text) {
                            if (!ch.isISOControl()) {
                                submit(YcNative.ACTION_KEY_PRESS, ch.code)
                            }
                        }
                        refreshUi()
                    }
                    else -> {
                        val code = key.keyCode ?: text.firstOrNull()?.code ?: return
                        if (isEngineLetter(code) || (asciiMode && isAsciiComposable(code))) {
                            if (candExpanded) collapseCandExpand()
                            if (isEngineLetter(code)) {
                                submit(YcNative.ACTION_KEY_PRESS, code)
                                refreshUi()
                            } else {
                                currentInputConnection?.commitText(code.toChar().toString(), 1)
                            }
                        } else {
                            if (hasInputCache()) {
                                discardComposing()
                                clearInputCache()
                            }
                            currentInputConnection?.commitText(text, 1)
                        }
                    }
                }
                panel?.consumeOnceShiftIfNeeded()
                return
            }
        }
    }

    private fun onShiftKey() {
        when (currentLangCode) {
            "zh" -> onLangPicked(LANG_OPTIONS.first { it.code == "en" })
            else -> panel?.cycleShift()
        }
    }

    private fun enterSymbolLayer(source: String) {
        numberLayerSource = source
        if (panel?.isInSymbolLayer() != true) {
            letterLayoutId = currentLayoutId
        }
        if (source == "handwriting" && (handwritingActive || panel?.isHandwritingMode() == true)) {
            panel?.setHandwritingMode(false)
            handwritingActive = false
        }
        val back = if (source == "handwriting" && allowsHandwriting()) "手写" else "ABC"
        val symbolLayoutId = if (currentLangCode == "en") "layout_en_symbol" else "layout_symbol"
        val packSymbol = LayoutLoader.loadOrNull(filesDir, symbolLayoutId)
        if (packSymbol != null && packSymbol.size >= 4) {
            val patched = packSymbol.map { row ->
                row.map { key ->
                    if (key.action == KeyAction.Letters) key.copy(label = back) else key
                }
            }
            panel?.setUseZhPunct(currentLangCode == "zh")
            panel?.setLayoutRows(patched, null, symbolLayoutId, -1)
        } else {
            panel?.showSymbolLayer(back)
        }
        currentLayoutId = symbolLayoutId
        Log.i(TAG, "enterSymbolLayer source=$source layout=$symbolLayoutId")
    }

    private fun exitSymbolLayer() {
        if (numberLayerSource == "handwriting" && allowsHandwriting()) {
            openHandwriting()
        } else {
            val id = letterLayoutId.ifEmpty { layoutIdForLang(currentLangCode) }
            reloadLayout(id)
        }
        numberLayerSource = "letters"
        Log.i(TAG, "exitSymbolLayer -> $currentLayoutId")
    }

    private fun onKeyDown(key: KeyDef) {
        if (key.action == KeyAction.Mic) {
            val name = LANG_OPTIONS.firstOrNull { it.code == currentLangCode }?.name ?: "中文"
            panel?.showVoiceOverlay(name)
            speechHost.start(
                currentLangCode,
                object : SpeechHost.Callback {
                    override fun onPartial(text: String) {
                        Log.i(TAG, "asr partial: $text")
                    }
                    override fun onFinal(text: String) {
                        panel?.setVoiceRecognizing()
                        hwHandler.postDelayed({
                            panel?.hideVoiceOverlay()
                            if (text.isNotBlank()) {
                                commitToEditor(text)
                            }
                        }, 200)
                    }
                    override fun onError(message: String) {
                        Log.w(TAG, "asr: $message")
                        panel?.hideVoiceOverlay()
                    }
                },
            )
        }
    }

    private fun onKeyUp(key: KeyDef) {
        if (key.action == KeyAction.Mic) {
            // Recognition continues until SpeechHost callback; stop listening early if needed.
            speechHost.stopListening()
        }
    }

    private fun onModeButtonClick() {
        val modes = modesForLang(currentLangCode)
        if (modes.size <= 1) {
            panel?.hideModePanel()
            return
        }
        if (panel?.isModePanelShowing() == true) {
            panel?.hideModePanel()
        } else {
            panel?.showModePanel(modes, currentModeId)
        }
    }

    private fun onModePicked(opt: ModeOption) {
        currentModeId = opt.id
        when (opt.id) {
            "handwriting" -> {
                if (allowsHandwriting()) openHandwriting()
            }
            else -> {
                if (handwritingActive || panel?.isHandwritingMode() == true) {
                    dismissHandwriting()
                }
                reloadLayout(letterLayoutId.ifEmpty { currentLayoutId })
            }
        }
        updateModeLabel()
    }

    private fun modesForLang(code: String): List<ModeOption> = when (code) {
        "zh" -> listOf(
            ModeOption("pinyin", "拼音", "⌨️"),
            ModeOption("handwriting", "手写", "✍️"),
        )
        else -> listOf(ModeOption("keyboard", "键盘", "⌨️"))
    }

    private fun updateModeLabel() {
        val label = when {
            handwritingActive || panel?.isHandwritingMode() == true -> "手写"
            currentLangCode == "zh" -> "拼音"
            else -> "键盘"
        }
        currentModeId = when {
            label == "手写" -> "handwriting"
            currentLangCode == "zh" -> "pinyin"
            else -> "keyboard"
        }
        panel?.setModeLabel(label)
        panel?.setUseZhPunct(currentLangCode == "zh")
    }

    private fun isAsciiComposable(code: Int): Boolean {
        val c = code.toChar()
        return c == '.' || c == '/' || c == ':' || c == '-' || c == '_' || c == '@'
    }

    private fun performEditorImeAction() {
        val ic = currentInputConnection ?: return
        val ei = currentInputEditorInfo
        val action = ei?.imeOptions?.and(EditorInfo.IME_MASK_ACTION) ?: EditorInfo.IME_ACTION_NONE
        if (action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED) {
            ic.performEditorAction(action)
        } else {
            ic.sendKeyEvent(android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_ENTER))
            ic.sendKeyEvent(android.view.KeyEvent(android.view.KeyEvent.ACTION_UP, android.view.KeyEvent.KEYCODE_ENTER))
        }
        Log.i(TAG, "performEditorImeAction action=$action")
    }

    private fun handleBackspace() {
        Log.i(
            TAG,
            "backspace preferEditorDelete=$preferEditorDelete composing='$lastComposing' cands=${lastCandidates.size} span=$composingRegionStart..$composingRegionEnd",
        )

        // 上屏后：只删正文一次，不在同一次按键里再 resolve（避免叠删）
        if (preferEditorDelete) {
            if (hasInputCache()) {
                clearInputCache()
            }
            deleteEditorCharBeforeCursor()
            return
        }

        when {
            lastComposing.isNotEmpty() -> {
                submit(YcNative.ACTION_BACKSPACE)
                refreshUi()
            }
            lastCandidates.isNotEmpty() -> {
                clearInputCache()
            }
            hasComposingRegion() -> {
                // 仅清拼音 span，本键不再删正文
                resolveStaleComposingSpan(composingRegionEnd - composingRegionStart)
            }
            else -> {
                deleteEditorCharBeforeCursor()
            }
        }
    }

    private fun hasComposingRegion(): Boolean =
        composingRegionStart >= 0 && composingRegionEnd > composingRegionStart

    private fun isEngineLetter(code: Int): Boolean {
        val c = code.toChar()
        return c in 'a'..'z' || c in 'A'..'Z'
    }

    private fun onCandidate(cand: CandidateItem) {
        val text = sanitizeCommitText(cand.text)
        if (text.isEmpty()) {
            Log.w(TAG, "select id=${cand.id} empty after sanitize raw='${cand.text}'")
            return
        }
        val pinyin = lastComposing
        Log.i(TAG, "select id=${cand.id} page=${cand.page} text='$text' pinyin='$pinyin'")

        if (handwritingActive || panel?.isHandwritingMode() == true) {
            commitToEditor(text)
            submit(YcNative.ACTION_SELECT_CANDIDATE, candidateId = cand.id)
            panel?.clearHandwritingInk()
            hwStrokes.clear()
            hwSessionStrokeId = 0L
            // 选词后仍留在手写板，刷新候选
            skipEditorCommands = true
            try {
                refreshUi()
            } finally {
                skipEditorCommands = false
            }
            enterEditorDeleteMode(commitSucceeded = textBeforeEndsWith(text))
            if (candExpanded) {
                collapseCandExpand()
                pushCandSnapshot()
            }
            return
        }

        // 展开列表可能跨页：先翻到候选所在页，再按文本对齐页内 id，保证引擎 Commit/学习一致
        ensureCandPage(cand.page)
        val engineId = lastCandidates
            .indexOfFirst { it.text == text }
            .takeIf { it >= 0 }
            ?: cand.id

        commitToEditor(text)
        stripLeakedPinyin(pinyin, text)
        submit(YcNative.ACTION_SELECT_CANDIDATE, candidateId = engineId)
        // 选词后保留引擎离线联想候选，勿 ACTION_INIT / clearInputCache
        skipEditorCommands = true
        try {
            refreshUi()
        } finally {
            skipEditorCommands = false
        }
        enterEditorDeleteMode(commitSucceeded = textBeforeEndsWith(text))

        if (candExpanded) {
            collapseCandExpand()
            pushCandSnapshot()
        }

        val before = currentInputConnection?.getTextBeforeCursor(32, 0)
        Log.i(
            TAG,
            "after select textBefore='$before' span=$composingRegionStart..$composingRegionEnd",
        )
    }

    private fun onToolbar(item: String) {
        when (item) {
            "手写" -> {
                if (hwPasswordBlocked || !allowsHandwriting()) {
                    Log.w(TAG, "handwriting disabled lang=$currentLangCode password=$hwPasswordBlocked")
                    return
                }
                openHandwriting()
            }
            "设置" -> openDiscover("settings")
            "皮肤" -> {
                if (panel?.isSkinPickerShowing() == true) {
                    panel?.hideSkinPicker()
                } else {
                    panel?.showSkinPicker(SkinRegistry.currentId(this))
                }
            }
            "表情" -> {
                if (panel?.isEmojiPanelShowing() == true) {
                    panel?.hideEmojiPanel()
                } else {
                    panel?.showEmojiPanel()
                }
            }
            "话术" -> openPhraseDeck()
            "翻译" -> openAiAssist("翻译")
            "AI" -> openAiAssist("智能回复")
            else -> Log.i(TAG, "toolbar: $item")
        }
    }

    private fun openAiAssist(mode: String) {
        val seed = currentInputConnection?.getSelectedText(0)?.toString()
            ?: lastComposing.takeIf { it.isNotBlank() }
            ?: ""
        if (panel?.isAiAssistShowing() == true && mode == "智能回复") {
            panel?.hideAiAssist()
            return
        }
        panel?.showAiAssist(mode, seed)
    }

    private fun openDiscover(tab: String) {
        try {
            val target = when (tab) {
                "settings" -> "settings"
                "llm", "ai" -> "llm"
                "skins", "content", "campaigns" -> "discover"
                else -> "discover"
            }
            val intent = Intent().apply {
                setClassName(packageName, "com.yc.input.MainActivity")
                putExtra("tab", target)
                if (tab in listOf("skins", "content", "campaigns")) {
                    putExtra("channel", tab)
                }
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            }
            startActivity(intent)
        } catch (e: Exception) {
            Log.w(TAG, "open MainActivity failed", e)
        }
    }

    private fun openPhraseDeck() {
        val pack = when (currentLangCode) {
            "zh" -> preferredPhrasePack()
            else -> null
        }
        if (pack == null) {
            Log.i(TAG, "no phrase deck for lang=$currentLangCode")
            return
        }
        try {
            assets.open(pack).bufferedReader().use { reader ->
                val (title, cards) = PhraseDeckPanel.parseDeckJson(reader.readText())
                panel?.showPhraseDeck(title, cards)
            }
        } catch (e: Exception) {
            Log.w(TAG, "load phrase deck $pack", e)
        }
    }

    private fun preferredPhrasePack(): String {
        val enabled = getSharedPreferences("yc_content", MODE_PRIVATE)
            .getString("active_industry", "industry-ecommerce-v1")
        return "content/$enabled/phrases/deck.json"
    }

    private fun wireEntertainmentPanels(kb: YcKeyboardPanel) {
        kb.setSkinPickListener { opt ->
            if (opt.id == "system") {
                SkinRegistry.save(this, "system")
                kb.applyTheme(ThemeTokens.from(this))
            } else {
                SkinRegistry.save(this, opt.id)
                kb.applyTheme(opt.tokens)
            }
            kb.hideSkinPicker()
        }
        kb.setSkinMoreListener { openDiscover("skins") }
        kb.setEmojiPickListener { emoji ->
            commitToEditor(emoji)
            kb.hideEmojiPanel()
        }
        kb.setPhrasePickListener { card ->
            commitToEditor(card.text)
            kb.hidePhraseDeck()
        }
        kb.setAiAssistGenerateListener { req ->
            onAiAssistGenerate(req)
        }
        kb.setAiAssistPickListener { text ->
            commitToEditor(text)
            kb.hideAiAssist()
        }
        kb.setAiAssistCloseListener {
            kb.hideAiAssist()
        }
    }

    private fun onAiAssistGenerate(req: com.yc.input.ui.AiAssistPanel.GenerateRequest) {
        if (hwPasswordBlocked) {
            panel?.setAiAssistStatus("当前输入框禁止使用 AI")
            return
        }
        val mode = when (req.modeLabel) {
            "智能回复" -> com.yc.input.llm.AiAssistMode.SmartReply
            "高情商" -> com.yc.input.llm.AiAssistMode.HighEqReply
            "撰写" -> com.yc.input.llm.AiAssistMode.Compose
            "改写" -> com.yc.input.llm.AiAssistMode.Rewrite
            "翻译" -> com.yc.input.llm.AiAssistMode.Translate
            else -> com.yc.input.llm.AiAssistMode.Polish
        }
        panel?.setAiAssistStatus("生成中…")
        val targetLang = when (currentLangCode) {
            "zh" -> "en"
            else -> "zh"
        }
        com.yc.input.llm.LlmAssistRouter.suggestAsync(
            this,
            com.yc.input.llm.AiAssistRequest(
                mode = mode,
                selectionText = req.input,
                peerMessage = if (mode == com.yc.input.llm.AiAssistMode.SmartReply ||
                    mode == com.yc.input.llm.AiAssistMode.HighEqReply
                ) {
                    req.input
                } else {
                    ""
                },
                userIntent = req.input,
                targetLang = req.targetLang.ifBlank { targetLang },
            ),
        ) { result ->
            if (result.error != null && result.variants.isEmpty()) {
                panel?.setAiAssistStatus(result.error)
            }
            panel?.showAiAssistVariants(result.variants.map { it.text }, result.local)
        }
    }

    private fun allowsHandwriting(): Boolean =
        currentLangCode == "zh" && !asciiMode

    private fun updateHandwritingToolbar() {
        val allow = allowsHandwriting() && !hwPasswordBlocked
        panel?.setToolbarItemEnabled("手写", allow)
        if (!allow && (handwritingActive || panel?.isHandwritingMode() == true)) {
            dismissHandwriting()
        }
    }

    private fun isPasswordInputType(inputType: Int): Boolean {
        val variation = inputType and InputType.TYPE_MASK_VARIATION
        val klass = inputType and InputType.TYPE_MASK_CLASS
        return when (variation) {
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
            -> true
            InputType.TYPE_NUMBER_VARIATION_PASSWORD ->
                klass == InputType.TYPE_CLASS_NUMBER
            else -> false
        }
    }

    private fun openHandwriting() {
        if (editorId == 0L) return
        if (!allowsHandwriting() || hwPasswordBlocked) {
            Log.w(TAG, "openHandwriting blocked lang=$currentLangCode")
            return
        }
        clientSeq++
        val rc = YcNative.ycHotSubmit(
            YcNative.buildAction(
                editorId,
                clientSeq,
                YcNative.ACTION_OPEN_HANDWRITING,
                0,
                0,
            ),
        )
        if (rc != YcNative.OK) {
            Log.w(TAG, "open handwriting blocked: ycHotSubmit rc=$rc (privacy/editor)")
            return
        }
        refreshUi()
        if (!handwritingActive) {
            Log.w(TAG, "handwriting: ReloadKeyboard missed after OK submit, forcing pad UI")
            panel?.setHandwritingMode(true)
            handwritingActive = true
        } else {
            Log.i(TAG, "handwriting opened")
        }
        hwStrokes.clear()
        preferEditorDelete = false
        skipEditorCommands = false
        updateModeLabel()
        // Preload on background; avoid runBlocking on UI thread.
        hwExecutor.execute { hwEngine?.ensureLoaded() }
    }

    private fun dismissHandwriting() {
        cancelHwDebounce()
        hwRecognizeGen.incrementAndGet()
        panel?.setHandwritingRecognizing(false)
        submit(YcNative.ACTION_DISMISS_HANDWRITING)
        refreshUi()
        panel?.setHandwritingMode(false)
        handwritingActive = false
        hwSessionStrokeId = 0L
        updateModeLabel()
        hwStrokes.clear()
        Log.i(TAG, "handwriting dismissed")
    }

    private fun onHwStroke(stroke: HandwritingPad.StrokePayload) {
        if (editorId == 0L) return
        // New ink cancels in-flight recognition.
        hwRecognizeGen.incrementAndGet()
        panel?.setHandwritingRecognizing(false)
        hwSessionStrokeId++
        val continuous = panel?.isHandwritingContinuous() == true
        val mode =
            if (continuous) {
                YcNative.WRITING_CONTINUOUS
            } else {
                YcNative.WRITING_SINGLE_CHAR
            }
        val rc = YcNative.ycHwPushStroke(
            editorId,
            stroke.xyPressure,
            stroke.timesMs,
            hwSessionStrokeId,
            stroke.canvasW,
            stroke.canvasH,
            mode,
        )
        Log.i(TAG, "hw push stroke id=$hwSessionStrokeId pts=${stroke.timesMs.size} mode=$mode rc=$rc")
        if (rc != YcNative.OK) return
        hwStrokes.add(stroke)
        cancelHwDebounce()
        val delay = if (continuous) HW_CONTINUOUS_IDLE_MS else HW_SINGLE_DEBOUNCE_MS
        val runnable = Runnable { runHwRecognize(fromButton = false) }
        hwDebounce = runnable
        hwHandler.postDelayed(runnable, delay)
    }

    private fun onHwRecognize() {
        cancelHwDebounce()
        runHwRecognize(fromButton = true)
    }

    private fun cancelHwDebounce() {
        hwDebounce?.let { hwHandler.removeCallbacks(it) }
        hwDebounce = null
    }

    private fun runHwRecognize(fromButton: Boolean) {
        if (editorId == 0L || hwStrokes.isEmpty()) return
        val continuous = panel?.isHandwritingContinuous() == true
        if (continuous && !fromButton && hwDebounce == null) {
            // idle timer already consumed
        }
        val strokes = hwStrokes.toList()
        val gen = hwRecognizeGen.incrementAndGet()
        panel?.setHandwritingRecognizing(true)
        val engine = hwEngine
        hwExecutor.execute {
            val started = System.currentTimeMillis()
            val output =
                if (engine != null && engine.ensureLoaded()) {
                    engine.recognizeStrokes(strokes, continuous, topK = PaddleOcrEngine.TOP_K)
                } else {
                    null
                }
            val elapsed = System.currentTimeMillis() - started
            hwHandler.post {
                if (gen != hwRecognizeGen.get()) return@post
                panel?.setHandwritingRecognizing(false)
                if (output == null || output.candidates.isEmpty()) {
                    val reason =
                        when {
                            engine == null -> "engine=null"
                            !engine.isReady() && engine.hasHardFailure() -> "model-init-failed"
                            !engine.isReady() -> "not-ready/opencv"
                            output == null -> "recognize-null"
                            else -> "empty-candidates"
                        }
                    Log.w(TAG, "PaddleOCR miss ($reason); falling back to template Recognize ($elapsed ms)")
                    submit(YcNative.ACTION_RECOGNIZE_HANDWRITING)
                    refreshUi()
                    maybeShowCloudConfirm()
                    return@post
                }
                val texts = output.candidates.map { it.text }.toTypedArray()
                val scores = FloatArray(output.candidates.size) { output.candidates[it].score }
                val flags = if (output.needsCloudConfirm) 1 else 0
                val rc = YcNative.ycHwApplyResult(editorId, texts, scores, flags)
                Log.i(
                    TAG,
                    "PaddleOCR apply n=${texts.size} cloud=$flags rc=$rc ${elapsed}ms top=${texts.firstOrNull()}",
                )
                if (rc != YcNative.OK) {
                    Log.w(TAG, "ycHwApplyResult failed rc=$rc; template fallback")
                    submit(YcNative.ACTION_RECOGNIZE_HANDWRITING)
                }
                // 选词后 preferEditorDelete 可能仍为 true，勿吞掉手写候选
                preferEditorDelete = false
                refreshUi()
                maybeShowCloudConfirm()
            }
            if (elapsed > HW_RECOGNIZE_TIMEOUT_MS) {
                Log.w(TAG, "PaddleOCR slow: ${elapsed}ms")
            }
        }
        // Soft timeout: clear "识别中…" if background stalls
        hwHandler.postDelayed({
            if (gen == hwRecognizeGen.get()) {
                panel?.setHandwritingRecognizing(false)
            }
        }, HW_RECOGNIZE_TIMEOUT_MS)
    }

    private fun maybeShowCloudConfirm() {
        val snap = YcNative.readArena() ?: return
        if (!snap.pendingCloudHw) return
        AlertDialog.Builder(this)
            .setTitle("连写云识别")
            .setMessage("端侧置信度较低。是否上传笔迹向量做云端识别？（当前为演示 stub）")
            .setPositiveButton("确认") { _, _ ->
                submit(YcNative.ACTION_CONFIRM_CLOUD_HW)
                refreshUi()
            }
            .setNegativeButton("取消") { _, _ ->
                submit(YcNative.ACTION_DISMISS_CLOUD_HW)
                refreshUi()
            }
            .show()
    }

    private fun onHwUndo() {
        cancelHwDebounce()
        if (hwStrokes.isNotEmpty()) {
            hwStrokes.removeAt(hwStrokes.lastIndex)
        }
        submit(YcNative.ACTION_UNDO_HANDWRITING)
        refreshUi()
    }

    private fun onHwClear() {
        cancelHwDebounce()
        hwRecognizeGen.incrementAndGet()
        panel?.setHandwritingRecognizing(false)
        submit(YcNative.ACTION_CLEAR_HANDWRITING)
        panel?.clearHandwritingInk()
        hwSessionStrokeId = 0L
        hwStrokes.clear()
        refreshUi()
    }

    companion object {
        private const val TAG = "YcImeService"
        private const val HW_SINGLE_DEBOUNCE_MS = 450L
        private const val HW_CONTINUOUS_IDLE_MS = 800L
        private const val HW_RECOGNIZE_TIMEOUT_MS = 2500L

        private val LANG_OPTIONS = listOf(
            LangOption("zh", "中文", "中文", "zh-pack-v1"),
            LangOption("en", "英语", "English", "en-v1"),
            LangOption("vi", "越南语", "Tiếng Việt", "vi-v1"),
            LangOption("th", "泰语", "ไทย", "th-v1"),
        )

        /** 与参考效果.html VI_TONE_MAP 一致 */
        private val VI_TONE_MAP: Map<String, Map<Char, Char>> = mapOf(
            "sac" to mapOf(
                'a' to 'á', 'ă' to 'ắ', 'â' to 'ấ', 'e' to 'é', 'ê' to 'ế', 'i' to 'í',
                'o' to 'ó', 'ô' to 'ố', 'ơ' to 'ớ', 'u' to 'ú', 'ư' to 'ứ', 'y' to 'ý',
                'A' to 'Á', 'Ă' to 'Ắ', 'Â' to 'Ấ', 'E' to 'É', 'Ê' to 'Ế', 'I' to 'Í',
                'O' to 'Ó', 'Ô' to 'Ố', 'Ơ' to 'Ớ', 'U' to 'Ú', 'Ư' to 'Ứ', 'Y' to 'Ý',
            ),
            "huyen" to mapOf(
                'a' to 'à', 'ă' to 'ằ', 'â' to 'ầ', 'e' to 'è', 'ê' to 'ề', 'i' to 'ì',
                'o' to 'ò', 'ô' to 'ồ', 'ơ' to 'ờ', 'u' to 'ù', 'ư' to 'ừ', 'y' to 'ỳ',
                'A' to 'À', 'Ă' to 'Ằ', 'Â' to 'Ầ', 'E' to 'È', 'Ê' to 'Ề', 'I' to 'Ì',
                'O' to 'Ò', 'Ô' to 'Ồ', 'Ơ' to 'Ờ', 'U' to 'Ù', 'Ư' to 'Ừ', 'Y' to 'Ỳ',
            ),
            "hoi" to mapOf(
                'a' to 'ả', 'ă' to 'ẳ', 'â' to 'ẩ', 'e' to 'ẻ', 'ê' to 'ể', 'i' to 'ỉ',
                'o' to 'ỏ', 'ô' to 'ổ', 'ơ' to 'ở', 'u' to 'ủ', 'ư' to 'ử', 'y' to 'ỷ',
                'A' to 'Ả', 'Ă' to 'Ẳ', 'Â' to 'Ẩ', 'E' to 'Ẻ', 'Ê' to 'Ể', 'I' to 'Ỉ',
                'O' to 'Ỏ', 'Ô' to 'Ổ', 'Ơ' to 'Ở', 'U' to 'Ủ', 'Ư' to 'Ử', 'Y' to 'Ỷ',
            ),
            "nga" to mapOf(
                'a' to 'ã', 'ă' to 'ẵ', 'â' to 'ẫ', 'e' to 'ẽ', 'ê' to 'ễ', 'i' to 'ĩ',
                'o' to 'õ', 'ô' to 'ỗ', 'ơ' to 'ỡ', 'u' to 'ũ', 'ư' to 'ữ', 'y' to 'ỹ',
                'A' to 'Ã', 'Ă' to 'Ẵ', 'Â' to 'Ẫ', 'E' to 'Ẽ', 'Ê' to 'Ễ', 'I' to 'Ĩ',
                'O' to 'Õ', 'Ô' to 'Ỗ', 'Ơ' to 'Ỡ', 'U' to 'Ũ', 'Ư' to 'Ữ', 'Y' to 'Ỹ',
            ),
            "nang" to mapOf(
                'a' to 'ạ', 'ă' to 'ặ', 'â' to 'ậ', 'e' to 'ẹ', 'ê' to 'ệ', 'i' to 'ị',
                'o' to 'ọ', 'ô' to 'ộ', 'ơ' to 'ợ', 'u' to 'ụ', 'ư' to 'ự', 'y' to 'ỵ',
                'A' to 'Ạ', 'Ă' to 'Ặ', 'Â' to 'Ậ', 'E' to 'Ẹ', 'Ê' to 'Ệ', 'I' to 'Ị',
                'O' to 'Ọ', 'Ô' to 'Ộ', 'Ơ' to 'Ợ', 'U' to 'Ụ', 'Ư' to 'Ự', 'Y' to 'Ỵ',
            ),
        )

        /** 与 yc-session hash_pack_id 一致：wrapping mul31（按 Int 位型等同 u32）。 */
        fun hashPackId(id: String): Int {
            var h = 0
            for (b in id.toByteArray(Charsets.UTF_8)) {
                h = h * 31 + (b.toInt() and 0xff)
            }
            return h
        }
    }

    private fun onCandPage(delta: Int) {
        if (delta > 0) {
            if (lastTotalPages <= 1 || lastCandPage + 1 >= lastTotalPages) return
            submit(YcNative.ACTION_PAGE_NEXT)
        } else if (delta < 0) {
            if (lastCandPage <= 0) return
            submit(YcNative.ACTION_PAGE_PREV)
        } else {
            return
        }
        refreshUi()
    }

    private fun onCandExpand() {
        if (candExpanded) {
            collapseCandExpand()
            pushCandSnapshot()
            return
        }
        if (lastCandidates.isEmpty() && expandedCandidates.isEmpty()) return
        candExpanded = true
        if (expandedCandidates.isEmpty()) {
            appendExpanded(lastCandidates, lastCandPage)
        }
        pushCandSnapshot()
        Log.i(TAG, "cand expand pages=$lastTotalPages seeded=${expandedCandidates.size}")
    }

    private fun onCandNeedMore() {
        if (lastTotalPages <= 1 || lastCandPage + 1 >= lastTotalPages) return
        // 收起/展开均可跟手滑动：触底时追加下一页
        if (expandedCandidates.isEmpty()) {
            appendExpanded(lastCandidates, lastCandPage)
        }
        submit(YcNative.ACTION_PAGE_NEXT)
        refreshUi()
    }

    private fun ensureCandPage(targetPage: Int) {
        if (targetPage < 0) return
        var guard = 0
        while (lastCandPage < targetPage && guard++ < 64) {
            submit(YcNative.ACTION_PAGE_NEXT)
            refreshUi()
        }
        guard = 0
        while (lastCandPage > targetPage && guard++ < 64) {
            submit(YcNative.ACTION_PAGE_PREV)
            refreshUi()
        }
    }

    private fun appendExpanded(pageCands: List<CandidateItem>, page: Int) {
        val existing = expandedCandidates.map { it.text }.toHashSet()
        for (c in pageCands) {
            if (c.text.isEmpty() || c.text in existing) continue
            expandedCandidates.add(c.copy(page = page))
            existing.add(c.text)
        }
    }

    private fun collapseCandExpand() {
        candExpanded = false
        panel?.hideCandidatePicker()
        panel?.setCandidateExpanded(false)
    }

    private fun clearCandScrollBuffer() {
        candExpanded = false
        expandedCandidates.clear()
        panel?.hideCandidatePicker()
        panel?.setCandidateExpanded(false)
    }

    private fun displayCandidates(): List<CandidateItem> =
        if (expandedCandidates.isNotEmpty()) expandedCandidates.toList() else lastCandidates

    private fun pushCandSnapshot() {
        panel?.onSnapshot(
            KeyboardSnapshot(
                editorId = editorId,
                seq = lastSeq,
                composing = if (preferEditorDelete) "" else lastComposing,
                candidates = displayCandidates(),
                candPage = lastCandPage,
                totalPages = lastTotalPages,
                expanded = candExpanded,
                asciiMode = asciiMode,
            ),
        )
    }

    private fun textBeforeEndsWith(suffix: String): Boolean {
        if (suffix.isEmpty()) return false
        val before = currentInputConnection?.getTextBeforeCursor(suffix.length + 8, 0)?.toString()
            ?: return false
        return before.endsWith(suffix)
    }

    /**
     * @param commitSucceeded 若刚上屏的汉字已在光标前，则不再 resolve/清空，避免误删正文
     */
    private fun enterEditorDeleteMode(commitSucceeded: Boolean = false) {
        preferEditorDelete = true
        lastComposing = ""
        // 保留 lastCandidates：选词后离线词表联想仍需展示在 CandBar
        if (!commitSucceeded && hasComposingRegion()) {
            resolveStaleComposingSpan(composingRegionEnd - composingRegionStart)
        }
        pushCandSnapshot()
        Log.i(
            TAG,
            "enterEditorDeleteMode commitSucceeded=$commitSucceeded cands=${lastCandidates.size}",
        )
    }

    private fun commitPinyinAsRawText() {
        val pinyin = lastComposing
        val ic = currentInputConnection
        if (pinyin.isNotEmpty() && ic != null) {
            ic.beginBatchEdit()
            ic.finishComposingText()
            ic.endBatchEdit()
            Log.i(TAG, "Enter commit pinyin '$pinyin'")
        }
        clearInputCache()
        enterEditorDeleteMode(commitSucceeded = pinyin.isNotEmpty())
    }

    private fun clearInputCache() {
        lastComposing = ""
        lastCandidates = emptyList()
        lastCandPage = 0
        lastTotalPages = 0
        clearCandScrollBuffer()
        submit(YcNative.ACTION_INIT)
        skipEditorCommands = true
        try {
            refreshUi()
        } finally {
            skipEditorCommands = false
        }
        lastComposing = ""
        lastCandidates = emptyList()
        lastCandPage = 0
        lastTotalPages = 0
        clearCandScrollBuffer()
        pushCandSnapshot()
        Log.i(TAG, "clearInputCache")
    }

    private fun submit(actionType: Int, keyCode: Int = 0, candidateId: Int = 0) {
        if (editorId == 0L) return
        clientSeq++
        val rc = YcNative.ycHotSubmit(
            YcNative.buildAction(editorId, clientSeq, actionType, keyCode, candidateId),
        )
        if (rc != YcNative.OK) {
            Log.w(TAG, "ycHotSubmit action=$actionType key=$keyCode cand=$candidateId rc=$rc")
        }
    }

    private fun reloadLayout(layoutId: String) {
        currentLayoutId = layoutId
        if (layoutId != "layout_symbol" && layoutId != "layout_en_symbol") {
            letterLayoutId = layoutId
        }
        val isThai = layoutId.contains("thai")
        val isVi = layoutId.contains("vietnamese")
        val rows = LayoutLoader.load(filesDir, layoutId)
        val shiftAlt = when {
            isThai -> LayoutLoader.load(filesDir, "layout_thai_shift")
            isVi -> LayoutLoader.load(filesDir, "layout_vietnamese_shift")
            else -> null
        }
        val base = when {
            layoutId == "layout_thai_shift" -> LayoutLoader.load(filesDir, "layout_thai")
            layoutId == "layout_vietnamese_shift" -> LayoutLoader.load(filesDir, "layout_vietnamese")
            else -> rows
        }
        val resolvedId = when (layoutId) {
            "layout_thai_shift" -> "layout_thai"
            "layout_vietnamese_shift" -> "layout_vietnamese"
            else -> layoutId
        }
        val viSpecial = if (resolvedId.contains("vietnamese")) 0 else -1
        panel?.setUseZhPunct(currentLangCode == "zh")
        panel?.setScriptHint(
            when {
                isVi || resolvedId.contains("vietnamese") -> "vi"
                isThai || resolvedId.contains("thai") -> "th"
                else -> "latn"
            },
        )
        panel?.setLayoutRows(
            rows = when (layoutId) {
                "layout_thai_shift", "layout_vietnamese_shift" -> base
                else -> rows
            },
            shiftAltRows = shiftAlt,
            layoutId = resolvedId,
            viSpecialRowIndex = viSpecial,
        )
        updateModeLabel()
    }

    /** @return true if a non-empty Commit or DeleteSurrounding was applied */
    private fun refreshUi(): Boolean {
        val snap = YcNative.readArena() ?: return false
        if (snap.editorId != editorId) return false
        if (snap.seq == lastSeq) return false
        lastSeq = snap.seq

        val ic = currentInputConnection
        var appliedEditorMutation = false

        if (!skipEditorCommands) {
            for (cmd in snap.commands) {
                when (cmd) {
                    is ArenaCommand.Commit -> {
                        val text = sanitizeCommitText(cmd.text)
                        if (text.isNotEmpty()) {
                            commitToEditor(text)
                            appliedEditorMutation = true
                            Log.i(TAG, "Commit '$text'")
                        } else {
                            Log.w(TAG, "ignore empty Commit")
                        }
                    }
                    is ArenaCommand.SetComposing -> {
                        if (!preferEditorDelete) {
                            ic?.setComposingText(cmd.text, 1)
                        }
                    }
                    is ArenaCommand.FinishComposing -> {
                        if (!preferEditorDelete) {
                            ic?.finishComposingText()
                            appliedEditorMutation = true
                        }
                    }
                    is ArenaCommand.DeleteSurrounding -> {
                        deleteEditorChars(cmd.before, cmd.after)
                        appliedEditorMutation = true
                    }
                    is ArenaCommand.ReloadKeyboard -> {
                        when {
                            cmd.layout == YcNative.LAYOUT_HANDWRITING_PAD ||
                                cmd.layoutId == "layout_handwriting" -> {
                                if (allowsHandwriting()) {
                                    panel?.setHandwritingMode(true)
                                    handwritingActive = true
                                } else {
                                    Log.w(TAG, "ignore handwriting ReloadKeyboard for lang=$currentLangCode")
                                }
                            }
                            else -> {
                                panel?.setHandwritingMode(false)
                                handwritingActive = false
                                val id = cmd.layoutId.ifEmpty { "layout_pinyin26" }
                                reloadLayout(id)
                                syncLangFromLayout(id)
                            }
                        }
                    }
                }
            }

            // 上屏后禁止再把引擎 composing 写回 IC（否则会制造幽灵 span）
            if (!appliedEditorMutation && !preferEditorDelete) {
                if (snap.composing.isEmpty()) {
                    if (lastComposing.isNotEmpty()) {
                        discardComposing()
                    }
                } else if (snap.composing != lastComposing) {
                    ic?.setComposingText(snap.composing, 1)
                }
            }
        }

        if (skipEditorCommands || preferEditorDelete) {
            // 选词后：手写 / 拼音均可能带离线联想候选，须写入 CandBar
            val inHw = handwritingActive || panel?.isHandwritingMode() == true
            if (inHw || snap.candidates.isNotEmpty()) {
                applyHwCandFromSnap(snap)
                if (!inHw) {
                    lastComposing = ""
                }
            } else {
                // preferEditorDelete 且 snapshot 空：保留已有联想，勿清空 CandBar
                lastComposing = ""
                if (!preferEditorDelete || lastCandidates.isEmpty()) {
                    lastCandidates = emptyList()
                    lastCandPage = 0
                    lastTotalPages = 0
                    expandedCandidates.clear()
                }
            }
        } else {
            asciiMode = if (handwritingActive) false else snap.asciiMode
            if (currentLangCode == "zh" || currentLangCode == "en") {
                currentLangCode = if (asciiMode) "en" else "zh"
                updateHandwritingToolbar()
            }
            val composingChanged = snap.composing != lastComposing
            lastComposing = snap.composing
            lastCandPage = snap.candPage
            lastTotalPages = snap.totalPages
            lastCandidates = snap.candidates.map {
                CandidateItem(it.id, it.text, page = snap.candPage)
            }
            val inHw = handwritingActive || panel?.isHandwritingMode() == true
            if (inHw) {
                // 手写无拼音 composing：新识别（page0）重置展开缓存，翻页则追加
                if (snap.candPage == 0) {
                    expandedCandidates.clear()
                } else {
                    appendExpanded(lastCandidates, snap.candPage)
                }
            } else if (composingChanged) {
                expandedCandidates.clear()
                if (lastComposing.isNotEmpty()) {
                    appendExpanded(lastCandidates, snap.candPage)
                }
                if (candExpanded && lastComposing.isEmpty()) {
                    // composing 被清空：关闭更多面板
                    candExpanded = false
                }
            } else if (candExpanded || lastComposing.isNotEmpty()) {
                // 弹窗打开或组字中：翻页追加，勿清空（否则列表闪回顶部）
                appendExpanded(lastCandidates, snap.candPage)
            } else {
                expandedCandidates.clear()
            }
        }

        val inHw = handwritingActive || panel?.isHandwritingMode() == true
        // VI/TH/EN 走专用键面 + 引擎查词，不受中文 asciiMode 影响；否则会误清空候选
        val hideAsciiCands =
            asciiMode && !inHw && currentLangCode != "vi" && currentLangCode != "th" && currentLangCode != "en"
        val displayCands = when {
            // 选词后若有离线联想，仍展示；仅在无候选时隐藏
            skipEditorCommands && !inHw && lastCandidates.isEmpty() -> emptyList()
            preferEditorDelete && !inHw && lastCandidates.isEmpty() -> emptyList()
            hideAsciiCands -> emptyList()
            else -> displayCandidates()
        }
        panel?.onSnapshot(
            KeyboardSnapshot(
                editorId = snap.editorId,
                seq = snap.seq,
                composing = if ((skipEditorCommands || preferEditorDelete) && !inHw) {
                    ""
                } else if (inHw) {
                    ""
                } else {
                    lastComposing
                },
                candidates = displayCands,
                candPage = lastCandPage,
                totalPages = lastTotalPages,
                expanded = candExpanded && displayCands.isNotEmpty(),
                asciiMode = if (inHw) false else asciiMode,
            ),
        )
        return appliedEditorMutation
    }

    private fun applyHwCandFromSnap(snap: com.yc.input.native.ArenaSnapshot) {
        asciiMode = false
        lastComposing = ""
        lastCandPage = snap.candPage
        lastTotalPages = snap.totalPages
        lastCandidates = snap.candidates.map {
            CandidateItem(it.id, it.text, page = snap.candPage)
        }
        if (snap.candPage == 0) {
            expandedCandidates.clear()
        } else {
            appendExpanded(lastCandidates, snap.candPage)
        }
        Log.i(
            TAG,
            "hw cand refresh n=${lastCandidates.size} page=${snap.candPage}/${snap.totalPages} top=${lastCandidates.firstOrNull()?.text}",
        )
    }

    private fun discardComposing() {
        val ic = currentInputConnection ?: return
        ic.beginBatchEdit()
        ic.commitText("", 1)
        ic.endBatchEdit()
    }

    /**
     * 若正文变成「拼音+汉字」（commit 未替换 composing），剥掉拼音只留汉字。
     */
    private fun stripLeakedPinyin(pinyin: String, committed: String) {
        if (pinyin.isEmpty() || committed.isEmpty()) return
        val ic = currentInputConnection ?: return
        val before = ic.getTextBeforeCursor(pinyin.length + committed.length + 8, 0)?.toString()
            ?: return
        if (before.endsWith(pinyin + committed)) {
            ic.beginBatchEdit()
            ic.deleteSurroundingText(pinyin.length + committed.length, 0)
            ic.commitText(committed, 1)
            ic.endBatchEdit()
            Log.w(TAG, "stripLeakedPinyin removed '$pinyin' before '$committed'")
        }
    }

    private fun deleteEditorCharBeforeCursor() {
        deleteEditorChars(before = 1, after = 0)
    }

    private fun deleteEditorChars(before: Int, after: Int) {
        if (before <= 0 && after <= 0) return
        val ic = currentInputConnection ?: return

        val selected = ic.getSelectedText(0)
        if (!selected.isNullOrEmpty()) {
            ic.beginBatchEdit()
            ic.commitText("", 1)
            ic.endBatchEdit()
            Log.i(TAG, "deleteEditorChars: clear selection")
            return
        }

        // 只走一条路径：API 返回 true 即结束，禁止再用 getTextBeforeCursor 触发二次 DEL（会叠删）
        val ok = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            ic.deleteSurroundingTextInCodePoints(before, after)
        } else {
            ic.deleteSurroundingText(before, after)
        }
        if (ok) {
            Log.i(TAG, "deleteEditorChars via deleteSurrounding before=$before after=$after")
            return
        }

        // 回退：用选区替换删 1 字，避免 KEYCODE_DEL 在部分编辑器上 DOWN/UP 各删一次
        if (before > 0 && after == 0) {
            val et = ic.getExtractedText(ExtractedTextRequest(), 0)
            val cursor = et?.selectionEnd ?: -1
            if (cursor >= before) {
                ic.beginBatchEdit()
                ic.setSelection(cursor - before, cursor)
                ic.commitText("", 1)
                ic.endBatchEdit()
                Log.i(TAG, "deleteEditorChars via setSelection cursor=$cursor before=$before")
                return
            }
        }

        Log.w(TAG, "deleteEditorChars failed before=$before after=$after")
    }

    /** 用 commitText 替换当前 composing 为汉字（标准上屏）。 */
    private fun commitToEditor(text: String) {
        if (text.isEmpty()) return
        val ic: InputConnection = currentInputConnection ?: run {
            Log.w(TAG, "commitToEditor: no InputConnection")
            return
        }
        val before = ic.getTextBeforeCursor(32, 0)
        ic.beginBatchEdit()
        ic.commitText(text, 1)
        ic.endBatchEdit()
        val after = ic.getTextBeforeCursor(32, 0)
        Log.i(TAG, "commitToEditor '$text' before='$before' after='$after'")
    }

    private fun onLangPicked(opt: LangOption) {
        Log.i(TAG, "lang picked ${opt.code} pack=${opt.packId} ascii=${opt.ascii}")
        if (handwritingActive || panel?.isHandwritingMode() == true) {
            dismissHandwriting()
        }
        when {
            opt.ascii -> {
                // 密码/直通：挂在中文包上 ToggleAscii（不学词）
                if (currentLangCode != "zh") {
                    switchLangPack("zh-pack-v1")
                }
                if (!asciiMode) {
                    submit(YcNative.ACTION_TOGGLE_ASCII)
                    refreshUi()
                }
                currentLangCode = "en"
                asciiMode = true
            }
            opt.code == "zh" -> {
                switchLangPack("zh-pack-v1")
                if (asciiMode) {
                    submit(YcNative.ACTION_TOGGLE_ASCII)
                    refreshUi()
                }
                currentLangCode = "zh"
                asciiMode = false
            }
            else -> {
                val packId = opt.packId ?: return
                switchLangPack(packId)
                currentLangCode = opt.code
                asciiMode = false
            }
        }
        getSharedPreferences("yc_lang", MODE_PRIVATE).edit()
            .putString("preferred_lang", currentLangCode)
            .apply()
        // 不依赖引擎 ReloadKeyboard：壳层按语种强制切换键面
        reloadLayout(layoutIdForLang(currentLangCode))
        updateHandwritingToolbar()
        updateModeLabel()
    }

    /** 各语种默认字母布局 id（与 pack.toml default_layout_id 对齐）。 */
    private fun layoutIdForLang(code: String): String = when (code) {
        "en" -> "layout_en_qwerty"
        "vi" -> "layout_vietnamese"
        "th" -> "layout_thai"
        else -> "layout_pinyin26"
    }

    private fun switchLangPack(packId: String) {
        val hash = hashPackId(packId)
        submit(YcNative.ACTION_SWITCH_LANG, keyCode = hash)
        refreshUi()
        Log.i(TAG, "SwitchLang $packId hash=$hash layout=$currentLayoutId")
    }

    private fun syncLangFromLayout(layoutId: String) {
        when {
            layoutId.contains("vietnamese") || layoutId.contains("telex") -> {
                currentLangCode = "vi"
                asciiMode = false
            }
            layoutId.contains("thai") -> {
                currentLangCode = "th"
                asciiMode = false
            }
            layoutId.contains("pinyin") || layoutId.contains("qwerty") -> {
                if (currentLangCode != "en") {
                    currentLangCode = if (asciiMode) "en" else "zh"
                }
            }
        }
        updateHandwritingToolbar()
    }

    /** 声调作用于 composing 末尾可加调元音；已带调元音则停止；无 composing 则改光标前一字符。 */
    private fun applyViTone(toneId: String) {
        val map = VI_TONE_MAP[toneId] ?: return
        if (lastComposing.isNotEmpty()) {
            val chars = lastComposing.toCharArray()
            var changed = false
            for (i in chars.indices.reversed()) {
                val ch = chars[i]
                val repl = map[ch]
                if (repl != null) {
                    chars[i] = repl
                    changed = true
                    break
                }
                if (isViVowelFamily(ch)) break
            }
            if (!changed) return
            val oldLen = lastComposing.length
            val newText = String(chars)
            // 同步引擎：退格清空再逐字喂入（保持查词与 composing 一致）
            repeat(oldLen) { submit(YcNative.ACTION_BACKSPACE) }
            for (ch in newText) {
                submit(YcNative.ACTION_KEY_PRESS, ch.code)
            }
            refreshUi()
            return
        }
        val ic = currentInputConnection ?: return
        val before = ic.getTextBeforeCursor(16, 0)?.toString() ?: return
        if (before.isEmpty()) return
        for (i in before.indices.reversed()) {
            val ch = before[i]
            val repl = map[ch]
            if (repl != null) {
                val tail = before.substring(i)
                ic.beginBatchEdit()
                ic.deleteSurroundingText(tail.length, 0)
                ic.commitText(repl + tail.drop(1), 1)
                ic.endBatchEdit()
                return
            }
            if (isViVowelFamily(ch)) break
        }
    }

    /** 越南元音族（含已带调），用于声调扫描边界。 */
    private fun isViVowelFamily(ch: Char): Boolean {
        val s = "aăâeêioôơuưyAĂÂEÊIOÔƠUƯY" +
            "áàảãạắằẳẵặấầẩẫậéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ" +
            "ÁÀẢÃẠẮẰẲẴẶẤẦẨẪẬÉÈẺẼẸẾỀỂỄỆÍÌỈĨỊÓÒỎÕỌỐỒỔỖỘỚỜỞỠỢÚÙỦŨỤỨỪỬỮỰÝỲỶỸỴ"
        return ch in s
    }
}
