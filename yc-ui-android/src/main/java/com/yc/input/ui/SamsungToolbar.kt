package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View

class SamsungToolbar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), ToolbarView {

    private var tokens = ThemeTokens()
    private var onItem: ((String) -> Unit)? = null
    private val items = listOf("设置", "翻译", "剪贴板", "语音", "表情", "手写")
    private val itemBounds = mutableListOf<Pair<String, RectF>>()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    override fun setOnItemClick(listener: (String) -> Unit) {
        onItem = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.keyboardBg)
        itemBounds.clear()
        val itemW = width / items.size.toFloat()
        paint.textSize = sp(12f)
        paint.color = tokens.toolbarText
        paint.textAlign = Paint.Align.CENTER
        items.forEachIndexed { i, label ->
            val left = i * itemW
            val rect = RectF(left, 0f, left + itemW, height.toFloat())
            itemBounds.add(label to rect)
            val ty = rect.centerY() - (paint.descent() + paint.ascent()) / 2
            canvas.drawText(label, rect.centerX(), ty, paint)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_UP) {
            for ((label, rect) in itemBounds) {
                if (rect.contains(event.x, event.y)) {
                    onItem?.invoke(label)
                    return true
                }
            }
        }
        return true
    }

    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
