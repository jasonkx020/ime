package com.yc.input.ui

/** Samsung One UI 6.x light theme tokens (KEYBOARD_UI_DESIGN §11.1). */
data class ThemeTokens(
    val keyboardBg: Int = 0xFFE8EAED.toInt(),
    val keyNormal: Int = 0xFFFFFFFF.toInt(),
    val keyUtility: Int = 0xFFDDE0E4.toInt(),
    val keyAccent: Int = 0xFF1A73E8.toInt(),
    val keyPressed: Int = 0xFFC8CCD2.toInt(),
    val candText: Int = 0xFF202124.toInt(),
    val candSelectedBg: Int = 0xFFFFFFFF.toInt(),
    val candSelectedBorder: Int = 0xFF1A73E8.toInt(),
    val composingText: Int = 0xFF1A73E8.toInt(),
    val toolbarText: Int = 0xFF5F6368.toInt(),
    val keyRadiusDp: Float = 12f,
    val keyFontSp: Float = 16f,
    val candFontSp: Float = 15f,
)
