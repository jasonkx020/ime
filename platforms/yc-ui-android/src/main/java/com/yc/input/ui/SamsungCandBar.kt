package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.util.TypedValue
import android.view.MotionEvent
import android.view.VelocityTracker
import android.view.View
import android.view.ViewConfiguration
import android.widget.OverScroller
import kotlin.math.abs
import kotlin.math.max

/** 顶栏候选：单行横滑，按引擎顺序合并字/词。 */
class SamsungCandBar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), CandBar {

    private var tokens = ThemeTokens.light()
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
        snapshot.candidates.isNotEmpty() && !snapshot.expanded

    private fun moreReserve(): Float = if (showMoreChip()) dp(36f) else 0f

    private fun maxScroll(): Float {
        val visible = (width - moreReserve()).coerceAtLeast(1f)
        return max(0f, contentWidth - visible + chipsOriginX)
    }

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
        chipBounds.clear()
        moreChipBounds = null

        val padX = dp(4f)
        chipsOriginX = padX
        val rowTop = dp(4f)
        val rowBot = (height - dp(4f)).coerceAtLeast(rowTop + dp(28f))
        val moreReserve = moreReserve()
        val clipRight = width - moreReserve

        val save = canvas.save()
        canvas.clipRect(0f, 0f, clipRight, height.toFloat())

        var x = padX - scrollOffset
        for (cand in snapshot.candidates) {
            x = drawPlain(canvas, cand, x, rowTop, rowBot)
        }
        canvas.restoreToCount(save)
        contentWidth = x + scrollOffset - padX + padX

        if (showMoreChip()) {
            moreChipBounds = drawMoreAt(
                canvas,
                width - moreReserve + dp(2f),
                rowTop,
                rowBot,
            )
        }
        clampScroll()
    }

    private fun drawPlain(
        canvas: Canvas,
        cand: CandidateItem,
        startX: Float,
        top: Float,
        bot: Float,
    ): Float {
        paint.textSize = sp(tokens.candFontSp)
        paint.isFakeBoldText = false
        paint.color = tokens.candText
        paint.textAlign = Paint.Align.LEFT
        val tw = paint.measureText(cand.text)
        val pad = dp(6f)
        val rect = RectF(startX, top, startX + tw + pad * 2, bot)
        val cy = (top + bot) / 2f
        canvas.drawText(
            cand.text,
            startX + pad,
            cy - (paint.descent() + paint.ascent()) / 2,
            paint,
        )
        chipBounds.add(cand to RectF(rect))
        return startX + tw + pad * 2 + dp(6f)
    }

    private fun drawMoreAt(canvas: Canvas, startX: Float, top: Float, bot: Float): RectF {
        paint.textSize = sp(18f)
        paint.color = tokens.toolbarText
        paint.textAlign = Paint.Align.LEFT
        paint.isFakeBoldText = false
        val label = "…"
        val tw = paint.measureText(label)
        val pad = dp(8f)
        val rect = RectF(startX, top, startX + tw + pad * 2, bot)
        val cy = (top + bot) / 2f
        canvas.drawText(label, startX + pad, cy - (paint.descent() + paint.ascent()) / 2, paint)
        return rect
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
    private fun sp(v: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, resources.displayMetrics)
}
