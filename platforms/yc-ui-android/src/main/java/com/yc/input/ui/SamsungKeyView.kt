package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import kotlin.math.max

class SamsungKeyView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), KeyView {

    private var tokens = ThemeTokens()
    private var rows: List<List<KeyDef>> = Layout26Pinyin.rows
    private var onKey: ((KeyDef) -> Unit)? = null
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val keyBounds = mutableListOf<Pair<KeyDef, RectF>>()
    private var pressedKey: KeyDef? = null

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    fun setLayoutRows(newRows: List<List<KeyDef>>) {
        rows = newRows
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
        val keyH = dp(44f)
        var y = margin
        val rows = this.rows

        for (row in rows) {
            val totalWeight = row.sumOf { it.widthWeight.toDouble() }.toFloat()
            val rowWidth = width - margin * 2
            var x = margin
            for (key in row) {
                val w = max(dp(28f), rowWidth * key.widthWeight / totalWeight)
                val rect = RectF(x, y, x + w - dp(3f), y + keyH)
                keyBounds.add(key to rect)
                val bg = when {
                    key == pressedKey -> tokens.keyPressed
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
            y += keyH + rowGap
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE -> {
                pressedKey = hitTest(event.x, event.y)
                invalidate()
            }
            MotionEvent.ACTION_UP -> {
                val key = hitTest(event.x, event.y)
                pressedKey = null
                invalidate()
                if (key != null) {
                    onKey?.invoke(key)
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                pressedKey = null
                invalidate()
            }
        }
        return true
    }

    private fun hitTest(x: Float, y: Float): KeyDef? {
        for ((key, rect) in keyBounds) {
            if (rect.contains(x, y)) return key
        }
        return null
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
