package com.yc.input.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.util.TypedValue
import android.view.MotionEvent
import android.view.SoundEffectConstants
import android.view.View
import android.view.HapticFeedbackConstants
import kotlin.math.max

class SamsungKeyView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs), KeyView {

    private var tokens = ThemeTokens.light()
    private var rows: List<List<KeyDef>> = Layout26Pinyin.rows
    private var onKey: ((KeyDef) -> Unit)? = null
    private var onKeyDown: ((KeyDef) -> Unit)? = null
    private var onKeyUp: ((KeyDef) -> Unit)? = null
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = 1f
    }
    private val shadowPaint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val keyBounds = mutableListOf<Triple<Int, Int, RectF>>()
    private var pressedRow: Int = -1
    private var pressedCol: Int = -1
    private var downRow: Int = -1
    private var downCol: Int = -1
    private var shiftState: ShiftState = ShiftState.Off
    /** 越南语顶行（专用字母）行下标，-1 表示无 */
    private var viSpecialRow: Int = -1
    /** latn | vi | th */
    private var scriptHint: String = "latn"
    private val backspaceRepeat = BackspaceRepeatController(this) { key ->
        onKey?.invoke(key)
    }

    init {
        isClickable = true
        isFocusable = false
    }

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        invalidate()
    }

    fun setScriptHint(hint: String) {
        scriptHint = hint
        invalidate()
    }

    fun setLayoutRows(newRows: List<List<KeyDef>>, viSpecialRowIndex: Int = -1) {
        rows = newRows
        viSpecialRow = viSpecialRowIndex
        pressedRow = -1
        pressedCol = -1
        downRow = -1
        downCol = -1
        backspaceRepeat.cancel()
        invalidate()
    }

    fun setShiftState(state: ShiftState) {
        shiftState = state
        invalidate()
    }

    override fun render(snapshot: KeyboardSnapshot) {
        invalidate()
    }

    override fun setOnKeyListener(listener: (KeyDef) -> Unit) {
        onKey = listener
    }

    fun setOnKeyDownListener(listener: (KeyDef) -> Unit) {
        onKeyDown = listener
    }

    fun setOnKeyUpListener(listener: (KeyDef) -> Unit) {
        onKeyUp = listener
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        canvas.drawColor(tokens.keyboardBg)
        keyBounds.clear()

        val margin = dp(8f)
        val rowGap = dp(5f)
        val keyH = dp(tokens.keyHeightDp)
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
        val radius = dp(tokens.keyRadiusDp)

        for ((ri, row) in rows.withIndex()) {
            val totalWeight = row.sumOf { it.widthWeight.toDouble() }.toFloat()
            val rowWidth = width - margin * 2
            var x = margin
            for ((ci, key) in row.withIndex()) {
                val w = max(dp(28f), rowWidth * key.widthWeight / totalWeight)
                val gap = dp(3f)
                val rect = RectF(x, y, x + w - gap, y + useH)
                keyBounds.add(Triple(ri, ci, rect))
                val pressed = ri == pressedRow && ci == pressedCol
                drawKey(canvas, key, rect, radius, pressed, ri)
                x += w
            }
            y += useH + rowGap
        }
    }

    private fun drawKey(
        canvas: Canvas,
        key: KeyDef,
        rect: RectF,
        radius: Float,
        pressed: Boolean,
        rowIndex: Int,
    ) {
        val drawRect = if (pressed) {
            val cx = rect.centerX()
            val cy = rect.centerY()
            val sw = rect.width() * 0.96f
            val sh = rect.height() * 0.96f
            RectF(cx - sw / 2, cy - sh / 2, cx + sw / 2, cy + sh / 2)
        } else {
            rect
        }

        val bg = when {
            pressed -> tokens.keyPressed
            key.action == KeyAction.Shift && shiftState == ShiftState.Locked -> tokens.keyAccent
            key.action == KeyAction.Shift && shiftState == ShiftState.Once -> tokens.shiftOn
            key.action == KeyAction.Tone -> tokens.toneBg
            key.style == KeyStyle.Accent && key.action != KeyAction.Search -> tokens.keyAccent
            key.action == KeyAction.Search -> tokens.keyUtility
            key.action == KeyAction.Space -> tokens.keySpace
            key.style == KeyStyle.Utility -> tokens.keyUtility
            rowIndex == viSpecialRow && key.action == KeyAction.Letter -> tokens.viSpecialBg
            else -> tokens.keyNormal
        }

        // 轻阴影（未按下）
        if (!pressed && key.action != KeyAction.Shift) {
            shadowPaint.color = if (tokens.isDark) 0x33000000 else 0x0D000000
            val shadow = RectF(drawRect.left, drawRect.top + dp(1f), drawRect.right, drawRect.bottom + dp(1f))
            canvas.drawRoundRect(shadow, radius, radius, shadowPaint)
        }

        paint.style = Paint.Style.FILL
        paint.color = bg
        canvas.drawRoundRect(drawRect, radius, radius, paint)

        if (key.action == KeyAction.Letter || key.action == KeyAction.Tone ||
            key.action == KeyAction.Space || key.action == KeyAction.Search ||
            key.style == KeyStyle.Utility
        ) {
            strokePaint.color = tokens.keyBorder
            strokePaint.strokeWidth = dp(1f)
            canvas.drawRoundRect(drawRect, radius, radius, strokePaint)
        }

        val label = when {
            key.action == KeyAction.Shift && shiftState == ShiftState.Locked -> "⇪"
            else -> key.label
        }
        val textColor = when {
            key.action == KeyAction.Shift && shiftState == ShiftState.Locked -> 0xFFFFFFFF.toInt()
            key.action == KeyAction.Shift && shiftState == ShiftState.Once -> tokens.shiftOnText
            key.action == KeyAction.Tone -> tokens.toneText
            key.action == KeyAction.Search -> tokens.enterText
            key.style == KeyStyle.Accent -> 0xFFFFFFFF.toInt()
            key.action == KeyAction.Space -> tokens.toolbarText
            key.style == KeyStyle.Utility -> tokens.toolbarText
            else -> tokens.candText
        }
        val fontSp = when {
            key.action == KeyAction.Tone -> 20f
            scriptHint == "th" && key.action == KeyAction.Letter -> 17f
            scriptHint == "vi" && key.action == KeyAction.Letter &&
                (rowIndex == viSpecialRow || label.length == 1) -> 18f
            key.action == KeyAction.Letter && label.length == 1 -> tokens.keyFontSp
            key.style == KeyStyle.Utility || key.action == KeyAction.Space ||
                key.action == KeyAction.Search || key.action == KeyAction.Shift ||
                key.action == KeyAction.Mic || key.action == KeyAction.Globe ||
                key.action == KeyAction.Symbol || key.action == KeyAction.Letters -> 13f
            else -> tokens.keyFontSp
        }
        paint.color = textColor
        paint.textSize = sp(fontSp)
        paint.textAlign = Paint.Align.CENTER
        paint.isFakeBoldText = key.action == KeyAction.Tone || key.action == KeyAction.Search
        val ty = drawRect.centerY() - (paint.descent() + paint.ascent()) / 2
        canvas.drawText(label, drawRect.centerX(), ty, paint)
        paint.isFakeBoldText = false
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                val hit = hitTest(event.x, event.y)
                if (hit != null) {
                    pressedRow = hit.first
                    pressedCol = hit.second
                    downRow = hit.first
                    downCol = hit.second
                    performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
                    playSoundEffect(SoundEffectConstants.CLICK)
                    val key = rows.getOrNull(hit.first)?.getOrNull(hit.second)
                    key?.let { onKeyDown?.invoke(it) }
                    if (key?.action == KeyAction.Backspace) {
                        backspaceRepeat.onDown(key)
                    } else {
                        backspaceRepeat.cancel()
                    }
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
                val downKey = rows.getOrNull(downRow)?.getOrNull(downCol)
                if (downKey?.action == KeyAction.Backspace) {
                    val still = hit != null && hit.first == downRow && hit.second == downCol
                    backspaceRepeat.onMoveStay(still)
                }
            }
            MotionEvent.ACTION_UP -> {
                val hit = hitTest(event.x, event.y)
                val downKey = if (downRow >= 0 && downCol >= 0) {
                    rows.getOrNull(downRow)?.getOrNull(downCol)
                } else null
                val wasRepeat = backspaceRepeat.consumedByRepeat()
                backspaceRepeat.onUpOrCancel()
                pressedRow = -1
                pressedCol = -1
                downRow = -1
                downCol = -1
                invalidate()
                // 按住说话：即使滑开也在抬手时结束
                downKey?.let { onKeyUp?.invoke(it) }
                if (hit != null) {
                    val (ri, ci) = hit
                    val key = rows.getOrNull(ri)?.getOrNull(ci) ?: return true
                    when {
                        key.action == KeyAction.Mic -> Unit
                        key.action == KeyAction.Backspace -> {
                            if (!wasRepeat && downKey?.action == KeyAction.Backspace && downKey == key) {
                                onKey?.invoke(key)
                            }
                        }
                        else -> onKey?.invoke(key)
                    }
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                backspaceRepeat.onUpOrCancel()
                pressedRow = -1
                pressedCol = -1
                downRow = -1
                downCol = -1
                invalidate()
                // 不触发 onKeyUp，避免按住说话误提交
            }
        }
        return true
    }

    override fun onDetachedFromWindow() {
        backspaceRepeat.cancel()
        super.onDetachedFromWindow()
    }

    private fun hitTest(x: Float, y: Float): Pair<Int, Int>? {
        for ((ri, ci, rect) in keyBounds) {
            if (rect.contains(x, y)) return ri to ci
        }
        return null
    }

    private fun dp(v: Float): Float = v * resources.displayMetrics.density
    private fun sp(v: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, resources.displayMetrics)
}
