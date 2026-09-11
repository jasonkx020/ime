package com.yc.input



import android.inputmethodservice.InputMethodService

import android.util.Log

import android.view.View

import android.view.inputmethod.EditorInfo

import com.yc.input.native.ArenaCommand

import com.yc.input.native.YcNative

import com.yc.input.ui.CandidateItem

import com.yc.input.ui.KeyAction

import com.yc.input.ui.KeyDef

import com.yc.input.ui.KeyboardSnapshot

import com.yc.input.ui.LayoutLoader

import com.yc.input.ui.YcKeyboardPanel

import java.io.File



class YcImeService : InputMethodService() {



    private var editorId: Long = 0

    private var clientSeq: Long = 0

    private var lastSeq: Long = -1

    private var panel: YcKeyboardPanel? = null

    private var coreInited = false

    private var currentLayoutId: String = "layout_pinyin26"



    override fun onCreate() {

        super.onCreate()

        if (!coreInited) {

            val rc = YcNative.ycCoreInit(filesDir.absolutePath)

            coreInited = rc == YcNative.OK

            Log.i(TAG, "ycCoreInit -> $rc")

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

        if (editorId != 0L) {

            YcNative.ycSessionStop(editorId, 0)

        }

        val inputType = attribute?.inputType ?: 0

        editorId = YcNative.ycSessionBeginWithInput(1L, inputType)

        clientSeq = 0

        lastSeq = -1

        submit(YcNative.ACTION_INIT)

        refreshUi()

    }



    override fun onFinishInput() {

        if (editorId != 0L) {

            YcNative.ycSessionStop(editorId, 0)

            editorId = 0

        }

        super.onFinishInput()

    }



    private fun onKey(key: KeyDef) {

        when (key.action) {

            KeyAction.Backspace -> submit(YcNative.ACTION_BACKSPACE)

            KeyAction.Letter, KeyAction.Space -> {

                val code = key.keyCode ?: return

                submit(YcNative.ACTION_KEY_PRESS, code)

            }

            else -> Log.i(TAG, "key stub: ${key.label}")

        }

        refreshUi()

    }



    private fun onCandidate(cand: CandidateItem) {

        submit(YcNative.ACTION_SELECT_CANDIDATE, candidateId = cand.id)

        refreshUi()

    }



    private fun submit(actionType: Int, keyCode: Int = 0, candidateId: Int = 0) {

        if (editorId == 0L) return

        clientSeq++

        val rc = YcNative.ycHotSubmit(

            YcNative.buildAction(editorId, clientSeq, actionType, keyCode, candidateId),

        )

        if (rc != YcNative.OK) {

            Log.w(TAG, "ycHotSubmit rc=$rc")

        }

    }



    private fun reloadLayout(layoutId: String) {

        currentLayoutId = layoutId

        val rows = LayoutLoader.load(filesDir, layoutId)

        panel?.setLayoutRows(rows)

    }



    private fun refreshUi() {

        val snap = YcNative.readArena() ?: return

        if (snap.editorId != editorId) return

        if (snap.seq == lastSeq) return

        lastSeq = snap.seq



        val ic = currentInputConnection

        for (cmd in snap.commands) {

            when (cmd) {

                is ArenaCommand.Commit -> ic?.commitText(cmd.text, 1)

                is ArenaCommand.SetComposing -> ic?.setComposingText(cmd.text, 1)

                is ArenaCommand.FinishComposing -> ic?.finishComposingText()

                is ArenaCommand.ReloadKeyboard -> {

                    if (cmd.layoutId.isNotEmpty()) {

                        reloadLayout(cmd.layoutId)

                    }

                }

            }

        }



        panel?.onSnapshot(

            KeyboardSnapshot(

                editorId = snap.editorId,

                seq = snap.seq,

                composing = snap.composing,

                candidates = snap.candidates.map { CandidateItem(it.id, it.text) },

            ),

        )

    }



    private companion object {

        const val TAG = "YcImeService"

    }

}


