package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import kotlin.math.max
import kotlin.math.min

/**
 * Handwriting pad: mode bar + **square** ink canvas + back-to-keyboard.
 * Listeners are invoked by shell; MOVE never calls FFI.
 */
class HandwritingPad @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : LinearLayout(context, attrs) {

    data class StrokePayload(
        val xyPressure: FloatArray,
        val timesMs: LongArray,
        val canvasW: Int,
        val canvasH: Int,
    )

    private var tokens = ThemeTokens()
    private val topBar = HwActionBar(context)
    private val inkHost = SquareInkHost(context)
    private val ink = InkCanvas(context)
    private val bottomBar = HwBottomBar(context)

    private var continuous = false
    private var recognizing = false
    private var onStroke: ((StrokePayload) -> Unit)? = null
    private var onRecognize: (() -> Unit)? = null
    private var onUndo: (() -> Unit)? = null
    private var onClear: (() -> Unit)? = null
    private var onDismiss: (() -> Unit)? = null
    private var onModeChanged: ((continuous: Boolean) -> Unit)? = null

    init {
        orientation = VERTICAL
        val barH = dp(36)
        addView(topBar, LayoutParams(LayoutParams.MATCH_PARENT, barH))
        inkHost.addView(
            ink,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
                Gravity.CENTER,
            ),
        )
        addView(
            inkHost,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT),
        )
        addView(bottomBar, LayoutParams(LayoutParams.MATCH_PARENT, barH))

        topBar.setOnAction { action ->
            if (recognizing && action == HwActionBar.Action.Recognize) return@setOnAction
            when (action) {
                HwActionBar.Action.Single -> {
                    continuous = false
                    topBar.setContinuous(false)
                    onModeChanged?.invoke(false)
                }
                HwActionBar.Action.Continuous -> {
                    continuous = true
                    topBar.setContinuous(true)
                    onModeChanged?.invoke(true)
                }
                HwActionBar.Action.Recognize -> onRecognize?.invoke()
                HwActionBar.Action.Undo -> {
                    ink.undoLastStroke()
                    onUndo?.invoke()
                }
                HwActionBar.Action.Clear -> {
                    ink.clearInk()
                    onClear?.invoke()
                }
            }
        }
        bottomBar.setOnBack { onDismiss?.invoke() }
        ink.setOnStrokeFinished { points ->
            val xy = FloatArray(points.size * 3)
            val ts = LongArray(points.size)
            for (i in points.indices) {
                val p = points[i]
                xy[i * 3] = p.xNorm
                xy[i * 3 + 1] = p.yNorm
                xy[i * 3 + 2] = p.pressure
                ts[i] = p.tMs
            }
            onStroke?.invoke(
                StrokePayload(
                    xyPressure = xy,
                    timesMs = ts,
                    canvasW = ink.width.coerceAtLeast(1),
                    canvasH = ink.height.coerceAtLeast(1),
                ),
            )
        }
        applyTheme(tokens)
    }

    fun setRecognizing(active: Boolean) {
        recognizing = active
        topBar.setRecognizing(active)
    }

    fun isRecognizing(): Boolean = recognizing

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        setBackgroundColor(tokens.keyboardBg)
        topBar.applyTheme(tokens)
        inkHost.applyTheme(tokens)
        ink.applyTheme(tokens)
        bottomBar.applyTheme(tokens)
    }

    fun setContinuous(continuous: Boolean) {
        this.continuous = continuous
        topBar.setContinuous(continuous)
    }

    fun isContinuous(): Boolean = continuous

    fun clearInk() = ink.clearInk()

    fun setOnStrokeListener(listener: (StrokePayload) -> Unit) {
        onStroke = listener
    }

    fun setOnRecognizeListener(listener: () -> Unit) {
        onRecognize = listener
    }

    fun setOnUndoListener(listener: () -> Unit) {
        onUndo = listener
    }

    fun setOnClearListener(listener: () -> Unit) {
        onClear = listener
    }

    fun setOnDismissListener(listener: () -> Unit) {
        onDismiss = listener
    }

    fun setOnModeChangedListener(listener: (Boolean) -> Unit) {
        onModeChanged = listener
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}

/**
 * Host that forces a centered square writing surface (边长 = min(可用宽, 最大边)).
 */
private class SquareInkHost(context: Context) : FrameLayout(context) {
    private var tokens = ThemeTokens()
    private val borderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = dp(1.5f)
    }
    private val margin = dp(8)
    private val maxSide = dp(320)

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        borderPaint.color = tokens.hwGrid
        setBackgroundColor(tokens.keyboardBg)
        invalidate()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec).coerceAtLeast(0)
        val side = min(max(width - margin * 2, dp(120)), maxSide)
        val height = side + margin * 2
        val childSpec = MeasureSpec.makeMeasureSpec(side, MeasureSpec.EXACTLY)
        for (i in 0 until childCount) {
            getChildAt(i).measure(childSpec, childSpec)
        }
        setMeasuredDimension(
            resolveSize(width, widthMeasureSpec),
            height,
        )
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        val side = min(max(width - margin * 2, dp(120)), maxSide)
        val childLeft = (width - side) / 2
        val childTop = margin
        for (i in 0 until childCount) {
            getChildAt(i).layout(childLeft, childTop, childLeft + side, childTop + side)
        }
    }

    override fun dispatchDraw(canvas: Canvas) {
        super.dispatchDraw(canvas)
        val side = min(max(width - margin * 2, dp(120)), maxSide)
        val l = (width - side) / 2f
        val t = margin.toFloat()
        canvas.drawRect(l, t, l + side, t + side, borderPaint)
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
    private fun dp(v: Float): Float = v * resources.displayMetrics.density
}

private class HwActionBar(context: Context) : View(context) {
    enum class Action { Single, Continuous, Recognize, Undo, Clear }

    private var tokens = ThemeTokens()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private var continuous = false
    private var recognizing = false
    private var onAction: ((Action) -> Unit)? = null
    private val hit = mutableListOf<Pair<Action, RectF>>()

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        setBackgroundColor(tokens.hwBarBg)
        invalidate()
    }

    fun setContinuous(continuous: Boolean) {
        this.continuous = continuous
        invalidate()
    }

    fun setRecognizing(recognizing: Boolean) {
        this.recognizing = recognizing
        invalidate()
    }

    fun setOnAction(listener: (Action) -> Unit) {
        onAction = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.hwBarBg)
        hit.clear()
        paint.textSize = sp(13f)
        paint.textAlign = Paint.Align.CENTER
        val cy = height / 2f - (paint.descent() + paint.ascent()) / 2
        var x = dp(8f)
        fun chip(label: String, action: Action, active: Boolean = false) {
            val tw = paint.measureText(label)
            val w = max(dp(44f), tw + dp(16f))
            val rect = RectF(x, dp(6f), x + w, height - dp(6f))
            paint.style = Paint.Style.FILL
            paint.color = if (active) tokens.hwBarAccent else tokens.candSelectedBg
            canvas.drawRoundRect(rect, dp(10f), dp(10f), paint)
            paint.color = if (active) 0xFFFFFFFF.toInt() else tokens.hwBarText
            canvas.drawText(label, rect.centerX(), cy, paint)
            hit.add(action to RectF(rect))
            x += w + dp(6f)
        }
        chip("单字", Action.Single, !continuous)
        chip("连写", Action.Continuous, continuous)
        if (continuous) chip("识别", Action.Recognize)
        if (recognizing) {
            paint.color = tokens.hwBarText
            paint.textAlign = Paint.Align.CENTER
            canvas.drawText("识别中…", width / 2f, cy, paint)
        }
        val rightLabels = listOf("撤销" to Action.Undo, "清空" to Action.Clear)
        var rx = width - dp(8f)
        for ((label, action) in rightLabels.asReversed()) {
            val tw = paint.measureText(label)
            val w = max(dp(44f), tw + dp(16f))
            rx -= w
            val rect = RectF(rx, dp(6f), rx + w, height - dp(6f))
            paint.style = Paint.Style.FILL
            paint.color = tokens.candSelectedBg
            canvas.drawRoundRect(rect, dp(10f), dp(10f), paint)
            paint.color = tokens.hwBarText
            canvas.drawText(label, rect.centerX(), cy, paint)
            hit.add(action to RectF(rect))
            rx -= dp(6f)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_UP) {
            for ((action, rect) in hit) {
                if (rect.contains(event.x, event.y)) {
                    performClick()
                    onAction?.invoke(action)
                    return true
                }
            }
        }
        return true
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}

private class HwBottomBar(context: Context) : View(context) {
    private var tokens = ThemeTokens()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private var onBack: (() -> Unit)? = null
    private var backRect = RectF()

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        setBackgroundColor(tokens.hwBarBg)
        invalidate()
    }

    fun setOnBack(listener: () -> Unit) {
        onBack = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.hwBarBg)
        paint.textSize = sp(14f)
        paint.textAlign = Paint.Align.CENTER
        val label = "返回键盘"
        val tw = paint.measureText(label)
        val w = max(dp(96f), tw + dp(24f))
        backRect = RectF(dp(10f), dp(4f), dp(10f) + w, height - dp(4f))
        paint.style = Paint.Style.FILL
        paint.color = tokens.hwBarAccent
        canvas.drawRoundRect(backRect, dp(10f), dp(10f), paint)
        paint.color = 0xFFFFFFFF.toInt()
        val cy = height / 2f - (paint.descent() + paint.ascent()) / 2
        canvas.drawText(label, backRect.centerX(), cy, paint)
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_UP && backRect.contains(event.x, event.y)) {
            performClick()
            onBack?.invoke()
        }
        return true
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
