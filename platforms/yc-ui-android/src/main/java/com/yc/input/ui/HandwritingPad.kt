package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.util.AttributeSet
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.SoundEffectConstants
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import kotlin.math.max
import kotlin.math.min

/**
 * 中文手写面板：对齐参考效果.html
 * [米字格写字区 | 右侧 ⌫？，。] + 底栏 [123 🌐 🎤 空格 下一步]
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

    private var tokens = ThemeTokens.light()
    private val mainRow = LinearLayout(context).apply {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER
    }
    private val inkHost = SquareInkHost(context)
    private val ink = InkCanvas(context)
    private val sideBar = HwSideBar(context)
    private val bottomBar = HwBottomBar(context)

    private var continuous = false
    private var recognizing = false
    private var onStroke: ((StrokePayload) -> Unit)? = null
    private var onRecognize: (() -> Unit)? = null
    private var onUndo: (() -> Unit)? = null
    private var onClear: (() -> Unit)? = null
    private var onDismiss: (() -> Unit)? = null
    private var onModeChanged: ((continuous: Boolean) -> Unit)? = null
    private var onChromeKey: ((KeyDef) -> Unit)? = null
    private var onChromeKeyDown: ((KeyDef) -> Unit)? = null
    private var onChromeKeyUp: ((KeyDef) -> Unit)? = null

    init {
        orientation = VERTICAL
        setPadding(dp(6), dp(6), dp(6), dp(6))

        inkHost.addView(
            ink,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
                Gravity.CENTER,
            ),
        )
        mainRow.addView(
            inkHost,
            LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f),
        )
        mainRow.addView(
            sideBar,
            LayoutParams(dp(52), LayoutParams.MATCH_PARENT).apply {
                leftMargin = dp(6)
            },
        )
        addView(mainRow, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        addView(
            bottomBar,
            LayoutParams(LayoutParams.MATCH_PARENT, dp(44)).apply {
                topMargin = dp(6)
            },
        )

        sideBar.setOnKey { key ->
            if (key.action == KeyAction.Backspace) {
                // 有墨迹时先撤销一笔，否则交给 shell 退格
                if (ink.hasInk()) {
                    ink.undoLastStroke()
                    onUndo?.invoke()
                } else {
                    onChromeKey?.invoke(key)
                }
            } else {
                onChromeKey?.invoke(key)
            }
        }
        bottomBar.setOnKey { onChromeKey?.invoke(it) }
        bottomBar.setOnKeyDown { onChromeKeyDown?.invoke(it) }
        bottomBar.setOnKeyUp { onChromeKeyUp?.invoke(it) }

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
        ink.setOnInkChanged { inkHost.setHasInk(ink.hasInk()) }
        applyTheme(tokens)
    }

    fun setRecognizing(active: Boolean) {
        recognizing = active
    }

    fun isRecognizing(): Boolean = recognizing

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        setBackgroundColor(tokens.keyboardBg)
        inkHost.applyTheme(tokens)
        ink.applyTheme(tokens)
        sideBar.applyTheme(tokens)
        bottomBar.applyTheme(tokens)
    }

    fun setContinuous(continuous: Boolean) {
        this.continuous = continuous
        onModeChanged?.invoke(continuous)
    }

    fun isContinuous(): Boolean = continuous

    fun clearInk() {
        ink.clearInk()
        inkHost.setHasInk(false)
    }

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

    fun setOnChromeKeyListener(listener: (KeyDef) -> Unit) {
        onChromeKey = listener
    }

    fun setOnChromeKeyDownListener(listener: (KeyDef) -> Unit) {
        onChromeKeyDown = listener
    }

    fun setOnChromeKeyUpListener(listener: (KeyDef) -> Unit) {
        onChromeKeyUp = listener
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}

/** 强制正方形写字区（边长 = min(可用宽, 最大边)）。 */
private class SquareInkHost(context: Context) : FrameLayout(context) {
    private var tokens = ThemeTokens.light()
    private val borderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = dp(1.5f)
    }
    private val clipPath = Path()
    private val margin = dp(0)
    private val maxSide = dp(300)
    private val radius = dp(10f)
    private var hasInk = false

    fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        borderPaint.color = tokens.hwBorder
        setBackgroundColor(tokens.keyboardBg)
        invalidate()
    }

    fun setHasInk(has: Boolean) {
        if (hasInk == has) return
        hasInk = has
        (getChildAt(0) as? InkCanvas)?.setShowHint(!has)
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
        val side = min(max(width - margin * 2, dp(120)), maxSide)
        val l = (width - side) / 2f
        val t = margin.toFloat()
        val rect = RectF(l, t, l + side, t + side)
        clipPath.reset()
        clipPath.addRoundRect(rect, radius, radius, Path.Direction.CW)
        val save = canvas.save()
        canvas.clipPath(clipPath)
        super.dispatchDraw(canvas)
        canvas.restoreToCount(save)
        canvas.drawRoundRect(rect, radius, radius, borderPaint)
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
    private fun dp(v: Float): Float = v * resources.displayMetrics.density
}

/** 右侧：⌫ ？ ， 。 */
private class HwSideBar(context: Context) : View(context) {
    private var tokens = ThemeTokens.light()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = 1f
    }
    private val keys = listOf(
        KeyDef("⌫", 1f, KeyStyle.Utility, action = KeyAction.Backspace),
        KeyDef("？", 1f, KeyStyle.Normal, keyCode = '？'.code, action = KeyAction.Letter, output = "？"),
        KeyDef("，", 1f, KeyStyle.Normal, keyCode = '，'.code, action = KeyAction.Letter, output = "，"),
        KeyDef("。", 1f, KeyStyle.Normal, keyCode = '。'.code, action = KeyAction.Letter, output = "。"),
    )
    private val bounds = mutableListOf<Pair<KeyDef, RectF>>()
    private var pressed = -1
    private var onKey: ((KeyDef) -> Unit)? = null

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        invalidate()
    }

    fun setOnKey(listener: (KeyDef) -> Unit) {
        onKey = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        bounds.clear()
        val gap = dp(4f)
        val n = keys.size
        val btnH = (height - gap * (n - 1)) / n
        var y = 0f
        val radius = dp(8f)
        paint.textAlign = Paint.Align.CENTER
        for ((i, key) in keys.withIndex()) {
            val rect = RectF(0f, y, width.toFloat(), y + btnH)
            bounds.add(key to RectF(rect))
            val bg = if (i == pressed) tokens.keyPressed else tokens.hwBarBg
            paint.style = Paint.Style.FILL
            paint.color = bg
            canvas.drawRoundRect(rect, radius, radius, paint)
            strokePaint.color = tokens.keyBorder
            canvas.drawRoundRect(rect, radius, radius, strokePaint)
            paint.color = tokens.hwBarText
            paint.textSize = sp(if (key.action == KeyAction.Backspace) 20f else 18f)
            val cy = rect.centerY() - (paint.descent() + paint.ascent()) / 2
            canvas.drawText(key.label, rect.centerX(), cy, paint)
            y += btnH + gap
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                pressed = hit(event.x, event.y)
                if (pressed >= 0) {
                    performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                    playSoundEffect(SoundEffectConstants.CLICK)
                    invalidate()
                }
            }
            MotionEvent.ACTION_UP -> {
                val i = hit(event.x, event.y)
                if (i >= 0 && i == pressed) {
                    performClick()
                    onKey?.invoke(keys[i])
                }
                pressed = -1
                invalidate()
            }
            MotionEvent.ACTION_CANCEL -> {
                pressed = -1
                invalidate()
            }
        }
        return true
    }

    private fun hit(x: Float, y: Float): Int {
        bounds.forEachIndexed { i, (_, r) -> if (r.contains(x, y)) return i }
        return -1
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, resources.displayMetrics)
}

/** 底栏：123 / 🌐 / 🎤 / 空格 / 下一步 */
private class HwBottomBar(context: Context) : View(context) {
    private var tokens = ThemeTokens.light()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = 1f
    }
    private val keys = listOf(
        KeyDef("123", 1.1f, KeyStyle.Utility, action = KeyAction.Symbol),
        KeyDef("🌐", 1.1f, KeyStyle.Utility, action = KeyAction.Globe),
        KeyDef("🎤", 1.1f, KeyStyle.Utility, action = KeyAction.Mic),
        KeyDef("空格", 3.2f, KeyStyle.Normal, keyCode = ' '.code, action = KeyAction.Space),
        KeyDef("下一步", 1.4f, KeyStyle.Utility, action = KeyAction.Search),
    )
    private val bounds = mutableListOf<Pair<KeyDef, RectF>>()
    private var pressed = -1
    private var onKey: ((KeyDef) -> Unit)? = null
    private var onKeyDown: ((KeyDef) -> Unit)? = null
    private var onKeyUp: ((KeyDef) -> Unit)? = null

    fun applyTheme(t: ThemeTokens) {
        tokens = t
        invalidate()
    }

    fun setOnKey(listener: (KeyDef) -> Unit) {
        onKey = listener
    }

    fun setOnKeyDown(listener: (KeyDef) -> Unit) {
        onKeyDown = listener
    }

    fun setOnKeyUp(listener: (KeyDef) -> Unit) {
        onKeyUp = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        bounds.clear()
        val gap = dp(5f)
        val totalW = keys.sumOf { it.widthWeight.toDouble() }.toFloat()
        val usable = width - gap * (keys.size - 1)
        var x = 0f
        val radius = dp(6f)
        val top = dp(0f)
        val bot = height.toFloat()
        paint.textAlign = Paint.Align.CENTER
        for ((i, key) in keys.withIndex()) {
            val w = usable * key.widthWeight / totalW
            val rect = RectF(x, top, x + w, bot)
            bounds.add(key to RectF(rect))
            val bg = when {
                i == pressed -> tokens.keyPressed
                key.action == KeyAction.Space -> tokens.keySpace
                else -> tokens.keyUtility
            }
            paint.style = Paint.Style.FILL
            paint.color = bg
            canvas.drawRoundRect(rect, radius, radius, paint)
            strokePaint.color = tokens.keyBorder
            canvas.drawRoundRect(rect, radius, radius, strokePaint)
            paint.color = when (key.action) {
                KeyAction.Search -> tokens.enterText
                KeyAction.Space -> tokens.toolbarText
                else -> tokens.toolbarText
            }
            paint.isFakeBoldText = key.action == KeyAction.Search
            paint.textSize = sp(13f)
            val cy = rect.centerY() - (paint.descent() + paint.ascent()) / 2
            canvas.drawText(key.label, rect.centerX(), cy, paint)
            paint.isFakeBoldText = false
            x += w + gap
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                pressed = hit(event.x, event.y)
                if (pressed >= 0) {
                    performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                    playSoundEffect(SoundEffectConstants.CLICK)
                    onKeyDown?.invoke(keys[pressed])
                    invalidate()
                }
            }
            MotionEvent.ACTION_UP -> {
                val down = pressed
                val up = hit(event.x, event.y)
                if (down >= 0) onKeyUp?.invoke(keys[down])
                if (up >= 0 && up == down && keys[up].action != KeyAction.Mic) {
                    performClick()
                    onKey?.invoke(keys[up])
                }
                pressed = -1
                invalidate()
            }
            MotionEvent.ACTION_CANCEL -> {
                if (pressed >= 0) onKeyUp?.invoke(keys[pressed])
                pressed = -1
                invalidate()
            }
        }
        return true
    }

    private fun hit(x: Float, y: Float): Int {
        bounds.forEachIndexed { i, (_, r) -> if (r.contains(x, y)) return i }
        return -1
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, resources.displayMetrics)
}
