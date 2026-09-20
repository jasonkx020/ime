package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject

data class PhraseCard(
    val id: String,
    val label: String,
    val text: String,
    val intentHint: String = "",
)

/** 行业话术面板：场景卡一键上屏 + AI 优化变体。 */
class PhraseDeckPanel(context: Context) : LinearLayout(context) {
    private val header = LinearLayout(context).apply {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
    }
    private val industryChip = TextView(context)
    private val status = TextView(context)
    private val column = LinearLayout(context).apply { orientation = VERTICAL }
    private val variantsCol = LinearLayout(context).apply { orientation = VERTICAL }
    private var tokens = ThemeTokens.light()
    private var onPick: ((PhraseCard) -> Unit)? = null
    private var onOptimize: ((PhraseCard) -> Unit)? = null
    private var onIndustryClick: (() -> Unit)? = null
    private var onClose: (() -> Unit)? = null
    private var onVariantPick: ((String) -> Unit)? = null

    init {
        orientation = VERTICAL
        visibility = GONE
        setPadding(dp(10), dp(8), dp(10), dp(8))

        header.addView(
            TextView(context).apply {
                text = "行业话术"
                typeface = Typeface.DEFAULT_BOLD
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            },
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f),
        )
        industryChip.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
        industryChip.setPadding(dp(8), dp(4), dp(8), dp(4))
        industryChip.setOnClickListener { onIndustryClick?.invoke() }
        header.addView(industryChip)
        header.addView(
            TextView(context).apply {
                text = "关闭"
                setTextColor(0xFF1A73E8.toInt())
                setPadding(dp(10), dp(4), 0, dp(4))
                setOnClickListener {
                    onClose?.invoke()
                    hide()
                }
            },
        )
        addView(header)

        status.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
        status.setTextColor(0xFF5F6368.toInt())
        status.visibility = GONE
        addView(status)

        val scroll = ScrollView(context)
        val body = LinearLayout(context).apply {
            orientation = VERTICAL
            addView(column)
            addView(
                TextView(context).apply {
                    text = "AI 变体"
                    typeface = Typeface.DEFAULT_BOLD
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(0, dp(8), 0, dp(4))
                },
            )
            addView(variantsCol)
        }
        scroll.addView(body)
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
    }

    fun setOnPick(listener: (PhraseCard) -> Unit) {
        onPick = listener
    }

    fun setOnOptimize(listener: (PhraseCard) -> Unit) {
        onOptimize = listener
    }

    fun setOnIndustryClick(listener: () -> Unit) {
        onIndustryClick = listener
    }

    fun setOnClose(listener: () -> Unit) {
        onClose = listener
    }

    fun setOnVariantPick(listener: (String) -> Unit) {
        onVariantPick = listener
    }

    fun setIndustryLabel(name: String) {
        industryChip.text = name
        industryChip.setTextColor(tokens.aiPrimaryText)
        industryChip.background = GradientDrawable().apply {
            setColor(tokens.aiPrimaryBg)
            cornerRadius = dp(8).toFloat()
        }
    }

    fun setStatus(msg: String) {
        status.text = msg
        status.visibility = if (msg.isBlank()) GONE else VISIBLE
    }

    fun show(title: String, cards: List<PhraseCard>, industryLabel: String, t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.panelBg)
        setIndustryLabel(industryLabel.ifBlank { title })
        setStatus("")
        variantsCol.removeAllViews()
        column.removeAllViews()
        cards.forEach { card ->
            val row = LinearLayout(context).apply {
                orientation = VERTICAL
                setPadding(dp(10), dp(8), dp(10), dp(8))
                background = GradientDrawable().apply {
                    setColor(t.keyNormal)
                    cornerRadius = dp(8).toFloat()
                    setStroke(dp(1), t.keyBorder)
                }
            }
            row.addView(
                TextView(context).apply {
                    text = card.label
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(t.candText)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                },
            )
            row.addView(
                TextView(context).apply {
                    text = card.text
                    setTextColor(t.candText)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(0, dp(4), 0, dp(6))
                },
            )
            val actions = LinearLayout(context).apply { orientation = HORIZONTAL }
            actions.addView(
                chipButton("上屏") { onPick?.invoke(card) },
                LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                    rightMargin = dp(8)
                },
            )
            actions.addView(chipButton("AI 优化") { onOptimize?.invoke(card) })
            row.addView(actions)
            val lp = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
            lp.bottomMargin = dp(8)
            column.addView(row, lp)
        }
        visibility = VISIBLE
    }

    fun showVariants(texts: List<String>, local: Boolean) {
        variantsCol.removeAllViews()
        if (texts.isEmpty()) {
            setStatus("无变体")
            return
        }
        setStatus(if (local) "本地示意（请配置 API Key）" else "点选一条上屏")
        texts.forEach { text ->
            variantsCol.addView(
                TextView(context).apply {
                    this.text = text
                    setTextColor(tokens.candText)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(dp(10), dp(8), dp(10), dp(8))
                    background = GradientDrawable().apply {
                        setColor(tokens.keyUtility)
                        cornerRadius = dp(8).toFloat()
                        setStroke(dp(1), tokens.keyBorder)
                    }
                    val lp = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
                    lp.bottomMargin = dp(6)
                    layoutParams = lp
                    setOnClickListener { onVariantPick?.invoke(text) }
                },
            )
        }
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    private fun chipButton(label: String, onClick: () -> Unit): TextView =
        TextView(context).apply {
            text = label
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            setTextColor(0xFF1A73E8.toInt())
            setPadding(dp(10), dp(4), dp(10), dp(4))
            background = GradientDrawable().apply {
                setColor(0xFFE8F0FE.toInt())
                cornerRadius = dp(6).toFloat()
            }
            setOnClickListener { onClick() }
        }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    companion object {
        fun parseDeckJson(raw: String): Pair<String, List<PhraseCard>> {
            val obj = JSONObject(raw)
            val title = obj.optString("title", "话术")
            val arr = obj.optJSONArray("cards") ?: return title to emptyList()
            val cards = mutableListOf<PhraseCard>()
            for (i in 0 until arr.length()) {
                val c = arr.getJSONObject(i)
                cards.add(
                    PhraseCard(
                        id = c.optString("id"),
                        label = c.optString("label"),
                        text = c.optString("text"),
                        intentHint = c.optString("intent_hint"),
                    ),
                )
            }
            return title to cards
        }
    }
}
