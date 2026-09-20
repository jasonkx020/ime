package com.yc.input.ui

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView

/** 娱乐/效率入口：设置、皮肤、表情、话术、手写。 */
class FeatureBar(context: Context) : HorizontalScrollView(context) {
    private val row = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(dp(10), dp(4), dp(10), dp(4))
        gravity = Gravity.CENTER_VERTICAL
    }
    private var tokens = ThemeTokens.light()
    private var onItem: ((String) -> Unit)? = null
    private val items = listOf("设置", "皮肤", "表情", "话术", "翻译", "AI", "手写")

    init {
        isHorizontalScrollBarEnabled = false
        addView(row)
        rebuild()
    }

    fun setOnItemClick(listener: (String) -> Unit) {
        onItem = listener
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.toolbarBg)
        rebuild()
    }

    fun setItemEnabled(label: String, enabled: Boolean) {
        for (i in 0 until row.childCount) {
            val v = row.getChildAt(i) as? TextView ?: continue
            if (v.tag == label) {
                v.isEnabled = enabled
                v.alpha = if (enabled) 1f else 0.35f
            }
        }
    }

    private fun rebuild() {
        row.removeAllViews()
        items.forEach { label ->
            val tv = TextView(context).apply {
                text = label
                tag = label
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                setTextColor(tokens.toolbarText)
                setPadding(dp(12), dp(6), dp(12), dp(6))
                background = GradientDrawable().apply {
                    setColor(tokens.aiChipBg)
                    cornerRadius = dp(8).toFloat()
                    setStroke(dp(1), tokens.aiChipBorder)
                }
                setOnClickListener { onItem?.invoke(label) }
            }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            lp.rightMargin = dp(8)
            row.addView(tv, lp)
        }
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
