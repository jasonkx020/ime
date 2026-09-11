package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import kotlin.math.max

class SamsungCandBar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), CandBar {

    private var tokens = ThemeTokens()
    private var snapshot = KeyboardSnapshot(0, 0, "", emptyList())
    private var onCandidate: ((CandidateItem) -> Unit)? = null
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val chipBounds = mutableListOf<Pair<CandidateItem, RectF>>()

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    override fun render(snapshot: KeyboardSnapshot) {
        this.snapshot = snapshot
        invalidate()
    }

    override fun setOnCandidateListener(listener: (CandidateItem) -> Unit) {
        onCandidate = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.keyboardBg)
        chipBounds.clear()
        var x = dp(10f)
        val cy = height / 2f

        if (snapshot.composing.isNotEmpty()) {
            paint.color = tokens.composingText
            paint.textSize = sp(13f)
            paint.textAlign = Paint.Align.LEFT
            canvas.drawText(snapshot.composing, x, cy - (paint.descent() + paint.ascent()) / 2, paint)
            x += paint.measureText(snapshot.composing) + dp(12f)
        }

        paint.textSize = sp(tokens.candFontSp)
        for (cand in snapshot.candidates) {
            val tw = paint.measureText(cand.text)
            val chipW = max(dp(36f), tw + dp(20f))
            val rect = RectF(x, dp(8f), x + chipW, height - dp(8f))
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
                cand.text,
                rect.centerX(),
                cy - (paint.descent() + paint.ascent()) / 2,
                paint,
            )
            chipBounds.add(cand to rect)
            x += chipW + dp(6f)
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_UP) {
            for ((cand, rect) in chipBounds) {
                if (rect.contains(event.x, event.y)) {
                    onCandidate?.invoke(cand)
                    return true
                }
            }
        }
        return true
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float = v * resources.displayMetrics.scaledDensity
}
