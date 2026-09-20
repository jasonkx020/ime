package com.yc.input.ui

import android.animation.ObjectAnimator
import android.animation.ValueAnimator
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.util.TypedValue
import android.view.ActionMode
import android.view.Gravity
import android.view.Menu
import android.view.MenuItem
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
        val relation: String = "",
        val intentLabel: String = "",
        val toneLabel: String = "",
        val backgroundNote: String = "",
        val userIntent: String = "",
        val sceneId: String = "",
    )

    private var tokens = ThemeTokens.light()
    private val modeRow = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val relationRow = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val intentRow = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val toneRow = LinearLayout(context).apply { orientation = HORIZONTAL }
    private val input = EditText(context)
    private val status = TextView(context)
    private val results = LinearLayout(context).apply { orientation = VERTICAL }
    private val tunnelShell = FrameLayout(context)
    private val tunnelGlow = View(context)
    private val tunnelLabel = TextView(context)
    private var selectedMode = "智能回复"
    private var selectedRelation = "客户"
    private var selectedIntent = "再问问"
    private var selectedTone = "默认"
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
    private val relations = listOf("客户", "老板", "同事", "朋友", "家人", "恋爱", "陌生", "客服对象")
    private val tones = listOf("默认", "更正式", "更轻松", "高情商", "更短")

    private fun intentsFor(relation: String): List<String> = when (relation) {
        "恋爱" -> listOf("接话", "调侃", "关心", "约出来", "缓和", "结束话题")
        "客服对象" -> listOf("查进度", "改地址", "退款安抚", "转人工", "道歉", "确认需求")
        "老板" -> listOf("汇报进展", "请示", "要资源", "给结论", "婉拒加活", "约时间")
        "家人", "朋友" -> listOf("接话", "关心", "约出来", "安慰", "分享近况")
        "陌生" -> listOf("自我介绍", "探需求", "约时间", "礼貌收尾")
        else -> listOf("答应", "婉拒", "再问问", "给方案", "约时间", "催一下", "道歉")
    }

    private val clearDraftActionMode = object : ActionMode.Callback {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
            menu.add(0, MENU_CLEAR_DRAFT, 0, "删除")
            return true
        }

        override fun onPrepareActionMode(mode: ActionMode, menu: Menu): Boolean = false

        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            if (item.itemId == MENU_CLEAR_DRAFT) {
                input.text?.clear()
                mode.finish()
                return true
            }
            return false
        }

        override fun onDestroyActionMode(mode: ActionMode) = Unit
    }

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

        addChipScroll(relationRow, "对谁")
        addChipScroll(intentRow, "想怎样")
        addChipScroll(toneRow, "语气")
        rebuildRelation()
        rebuildIntent()
        rebuildTone()

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

        input.hint = "复制对方消息，或点下方键盘输入…"
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
        input.customSelectionActionModeCallback = clearDraftActionMode
        input.customInsertionActionModeCallback = clearDraftActionMode
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
        tunnelShell.setOnClickListener { fireGenerate() }
    }

    private fun fireGenerate() {
        val ctx = compileContext()
        onGenerate?.invoke(
            GenerateRequest(
                modeLabel = selectedMode,
                input = input.text?.toString().orEmpty(),
                targetLang = "en",
                relation = selectedRelation,
                intentLabel = selectedIntent,
                toneLabel = selectedTone,
                backgroundNote = ctx.background,
                userIntent = ctx.intent,
                sceneId = ctx.sceneId,
            ),
        )
    }

    private data class CompiledContext(
        val background: String,
        val intent: String,
        val sceneId: String,
    )

    private fun compileContext(): CompiledContext {
        val scene = when (selectedRelation) {
            "恋爱" -> "dating"
            "客服对象" -> "customer_followup"
            "老板", "同事" -> "work_chat"
            "客户" -> "customer_followup"
            else -> "work_chat"
        }
        val toneHint = when (selectedTone) {
            "更正式" -> "语气偏正式、礼貌。"
            "更轻松" -> "语气轻松自然。"
            "高情商" -> "语气高情商、留有余地。"
            "更短" -> "回复尽量短。"
            else -> when (selectedRelation) {
                "客户", "客服对象" -> "语气专业有温度。"
                "老板" -> "简洁、尊重、结论先行。"
                "恋爱" -> "真诚、有分寸、不油腻。"
                "家人", "朋友" -> "亲切自然。"
                "陌生" -> "礼貌、探需求、不施压。"
                else -> "得体清晰。"
            }
        }
        val taboo = when (selectedRelation) {
            "客户", "客服对象" -> "不贬低竞品、不承诺未核实事项。"
            "恋爱" -> "不油腻、不做道德绑架。"
            "老板" -> "不推诿、不空话。"
            else -> "不冒犯、不夸张承诺。"
        }
        val intentText = when (selectedIntent) {
            "答应" -> "表示同意并推进下一步。"
            "婉拒" -> "礼貌婉拒，尽量留后续空间。"
            "再问问" -> "追问关键细节，便于继续推进。"
            "给方案" -> "给出可行方案或选项。"
            "约时间" -> "推动约定具体时间。"
            "催一下" -> "礼貌催促，不显得强硬。"
            "道歉" -> "真诚道歉并给补救动作。"
            "接话" -> "自然接话，延续对话。"
            "调侃" -> "轻度幽默调侃，把握分寸。"
            "关心" -> "表达关心，询问近况。"
            "约出来" -> "自然邀约见面。"
            "缓和" -> "缓和气氛，降低对立。"
            "结束话题" -> "得体收尾。"
            "查进度" -> "询问处理进度。"
            "改地址" -> "协助确认/修改地址信息。"
            "退款安抚" -> "安抚情绪并说明退款处理。"
            "转人工" -> "引导转人工并安抚等待。"
            "确认需求" -> "确认用户具体需求。"
            "汇报进展" -> "简要汇报进展与下一步。"
            "请示" -> "请示决策并给建议选项。"
            "要资源" -> "明确要什么资源及原因。"
            "给结论" -> "先给结论再补依据。"
            "婉拒加活" -> "婉拒额外工作量并给替代方案。"
            "自我介绍" -> "简短自我介绍并说明来意。"
            "探需求" -> "了解对方需求。"
            "礼貌收尾" -> "礼貌结束本轮沟通。"
            "安慰" -> "安慰对方并表示支持。"
            "分享近况" -> "分享近况并回问对方。"
            else -> "按「$selectedIntent」完成得体回复。"
        }
        val background =
            "对象：$selectedRelation。$toneHint$taboo"
        return CompiledContext(background, intentText, scene)
    }

    private fun addChipScroll(row: LinearLayout, contentDescription: String) {
        val scroll = HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            this.contentDescription = contentDescription
            addView(row)
        }
        addView(
            scroll,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                topMargin = dp(2)
            },
        )
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
        rebuildRelation()
        rebuildIntent()
        rebuildTone()
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
        rebuildChipRow(modeRow, modes, selectedMode) { label ->
            selectedMode = label
            rebuildModes()
        }
    }

    private fun rebuildRelation() {
        rebuildChipRow(relationRow, relations, selectedRelation) { label ->
            selectedRelation = label
            val intents = intentsFor(label)
            if (selectedIntent !in intents) selectedIntent = intents.first()
            rebuildRelation()
            rebuildIntent()
        }
    }

    private fun rebuildIntent() {
        rebuildChipRow(intentRow, intentsFor(selectedRelation), selectedIntent) { label ->
            selectedIntent = label
            rebuildIntent()
        }
    }

    private fun rebuildTone() {
        rebuildChipRow(toneRow, tones, selectedTone) { label ->
            selectedTone = label
            rebuildTone()
        }
    }

    private fun rebuildChipRow(
        row: LinearLayout,
        labels: List<String>,
        selected: String,
        onClick: (String) -> Unit,
    ) {
        row.removeAllViews()
        labels.forEach { label ->
            val on = label == selected
            row.addView(
                TextView(context).apply {
                    text = label
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
                    setTextColor(if (on) 0xFF1A73E8.toInt() else tokens.toolbarText)
                    typeface = if (on) Typeface.DEFAULT_BOLD else Typeface.DEFAULT
                    setPadding(dp(7), dp(2), dp(7), dp(2))
                    background = GradientDrawable().apply {
                        setColor(if (on) 0xFFE8F0FE.toInt() else tokens.keyUtility)
                        cornerRadius = dp(6).toFloat()
                    }
                    setOnClickListener { onClick(label) }
                },
                LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                    rightMargin = dp(3)
                },
            )
        }
    }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    companion object {
        private const val MENU_CLEAR_DRAFT = 0x11C1
    }
}
