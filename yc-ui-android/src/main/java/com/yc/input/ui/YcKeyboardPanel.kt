package com.yc.input.ui

import android.content.Context
import android.widget.LinearLayout

class YcKeyboardPanel(context: Context) : LinearLayout(context), UiBinder {

    private val candBar = SamsungCandBar(context)
    private val toolbar = SamsungToolbar(context)
    private val keyView = SamsungKeyView(context)
    private val tokens = ThemeTokens()

    init {
        orientation = VERTICAL
        val candLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(52))
        val toolLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(36))
        val keyLp = LayoutParams(LayoutParams.MATCH_PARENT, dp(220))
        addView(candBar, candLp)
        addView(toolbar, toolLp)
        addView(keyView, keyLp)
        applyTheme(tokens)
    }

    fun setLayoutRows(rows: List<List<KeyDef>>) {
        keyView.setLayoutRows(rows)
    }

    override fun onSnapshot(snapshot: KeyboardSnapshot) {
        candBar.render(snapshot)
        keyView.render(snapshot)
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

    override fun setToolbarListener(listener: (String) -> Unit) {
        toolbar.setOnItemClick(listener)
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}
