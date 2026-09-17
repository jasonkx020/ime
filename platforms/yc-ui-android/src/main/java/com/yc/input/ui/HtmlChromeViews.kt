package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.AttributeSet
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView

/** 顶栏：模式 + 候选槽 + AI 点 + 收起（对齐参考效果 top-toolbar）。 */
class TopToolbar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : LinearLayout(context, attrs) {

    private val modeBtn: TextView
    private val candHost: FrameLayout
    private val aiDot: TextView
    private val collapseBtn: TextView
    private var tokens = ThemeTokens.light()
    private var onModeClick: (() -> Unit)? = null
    private var onCollapseClick: (() -> Unit)? = null

    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(8), dp(4), dp(8), dp(4))
        setBackgroundColor(tokens.toolbarBg)

        modeBtn = TextView(context).apply {
            text = "☰ 拼音"
            setTextColor(tokens.toolbarText)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            typeface = Typeface.DEFAULT_BOLD
            setPadding(dp(8), dp(8), dp(8), dp(8))
            setBackgroundColor(0x00000000)
            setOnClickListener { onModeClick?.invoke() }
        }
        candHost = FrameLayout(context).apply {
            layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT, 1f)
        }
        aiDot = TextView(context).apply {
            text = "✨"
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setPadding(dp(6), dp(4), dp(6), dp(4))
        }
        collapseBtn = TextView(context).apply {
            text = "∨"
            setTextColor(tokens.toolbarText)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setPadding(dp(10), dp(8), dp(10), dp(8))
            setOnClickListener { onCollapseClick?.invoke() }
        }
        addView(modeBtn, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        addView(candHost)
        addView(aiDot, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        addView(collapseBtn, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
    }

    fun attachCandidateBar(cand: View) {
        candHost.removeAllViews()
        candHost.addView(
            cand,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
    }

    fun setModeLabel(label: String) {
        modeBtn.text = "☰ $label"
    }

    fun setOnModeClick(listener: () -> Unit) {
        onModeClick = listener
    }

    fun setOnCollapseClick(listener: () -> Unit) {
        onCollapseClick = listener
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.toolbarBg)
        modeBtn.setTextColor(t.toolbarText)
        collapseBtn.setTextColor(t.toolbarText)
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}

class AiBar(context: Context) : HorizontalScrollView(context) {
    private val row = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(dp(12), dp(6), dp(12), dp(6))
    }
    private var onChip: ((String) -> Unit)? = null
    private var tokens = ThemeTokens.light()
    private val chips = listOf(
        Chip("✨ 润色", "润色", "primary"),
        Chip("💬 高情商", "高情商", "eq"),
        Chip("🌐 翻译", "翻译", "translate"),
        Chip("正式", "正式", "normal"),
        Chip("简洁", "简洁", "normal"),
        Chip("生动", "生动", "normal"),
    )

    data class Chip(val label: String, val action: String, val style: String)

    init {
        isHorizontalScrollBarEnabled = false
        setBackgroundColor(tokens.toolbarBg)
        addView(row, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT))
        rebuild()
    }

    fun setOnChipClick(listener: (String) -> Unit) {
        onChip = listener
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.toolbarBg)
        rebuild()
    }

    private fun rebuild() {
        row.removeAllViews()
        chips.forEach { chip ->
            val (bg, fg, border) = when (chip.style) {
                "primary" -> Triple(tokens.aiPrimaryBg, tokens.aiPrimaryText, tokens.aiPrimaryBg)
                "eq" -> Triple(tokens.aiEqBg, tokens.aiEqText, if (tokens.isDark) 0xFF4A3A6A.toInt() else 0xFFD6BCFA.toInt())
                "translate" -> Triple(
                    tokens.aiTranslateBg,
                    tokens.aiTranslateText,
                    if (tokens.isDark) 0xFF2A4A6A.toInt() else 0xFFBEE3F8.toInt(),
                )
                else -> Triple(tokens.aiChipBg, tokens.aiChipText, tokens.aiChipBorder)
            }
            val tv = TextView(context).apply {
                text = chip.label
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                setPadding(dp(14), dp(6), dp(14), dp(6))
                setTextColor(fg)
                background = GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    cornerRadius = dp(8).toFloat()
                    setColor(bg)
                    setStroke(dp(1), border)
                }
                setOnClickListener { onChip?.invoke(chip.action) }
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

class ModePanel(context: Context) : LinearLayout(context) {
    private var onPick: ((ModeOption) -> Unit)? = null
    private var tokens = ThemeTokens.light()

    init {
        orientation = VERTICAL
        setPadding(dp(16), dp(14), dp(16), dp(14))
        setBackgroundColor(tokens.panelBg)
        visibility = GONE
        elevation = dp(8).toFloat()
    }

    fun setOnPickListener(listener: (ModeOption) -> Unit) {
        onPick = listener
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(if (t.isDark) 0xFF2A2A2A.toInt() else 0xFFFFFFFF.toInt())
        if (visibility == VISIBLE) {
            // 下次 show 时用新色
        }
    }

    fun show(options: List<ModeOption>, selectedId: String) {
        removeAllViews()
        val titleColor = if (tokens.isDark) 0xFF888888.toInt() else 0xFF333333.toInt()
        val cellBg = if (tokens.isDark) 0xFF333333.toInt() else 0xFFF5F5F5.toInt()
        val cellActive = tokens.candSelectedBg
        val nameColor = if (tokens.isDark) 0xFFCCCCCC.toInt() else 0xFF333333.toInt()
        val nameActive = tokens.composingText

        val title = TextView(context).apply {
            text = "选择输入方式"
            setTextColor(titleColor)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            setPadding(0, 0, 0, dp(10))
        }
        addView(title)
        val row = LinearLayout(context).apply { orientation = HORIZONTAL }
        options.forEach { opt ->
            val selected = opt.id == selectedId
            val cell = LinearLayout(context).apply {
                orientation = VERTICAL
                gravity = Gravity.CENTER
                setPadding(dp(20), dp(14), dp(20), dp(14))
                background = GradientDrawable().apply {
                    cornerRadius = dp(10).toFloat()
                    setColor(if (selected) cellActive else cellBg)
                    if (selected) setStroke(dp(1), tokens.candSelectedBorder)
                }
                setOnClickListener {
                    onPick?.invoke(opt)
                    hide()
                }
            }
            cell.addView(TextView(context).apply {
                text = opt.icon
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 26f)
                gravity = Gravity.CENTER
            })
            cell.addView(TextView(context).apply {
                text = opt.name
                setTextColor(if (selected) nameActive else nameColor)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                gravity = Gravity.CENTER
            })
            val lp = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f)
            lp.rightMargin = dp(10)
            row.addView(cell, lp)
        }
        addView(row, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        visibility = VISIBLE
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}

/** 按住说话浮层（演示）。 */
class VoiceOverlay(context: Context) : LinearLayout(context) {
    private val langTv: TextView
    private val hintTv: TextView
    private val wave: TextView
    private var tokens = ThemeTokens.light()

    init {
        orientation = VERTICAL
        gravity = Gravity.CENTER
        setPadding(dp(20), dp(16), dp(20), dp(16))
        background = GradientDrawable().apply {
            cornerRadius = dp(12).toFloat()
            setColor(0xE6FFFFFF.toInt())
            setStroke(dp(1), 0xFFE0E0E0.toInt())
        }
        elevation = dp(6).toFloat()
        visibility = GONE
        langTv = TextView(context).apply {
            setTextColor(0xFF2B6CB0.toInt())
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            gravity = Gravity.CENTER
        }
        hintTv = TextView(context).apply {
            text = "松开结束"
            setTextColor(0xFF666666.toInt())
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            gravity = Gravity.CENTER
            setPadding(0, dp(8), 0, 0)
        }
        wave = TextView(context).apply {
            text = "▁▂▃▅▃▂▁▂▃▅"
            setTextColor(0xFF2B6CB0.toInt())
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            gravity = Gravity.CENTER
            setPadding(0, dp(10), 0, 0)
        }
        addView(langTv)
        addView(wave)
        addView(hintTv)
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        background = GradientDrawable().apply {
            cornerRadius = dp(12).toFloat()
            if (t.isDark) {
                setColor(0xF52A2A2A.toInt())
                setStroke(dp(1), 0xFF3A3A3A.toInt())
            } else {
                setColor(0xE6FFFFFF.toInt())
                setStroke(dp(1), 0xFFE0E0E0.toInt())
            }
        }
        langTv.setTextColor(if (t.isDark) 0xFF888888.toInt() else tokens.composingText)
        hintTv.setTextColor(if (t.isDark) 0xFFE8E8E8.toInt() else 0xFF666666.toInt())
        wave.setTextColor(tokens.composingText)
    }

    fun show(langName: String) {
        langTv.text = langName
        hintTv.text = "松开结束"
        visibility = VISIBLE
    }

    fun setRecognizing() {
        hintTv.text = "识别中…"
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
