package com.yc.input

import android.inputmethodservice.InputMethodService
import android.os.Build
import android.util.Log
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import com.yc.input.native.ArenaCommand
import com.yc.input.native.YcNative
import com.yc.input.ui.CandidateItem
import com.yc.input.ui.KeyAction
import com.yc.input.ui.KeyDef
import com.yc.input.ui.KeyboardSnapshot
import com.yc.input.ui.LayoutLoader
import com.yc.input.ui.YcKeyboardPanel
import java.io.File

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
    }

    override fun onCreateInputView(): View {
        val kb = YcKeyboardPanel(this)
        panel = kb
        kb.setKeyListener { key -> onKey(key) }
        kb.setCandidateListener { cand -> onCandidate(cand) }
        kb.setToolbarListener { item -> Log.i(TAG, "toolbar: $item") }
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
        skipEditorCommands = false
        preferEditorDelete = false
        composingRegionStart = -1
        composingRegionEnd = -1
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
        preferEditorDelete = false
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
                commitPinyinAsRawText()
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
                if (isEngineLetter(code)) {
                    submit(YcNative.ACTION_KEY_PRESS, code)
                    refreshUi()
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
        Log.i(TAG, "select id=${cand.id} text='$text' raw='${cand.text}' pinyin='$pinyin'")

        // 1) 汉字替换拼音并提交到正文
        commitToEditor(text)
        // 2) 若拼音曾作为正文泄漏，剥掉
        stripLeakedPinyin(pinyin, text)
        // 3) 通知引擎选词 + 重置会话缓存
        submit(YcNative.ACTION_SELECT_CANDIDATE, candidateId = cand.id)
        clearInputCache()
        // 4) 之后退格只删正文（上屏已成功则不再 commitText("")）
        enterEditorDeleteMode(commitSucceeded = textBeforeEndsWith(text))

        val before = currentInputConnection?.getTextBeforeCursor(32, 0)
        Log.i(
            TAG,
            "after select textBefore='$before' span=$composingRegionStart..$composingRegionEnd",
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
        if (!commitSucceeded && hasComposingRegion()) {
            resolveStaleComposingSpan(composingRegionEnd - composingRegionStart)
        }
        panel?.onSnapshot(
            KeyboardSnapshot(editorId, lastSeq, composing = "", candidates = emptyList()),
        )
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
        submit(YcNative.ACTION_INIT)
        skipEditorCommands = true
        try {
            refreshUi()
        } finally {
            skipEditorCommands = false
        }
        lastComposing = ""
        lastCandidates = emptyList()
        panel?.onSnapshot(
            KeyboardSnapshot(editorId, lastSeq, composing = "", candidates = emptyList()),
        )
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
                        if (cmd.layoutId.isNotEmpty()) {
                            reloadLayout(cmd.layoutId)
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
            lastComposing = ""
            lastCandidates = emptyList()
        } else {
            lastComposing = snap.composing
            lastCandidates = snap.candidates.map { CandidateItem(it.id, it.text) }
        }

        panel?.onSnapshot(
            KeyboardSnapshot(
                editorId = snap.editorId,
                seq = snap.seq,
                composing = lastComposing,
                candidates = lastCandidates,
            ),
        )
        return appliedEditorMutation
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

    private companion object {
        const val TAG = "YcImeService"
    }
}
