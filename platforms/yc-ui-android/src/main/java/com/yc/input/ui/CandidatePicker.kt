package com.yc.input.ui

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** 「…」候选选择面板：统一网格（引擎顺序），下滑加载更多。 */
class CandidatePicker(context: Context) : LinearLayout(context) {

    companion object {
        private const val COLS = 4
        private const val GRID_TAG = "grid:all"
    }

    private var tokens = ThemeTokens.light()
    private var onPick: ((CandidateItem) -> Unit)? = null
    private var onClose: (() -> Unit)? = null
    private var onNeedMore: (() -> Unit)? = null
    private var needMoreSent = false
    private var rebuilding = false

    private val titleTv: TextView
    private val closeTv: TextView
    private val scroll: ScrollView
    private val content: LinearLayout
    private var lastCands: List<CandidateItem> = emptyList()

    init {
        orientation = VERTICAL
        visibility = View.GONE
        elevation = dp(12).toFloat()
        setPadding(dp(12), dp(10), dp(12), dp(12))

        val header = LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(8))
        }
        titleTv = TextView(context).apply {
            text = "选择候选"
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            typeface = Typeface.DEFAULT_BOLD
            layoutParams = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f)
        }
        closeTv = TextView(context).apply {
            text = "×"
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 22f)
            setPadding(dp(8), 0, dp(4), 0)
            setOnClickListener {
                hide()
                onClose?.invoke()
            }
        }
        header.addView(titleTv)
        header.addView(closeTv)
        addView(header, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))

        content = LinearLayout(context).apply {
            orientation = VERTICAL
        }
        scroll = ScrollView(context).apply {
            isFillViewport = false
            isVerticalScrollBarEnabled = false
            overScrollMode = OVER_SCROLL_IF_CONTENT_SCROLLS
            addView(
                content,
                LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT),
            )
            viewTreeObserver.addOnScrollChangedListener {
                onScrollChanged()
            }
        }
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        applyTheme(tokens)
    }

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        setBackgroundColor(if (t.isDark) 0xFF1E1E1E.toInt() else 0xFFFFFFFF.toInt())
        titleTv.setTextColor(if (t.isDark) 0xFFE8E8E8.toInt() else 0xFF333333.toInt())
        closeTv.setTextColor(if (t.isDark) 0xFF777777.toInt() else 0xFF999999.toInt())
        if (isShowing() && lastCands.isNotEmpty()) {
            rebuildPreservingScroll(lastCands, scroll.scrollY)
        }
    }

    fun setOnPickListener(listener: (CandidateItem) -> Unit) {
        onPick = listener
    }

    fun setOnCloseListener(listener: () -> Unit) {
        onClose = listener
    }

    fun setOnNeedMoreListener(listener: () -> Unit) {
        onNeedMore = listener
    }

    fun show(cands: List<CandidateItem>) {
        needMoreSent = false
        lastCands = cands.toList()
        rebuildPreservingScroll(lastCands, scrollY = 0)
        visibility = View.VISIBLE
        scroll.scrollTo(0, 0)
    }

    fun update(cands: List<CandidateItem>) {
        if (!isShowing()) return
        if (cands == lastCands) return
        val prev = lastCands
        val grew = cands.size > prev.size &&
            prev.isNotEmpty() &&
            cands.take(prev.size) == prev
        lastCands = cands.toList()
        val y = scroll.scrollY
        if (grew) {
            appendNewItems(prev.size)
            if (!nearBottom()) needMoreSent = false
        } else {
            rebuildPreservingScroll(lastCands, y)
            needMoreSent = false
        }
    }

    fun hide() {
        visibility = View.GONE
        needMoreSent = false
        rebuilding = false
    }

    fun isShowing(): Boolean = visibility == View.VISIBLE

    private fun onScrollChanged() {
        if (rebuilding) return
        if (needMoreSent && !nearBottom(thresholdDp = 120)) {
            needMoreSent = false
        }
        maybeRequestMore()
    }

    private fun nearBottom(thresholdDp: Int = 80): Boolean {
        val child = scroll.getChildAt(0) ?: return false
        if (scroll.height <= 0) return false
        val bottom = child.bottom - scroll.scrollY - scroll.height
        return bottom <= dp(thresholdDp)
    }

    private fun maybeRequestMore() {
        if (!isShowing() || needMoreSent || rebuilding) return
        if (!nearBottom()) return
        needMoreSent = true
        onNeedMore?.invoke()
    }

    private fun rebuildPreservingScroll(cands: List<CandidateItem>, scrollY: Int) {
        rebuilding = true
        content.removeAllViews()
        if (cands.isEmpty()) {
            content.addView(
                TextView(context).apply {
                    text = "暂无候选"
                    setTextColor(if (tokens.isDark) 0xFF888888.toInt() else 0xFF999999.toInt())
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                    setPadding(dp(8), dp(16), dp(8), dp(16))
                },
            )
        } else {
            content.addView(buildGrid(cands))
        }
        content.post {
            scroll.scrollTo(0, scrollY.coerceAtLeast(0))
            rebuilding = false
        }
    }

    private fun appendNewItems(prevSize: Int) {
        val added = lastCands.drop(prevSize)
        if (added.isEmpty()) return
        rebuilding = true
        var outer = content.findViewWithTag<LinearLayout>(GRID_TAG)
        if (outer == null) {
            outer = buildGrid(emptyList())
            content.removeAllViews()
            content.addView(outer)
        }
        trimTrailingPlaceholders(outer)
        fillGrid(outer, added)
        content.post { rebuilding = false }
    }

    private fun buildGrid(items: List<CandidateItem>): LinearLayout {
        val outer = LinearLayout(context).apply {
            orientation = VERTICAL
            tag = GRID_TAG
        }
        fillGrid(outer, items)
        return outer
    }

    private fun trimTrailingPlaceholders(outer: LinearLayout) {
        if (outer.childCount == 0) return
        val lastRow = outer.getChildAt(outer.childCount - 1) as? LinearLayout ?: return
        for (j in lastRow.childCount - 1 downTo 0) {
            if (lastRow.getChildAt(j) !is TextView) {
                lastRow.removeViewAt(j)
            } else {
                break
            }
        }
        if (lastRow.childCount == 0) {
            outer.removeView(lastRow)
        }
    }

    private fun fillGrid(outer: LinearLayout, items: List<CandidateItem>) {
        var row: LinearLayout? = null
        if (outer.childCount > 0) {
            val last = outer.getChildAt(outer.childCount - 1) as? LinearLayout
            if (last != null && last.childCount in 1 until COLS) {
                row = last
            }
        }
        var colInRow = row?.childCount ?: 0
        items.forEach { cand ->
            if (row == null || colInRow >= COLS) {
                row = LinearLayout(context).apply {
                    orientation = HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                }
                outer.addView(
                    row,
                    LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                        bottomMargin = dp(6)
                    },
                )
                colInRow = 0
            }
            val cell = chip(cand)
            val lp = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f).apply {
                marginEnd = dp(6)
            }
            row!!.addView(cell, lp)
            colInRow++
        }
        if (row != null && colInRow in 1 until COLS) {
            repeat(COLS - colInRow) {
                row!!.addView(View(context), LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            }
        }
    }

    private fun chip(cand: CandidateItem): TextView {
        val bg = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = dp(10).toFloat()
            setColor(if (tokens.isDark) 0xFF2A2A2A.toInt() else 0xFFF5F5F5.toInt())
        }
        return TextView(context).apply {
            text = cand.text
            gravity = Gravity.CENTER
            setTextColor(tokens.candText)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setPadding(dp(8), dp(12), dp(8), dp(12))
            background = bg
            setOnClickListener {
                onPick?.invoke(cand)
                hide()
            }
        }
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}
