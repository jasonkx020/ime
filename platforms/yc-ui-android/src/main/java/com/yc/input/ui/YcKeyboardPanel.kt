package com.yc.input.ui

import android.content.Context
import android.widget.LinearLayout

class YcKeyboardPanel(context: Context) : LinearLayout(context), UiBinder {

    private val candBar = SamsungCandBar(context)
    private val toolbar = SamsungToolbar(context)
    private val keyView = SamsungKeyView(context)
    private val tokens = ThemeTokens()

    private val candCollapsedH = dp(52)
    private val candExpandedH = dp(140)

    init {
        orientation = VERTICAL
        val candLp = LayoutParams(LayoutParams.MATCH_PARENT, candCollapsedH)
        val toolLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(36))
        val keyLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(280))
        addView(candBar, candLp)
        addView(toolbar, toolLp)
        addView(keyView, keyLp)
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

    override fun onSnapshot(snapshot: KeyboardSnapshot) {
        setCandidateExpanded(snapshot.expanded)
        candBar.render(snapshot)
        keyView.render(snapshot)
    }

    override fun setCandidateExpanded(expanded: Boolean) {
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
