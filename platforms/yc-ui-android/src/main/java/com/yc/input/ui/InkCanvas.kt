package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View

/** One ink point in canvas pixel coords (for local draw) + normalized 0..1 for FFI. */
data class InkPoint(
    val xPx: Float,
    val yPx: Float,
    val xNorm: Float,
    val yNorm: Float,
    val tMs: Long,
    val pressure: Float = 1f,
)

/**
 * Local ink surface. MOVE only draws; UP delivers the finished stroke (normalized points).
 * Does not call FFI.
 */
class InkCanvas @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {

    private var tokens = ThemeTokens()
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
        strokeWidth = 6f
    }
    private val gridPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = 1f
    }
    private val paths = mutableListOf<Pair<Path, Float>>()
    private var currentPath: Path? = null
    private var currentWidth = 6f
    private var currentStroke = mutableListOf<InkPoint>()
    private var onStrokeFinished: ((List<InkPoint>) -> Unit)? = null
    private var startElapsed = 0L
    private var showGrid = true

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        strokePaint.color = tokens.hwInk
        gridPaint.color = tokens.hwGrid
        setBackgroundColor(tokens.hwCanvasBg)
        invalidate()
    }

    fun setShowGrid(show: Boolean) {
        showGrid = show
        invalidate()
    }

    fun setOnStrokeFinished(listener: (List<InkPoint>) -> Unit) {
        onStrokeFinished = listener
    }

    fun clearInk() {
        paths.clear()
        currentPath = null
        currentStroke.clear()
        invalidate()
    }

    fun undoLastStroke() {
        if (paths.isNotEmpty()) {
            paths.removeAt(paths.lastIndex)
            invalidate()
        }
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.hwCanvasBg)
        if (showGrid) {
            val step = dp(24f)
            var x = step
            while (x < width) {
                canvas.drawLine(x, 0f, x, height.toFloat(), gridPaint)
                x += step
            }
            var y = step
            while (y < height) {
                canvas.drawLine(0f, y, width.toFloat(), y, gridPaint)
                y += step
            }
        }
        for ((p, w) in paths) {
            strokePaint.strokeWidth = w
            canvas.drawPath(p, strokePaint)
        }
        currentPath?.let {
            strokePaint.strokeWidth = currentWidth
            canvas.drawPath(it, strokePaint)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val w = width.coerceAtLeast(1).toFloat()
        val h = height.coerceAtLeast(1).toFloat()
        val x = event.x.coerceIn(0f, w)
        val y = event.y.coerceIn(0f, h)
        val t = android.os.SystemClock.uptimeMillis()
        val pressure = if (event.pressure > 0f) event.pressure.coerceIn(0.05f, 1f) else 1f
        val strokeW = 3f + 10f * pressure
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                if (startElapsed == 0L) startElapsed = t
                currentStroke = mutableListOf()
                val pt = InkPoint(x, y, x / w, y / h, t - startElapsed, pressure)
                currentStroke.add(pt)
                currentWidth = strokeW
                currentPath = Path().also {
                    it.moveTo(x, y)
                    paths.add(it to currentWidth)
                }
                invalidate()
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                val pt = InkPoint(x, y, x / w, y / h, t - startElapsed, pressure)
                currentStroke.add(pt)
                currentWidth = (currentWidth + strokeW) / 2f
                currentPath?.lineTo(x, y)
                // refresh width on last path entry
                if (paths.isNotEmpty() && currentPath != null) {
                    paths[paths.lastIndex] = currentPath!! to currentWidth
                }
                invalidate()
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                val pt = InkPoint(x, y, x / w, y / h, t - startElapsed, pressure)
                currentStroke.add(pt)
                currentPath?.lineTo(x, y)
                currentPath = null
                val finished = currentStroke.toList()
                currentStroke.clear()
                invalidate()
                if (event.actionMasked == MotionEvent.ACTION_UP && finished.size >= 2) {
                    onStrokeFinished?.invoke(finished)
                }
                return true
            }
        }
        return super.onTouchEvent(event)
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
}
