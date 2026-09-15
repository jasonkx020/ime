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
import com.yc.input.handwriting.HandwrittenEngine
import com.yc.input.native.ArenaCommand
import com.yc.input.native.YcNative
import com.yc.input.ui.CandidateItem
import com.yc.input.ui.HandwritingPad
import com.yc.input.ui.KeyAction
import com.yc.input.ui.KeyDef
import com.yc.input.ui.KeyboardSnapshot
import com.yc.input.ui.LayoutLoader
import com.yc.input.ui.YcKeyboardPanel
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
    private var hwSessionStrokeId = 0L
    private var hwPasswordBlocked = false
    private val hwStrokes = mutableListOf<HandwritingPad.StrokePayload>()
    private val hwHandler = Handler(Looper.getMainLooper())
    private var hwDebounce: Runnable? = null
    private val hwRecognizeGen = AtomicInteger(0)
    private val hwExecutor = Executors.newSingleThreadExecutor()
    private var hwEngine: HandwrittenEngine? = null

    override fun onCreate() {
        super.onCreate()
        if (!coreInited) {
            val rc = YcNative.ycCoreInit(filesDir.absolutePath)
            coreInited = rc == YcNative.OK
            Log.i(TAG, "ycCoreInit -> $rc")
            if (coreInited) {
                ensureZhPack()
            }
        }
        hwEngine = HandwrittenEngine(applicationContext)
        hwExecutor.execute { hwEngine?.ensureLoaded() }
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
        kb.setHandwritingStrokeListener { stroke -> onHwStroke(stroke) }
        kb.setHandwritingRecognizeListener { onHwRecognize() }
        kb.setHandwritingUndoListener { onHwUndo() }
        kb.setHandwritingClearListener { onHwClear() }
        kb.setHandwritingDismissListener { dismissHandwriting() }
        reloadLayout(currentLayoutId)
        return kb
    }

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        if (!coreInited) return
        ensureZhPack()
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
        handwritingActive = false
        hwSessionStrokeId = 0L
        hwStrokes.clear()
        cancelHwDebounce()
        hwPasswordBlocked = isPasswordInputType(inputType)
        panel?.setToolbarItemEnabled("手写", !hwPasswordBlocked)
        clearCandScrollBuffer()
        skipEditorCommands = false
        preferEditorDelete = false
        composingRegionStart = -1
        composingRegionEnd = -1
        panel?.setHandwritingMode(false)
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

    /** 去掉 NUL/替换符/控制字符，避免脏候选上屏后再被误清。 */
    private fun sanitizeCommitText(raw: String): String =
        raw.filter { ch ->
            ch != '\u0000' && ch != '\uFFFD' && !ch.isISOControl()
        }.trim()

    private fun ensureZhPack() {
        val installed = File(filesDir, "langpacks/zh-pack-v1")
        if (installed.isDirectory && File(installed, "manifest.fb").isFile) {
            val rc = YcNative.ycCoreSyncLangPacks()
            Log.i(TAG, "ycCoreSyncLangPacks -> $rc")
            return
        }
        val packFile = File(filesDir, "zh-pack-v1.imepack")
        try {
            assets.open("langpacks/zh-pack-v1.imepack").use { input ->
                packFile.outputStream().use { output -> input.copyTo(output) }
            }
            val rc = YcNative.ycCoreInstallLangpack(packFile.absolutePath)
            Log.i(TAG, "ycCoreInstallLangpack -> $rc")
        } catch (e: Exception) {
            Log.w(TAG, "zh-pack not in assets", e)
            YcNative.ycCoreSyncLangPacks()
        }
    }

    private fun hasInputCache(): Boolean =
        lastComposing.isNotEmpty() || lastCandidates.isNotEmpty()

    private fun onKey(key: KeyDef) {
        when (key.action) {
            KeyAction.Backspace -> {
                handleBackspace()
                return
            }
            KeyAction.Shift -> {
                panel?.toggleShift()
                return
            }
            KeyAction.Search -> {
                if (lastComposing.isNotEmpty()) {
                    commitPinyinAsRawText()
                } else {
                    performEditorImeAction()
                }
                return
            }
            KeyAction.Globe -> {
                submit(YcNative.ACTION_TOGGLE_ASCII)
                refreshUi()
                Log.i(TAG, "toggle ascii -> $asciiMode")
                return
            }
            KeyAction.Space -> {
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
                preferEditorDelete = false
                val code = key.keyCode ?: return
                if (isEngineLetter(code) || (asciiMode && isAsciiComposable(code))) {
                    // 继续输入拼音时收起展开面板，避免跨查询混页
                    if (candExpanded) collapseCandExpand()
                    if (isEngineLetter(code)) {
                        submit(YcNative.ACTION_KEY_PRESS, code)
                        refreshUi()
                    } else {
                        // ASCII 标点：直通上屏（保留英文 composing）
                        currentInputConnection?.commitText(code.toChar().toString(), 1)
                    }
                } else {
                    if (hasInputCache()) {
                        discardComposing()
                        clearInputCache()
                    }
                    currentInputConnection?.commitText(code.toChar().toString(), 1)
                }
                if (panel?.isShifted() == true) {
                    panel?.setShifted(false)
                }
                return
            }
            else -> Log.i(TAG, "key stub: ${key.label}")
        }
        refreshUi()
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
        clearInputCache()
        enterEditorDeleteMode(commitSucceeded = textBeforeEndsWith(text))

        val before = currentInputConnection?.getTextBeforeCursor(32, 0)
        Log.i(
            TAG,
            "after select textBefore='$before' span=$composingRegionStart..$composingRegionEnd",
        )
    }

    private fun onToolbar(item: String) {
        when (item) {
            "手写" -> {
                if (hwPasswordBlocked) {
                    Log.w(TAG, "handwriting disabled for password field")
                    return
                }
                openHandwriting()
            }
            else -> Log.i(TAG, "toolbar: $item")
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
        hwEngine?.ensureLoaded()
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
                    engine.recognizeStrokes(strokes, continuous, topK = HandwrittenEngine.TOP_K)
                } else {
                    null
                }
            val elapsed = System.currentTimeMillis() - started
            hwHandler.post {
                if (gen != hwRecognizeGen.get()) return@post
                panel?.setHandwritingRecognizing(false)
                if (output == null || output.candidates.isEmpty()) {
                    Log.w(TAG, "Handwritten miss; falling back to template Recognize ($elapsed ms)")
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
                    "Handwritten apply n=${texts.size} cloud=$flags rc=$rc ${elapsed}ms top=${texts.firstOrNull()}",
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
                Log.w(TAG, "Handwritten slow: ${elapsed}ms")
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
        private const val HW_RECOGNIZE_TIMEOUT_MS = 800L
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
        if (lastTotalPages <= 1 && lastCandidates.isEmpty()) return
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
        // 保留已加载候选，收起后仍可跟手横滑
        panel?.setCandidateExpanded(false)
    }

    private fun clearCandScrollBuffer() {
        candExpanded = false
        expandedCandidates.clear()
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
        lastCandidates = emptyList()
        lastCandPage = 0
        lastTotalPages = 0
        clearCandScrollBuffer()
        if (!commitSucceeded && hasComposingRegion()) {
            resolveStaleComposingSpan(composingRegionEnd - composingRegionStart)
        }
        pushCandSnapshot()
        Log.i(TAG, "enterEditorDeleteMode commitSucceeded=$commitSucceeded")
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
        val rows = LayoutLoader.load(filesDir, layoutId)
        panel?.setLayoutRows(rows)
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
                                panel?.setHandwritingMode(true)
                                handwritingActive = true
                            }
                            else -> {
                                panel?.setHandwritingMode(false)
                                handwritingActive = false
                                val id = cmd.layoutId.ifEmpty { "layout_pinyin26" }
                                reloadLayout(id)
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
            // 拼音上屏后 preferEditorDelete 会吞掉候选；手写态必须继续刷新 CandBar
            val inHw = handwritingActive || panel?.isHandwritingMode() == true
            if (!inHw) {
                lastComposing = ""
                lastCandidates = emptyList()
                lastCandPage = 0
                lastTotalPages = 0
                expandedCandidates.clear()
            } else {
                applyHwCandFromSnap(snap)
            }
        } else {
            asciiMode = if (handwritingActive) false else snap.asciiMode
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
            } else if (lastComposing.isNotEmpty()) {
                appendExpanded(lastCandidates, snap.candPage)
            } else {
                expandedCandidates.clear()
            }
        }

        val inHw = handwritingActive || panel?.isHandwritingMode() == true
        val displayCands = when {
            skipEditorCommands && !inHw -> emptyList()
            preferEditorDelete && !inHw -> emptyList()
            asciiMode && !inHw -> emptyList()
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
}
