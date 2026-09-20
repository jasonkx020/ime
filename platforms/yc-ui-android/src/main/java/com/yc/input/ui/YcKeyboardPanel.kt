package com.yc.input.ui

import android.content.Context
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout

class YcKeyboardPanel(context: Context) : LinearLayout(context), UiBinder {

    private val topToolbar = TopToolbar(context)
    private val candBar = SamsungCandBar(context)
    private val featureBar = FeatureBar(context)
    private val aiBar = AiBar(context)
    private val keyView = SamsungKeyView(context)
    private val handwritingPad = HandwritingPad(context)
    private val langPicker = LangPicker(context)
    private val candidatePicker = CandidatePicker(context)
    private val modePanel = ModePanel(context)
    private val voiceOverlay = VoiceOverlay(context)
    private val skinPicker = SkinPicker(context)
    private val emojiPanel = EmojiPanel(context)
    private val phraseDeckPanel = PhraseDeckPanel(context)
    private val aiAssistPanel = AiAssistPanel(context)
    private var tokens = ThemeTokens.from(context)

    private val toolbarCollapsedH = dp(56)
    private var keyH = dp(280)

    private var handwritingMode = false
    private val inputSlotLp: LayoutParams
    private val overlayHost: FrameLayout
    private val toolbarLp: LayoutParams

    private var baseRows: List<List<KeyDef>> = Layout26Pinyin.rows
    private var shiftRows: List<List<KeyDef>>? = null
    private var layoutId: String = "layout_pinyin26"
    private var shiftState: ShiftState = ShiftState.Off
    private var useZhPunct: Boolean = true
    private var viSpecialRow: Int = -1
    private var inSymbolLayer: Boolean = false
    private var modeLabel: String = "拼音"
    /** latn | vi | th — 影响键面字号 */
    private var scriptHint: String = "latn"

    init {
        orientation = VERTICAL
        setBackgroundColor(tokens.keyboardBg)

        overlayHost = FrameLayout(context)
        toolbarLp = LayoutParams(LayoutParams.MATCH_PARENT, toolbarCollapsedH)
        inputSlotLp = LayoutParams(LayoutParams.MATCH_PARENT, keyH)

        topToolbar.attachCandidateBar(candBar)
        addView(topToolbar, toolbarLp)
        addView(featureBar, LayoutParams(LayoutParams.MATCH_PARENT, dp(40)))
        addView(aiBar, LayoutParams(LayoutParams.MATCH_PARENT, dp(44)))

        overlayHost.addView(keyView, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT,
        ))
        overlayHost.addView(
            handwritingPad,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER,
            ),
        )
        handwritingPad.visibility = View.GONE
        overlayHost.addView(
            voiceOverlay,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER,
            ).apply {
                leftMargin = dp(12)
                rightMargin = dp(12)
            },
        )
        overlayHost.addView(
            modePanel,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP,
            ).apply {
                leftMargin = dp(8)
                rightMargin = dp(8)
                topMargin = dp(4)
            },
        )
        overlayHost.addView(
            skinPicker,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP,
            ).apply {
                leftMargin = dp(8)
                rightMargin = dp(8)
                topMargin = dp(4)
            },
        )
        overlayHost.addView(
            emojiPanel,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        overlayHost.addView(
            phraseDeckPanel,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        overlayHost.addView(
            aiAssistPanel,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        // 候选更多面板：盖住键区，贴顶栏/AI 栏下方
        overlayHost.addView(
            candidatePicker,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
                Gravity.TOP,
            ),
        )
        addView(overlayHost, inputSlotLp)
        addView(langPicker, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        applyTheme(tokens)
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        applyTheme(ThemeTokens.from(context))
    }

    fun setScriptHint(hint: String) {
        scriptHint = hint
        keyView.setScriptHint(hint)
    }

    fun setLayoutRows(
        rows: List<List<KeyDef>>,
        shiftAltRows: List<List<KeyDef>>? = null,
        layoutId: String = this.layoutId,
        viSpecialRowIndex: Int = -1,
    ) {
        this.layoutId = layoutId
        baseRows = LayoutCaseShift.applyPunctuation(ensureMicKey(rows), useZhPunct)
        shiftRows = shiftAltRows?.let { LayoutCaseShift.applyPunctuation(ensureMicKey(it), useZhPunct) }
        viSpecialRow = viSpecialRowIndex
        shiftState = ShiftState.Off
        inSymbolLayer = layoutId == "layout_symbol"
        refreshKeyDisplay()
        adjustKeyHeight()
    }

    private fun ensureMicKey(rows: List<List<KeyDef>>): List<List<KeyDef>> {
        if (rows.isEmpty()) return rows
        if (rows.any { row -> row.any { it.action == KeyAction.Mic } }) return rows
        val last = rows.last().toMutableList()
        val globeIdx = last.indexOfFirst { it.action == KeyAction.Globe }
        val insertAt = if (globeIdx >= 0) globeIdx + 1 else 1.coerceAtMost(last.size)
        last.add(insertAt, KeyDef("🎤", 1.1f, KeyStyle.Utility, action = KeyAction.Mic))
        // 将 !#1 文案统一为 123
        val normalized = last.map { key ->
            if (key.action == KeyAction.Symbol && (key.label == "!#1" || key.label.isEmpty())) {
                key.copy(label = "123")
            } else {
                key
            }
        }
        return rows.dropLast(1) + listOf(normalized)
    }

    fun currentLayoutId(): String = layoutId

    fun isInSymbolLayer(): Boolean = inSymbolLayer

    fun setUseZhPunct(zh: Boolean) {
        if (useZhPunct == zh) return
        useZhPunct = zh
        baseRows = LayoutCaseShift.applyPunctuation(baseRows, useZhPunct)
        shiftRows = shiftRows?.let { LayoutCaseShift.applyPunctuation(it, useZhPunct) }
        refreshKeyDisplay()
    }

    fun showSymbolLayer(backLabel: String) {
        inSymbolLayer = true
        shiftState = ShiftState.Off
        val rows = LayoutSymbol.rows(useZhPunct, backLabel)
        baseRows = rows
        shiftRows = null
        keyView.setShiftState(ShiftState.Off)
        keyView.setLayoutRows(rows, -1)
        layoutId = "layout_symbol"
        adjustKeyHeight()
    }

    fun getShiftState(): ShiftState = shiftState

    fun setShiftState(state: ShiftState) {
        shiftState = state
        refreshKeyDisplay()
    }

    /** 循环 once → locked → off；泰语有第二套布局时 once/locked 都显示 shift 层。 */
    fun cycleShift(): ShiftState {
        shiftState = when (shiftState) {
            ShiftState.Off -> ShiftState.Once
            ShiftState.Once -> ShiftState.Locked
            ShiftState.Locked -> ShiftState.Off
        }
        refreshKeyDisplay()
        return shiftState
    }

    fun consumeOnceShiftIfNeeded() {
        if (shiftState == ShiftState.Once) {
            shiftState = ShiftState.Off
            refreshKeyDisplay()
        }
    }

    @Deprecated("use cycleShift / setShiftState")
    fun setShifted(shifted: Boolean) {
        setShiftState(if (shifted) ShiftState.Once else ShiftState.Off)
    }

    @Deprecated("use cycleShift")
    fun toggleShift(): Boolean {
        cycleShift()
        return shiftState != ShiftState.Off
    }

    fun isShifted(): Boolean = shiftState != ShiftState.Off

    private fun refreshKeyDisplay() {
        val shifted = shiftState != ShiftState.Off
        val display = when {
            // 符号层：优先用 setLayoutRows 写入的 baseRows（语言包 layout_symbol）
            inSymbolLayer -> baseRows
            shiftRows != null && shifted -> {
                shiftRows!!.map { row ->
                    row.map { key ->
                        if (key.action == KeyAction.Shift) key else key
                    }
                }
            }
            shifted -> LayoutCaseShift.apply(baseRows, true)
            else -> baseRows
        }
        keyView.setShiftState(shiftState)
        keyView.setScriptHint(scriptHint)
        keyView.setLayoutRows(display, if (inSymbolLayer) -1 else viSpecialRow)
    }

    private fun adjustKeyHeight() {
        val rows = when {
            inSymbolLayer -> 5
            layoutId.contains("vietnamese") -> 6
            layoutId.contains("thai") -> 4
            else -> 5
        }
        // 泰/越键更多，略增高行距预算
        val rowBudget = when {
            layoutId.contains("vietnamese") -> 52
            layoutId.contains("thai") -> 54
            else -> 51
        }
        keyH = dp(12 + rows * rowBudget)
        val lp = overlayHost.layoutParams as LayoutParams
        if (lp.height != keyH && !handwritingMode) {
            lp.height = keyH
            overlayHost.layoutParams = lp
            requestLayout()
        }
    }

    fun setModeLabel(label: String) {
        modeLabel = label
        topToolbar.setModeLabel(label)
    }

    fun getModeLabel(): String = modeLabel

    fun setOnModeClick(listener: () -> Unit) {
        topToolbar.setOnModeClick(listener)
    }

    fun setOnCollapseClick(listener: () -> Unit) {
        topToolbar.setOnCollapseClick(listener)
    }

    fun setOnAiChipClick(listener: (String) -> Unit) {
        aiBar.setOnChipClick(listener)
    }

    fun showModePanel(options: List<ModeOption>, selectedId: String) {
        if (options.size <= 1) return
        hideLangPicker()
        modePanel.show(options, selectedId)
    }

    fun hideModePanel() = modePanel.hide()

    fun isModePanelShowing(): Boolean = modePanel.isShowing()

    fun setModePickListener(listener: (ModeOption) -> Unit) {
        modePanel.setOnPickListener(listener)
    }

    fun showVoiceOverlay(langName: String) = voiceOverlay.show(langName)

    fun setVoiceRecognizing() = voiceOverlay.setRecognizing()

    fun hideVoiceOverlay() = voiceOverlay.hide()

    fun isVoiceOverlayShowing(): Boolean = voiceOverlay.isShowing()

    fun setKeyDownListener(listener: (KeyDef) -> Unit) {
        keyView.setOnKeyDownListener(listener)
        handwritingPad.setOnChromeKeyDownListener(listener)
    }

    fun setKeyUpListener(listener: (KeyDef) -> Unit) {
        keyView.setOnKeyUpListener(listener)
        handwritingPad.setOnChromeKeyUpListener(listener)
    }

    fun isHandwritingMode(): Boolean = handwritingMode

    fun setHandwritingMode(enabled: Boolean) {
        if (handwritingMode == enabled) return
        handwritingMode = enabled
        if (enabled) {
            hideLangPicker()
            hideModePanel()
            keyView.visibility = View.GONE
            handwritingPad.visibility = View.VISIBLE
            // 对齐参考效果 handwriting ≈ 360dp
            val hwH = dp(360)
            val lp = overlayHost.layoutParams as LayoutParams
            if (lp.height != hwH) {
                lp.height = hwH
                overlayHost.layoutParams = lp
            }
        } else {
            handwritingPad.visibility = View.GONE
            handwritingPad.clearInk()
            keyView.visibility = View.VISIBLE
            adjustKeyHeight()
        }
        requestLayout()
    }

    fun clearHandwritingInk() = handwritingPad.clearInk()

    fun setHandwritingStrokeListener(listener: (HandwritingPad.StrokePayload) -> Unit) {
        handwritingPad.setOnStrokeListener(listener)
    }

    fun setHandwritingRecognizeListener(listener: () -> Unit) {
        handwritingPad.setOnRecognizeListener(listener)
    }

    fun setHandwritingUndoListener(listener: () -> Unit) {
        handwritingPad.setOnUndoListener(listener)
    }

    fun setHandwritingClearListener(listener: () -> Unit) {
        handwritingPad.setOnClearListener(listener)
    }

    fun setHandwritingDismissListener(listener: () -> Unit) {
        handwritingPad.setOnDismissListener(listener)
    }

    fun setHandwritingModeChangedListener(listener: (Boolean) -> Unit) {
        handwritingPad.setOnModeChangedListener(listener)
    }

    fun isHandwritingContinuous(): Boolean = handwritingPad.isContinuous()

    fun setHandwritingRecognizing(active: Boolean) {
        handwritingPad.setRecognizing(active)
    }

    fun setToolbarItemEnabled(item: String, enabled: Boolean) {
        featureBar.setItemEnabled(item, enabled)
    }

    fun showSkinPicker(selectedId: String) {
        hideEntertainmentPanels()
        hideModePanel()
        hideLangPicker()
        skinPicker.show(selectedId, tokens)
    }

    fun hideSkinPicker() = skinPicker.hide()

    fun isSkinPickerShowing(): Boolean = skinPicker.isShowing()

    fun setSkinPickListener(listener: (SkinOption) -> Unit) {
        skinPicker.setOnPick(listener)
    }

    fun setSkinMoreListener(listener: () -> Unit) {
        skinPicker.setOnMore(listener)
    }

    fun showEmojiPanel() {
        hideEntertainmentPanels()
        hideModePanel()
        hideLangPicker()
        keyView.visibility = View.GONE
        emojiPanel.show(tokens)
    }

    fun hideEmojiPanel() {
        emojiPanel.hide()
        if (!handwritingMode && !phraseDeckPanel.isShowing()) {
            keyView.visibility = View.VISIBLE
        }
    }

    fun isEmojiPanelShowing(): Boolean = emojiPanel.isShowing()

    fun setEmojiPickListener(listener: (String) -> Unit) {
        emojiPanel.setOnPick(listener)
    }

    fun showPhraseDeck(title: String, cards: List<PhraseCard>) {
        hideEntertainmentPanels()
        hideModePanel()
        hideLangPicker()
        keyView.visibility = View.GONE
        phraseDeckPanel.show(title, cards, tokens)
    }

    fun hidePhraseDeck() {
        phraseDeckPanel.hide()
        if (!handwritingMode && !emojiPanel.isShowing()) {
            keyView.visibility = View.VISIBLE
        }
    }

    fun isPhraseDeckShowing(): Boolean = phraseDeckPanel.isShowing()

    fun setPhrasePickListener(listener: (PhraseCard) -> Unit) {
        phraseDeckPanel.setOnPick(listener)
    }

    fun showAiAssist(preselectMode: String? = null, seedText: String = "") {
        hideEntertainmentPanels()
        hideModePanel()
        hideLangPicker()
        keyView.visibility = View.GONE
        aiAssistPanel.show(preselectMode, seedText, tokens)
    }

    fun hideAiAssist() {
        aiAssistPanel.hide()
        if (!handwritingMode && !emojiPanel.isShowing() && !phraseDeckPanel.isShowing()) {
            keyView.visibility = View.VISIBLE
        }
    }

    fun isAiAssistShowing(): Boolean = aiAssistPanel.isShowing()

    fun setAiAssistGenerateListener(listener: (AiAssistPanel.GenerateRequest) -> Unit) {
        aiAssistPanel.setOnGenerate(listener)
    }

    fun setAiAssistPickListener(listener: (String) -> Unit) {
        aiAssistPanel.setOnPick(listener)
    }

    fun setAiAssistCloseListener(listener: () -> Unit) {
        aiAssistPanel.setOnClose(listener)
    }

    fun setAiAssistStatus(msg: String) = aiAssistPanel.setStatus(msg)

    fun showAiAssistVariants(texts: List<String>, local: Boolean) {
        aiAssistPanel.showVariants(texts, local)
    }

    fun prefillAiAssist(text: String) {
        aiAssistPanel.ingestExternalText(text)
    }

    private fun hideEntertainmentPanels() {
        skinPicker.hide()
        emojiPanel.hide()
        phraseDeckPanel.hide()
        aiAssistPanel.hide()
        if (!handwritingMode) {
            keyView.visibility = View.VISIBLE
        }
    }

    override fun applyTheme(tokens: ThemeTokens) {
        this.tokens = tokens
        topToolbar.applyTheme(tokens)
        featureBar.applyTheme(tokens)
        candBar.applyTheme(tokens)
        aiBar.applyTheme(tokens)
        keyView.applyTheme(tokens)
        handwritingPad.applyTheme(tokens)
        modePanel.applyTheme(tokens)
        voiceOverlay.applyTheme(tokens)
        langPicker.applyTheme(tokens)
        candidatePicker.applyTheme(tokens)
        setBackgroundColor(tokens.keyboardBg)
    }

    override fun setToolbarListener(listener: (String) -> Unit) {
        featureBar.setOnItemClick(listener)
        aiBar.setOnChipClick(listener)
    }

    fun showLangPicker(options: List<LangOption>, selectedCode: String) {
        hideModePanel()
        hideCandidatePicker()
        langPicker.show(options, selectedCode)
    }

    fun hideLangPicker() = langPicker.hide()

    fun isLangPickerShowing(): Boolean = langPicker.isShowing()

    fun setLangPickListener(listener: (LangOption) -> Unit) {
        langPicker.setOnPickListener(listener)
    }

    fun showCandidatePicker(cands: List<CandidateItem>) {
        hideLangPicker()
        hideModePanel()
        candidatePicker.show(cands)
    }

    fun hideCandidatePicker() = candidatePicker.hide()

    fun isCandidatePickerShowing(): Boolean = candidatePicker.isShowing()

    override fun onSnapshot(snapshot: KeyboardSnapshot) {
        val uiSnap =
            if (handwritingMode) {
                snapshot.copy(composing = "", asciiMode = false, expanded = false)
            } else {
                snapshot
            }
        // 顶栏双行不变；expanded 仅用于隐藏「…」（弹窗已开时）
        candBar.render(uiSnap)
        if (handwritingMode) {
            hideCandidatePicker()
        } else if (snapshot.expanded) {
            if (candidatePicker.isShowing()) {
                candidatePicker.update(snapshot.candidates)
            } else {
                candidatePicker.show(snapshot.candidates)
            }
        } else if (candidatePicker.isShowing()) {
            hideCandidatePicker()
        }
        if (snapshot.candidates.isEmpty() && candidatePicker.isShowing()) {
            hideCandidatePicker()
        }
        if (!handwritingMode) {
            keyView.render(snapshot)
        }
    }

    override fun setCandidateExpanded(expanded: Boolean) {
        // 不再拉高顶栏；展开语义由 CandidatePicker 承担
        if (!expanded) {
            hideCandidatePicker()
        }
    }

    override fun setKeyListener(listener: (KeyDef) -> Unit) {
        keyView.setOnKeyListener(listener)
        handwritingPad.setOnChromeKeyListener(listener)
    }

    override fun setCandidateListener(listener: (CandidateItem) -> Unit) {
        candBar.setOnCandidateListener(listener)
        candidatePicker.setOnPickListener(listener)
    }

    override fun setPageListener(listener: (Int) -> Unit) {
        candBar.setOnPageListener(listener)
    }

    override fun setExpandListener(listener: () -> Unit) {
        candBar.setOnExpandListener(listener)
        candidatePicker.setOnCloseListener(listener)
    }

    override fun setNeedMoreListener(listener: () -> Unit) {
        candBar.setOnNeedMoreListener(listener)
        candidatePicker.setOnNeedMoreListener(listener)
    }

    private fun dp(v: Int): Int =
        (v * resources.displayMetrics.density).toInt()
}
