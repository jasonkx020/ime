package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView

/** 底部语言选择列表（对齐参考效果 inputLangPanel）。 */
class LangPicker(context: Context) : LinearLayout(context) {

    private var onPick: ((LangOption) -> Unit)? = null
    private var selectedCode: String = "zh"
    private var tokens = ThemeTokens.light()

    init {
        orientation = VERTICAL
        setBackgroundColor(tokens.panelBg)
        setPadding(dp(16), dp(14), dp(16), dp(20))
        visibility = View.GONE
        elevation = dp(10).toFloat()
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(if (t.isDark) 0xFF1E1E1E.toInt() else 0xFFFFFFFF.toInt())
    }

    fun setOnPickListener(listener: (LangOption) -> Unit) {
        onPick = listener
    }

    fun show(options: List<LangOption>, selectedCode: String) {
        this.selectedCode = selectedCode
        removeAllViews()

        val titleColor = if (tokens.isDark) 0xFFE8E8E8.toInt() else 0xFF333333.toInt()
        val closeColor = if (tokens.isDark) 0xFF777777.toInt() else 0xFF999999.toInt()
        val rowBg = if (tokens.isDark) 0xFF2A2A2A.toInt() else 0xFFF8F8F8.toInt()
        val rowActive = tokens.candSelectedBg
        val textColor = if (tokens.isDark) 0xFFDDDDDD.toInt() else 0xFF222222.toInt()

        val header = LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(12))
        }
        header.addView(
            TextView(context).apply {
                text = "选择输入语言"
                setTextColor(titleColor)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                typeface = Typeface.DEFAULT_BOLD
                layoutParams = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f)
            },
        )
        header.addView(
            TextView(context).apply {
                text = "×"
                setTextColor(closeColor)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 22f)
                setPadding(dp(8), 0, dp(4), 0)
                setOnClickListener { hide() }
            },
        )
        addView(header, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))

        options.forEach { opt ->
            val selected = opt.code == selectedCode
            val row = LinearLayout(context).apply {
                orientation = HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(dp(14), dp(14), dp(14), dp(14))
                setBackgroundColor(if (selected) rowActive else rowBg)
                setOnClickListener {
                    onPick?.invoke(opt)
                    hide()
                }
            }
            val label = TextView(context).apply {
                text = "${opt.name}  ${opt.native}"
                setTextColor(if (selected) tokens.composingText else textColor)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
                layoutParams = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f)
            }
            val check = TextView(context).apply {
                text = if (selected) "✓" else ""
                setTextColor(tokens.composingText)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            }
            row.addView(label)
            row.addView(check)
            val lp = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
            lp.bottomMargin = dp(8)
            addView(row, lp)
        }
        visibility = View.VISIBLE
    }

    fun hide() {
        visibility = View.GONE
    }

    fun isShowing(): Boolean = visibility == View.VISIBLE

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}
