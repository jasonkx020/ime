package com.yc.input.ui

interface KeyView {
    fun render(snapshot: KeyboardSnapshot)
    fun applyTheme(tokens: ThemeTokens)
    fun setOnKeyListener(listener: (KeyDef) -> Unit)
}

interface CandBar {
    fun render(snapshot: KeyboardSnapshot)
    fun applyTheme(tokens: ThemeTokens)
    fun setOnCandidateListener(listener: (CandidateItem) -> Unit)
}

interface ToolbarView {
    fun applyTheme(tokens: ThemeTokens)
    fun setOnItemClick(listener: (String) -> Unit)
}

interface UiBinder {
    fun onSnapshot(snapshot: KeyboardSnapshot)
    fun applyTheme(tokens: ThemeTokens)
    fun setKeyListener(listener: (KeyDef) -> Unit)
    fun setCandidateListener(listener: (CandidateItem) -> Unit)
    fun setToolbarListener(listener: (String) -> Unit)
}
