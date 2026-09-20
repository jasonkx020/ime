package com.yc.input.ui

import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.animation.AccelerateDecelerateInterpolator
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/**
 * 键盘内 AI 辅助面板：左输入 / 中魔法隧道 / 右结果。
 * 打开期间监听剪贴板，复制纯文本自动填入输入框。
 */
class AiAssistPanel(context: Context) : LinearLayout(context) {
    data class GenerateRequest(
        val modeLabel: String,
        val input: String,
        val targetLang: String,
    )

    private var tokens = ThemeTokens.light()
    private val modeRow = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val input = EditText(context)
    private val status = TextView(context)
    private val results = LinearLayout(context).apply { orientation = VERTICAL }
    private val tunnelShell = FrameLayout(context)
    private val tunnelGlow = View(context)
    private val tunnelLabel = TextView(context)
    private var selectedMode = "润色"
    private var onGenerate: ((GenerateRequest) -> Unit)? = null
    private var onPick: ((String) -> Unit)? = null
    private var onClose: (() -> Unit)? = null
    private var clipListening = false
    private var tunnelAlphaAnim: ObjectAnimator? = null
    private var tunnelTravelAnim: ObjectAnimator? = null

    private val clipboard =
        context.applicationContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (!isShowing()) return@OnPrimaryClipChangedListener
        val text = readPrimaryPlainText() ?: return@OnPrimaryClipChangedListener
        ingestExternalText(text)
    }

    private val modes = listOf("智能回复", "高情商", "润色", "翻译", "撰写")

    init {
        orientation = VERTICAL
        visibility = GONE
        setPadding(dp(8), dp(4), dp(8), dp(4))

        addView(
            LinearLayout(context).apply {
                orientation = HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                addView(
                    TextView(context).apply {
                        text = "AI 辅助"
                        typeface = Typeface.DEFAULT_BOLD
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    },
                    LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f),
                )
                addView(
                    TextView(context).apply {
                        text = "关闭"
                        setTextColor(0xFF1A73E8.toInt())
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                        setPadding(dp(8), dp(2), 0, dp(2))
                        setOnClickListener {
                            onClose?.invoke()
                            hide()
                        }
                    },
                )
            },
        )

        val modeScroll = HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(modeRow)
        }
        addView(
            modeScroll,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                topMargin = dp(2)
            },
        )
        rebuildModes()

        status.setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
        status.setTextColor(0xFF5F6368.toInt())
        status.setPadding(0, dp(2), 0, 0)
        status.visibility = GONE
        addView(status)

        // 主体：左输入 | 中隧道 | 右结果
        val body = LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        input.hint = "输入或复制文本…"
        input.minLines = 1
        input.maxLines = Int.MAX_VALUE
        input.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
        input.setPadding(dp(8), dp(6), dp(8), dp(6))
        input.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
        input.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        input.isFocusable = true
        input.isFocusableInTouchMode = true
        input.background = GradientDrawable().apply {
            setColor(0xFFF8F9FA.toInt())
            cornerRadius = dp(8).toFloat()
            setStroke(dp(1), 0xFFE0E0E0.toInt())
        }
        val inputScroll = ScrollView(context).apply {
            isFillViewport = true
            addView(
                input,
                LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT),
            )
        }
        body.addView(
            inputScroll,
            LayoutParams(0, LayoutParams.MATCH_PARENT, 1f),
        )

        buildTunnel()
        body.addView(
            tunnelShell,
            LayoutParams(dp(52), LayoutParams.MATCH_PARENT).apply {
                leftMargin = dp(6)
                rightMargin = dp(6)
            },
        )

        val resultScroll = ScrollView(context).apply {
            isFillViewport = true
            addView(
                results,
                LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT),
            )
        }
        body.addView(
            resultScroll,
            LayoutParams(0, LayoutParams.MATCH_PARENT, 1f),
        )

        addView(
            body,
            LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f).apply {
                topMargin = dp(4)
            },
        )
    }

    private fun buildTunnel() {
        tunnelShell.background = GradientDrawable().apply {
            orientation = GradientDrawable.Orientation.TOP_BOTTOM
            colors = intArrayOf(0xFF90CAF9.toInt(), 0xFF1565C0.toInt(), 0xFF90CAF9.toInt())
            cornerRadius = dp(26).toFloat()
        }

        tunnelGlow.background = GradientDrawable().apply {
            setColor(0x66FFFFFF)
            cornerRadius = dp(10).toFloat()
        }
        tunnelGlow.alpha = 0.35f
        tunnelShell.addView(
            tunnelGlow,
            FrameLayout.LayoutParams(dp(18), dp(36), Gravity.CENTER),
        )

        tunnelLabel.text = "✨"
        tunnelLabel.gravity = Gravity.CENTER
        tunnelLabel.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        tunnelLabel.setTextColor(0xFFFFFFFF.toInt())
        tunnelShell.addView(
            tunnelLabel,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )

        tunnelShell.isClickable = true
        tunnelShell.setOnClickListener {
            onGenerate?.invoke(
                GenerateRequest(
                    modeLabel = selectedMode,
                    input = input.text?.toString().orEmpty(),
                    targetLang = "en",
                ),
            )
        }
    }

    fun setOnGenerate(listener: (GenerateRequest) -> Unit) {
        onGenerate = listener
    }

    fun setOnPick(listener: (String) -> Unit) {
        onPick = listener
    }

    fun setOnClose(listener: () -> Unit) {
        onClose = listener
    }

    fun show(preselectMode: String? = null, seedText: String = "", t: ThemeTokens) {
        tokens = t
        setBackgroundColor(t.panelBg)
        input.setTextColor(t.candText)
        input.setHintTextColor(0x995F6368.toInt())
        input.background = GradientDrawable().apply {
            setColor(t.keyNormal)
            cornerRadius = dp(8).toFloat()
            setStroke(dp(1), t.keyBorder)
        }
        if (!preselectMode.isNullOrBlank()) selectedMode = preselectMode
        if (seedText.isNotBlank()) input.setText(seedText)
        rebuildModes()
        setStatus("")
        results.removeAllViews()
        stopTunnelAnim()
        visibility = VISIBLE
        input.clearFocus()
        clearFocus()
        registerClipboardListener()
    }

    fun hide() {
        unregisterClipboardListener()
        stopTunnelAnim()
        input.clearFocus()
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    fun setStatus(msg: String) {
        if (msg == "生成中…") {
            status.visibility = GONE
            status.text = ""
            startTunnelAnim()
            return
        }
        stopTunnelAnim()
        status.text = msg
        status.visibility = if (msg.isBlank()) GONE else VISIBLE
    }

    fun showVariants(texts: List<String>, local: Boolean) {
        stopTunnelAnim()
        results.removeAllViews()
        if (texts.isEmpty()) {
            setStatus("无结果")
            return
        }
        if (local) {
            setStatus("未配置 API Key，请到 App 设置 → AI 大模型")
        } else {
            setStatus("")
        }
        texts.forEachIndexed { index, text ->
            val card = TextView(context).apply {
                this.text = text
                setTextColor(tokens.candText)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                setPadding(dp(8), dp(6), dp(8), dp(6))
                background = GradientDrawable().apply {
                    setColor(tokens.keyNormal)
                    cornerRadius = dp(8).toFloat()
                    setStroke(dp(1), tokens.keyBorder)
                }
                val lp = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
                lp.bottomMargin = dp(4)
                layoutParams = lp
                setOnClickListener { onPick?.invoke(text) }
                // 从隧道侧（左侧）滑入
                alpha = 0f
                translationX = -dp(24).toFloat()
            }
            results.addView(card)
            card.animate()
                .alpha(1f)
                .translationX(0f)
                .setStartDelay((index * 40).toLong())
                .setDuration(200)
                .setInterpolator(AccelerateDecelerateInterpolator())
                .start()
        }
    }

    fun prefill(text: String) {
        ingestExternalText(text)
    }

    /** 复制/剪切得到的外部文本填入输入框（相同内容则跳过）。 */
    fun ingestExternalText(text: String) {
        val t = text.trim()
        if (t.isEmpty()) return
        if (t == input.text?.toString()) return
        input.setText(t)
        input.setSelection(t.length)
    }

    private fun startTunnelAnim() {
        stopTunnelAnim()
        tunnelLabel.text = "→"
        tunnelAlphaAnim = ObjectAnimator.ofFloat(tunnelGlow, View.ALPHA, 0.2f, 0.95f, 0.2f).apply {
            duration = 600
            repeatCount = ValueAnimator.INFINITE
            interpolator = AccelerateDecelerateInterpolator()
            start()
        }
        tunnelTravelAnim = ObjectAnimator.ofFloat(
            tunnelGlow,
            View.TRANSLATION_Y,
            -dp(28).toFloat(),
            dp(28).toFloat(),
        ).apply {
            duration = 600
            repeatCount = ValueAnimator.INFINITE
            repeatMode = ValueAnimator.REVERSE
            interpolator = AccelerateDecelerateInterpolator()
            start()
        }
    }

    private fun stopTunnelAnim() {
        tunnelAlphaAnim?.cancel()
        tunnelAlphaAnim = null
        tunnelTravelAnim?.cancel()
        tunnelTravelAnim = null
        tunnelGlow.alpha = 0.35f
        tunnelGlow.translationY = 0f
        tunnelLabel.text = "✨"
    }

    private fun registerClipboardListener() {
        if (clipListening) return
        clipboard.addPrimaryClipChangedListener(clipListener)
        clipListening = true
    }

    private fun unregisterClipboardListener() {
        if (!clipListening) return
        clipboard.removePrimaryClipChangedListener(clipListener)
        clipListening = false
    }

    private fun readPrimaryPlainText(): String? {
        val clip = clipboard.primaryClip ?: return null
        if (clip.itemCount <= 0) return null
        val item = clip.getItemAt(0) ?: return null
        return item.coerceToText(context)?.toString()?.trim()?.takeIf { it.isNotEmpty() }
    }

    private fun rebuildModes() {
        modeRow.removeAllViews()
        modes.forEach { label ->
            val on = label == selectedMode
            modeRow.addView(
                TextView(context).apply {
                    text = label
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
                    setTextColor(if (on) 0xFF1A73E8.toInt() else tokens.toolbarText)
                    typeface = if (on) Typeface.DEFAULT_BOLD else Typeface.DEFAULT
                    setPadding(dp(8), dp(3), dp(8), dp(3))
                    background = GradientDrawable().apply {
                        setColor(if (on) 0xFFE8F0FE.toInt() else tokens.keyUtility)
                        cornerRadius = dp(6).toFloat()
                    }
                    setOnClickListener {
                        selectedMode = label
                        rebuildModes()
                    }
                },
                LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                    rightMargin = dp(4)
                },
            )
        }
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
