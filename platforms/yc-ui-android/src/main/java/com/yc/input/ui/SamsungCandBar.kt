package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.VelocityTracker
import android.view.View
import android.view.ViewConfiguration
import android.widget.OverScroller
import kotlin.math.abs
import kotlin.math.max

class SamsungCandBar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), CandBar {

    private var tokens = ThemeTokens()
    private var snapshot = KeyboardSnapshot(0, 0, "", emptyList())
    private var onCandidate: ((CandidateItem) -> Unit)? = null
    private var onPage: ((Int) -> Unit)? = null
    private var onExpand: (() -> Unit)? = null
    private var onNeedMore: (() -> Unit)? = null
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val chipBounds = mutableListOf<Pair<CandidateItem, RectF>>()
    private var moreChipBounds: RectF? = null

    private var downX = 0f
    private var downY = 0f
    private var lastX = 0f
    private var scrollOffset = 0f
    private var contentWidth = 0f
    private var chipsOriginX = 0f
    private var dragging = false
    private var needMoreSent = false

    private val scroller = OverScroller(context)
    private var velocityTracker: VelocityTracker? = null
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop
    private val minFlingVelocity = ViewConfiguration.get(context).scaledMinimumFlingVelocity
    private val maxFlingVelocity = ViewConfiguration.get(context).scaledMaximumFlingVelocity

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    override fun render(snapshot: KeyboardSnapshot) {
        val prev = this.snapshot
        this.snapshot = snapshot
        if (snapshot.composing != prev.composing ||
            (!snapshot.expanded && prev.expanded) ||
            snapshot.candidates.isEmpty()
        ) {
            // 新拼音 / 收起 / 清空：复位滚动；追加候选时保留偏移
            if (snapshot.composing != prev.composing || snapshot.candidates.isEmpty()) {
                abortScroll()
                scrollOffset = 0f
                needMoreSent = false
            }
        } else if (snapshot.candidates.size < prev.candidates.size) {
            abortScroll()
            scrollOffset = 0f
            needMoreSent = false
        } else if (snapshot.candidates.size > prev.candidates.size) {
            // 追加后允许再次触底加载
            needMoreSent = false
            clampScroll()
        }
        invalidate()
    }

    override fun setOnCandidateListener(listener: (CandidateItem) -> Unit) {
        onCandidate = listener
    }

    override fun setOnPageListener(listener: (Int) -> Unit) {
        onPage = listener
    }

    override fun setOnExpandListener(listener: () -> Unit) {
        onExpand = listener
    }

    override fun setOnNeedMoreListener(listener: () -> Unit) {
        onNeedMore = listener
    }

    private fun hasMorePages(): Boolean =
        snapshot.totalPages > 1 && snapshot.candPage + 1 < snapshot.totalPages

    private fun showMoreChip(): Boolean =
        snapshot.totalPages > 1 && !snapshot.expanded

    private fun maxScroll(): Float = max(0f, contentWidth - width + chipsOriginX)

    private fun clampScroll() {
        scrollOffset = scrollOffset.coerceIn(0f, maxScroll())
    }

    private fun abortScroll() {
        if (!scroller.isFinished) {
            scroller.forceFinished(true)
        }
    }

    override fun computeScroll() {
        if (scroller.computeScrollOffset()) {
            scrollOffset = scroller.currX.toFloat()
            invalidate()
            maybeRequestMore()
            if (!scroller.isFinished) {
                postInvalidateOnAnimation()
            } else {
                needMoreSent = false
            }
        }
    }

    private fun maybeRequestMore() {
        val max = maxScroll()
        if (max > 0f &&
            scrollOffset >= max - dp(72f) &&
            hasMorePages() &&
            !needMoreSent
        ) {
            needMoreSent = true
            onNeedMore?.invoke()
        }
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.keyboardBg)
        chipBounds.clear()
        moreChipBounds = null

        val pad = dp(10f)
        val top = dp(8f)
        val bot = height - dp(8f)
        val cy = height / 2f
        var x = pad
        chipsOriginX = pad

        if (!snapshot.expanded && snapshot.composing.isNotEmpty()) {
            paint.color = tokens.composingText
            paint.textSize = sp(13f)
            paint.textAlign = Paint.Align.LEFT
            canvas.drawText(
                snapshot.composing,
                x,
                cy - (paint.descent() + paint.ascent()) / 2,
                paint,
            )
            x += paint.measureText(snapshot.composing) + dp(12f)
            chipsOriginX = x
        } else if (!snapshot.expanded && snapshot.asciiMode) {
            paint.color = tokens.composingText
            paint.textSize = sp(13f)
            paint.textAlign = Paint.Align.LEFT
            canvas.drawText(
                "EN",
                x,
                cy - (paint.descent() + paint.ascent()) / 2,
                paint,
            )
            x += paint.measureText("EN") + dp(12f)
            chipsOriginX = x
        }

        val save = canvas.save()
        if (!snapshot.expanded) {
            canvas.clipRect(chipsOriginX, 0f, width.toFloat(), height.toFloat())
        }

        var chipX = chipsOriginX - scrollOffset
        paint.textSize = sp(tokens.candFontSp)
        for (cand in snapshot.candidates) {
            chipX = drawChip(canvas, cand, chipX, top, bot, cy)
        }

        if (showMoreChip()) {
            moreChipBounds = drawLabelChip(canvas, "…", chipX, top, bot, cy)
            chipX = moreChipBounds!!.right + dp(6f)
        }

        canvas.restoreToCount(save)

        // contentWidth = 芯片区总宽（不含左侧固定 composing）
        contentWidth = chipX + scrollOffset - chipsOriginX + pad
        clampScroll()
    }

    private fun drawChip(
        canvas: Canvas,
        cand: CandidateItem,
        startX: Float,
        top: Float,
        bot: Float,
        cy: Float,
    ): Float {
        val tw = paint.measureText(cand.text)
        val chipW = max(dp(36f), tw + dp(20f))
        val rect = RectF(startX, top, startX + chipW, bot)
        fillChip(canvas, rect, cand.text, cy)
        chipBounds.add(cand to RectF(rect))
        return startX + chipW + dp(6f)
    }

    private fun drawLabelChip(
        canvas: Canvas,
        label: String,
        startX: Float,
        top: Float,
        bot: Float,
        cy: Float,
    ): RectF {
        val tw = paint.measureText(label)
        val chipW = max(dp(36f), tw + dp(20f))
        val rect = RectF(startX, top, startX + chipW, bot)
        fillChip(canvas, rect, label, cy)
        return rect
    }

    private fun fillChip(canvas: Canvas, rect: RectF, label: String, cy: Float) {
        paint.color = tokens.candSelectedBg
        canvas.drawRoundRect(rect, dp(16f), dp(16f), paint)
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = dp(1f)
        paint.color = tokens.candSelectedBorder
        canvas.drawRoundRect(rect, dp(16f), dp(16f), paint)
        paint.style = Paint.Style.FILL
        paint.color = tokens.candText
        paint.textAlign = Paint.Align.CENTER
        canvas.drawText(
            label,
            rect.centerX(),
            cy - (paint.descent() + paint.ascent()) / 2,
            paint,
        )
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                abortScroll()
                downX = event.x
                downY = event.y
                lastX = event.x
                dragging = false
                velocityTracker?.recycle()
                velocityTracker = VelocityTracker.obtain().also { it.addMovement(event) }
                parent?.requestDisallowInterceptTouchEvent(true)
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                velocityTracker?.addMovement(event)
                val totalDx = event.x - downX
                val totalDy = event.y - downY
                if (!dragging) {
                    if (abs(totalDx) > touchSlop && abs(totalDx) > abs(totalDy)) {
                        dragging = true
                    } else if (abs(totalDy) > touchSlop && abs(totalDy) > abs(totalDx)) {
                        parent?.requestDisallowInterceptTouchEvent(false)
                        return false
                    }
                }
                if (dragging) {
                    val dx = event.x - lastX
                    lastX = event.x
                    scrollOffset = (scrollOffset - dx).coerceIn(0f, maxScroll())
                    invalidate()
                    maybeRequestMore()
                }
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                val tracker = velocityTracker
                tracker?.addMovement(event)
                val wasDragging = dragging
                if (wasDragging && tracker != null && event.actionMasked == MotionEvent.ACTION_UP) {
                    tracker.computeCurrentVelocity(1000, maxFlingVelocity.toFloat())
                    val vx = tracker.xVelocity
                    if (abs(vx) > minFlingVelocity) {
                        val max = maxScroll().toInt().coerceAtLeast(0)
                        scroller.fling(
                            scrollOffset.toInt(),
                            0,
                            (-vx).toInt(),
                            0,
                            0,
                            max,
                            0,
                            0,
                            dp(32f).toInt(),
                            0,
                        )
                        postInvalidateOnAnimation()
                    } else {
                        needMoreSent = false
                    }
                } else if (!wasDragging && event.actionMasked == MotionEvent.ACTION_UP) {
                    moreChipBounds?.let { rect ->
                        if (rect.contains(event.x, event.y)) {
                            performClick()
                            onExpand?.invoke()
                            recycleTracker()
                            return true
                        }
                    }
                    for ((cand, rect) in chipBounds) {
                        if (rect.contains(event.x, event.y)) {
                            performClick()
                            onCandidate?.invoke(cand)
                            recycleTracker()
                            return true
                        }
                    }
                } else {
                    needMoreSent = false
                }
                dragging = false
                recycleTracker()
                return true
            }
        }
        return true
    }

    private fun recycleTracker() {
        velocityTracker?.recycle()
        velocityTracker = null
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
