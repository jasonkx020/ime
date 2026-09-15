package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.SoundEffectConstants
import android.view.View
import android.view.HapticFeedbackConstants
import kotlin.math.max

class SamsungKeyView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), KeyView {

    private var tokens = ThemeTokens()
    private var rows: List<List<KeyDef>> = Layout26Pinyin.rows
    private var onKey: ((KeyDef) -> Unit)? = null
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val keyBounds = mutableListOf<Triple<Int, Int, RectF>>() // row, col, rect
    private var pressedRow: Int = -1
    private var pressedCol: Int = -1

    init {
        isClickable = true
        isFocusable = false
    }

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    fun setLayoutRows(newRows: List<List<KeyDef>>) {
        rows = newRows
        pressedRow = -1
        pressedCol = -1
        invalidate()
    }

    override fun render(snapshot: KeyboardSnapshot) {
        invalidate()
    }

    override fun setOnKeyListener(listener: (KeyDef) -> Unit) {
        onKey = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.keyboardBg)
        keyBounds.clear()

        val margin = dp(12f)
        val rowGap = dp(6f)
        val keyH = dp(42f)
        var y = margin
        val rows = this.rows

        val rowGapUsed = if (rows.size > 1) rowGap else 0f
        val contentH = height - margin * 2
        val fittedKeyH = if (rows.isNotEmpty()) {
            max(dp(36f), (contentH - rowGapUsed * (rows.size - 1)) / rows.size)
        } else {
            keyH
        }
        val useH = minOf(keyH, fittedKeyH)

        for ((ri, row) in rows.withIndex()) {
            val totalWeight = row.sumOf { it.widthWeight.toDouble() }.toFloat()
            val rowWidth = width - margin * 2
            var x = margin
            for ((ci, key) in row.withIndex()) {
                val w = max(dp(28f), rowWidth * key.widthWeight / totalWeight)
                val rect = RectF(x, y, x + w - dp(3f), y + useH)
                keyBounds.add(Triple(ri, ci, rect))
                val pressed = ri == pressedRow && ci == pressedCol
                val bg = when {
                    pressed -> tokens.keyPressed
                    key.style == KeyStyle.Utility -> tokens.keyUtility
                    key.style == KeyStyle.Accent -> tokens.keyAccent
                    else -> tokens.keyNormal
                }
                paint.color = bg
                canvas.drawRoundRect(rect, dp(tokens.keyRadiusDp), dp(tokens.keyRadiusDp), paint)
                paint.color = if (key.style == KeyStyle.Accent) 0xFFFFFFFF.toInt() else tokens.candText
                paint.textSize = sp(tokens.keyFontSp)
                paint.textAlign = Paint.Align.CENTER
                val ty = rect.centerY() - (paint.descent() + paint.ascent()) / 2
                canvas.drawText(key.label, rect.centerX(), ty, paint)
                x += w
            }
            y += useH + rowGap
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                val hit = hitTest(event.x, event.y)
                if (hit != null) {
                    pressedRow = hit.first
                    pressedCol = hit.second
                    performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                    playSoundEffect(SoundEffectConstants.CLICK)
                    invalidate()
                }
            }
            MotionEvent.ACTION_MOVE -> {
                val hit = hitTest(event.x, event.y)
                val nr = hit?.first ?: -1
                val nc = hit?.second ?: -1
                if (nr != pressedRow || nc != pressedCol) {
                    pressedRow = nr
                    pressedCol = nc
                    invalidate()
                }
            }
            MotionEvent.ACTION_UP -> {
                val hit = hitTest(event.x, event.y)
                pressedRow = -1
                pressedCol = -1
                invalidate()
                if (hit != null) {
                    val (ri, ci) = hit
                    rows.getOrNull(ri)?.getOrNull(ci)?.let { onKey?.invoke(it) }
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                pressedRow = -1
                pressedCol = -1
                invalidate()
            }
        }
        return true
    }

    private fun hitTest(x: Float, y: Float): Pair<Int, Int>? {
        for ((ri, ci, rect) in keyBounds) {
            if (rect.contains(x, y)) return ri to ci
        }
        return null
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
