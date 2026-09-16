package com.yc.input.ui

import android.content.Context
import android.view.View
import android.widget.LinearLayout

class YcKeyboardPanel(context: Context) : LinearLayout(context), UiBinder {

    private val candBar = SamsungCandBar(context)
    private val toolbar = SamsungToolbar(context)
    private val keyView = SamsungKeyView(context)
    private val handwritingPad = HandwritingPad(context)
    private val tokens = ThemeTokens()

    private val candCollapsedH = dp(52)
    private val candExpandedH = dp(140)
    private val keyH = dp(280)

    private var handwritingMode = false
    private val inputSlotLp: LayoutParams

    init {
        orientation = VERTICAL
        val candLp = LayoutParams(LayoutParams.MATCH_PARENT, candCollapsedH)
        val toolLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(36))
        inputSlotLp = LayoutParams(LayoutParams.MATCH_PARENT, keyH)
        addView(candBar, candLp)
        addView(toolbar, toolLp)
        addView(keyView, inputSlotLp)
        addView(
            handwritingPad,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT),
        )
        handwritingPad.visibility = View.GONE
        applyTheme(tokens)
    }

    private var shifted = false

    fun setLayoutRows(rows: List<List<KeyDef>>) {
        shifted = false
        keyView.setLayoutRows(rows)
    }

    fun setShifted(shifted: Boolean) {
        this.shifted = shifted
        keyView.setLayoutRows(Layout26Pinyin.rows(shifted))
    }

    fun toggleShift(): Boolean {
        setShifted(!shifted)
        return shifted
    }

    fun isShifted(): Boolean = shifted

    fun isHandwritingMode(): Boolean = handwritingMode

    fun setHandwritingMode(enabled: Boolean) {
        if (handwritingMode == enabled) return
        handwritingMode = enabled
        if (enabled) {
            keyView.visibility = View.GONE
            handwritingPad.visibility = View.VISIBLE
            val lp = handwritingPad.layoutParams as LayoutParams
            lp.width = LayoutParams.MATCH_PARENT
            lp.height = LayoutParams.WRAP_CONTENT
            handwritingPad.layoutParams = lp
        } else {
            handwritingPad.visibility = View.GONE
            handwritingPad.clearInk()
            keyView.visibility = View.VISIBLE
            val lp = keyView.layoutParams as LayoutParams
            lp.height = keyH
            keyView.layoutParams = lp
        }
        requestLayout()
    }

    fun clearHandwritingInk() = handwritingPad.clearInk()

    fun setHandwritingStrokeListener(listener: (HandwritingPad.StrokePayload) -> Unit) {
        handwritingPad.setOnStrokeListener(listener)
    }

    fun setHandwritingRecognizeListener(listener: () -> Unit) {
        handwritingPad.setOnRecognizeListener(listener)
    }

    fun setHandwritingUndoListener(listener: () -> Unit) {
        handwritingPad.setOnUndoListener(listener)
    }

    fun setHandwritingClearListener(listener: () -> Unit) {
        handwritingPad.setOnClearListener(listener)
    }

    fun setHandwritingDismissListener(listener: () -> Unit) {
        handwritingPad.setOnDismissListener(listener)
    }

    fun setHandwritingModeChangedListener(listener: (Boolean) -> Unit) {
        handwritingPad.setOnModeChangedListener(listener)
    }

    fun isHandwritingContinuous(): Boolean = handwritingPad.isContinuous()

    fun setHandwritingRecognizing(active: Boolean) {
        handwritingPad.setRecognizing(active)
    }

    fun setToolbarItemEnabled(item: String, enabled: Boolean) {
        toolbar.setItemEnabled(item, enabled)
    }

    override fun onSnapshot(snapshot: KeyboardSnapshot) {
        setCandidateExpanded(snapshot.expanded)
        // 手写态不展示拼音 composing
        val uiSnap =
            if (handwritingMode) {
                snapshot.copy(composing = "", asciiMode = false, expanded = false)
            } else {
                snapshot
            }
        candBar.render(uiSnap)
        if (!handwritingMode) {
            keyView.render(snapshot)
        }
    }

    override fun setCandidateExpanded(expanded: Boolean) {
        if (handwritingMode) return
        val lp = candBar.layoutParams as LayoutParams
        val target = if (expanded) candExpandedH else candCollapsedH
        if (lp.height != target) {
            lp.height = target
            candBar.layoutParams = lp
            requestLayout()
        }
    }

    override fun applyTheme(tokens: ThemeTokens) {
        candBar.applyTheme(tokens)
        toolbar.applyTheme(tokens)
        keyView.applyTheme(tokens)
        handwritingPad.applyTheme(tokens)
        setBackgroundColor(tokens.keyboardBg)
    }

    override fun setKeyListener(listener: (KeyDef) -> Unit) {
        keyView.setOnKeyListener(listener)
    }

    override fun setCandidateListener(listener: (CandidateItem) -> Unit) {
        candBar.setOnCandidateListener(listener)
    }

    override fun setPageListener(listener: (Int) -> Unit) {
        candBar.setOnPageListener(listener)
    }

    override fun setExpandListener(listener: () -> Unit) {
        candBar.setOnExpandListener(listener)
    }

    override fun setNeedMoreListener(listener: () -> Unit) {
        candBar.setOnNeedMoreListener(listener)
    }

    override fun setToolbarListener(listener: (String) -> Unit) {
        toolbar.setOnItemClick(listener)
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}
