package com.yc.input.ui

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.GridLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** 系统 Emoji 面板（不依赖运营包）。 */
class EmojiPanel(context: Context) : LinearLayout(context) {
    private val grid = GridLayout(context).apply {
        columnCount = 8
        setPadding(dp(8), dp(8), dp(8), dp(8))
    }
    private var tokens = ThemeTokens.light()
    private var onPick: ((String) -> Unit)? = null

    private val emojis = listOf(
        "😀", "😁", "😂", "🤣", "😊", "😍", "😘", "😜",
        "🤔", "😎", "😢", "😭", "😡", "👍", "👎", "🙏",
        "👏", "🔥", "✨", "🎉", "❤️", "💯", "⭐", "🌙",
        "✅", "❌", "⚡", "📌", "📎", "💡", "🎵", "🎮",
    )

    init {
        orientation = VERTICAL
        visibility = GONE
        val scroll = ScrollView(context)
        scroll.addView(grid)
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
        rebuild()
    }

    fun setOnPick(listener: (String) -> Unit) {
        onPick = listener
    }

    fun show(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.panelBg)
        rebuild()
        visibility = VISIBLE
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    private fun rebuild() {
        grid.removeAllViews()
        emojis.forEach { e ->
            val tv = TextView(context).apply {
                text = e
                gravity = Gravity.CENTER
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 22f)
                setPadding(dp(6), dp(10), dp(6), dp(10))
                background = GradientDrawable().apply {
                    setColor(tokens.keyNormal)
                    cornerRadius = dp(8).toFloat()
                }
                setOnClickListener { onPick?.invoke(e) }
            }
            val lp = GridLayout.LayoutParams().apply {
                width = 0
                height = LayoutParams.WRAP_CONTENT
                columnSpec = GridLayout.spec(GridLayout.UNDEFINED, 1f)
                setMargins(dp(3), dp(3), dp(3), dp(3))
            }
            grid.addView(tv, lp)
        }
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
