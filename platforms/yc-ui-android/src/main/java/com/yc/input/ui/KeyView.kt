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
    fun setOnPageListener(listener: (Int) -> Unit)
    fun setOnExpandListener(listener: () -> Unit)
    fun setOnNeedMoreListener(listener: () -> Unit)
}

interface ToolbarView {
    fun applyTheme(tokens: ThemeTokens)
    fun setOnItemClick(listener: (String) -> Unit)
    fun setItemEnabled(item: String, enabled: Boolean) {}
}

interface UiBinder {
    fun onSnapshot(snapshot: KeyboardSnapshot)
    fun applyTheme(tokens: ThemeTokens)
    fun setKeyListener(listener: (KeyDef) -> Unit)
    fun setCandidateListener(listener: (CandidateItem) -> Unit)
    fun setPageListener(listener: (Int) -> Unit)
    fun setExpandListener(listener: () -> Unit)
    fun setNeedMoreListener(listener: () -> Unit)
    fun setToolbarListener(listener: (String) -> Unit)
    fun setCandidateExpanded(expanded: Boolean)
}
