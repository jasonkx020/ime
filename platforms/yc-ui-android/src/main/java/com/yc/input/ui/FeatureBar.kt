package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView

/** 娱乐/效率入口：设置、皮肤、表情、话术、AI。AI 为唯一 AI 总入口。 */
class FeatureBar(context: Context) : HorizontalScrollView(context) {
    private val row = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(dp(10), dp(4), dp(10), dp(4))
        gravity = Gravity.CENTER_VERTICAL
    }
    private var tokens = ThemeTokens.light()
    private var onItem: ((String) -> Unit)? = null
    private var activeLabel: String? = null
    private val items = listOf("设置", "皮肤", "表情", "话术", "AI")

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

    /** 单选高亮：当前使用中的入口；null 表示全部普通态。 */
    fun setActiveItem(label: String?) {
        activeLabel = label
        applyActiveStyles()
    }

    private fun rebuild() {
        row.removeAllViews()
        items.forEach { label ->
            val tv = TextView(context).apply {
                text = label
                tag = label
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                setPadding(dp(12), dp(6), dp(12), dp(6))
                setOnClickListener { onItem?.invoke(label) }
            }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            lp.rightMargin = dp(8)
            row.addView(tv, lp)
        }
        applyActiveStyles()
    }

    private fun applyActiveStyles() {
        for (i in 0 until row.childCount) {
            val v = row.getChildAt(i) as? TextView ?: continue
            val label = v.tag as? String ?: continue
            val on = label == activeLabel
            v.setTextColor(if (on) tokens.aiPrimaryText else tokens.toolbarText)
            v.typeface = if (on) Typeface.DEFAULT_BOLD else Typeface.DEFAULT
            v.background = GradientDrawable().apply {
                setColor(if (on) tokens.aiPrimaryBg else tokens.aiChipBg)
                cornerRadius = dp(8).toFloat()
                if (!on) setStroke(dp(1), tokens.aiChipBorder)
            }
        }
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
