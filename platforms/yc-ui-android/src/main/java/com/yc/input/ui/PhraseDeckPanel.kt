package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject

data class PhraseCard(val id: String, val label: String, val text: String)

/** 本地话术卡面板（行业 ContentPack phrases）。 */
class PhraseDeckPanel(context: Context) : LinearLayout(context) {
    private val column = LinearLayout(context).apply {
        orientation = VERTICAL
        setPadding(dp(12), dp(10), dp(12), dp(10))
    }
    private var tokens = ThemeTokens.light()
    private var onPick: ((PhraseCard) -> Unit)? = null

    init {
        orientation = VERTICAL
        visibility = GONE
        val scroll = ScrollView(context)
        scroll.addView(column)
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
    }

    fun setOnPick(listener: (PhraseCard) -> Unit) {
        onPick = listener
    }

    fun show(title: String, cards: List<PhraseCard>, t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.panelBg)
        column.removeAllViews()
        column.addView(
            TextView(context).apply {
                text = title
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(t.candText)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                setPadding(dp(4), dp(4), dp(4), dp(10))
            },
        )
        cards.forEach { card ->
            column.addView(
                TextView(context).apply {
                    text = "${card.label}\n${card.text}"
                    setTextColor(t.candText)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    setPadding(dp(12), dp(10), dp(12), dp(10))
                    background = GradientDrawable().apply {
                        setColor(t.keyNormal)
                        cornerRadius = dp(8).toFloat()
                        setStroke(dp(1), t.keyBorder)
                    }
                    val lp = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
                    lp.bottomMargin = dp(8)
                    layoutParams = lp
                    setOnClickListener { onPick?.invoke(card) }
                },
            )
        }
        visibility = VISIBLE
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

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
                    ),
                )
            }
            return title to cards
        }
    }
}
