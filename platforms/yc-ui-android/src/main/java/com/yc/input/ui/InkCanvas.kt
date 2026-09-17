package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.os.SystemClock
import android.util.AttributeSet
import android.util.TypedValue
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

    private var tokens = ThemeTokens.light()
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
    private var dashEffect = DashPathEffect(floatArrayOf(4f, 4f), 0f)
    private val hintPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        textAlign = Paint.Align.CENTER
    }
    private val paths = mutableListOf<Pair<Path, Float>>()
    private var currentPath: Path? = null
    private var currentWidth = 6f
    private var currentStroke = mutableListOf<InkPoint>()
    private var onStrokeFinished: ((List<InkPoint>) -> Unit)? = null
    private var onInkChanged: ((Boolean) -> Unit)? = null
    private var startElapsed = 0L
    private var showGrid = true
    private var showHint = true

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        strokePaint.color = tokens.hwInk
        gridPaint.color = tokens.hwGrid
        hintPaint.color = tokens.hwHint
        dashEffect = DashPathEffect(floatArrayOf(dp(4f), dp(4f)), 0f)
        setBackgroundColor(tokens.hwCanvasBg)
        invalidate()
    }

    fun setShowGrid(show: Boolean) {
        showGrid = show
        invalidate()
    }

    fun setShowHint(show: Boolean) {
        if (showHint == show) return
        showHint = show
        invalidate()
    }

    fun hasInk(): Boolean = paths.isNotEmpty() || currentPath != null

    fun setOnStrokeFinished(listener: (List<InkPoint>) -> Unit) {
        onStrokeFinished = listener
    }

    fun setOnInkChanged(listener: (Boolean) -> Unit) {
        onInkChanged = listener
    }

    fun clearInk() {
        paths.clear()
        currentPath = null
        currentStroke.clear()
        showHint = true
        invalidate()
        onInkChanged?.invoke(false)
    }

    fun undoLastStroke() {
        if (paths.isNotEmpty()) {
            paths.removeAt(paths.lastIndex)
            showHint = paths.isEmpty()
            invalidate()
            onInkChanged?.invoke(paths.isNotEmpty())
        }
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.hwCanvasBg)
        if (showGrid && width > 0 && height > 0) {
            val inset = dp(1f)
            val l = inset
            val t = inset
            val r = width - inset
            val b = height - inset
            val cx = width / 2f
            val cy = height / 2f
            // 田字实线
            gridPaint.pathEffect = null
            gridPaint.strokeWidth = dp(1f)
            canvas.drawLine(cx, t, cx, b, gridPaint)
            canvas.drawLine(l, cy, r, cy, gridPaint)
            // 米字虚线对角
            gridPaint.pathEffect = dashEffect
            gridPaint.strokeWidth = dp(1f)
            canvas.drawLine(l, t, r, b, gridPaint)
            canvas.drawLine(r, t, l, b, gridPaint)
            gridPaint.pathEffect = null
        }
        if (showHint && paths.isEmpty() && currentPath == null) {
            hintPaint.textSize = sp(14f)
            hintPaint.color = tokens.hwHint
            val cy = height / 2f - (hintPaint.descent() + hintPaint.ascent()) / 2
            canvas.drawText("请在此处写字", width / 2f, cy, hintPaint)
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
        val t = SystemClock.uptimeMillis()
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
                if (showHint) {
                    showHint = false
                    onInkChanged?.invoke(true)
                }
                invalidate()
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                val pt = InkPoint(x, y, x / w, y / h, t - startElapsed, pressure)
                currentStroke.add(pt)
                currentWidth = (currentWidth + strokeW) / 2f
                currentPath?.lineTo(x, y)
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
    private fun sp(v: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, resources.displayMetrics)
}
