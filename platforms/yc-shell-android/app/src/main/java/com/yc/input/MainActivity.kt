package com.yc.input

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.provider.Settings
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import com.yc.input.llm.HabitPersonaPipeline
import com.yc.input.llm.LlmAssistRouter
import com.yc.input.llm.LlmByokPrefs
import com.yc.input.llm.LlmProviderCatalog
import com.yc.input.native.YcNative
import com.yc.input.phrase.PhraseKbStore
import com.yc.input.phrase.SceneProfileStore
import com.yc.input.ui.SkinRegistry
import java.io.File
import java.util.concurrent.Executors

/**
 * 主 App 壳：首页 / 发现 / 设置（底栏）；语言在设置子页。
 * 语言包安装静默执行，不在首页展示日志。
 */
class MainActivity : Activity() {

    private lateinit var contentHost: FrameLayout
    private lateinit var navHome: LinearLayout
    private lateinit var navDiscover: LinearLayout
    private lateinit var navSettings: LinearLayout
    private var currentPage = PAGE_HOME
    private var discoverChannel = "skins"
    private val installExecutor = Executors.newSingleThreadExecutor()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        silentInitCoreAndLangPacks()

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(AppUi.BG)
        }
        contentHost = FrameLayout(this)
        root.addView(
            contentHost,
            LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f),
        )
        root.addView(buildBottomNav())
        setContentView(root)

        val raw = intent.getStringExtra(EXTRA_TAB) ?: PAGE_HOME
        when (raw) {
            "settings" -> showPage(PAGE_SETTINGS)
            "llm", "ai" -> showPage(PAGE_LLM)
            "langs", "languages" -> showPage(PAGE_LANGS)
            "phrase", "content" -> showPage(PAGE_PHRASE)
            "discover" -> {
                discoverChannel = intent.getStringExtra(EXTRA_CHANNEL) ?: "skins"
                showPage(PAGE_DISCOVER)
            }
            "skins", "campaigns" -> {
                discoverChannel = raw
                showPage(PAGE_DISCOVER)
            }
            else -> showPage(PAGE_HOME)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val raw = intent.getStringExtra(EXTRA_TAB) ?: PAGE_HOME
        when (raw) {
            "settings" -> showPage(PAGE_SETTINGS)
            "llm", "ai" -> showPage(PAGE_LLM)
            "langs", "languages" -> showPage(PAGE_LANGS)
            "phrase", "content" -> showPage(PAGE_PHRASE)
            "discover" -> {
                discoverChannel = intent.getStringExtra(EXTRA_CHANNEL) ?: discoverChannel
                showPage(PAGE_DISCOVER)
            }
            "skins", "campaigns" -> {
                discoverChannel = raw
                showPage(PAGE_DISCOVER)
            }
            else -> showPage(PAGE_HOME)
        }
    }

    @Deprecated("Deprecated in Java")
    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        if (currentPage == PAGE_LANGS || currentPage == PAGE_LLM || currentPage == PAGE_PHRASE) {
            showPage(PAGE_SETTINGS)
        } else {
            super.onBackPressed()
        }
    }

    private fun silentInitCoreAndLangPacks() {
        val dataDir = filesDir.apply { mkdirs() }
        installExecutor.execute {
            try {
                YcNative.ycCoreInit(dataDir.absolutePath)
                installLangPack(dataDir, "en-v1.imepack", enable = false)
                installLangPack(dataDir, "vi-v1.imepack", enable = false)
                installLangPack(dataDir, "th-v1.imepack", enable = false)
                installLangPack(dataDir, "zh-pack-v1.imepack", enable = true)
            } catch (_: Exception) {
            }
        }
    }

    private fun installLangPack(dataDir: File, fileName: String, enable: Boolean) {
        val out = File(dataDir, fileName)
        try {
            assets.open("langpacks/$fileName").use { input ->
                out.outputStream().use { output -> input.copyTo(output) }
            }
            if (enable) {
                YcNative.ycCoreInstallLangpack(out.absolutePath)
            } else {
                YcNative.coldSubmit(
                    editorId = 0,
                    kind = 1,
                    payload = out.absolutePath.toByteArray(),
                )
            }
        } catch (_: Exception) {
        }
    }

    private fun buildBottomNav(): LinearLayout {
        val nav = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(AppUi.SURFACE)
            setPadding(0, AppUi.dp(this@MainActivity, 6), 0, AppUi.dp(this@MainActivity, 10))
            background = GradientDrawable().apply {
                setColor(AppUi.SURFACE)
                setStroke(AppUi.dp(this@MainActivity, 1), AppUi.LINE)
            }
        }
        navHome = makeNavButton("⌂", "首页") { showPage(PAGE_HOME) }
        navDiscover = makeNavButton("◇", "发现") { showPage(PAGE_DISCOVER) }
        navSettings = makeNavButton("☰", "设置") { showPage(PAGE_SETTINGS) }
        nav.addView(navHome, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        nav.addView(navDiscover, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        nav.addView(navSettings, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        return nav
    }

    private fun makeNavButton(icon: String, label: String, onClick: () -> Unit): LinearLayout {
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, AppUi.dp(this@MainActivity, 8), 0, AppUi.dp(this@MainActivity, 4))
            setOnClickListener { onClick() }
        }
        col.addView(
            TextView(this).apply {
                text = icon
                gravity = Gravity.CENTER
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
            },
        )
        col.addView(
            TextView(this).apply {
                text = label
                gravity = Gravity.CENTER
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
                typeface = Typeface.DEFAULT_BOLD
            },
        )
        return col
    }

    private fun showPage(page: String) {
        currentPage = page
        contentHost.removeAllViews()
        contentHost.addView(
            when (currentPage) {
                PAGE_DISCOVER -> buildDiscoverPage()
                PAGE_SETTINGS -> buildSettingsPage()
                PAGE_LANGS -> buildLangsPage()
                PAGE_LLM -> buildLlmPage()
                PAGE_PHRASE -> buildPhrasePage()
                else -> buildHomePage()
            },
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT,
            ),
        )
        updateNavHighlight()
    }

    private fun updateNavHighlight() {
        val active = when (currentPage) {
            PAGE_DISCOVER -> PAGE_DISCOVER
            PAGE_SETTINGS, PAGE_LANGS, PAGE_LLM, PAGE_PHRASE -> PAGE_SETTINGS
            else -> PAGE_HOME
        }
        styleNav(navHome, active == PAGE_HOME)
        styleNav(navDiscover, active == PAGE_DISCOVER)
        styleNav(navSettings, active == PAGE_SETTINGS)
    }

    private fun styleNav(btn: LinearLayout, on: Boolean) {
        val color = if (on) AppUi.ACCENT else AppUi.MUTED
        for (i in 0 until btn.childCount) {
            (btn.getChildAt(i) as? TextView)?.setTextColor(color)
        }
    }

    private fun pageShell(title: String, subtitle: String, body: LinearLayout.() -> Unit): View {
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(AppUi.BG)
        }
        col.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 12), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 4))
                addView(AppUi.title(this@MainActivity, title))
                addView(AppUi.subtitle(this@MainActivity, subtitle))
            },
        )
        val scroll = ScrollView(this)
        val inner = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 8), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 24))
            body()
        }
        scroll.addView(inner)
        col.addView(scroll, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        return col
    }

    private fun buildHomePage(): View =
        pageShell("YC 输入法", "打字要快 · 换肤要爽 · 行业要准") {
            addView(
                AppUi.chipLink(
                    this@MainActivity,
                    "当前输入：${LangPrefs.name(this@MainActivity)}",
                    "语言设置 ›",
                ) { showPage(PAGE_LANGS) },
            )
            addView(AppUi.primaryButton(this@MainActivity, "启用系统输入法") {
                startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
            })
            addView(
                TextView(this@MainActivity).apply {
                    text = "请在系统设置中开启「YC 输入法」并设为默认。" +
                        "首次启用时系统会提示「可能读取全部输入」——这是 Android 对所有第三方输入法的统一说明，点确定即可。" +
                        "密码与支付框我们不会学词、不会上报；语言包在后台静默安装。"
                    setTextColor(AppUi.MUTED)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(0, AppUi.dp(this@MainActivity, 8), 0, AppUi.dp(this@MainActivity, 16))
                    setLineSpacing(0f, 1.35f)
                },
            )
            addView(AppUi.sectionLabel(this@MainActivity, "快捷入口"))
            val card = AppUi.card(this@MainActivity)
            card.addView(
                AppUi.listRow(
                    this@MainActivity, "肤", "皮肤商城",
                    "已装 ${SkinRegistry.currentId(this@MainActivity)} · 再逛逛",
                    trailing = chevron(),
                    onClick = {
                        discoverChannel = "skins"
                        showPage(PAGE_DISCOVER)
                    },
                ),
            )
            card.addView(AppUi.divider(this@MainActivity))
            card.addView(
                AppUi.listRow(
                    this@MainActivity, "话", "行业话术",
                    "电商客服 · 游戏开黑",
                    trailing = chevron(),
                    iconBg = AppUi.OK_SOFT,
                    iconFg = AppUi.OK,
                    onClick = {
                        discoverChannel = "content"
                        showPage(PAGE_DISCOVER)
                    },
                ),
            )
            card.addView(AppUi.divider(this@MainActivity))
            card.addView(
                AppUi.listRow(
                    this@MainActivity, "活", "活动",
                    "新春主题 · 开黑周",
                    trailing = chevron(),
                    iconBg = AppUi.WARM_SOFT,
                    iconFg = AppUi.WARM,
                    showDivider = false,
                    onClick = {
                        discoverChannel = "campaigns"
                        showPage(PAGE_DISCOVER)
                    },
                ),
            )
            addView(card)
        }

    private fun buildDiscoverPage(): View =
        pageShell("发现", "皮肤 · 行业包 · 活动") {
            addView(channelTabs())
            when (discoverChannel) {
                "content" -> renderContent(this)
                "campaigns" -> renderCampaigns(this)
                else -> renderSkins(this)
            }
        }

    private fun channelTabs(): LinearLayout {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setPadding(0, 0, 0, AppUi.dp(this@MainActivity, 12))
        }
        listOf("skins" to "皮肤", "content" to "行业包", "campaigns" to "活动").forEach { (id, label) ->
            val on = discoverChannel == id
            row.addView(
                TextView(this).apply {
                    text = label
                    gravity = Gravity.CENTER
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    typeface = if (on) Typeface.DEFAULT_BOLD else Typeface.DEFAULT
                    setTextColor(if (on) AppUi.ACCENT else AppUi.MUTED)
                    setPadding(0, AppUi.dp(this@MainActivity, 8), 0, AppUi.dp(this@MainActivity, 8))
                    background = GradientDrawable().apply {
                        setColor(if (on) AppUi.ACCENT_SOFT else AppUi.SURFACE)
                        cornerRadius = AppUi.dp(this@MainActivity, 8).toFloat()
                        if (!on) setStroke(AppUi.dp(this@MainActivity, 1), AppUi.LINE)
                    }
                    setOnClickListener {
                        discoverChannel = id
                        showPage(PAGE_DISCOVER)
                    }
                },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    marginEnd = AppUi.dp(this@MainActivity, 6)
                },
            )
        }
        return row
    }

    private fun renderSkins(host: LinearLayout) {
        val catalog = LocalCatalog.load(this)
        val card = AppUi.card(this)
        SkinRegistry.builtins().forEachIndexed { index, skin ->
            if (index > 0) card.addView(AppUi.divider(this))
            card.addView(
                AppUi.listRow(
                    this,
                    skin.name.take(1),
                    skin.name,
                    skin.id,
                    trailing = AppUi.badge(this, if (SkinRegistry.currentId(this) == skin.id) "使用中" else "应用", SkinRegistry.currentId(this) == skin.id),
                    onClick = {
                        SkinRegistry.save(this, skin.id)
                        Toast.makeText(this, "已切换：${skin.name}（下次打开键盘生效）", Toast.LENGTH_SHORT).show()
                        showPage(PAGE_DISCOVER)
                    },
                ),
            )
        }
        host.addView(card)
        host.addView(AppUi.sectionLabel(this, "商城目录"))
        val mall = AppUi.card(this)
        catalog.entries.filter { it.kind == "skin" }.forEachIndexed { i, e ->
            if (i > 0) mall.addView(AppUi.divider(this))
            mall.addView(
                AppUi.listRow(
                    this,
                    "装",
                    e.displayName,
                    e.tags.joinToString(" · "),
                    trailing = chevron(),
                    iconBg = 0xFFE8EAED.toInt(),
                    iconFg = 0xFF3C4043.toInt(),
                    onClick = {
                        val opt = SkinRegistry.builtins().firstOrNull { it.id == e.packId }
                        if (opt != null) {
                            SkinRegistry.save(this, opt.id)
                            Toast.makeText(this, "已切换：${opt.name}", Toast.LENGTH_SHORT).show()
                            showPage(PAGE_DISCOVER)
                        } else {
                            Toast.makeText(this, "皮肤包待下载：${e.packId}", Toast.LENGTH_SHORT).show()
                        }
                    },
                ),
            )
        }
        host.addView(mall)
    }

    private fun renderContent(host: LinearLayout) {
        host.addView(
            TextView(this).apply {
                text = "也可在「设置 → 行业话术与知识库」管理自定义场景与知识库。"
                setTextColor(AppUi.MUTED)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                setPadding(0, 0, 0, dp(8))
            },
        )
        val catalog = LocalCatalog.load(this)
        val active = SceneProfileStore.activePackId(this)
        val card = AppUi.card(this)
        catalog.entries.filter { it.kind == "content" }.forEachIndexed { i, e ->
            if (i > 0) card.addView(AppUi.divider(this))
            val on = e.packId == active && SceneProfileStore.mode(this) == "builtin"
            card.addView(
                AppUi.listRow(
                    this,
                    e.displayName.take(1),
                    e.displayName,
                    e.tags.joinToString(" · "),
                    trailing = AppUi.badge(this, if (on) "启用中" else "启用", on),
                    iconBg = if (on) AppUi.OK_SOFT else 0xFFE8EAED.toInt(),
                    iconFg = if (on) AppUi.OK else 0xFF3C4043.toInt(),
                    onClick = {
                        val ok = IndustryPackInstaller.enable(this, e.packId)
                        SceneProfileStore.setActivePackId(this, e.packId)
                        Toast.makeText(
                            this,
                            if (ok) "已启用 ${e.displayName}（词库增量 + 话术）" else "已启用 ${e.displayName}（话术）",
                            Toast.LENGTH_SHORT,
                        ).show()
                        showPage(PAGE_DISCOVER)
                    },
                ),
            )
        }
        host.addView(card)
        host.addView(
            TextView(this).apply {
                text = "启用后键盘「话术」加载对应 deck；AI 优化按当前行业场景约束。"
                setTextColor(AppUi.MUTED)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            },
        )
        host.addView(
            TextView(this).apply {
                text = "管理自定义行业与知识库 ›"
                setTextColor(AppUi.ACCENT)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                setPadding(0, dp(12), 0, 0)
                setOnClickListener { showPage(PAGE_PHRASE) }
            },
        )
    }

    private fun buildPhrasePage(): View =
        pageShell("行业话术", "选择场景 · 自定义 · 知识库") {
            val cur = SceneProfileStore.current(this@MainActivity)
            addView(AppUi.sectionLabel(this@MainActivity, "当前场景"))
            addView(
                TextView(this@MainActivity).apply {
                    text = "${cur.displayName}（${if (cur.type == "custom") "自定义" else "内置"}）\n${cur.hint}"
                    setTextColor(AppUi.INK)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    setPadding(0, 0, 0, dp(8))
                },
            )

            addView(AppUi.sectionLabel(this@MainActivity, "内置行业"))
            val builtins = AppUi.card(this@MainActivity)
            SceneProfileStore.BUILTINS.forEachIndexed { i, b ->
                if (i > 0) builtins.addView(AppUi.divider(this@MainActivity))
                val on = cur.type == "builtin" && cur.packId == b.packId
                builtins.addView(
                    AppUi.listRow(
                        this@MainActivity,
                        b.displayName.take(1),
                        b.displayName,
                        b.hint.take(36),
                        trailing = AppUi.badge(this@MainActivity, if (on) "当前" else "选用", on),
                        iconBg = if (on) AppUi.OK_SOFT else 0xFFE8EAED.toInt(),
                        iconFg = if (on) AppUi.OK else 0xFF3C4043.toInt(),
                        onClick = {
                            IndustryPackInstaller.enable(this@MainActivity, b.packId)
                            SceneProfileStore.setActivePackId(this@MainActivity, b.packId)
                            Toast.makeText(this@MainActivity, "已切换到 ${b.displayName}", Toast.LENGTH_SHORT).show()
                            showPage(PAGE_PHRASE)
                        },
                    ),
                )
            }
            addView(builtins)

            addView(AppUi.sectionLabel(this@MainActivity, "自定义行业"))
            val nameEt = EditText(this@MainActivity).apply {
                hint = "行业名称（如教培咨询）"
                setText(SceneProfileStore.customRaw(this@MainActivity)?.first.orEmpty())
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            }
            val hintEt = EditText(this@MainActivity).apply {
                hint = "场景说明（服务谁、做什么、语气）"
                setText(SceneProfileStore.customRaw(this@MainActivity)?.second.orEmpty())
                minLines = 2
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            }
            val tabooEt = EditText(this@MainActivity).apply {
                hint = "禁语/红线（可选）"
                setText(SceneProfileStore.customRaw(this@MainActivity)?.third.orEmpty())
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            }
            addView(nameEt)
            addView(hintEt)
            addView(tabooEt)
            addView(
                TextView(this@MainActivity).apply {
                    text = "保存并启用自定义"
                    gravity = Gravity.CENTER
                    setTextColor(0xFFFFFFFF.toInt())
                    setPadding(dp(16), dp(12), dp(16), dp(12))
                    background = GradientDrawable().apply {
                        setColor(AppUi.ACCENT)
                        cornerRadius = dp(8).toFloat()
                    }
                    setOnClickListener {
                        val n = nameEt.text?.toString().orEmpty().trim()
                        val h = hintEt.text?.toString().orEmpty().trim()
                        if (n.isEmpty() || h.isEmpty()) {
                            Toast.makeText(this@MainActivity, "请填写名称与场景说明", Toast.LENGTH_SHORT).show()
                            return@setOnClickListener
                        }
                        SceneProfileStore.saveCustom(
                            this@MainActivity,
                            n,
                            h,
                            tabooEt.text?.toString().orEmpty(),
                        )
                        Toast.makeText(this@MainActivity, "已启用自定义场景", Toast.LENGTH_SHORT).show()
                        showPage(PAGE_PHRASE)
                    }
                },
            )

            addView(AppUi.sectionLabel(this@MainActivity, "我的知识库（可选）"))
            val bucket = SceneProfileStore.current(this@MainActivity).industryId
            val kbList = PhraseKbStore.list(this@MainActivity, bucket)
            addView(
                TextView(this@MainActivity).apply {
                    text = "当前桶：$bucket · ${kbList.size} 条 · ${PhraseKbStore.totalChars(this@MainActivity, bucket)} 字"
                    setTextColor(AppUi.MUTED)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                },
            )
            val kbCard = AppUi.card(this@MainActivity)
            if (kbList.isEmpty()) {
                kbCard.addView(
                    TextView(this@MainActivity).apply {
                        text = "尚未导入。可粘贴店铺规则/商品说明，增强 AI 优化。"
                        setTextColor(AppUi.MUTED)
                        setPadding(dp(12), dp(12), dp(12), dp(12))
                    },
                )
            } else {
                kbList.forEachIndexed { i, e ->
                    if (i > 0) kbCard.addView(AppUi.divider(this@MainActivity))
                    kbCard.addView(
                        AppUi.listRow(
                            this@MainActivity,
                            e.role.take(1),
                            e.name,
                            e.text.take(40),
                            trailing = TextView(this@MainActivity).apply {
                                text = "删除"
                                setTextColor(0xFFD93025.toInt())
                                setOnClickListener {
                                    PhraseKbStore.delete(this@MainActivity, bucket, e.id)
                                    showPage(PAGE_PHRASE)
                                }
                            },
                        ),
                    )
                }
            }
            addView(kbCard)

            val kbName = EditText(this@MainActivity).apply {
                hint = "条目名称"
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            }
            val kbBody = EditText(this@MainActivity).apply {
                hint = "粘贴文本内容"
                minLines = 3
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            }
            addView(kbName)
            addView(kbBody)
            addView(
                TextView(this@MainActivity).apply {
                    text = "导入为参考资料"
                    gravity = Gravity.CENTER
                    setTextColor(AppUi.ACCENT)
                    setPadding(dp(12), dp(10), dp(12), dp(10))
                    setOnClickListener {
                        val ok = PhraseKbStore.importText(
                            this@MainActivity,
                            bucket,
                            kbName.text?.toString().orEmpty().ifBlank { "未命名" },
                            kbBody.text?.toString().orEmpty(),
                            "ref",
                        )
                        Toast.makeText(
                            this@MainActivity,
                            if (ok) "已导入参考" else "导入失败（空内容或超限）",
                            Toast.LENGTH_SHORT,
                        ).show()
                        if (ok) showPage(PAGE_PHRASE)
                    }
                },
            )
            addView(
                TextView(this@MainActivity).apply {
                    text = "导入为硬约束（规则）"
                    gravity = Gravity.CENTER
                    setTextColor(0xFFD93025.toInt())
                    setPadding(dp(12), dp(10), dp(12), dp(10))
                    setOnClickListener {
                        val ok = PhraseKbStore.importText(
                            this@MainActivity,
                            bucket,
                            kbName.text?.toString().orEmpty().ifBlank { "规则" },
                            kbBody.text?.toString().orEmpty(),
                            "rule",
                        )
                        Toast.makeText(
                            this@MainActivity,
                            if (ok) "已导入约束" else "导入失败（空内容或超限）",
                            Toast.LENGTH_SHORT,
                        ).show()
                        if (ok) showPage(PAGE_PHRASE)
                    }
                },
            )
            addView(
                TextView(this@MainActivity).apply {
                    text = "‹ 返回设置"
                    setTextColor(AppUi.MUTED)
                    setPadding(0, dp(16), 0, 0)
                    setOnClickListener { showPage(PAGE_SETTINGS) }
                },
            )
        }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()

    private fun renderCampaigns(host: LinearLayout) {
        val catalog = LocalCatalog.load(this)
        val card = AppUi.card(this)
        catalog.campaigns.forEachIndexed { i, c ->
            if (i > 0) card.addView(AppUi.divider(this))
            card.addView(
                AppUi.listRow(
                    this,
                    "活",
                    c.title,
                    "皮肤 ${c.skinId} · 至 ${c.expires}",
                    trailing = TextView(this).apply {
                        text = "参与 ›"
                        setTextColor(AppUi.MUTED)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    },
                    iconBg = AppUi.WARM_SOFT,
                    iconFg = AppUi.WARM,
                    onClick = {
                        SkinRegistry.save(this, c.skinId)
                        if (c.phraseDeck.isNotBlank()) {
                            val packId = when (c.phraseDeck) {
                                "ecommerce-cs" -> "industry-ecommerce-v1"
                                "gaming-slang" -> "industry-gaming-v1"
                                else -> c.phraseDeck
                            }
                            IndustryPackInstaller.enable(this, packId)
                            SceneProfileStore.setActivePackId(this, packId)
                        }
                        Toast.makeText(this, "已应用活动皮肤与话术", Toast.LENGTH_SHORT).show()
                    },
                ),
            )
        }
        host.addView(card)
    }

    private fun buildSettingsPage(): View =
        pageShell("设置", "语言、隐私与关于") {
            addView(AppUi.sectionLabel(this@MainActivity, "输入"))
            val input = AppUi.card(this@MainActivity)
            input.addView(
                AppUi.listRow(
                    this@MainActivity, "语", "语言与布局",
                    "已装 ${LangPrefs.OPTIONS.size} 种 · 当前 ${LangPrefs.name(this@MainActivity)}",
                    trailing = chevron(),
                    onClick = { showPage(PAGE_LANGS) },
                ),
            )
            input.addView(AppUi.divider(this@MainActivity))
            input.addView(
                AppUi.listRow(
                    this@MainActivity, "AI", "AI 大模型（自备 Key）",
                    llmSettingsSubtitle(),
                    trailing = chevron(),
                    iconBg = AppUi.ACCENT_SOFT,
                    iconFg = AppUi.ACCENT,
                    onClick = { showPage(PAGE_LLM) },
                ),
            )
            input.addView(AppUi.divider(this@MainActivity))
            val scene = SceneProfileStore.current(this@MainActivity)
            input.addView(
                AppUi.listRow(
                    this@MainActivity, "话", "行业话术与知识库",
                    "${scene.displayName} · ${if (scene.type == "custom") "自定义" else "内置"}",
                    trailing = chevron(),
                    iconBg = AppUi.WARM_SOFT,
                    iconFg = AppUi.WARM,
                    onClick = { showPage(PAGE_PHRASE) },
                ),
            )
            input.addView(AppUi.divider(this@MainActivity))
            val personalOn = SettingsPrefs.personalizationEnabled(this@MainActivity)
            val lastSum = HabitPersonaPipeline.lastSummary(this@MainActivity).ifBlank { "尚未优化" }
            input.addView(
                AppUi.listRow(
                    this@MainActivity, "习", "个性化学习",
                    if (personalOn) "本地学词 · BYOK 优化候选 · $lastSum" else "已关闭",
                    trailing = AppUi.badge(this@MainActivity, if (personalOn) "开" else "关", personalOn),
                    iconBg = AppUi.OK_SOFT,
                    iconFg = AppUi.OK,
                    onClick = {
                        val next = !SettingsPrefs.personalizationEnabled(this@MainActivity)
                        SettingsPrefs.setPersonalization(this@MainActivity, next)
                        Toast.makeText(this@MainActivity, if (next) "已开启个性化学习" else "已关闭个性化学习", Toast.LENGTH_SHORT).show()
                        showPage(PAGE_SETTINGS)
                    },
                ),
            )
            input.addView(AppUi.divider(this@MainActivity))
            input.addView(
                AppUi.listRow(
                    this@MainActivity, "优", "立即优化选词画像",
                    "使用「AI 大模型」页配置的 Key；后台运行，不影响打字",
                    trailing = chevron(),
                    iconBg = AppUi.ACCENT_SOFT,
                    iconFg = AppUi.ACCENT,
                    showDivider = false,
                    onClick = {
                        when {
                            !SettingsPrefs.personalizationEnabled(this@MainActivity) ->
                                Toast.makeText(this@MainActivity, "请先开启个性化学习", Toast.LENGTH_SHORT).show()
                            HabitPersonaPipeline.isRunning() ->
                                Toast.makeText(this@MainActivity, "优化进行中…", Toast.LENGTH_SHORT).show()
                            else -> {
                                Toast.makeText(this@MainActivity, "开始优化…", Toast.LENGTH_SHORT).show()
                                HabitPersonaPipeline.optimizeNowAsync(this@MainActivity) { pack ->
                                    val msg = when {
                                        !pack.error.isNullOrBlank() &&
                                            pack.preferPairs.length() == 0 &&
                                            pack.boosts.length() == 0 ->
                                            pack.error ?: "失败"
                                        else -> "完成：${pack.summary()}"
                                    }
                                    Toast.makeText(this@MainActivity, msg, Toast.LENGTH_LONG).show()
                                    showPage(PAGE_SETTINGS)
                                }
                            }
                        }
                    },
                ),
            )
            addView(input)

            addView(AppUi.sectionLabel(this@MainActivity, "隐私"))
            val privacy = AppUi.card(this@MainActivity)
            privacy.addView(
                AppUi.listRow(
                    this@MainActivity, "隐", "隐私与数据",
                    "密码框不学词、不上报习惯",
                    trailing = chevron(),
                    iconBg = 0xFFE8EAED.toInt(),
                    iconFg = 0xFF3C4043.toInt(),
                    showDivider = false,
                    onClick = {
                        Toast.makeText(this@MainActivity, "ForbiddenCloud / 密码框：不展示娱乐入口、不学词", Toast.LENGTH_LONG).show()
                    },
                ),
            )
            addView(privacy)

            addView(AppUi.sectionLabel(this@MainActivity, "关于"))
            val about = AppUi.card(this@MainActivity)
            about.addView(
                AppUi.listRow(
                    this@MainActivity, "关", "YC Input",
                    "账号/付费 SKU 预留 · 首期免费",
                    showDivider = false,
                ),
            )
            addView(about)
        }

    private fun llmSettingsSubtitle(): String {
        val profiles = LlmProviderCatalog.load(this)
        val id = LlmByokPrefs.selectedProfileId(this)
        val name = profiles.firstOrNull { it.id == id }?.displayName ?: id
        val key = if (LlmByokPrefs.hasKey(this)) "已保存 Key" else "未配置 Key"
        return "$name · $key"
    }

    private fun buildLlmPage(): View {
        val profiles = LlmProviderCatalog.load(this)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(AppUi.BG)
        }
        col.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 8), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 4))
                addView(
                    TextView(this@MainActivity).apply {
                        text = "‹ 设置"
                        setTextColor(AppUi.ACCENT)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                        setPadding(0, 0, 0, AppUi.dp(this@MainActivity, 8))
                        setOnClickListener { showPage(PAGE_SETTINGS) }
                    },
                )
                addView(AppUi.title(this@MainActivity, "AI 大模型"))
                addView(AppUi.subtitle(this@MainActivity, "自备 Key · 设备直连 · 我们不转发"))
            },
        )
        val scroll = ScrollView(this)
        val inner = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 8), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 24))

            val enabled = LlmByokPrefs.enabled(this@MainActivity)
            addView(AppUi.sectionLabel(this@MainActivity, "总开关"))
            val sw = AppUi.card(this@MainActivity)
            sw.addView(
                AppUi.listRow(
                    this@MainActivity, "启", "启用大模型辅助",
                    "润色 / 高情商 / 翻译 / 智能回复",
                    trailing = AppUi.badge(this@MainActivity, if (enabled) "开" else "关", enabled),
                    showDivider = false,
                    onClick = {
                        LlmByokPrefs.setEnabled(this@MainActivity, !LlmByokPrefs.enabled(this@MainActivity))
                        showPage(PAGE_LLM)
                    },
                ),
            )
            addView(sw)

            addView(AppUi.sectionLabel(this@MainActivity, "选择套餐"))
            val selectedId = LlmByokPrefs.selectedProfileId(this@MainActivity)
            var cur = profiles.firstOrNull { it.id == selectedId } ?: profiles.getOrNull(0)
            val packageCard = AppUi.card(this@MainActivity)
            packageCard.addView(
                AppUi.listRow(
                    this@MainActivity,
                    "模",
                    cur?.displayName ?: "请选择",
                    "点击选择套餐（弹窗）",
                    trailing = chevron(),
                    showDivider = false,
                    onClick = {
                        val names = profiles.map { it.displayName }.toTypedArray()
                        val checked = profiles.indexOfFirst { it.id == LlmByokPrefs.selectedProfileId(this@MainActivity) }
                            .coerceAtLeast(0)
                        AlertDialog.Builder(this@MainActivity)
                            .setTitle("选择大模型套餐")
                            .setSingleChoiceItems(names, checked) { dialog, which ->
                                val p = profiles.getOrNull(which) ?: return@setSingleChoiceItems
                                LlmByokPrefs.setSelectedProfileId(this@MainActivity, p.id)
                                dialog.dismiss()
                                showPage(PAGE_LLM)
                            }
                            .setNegativeButton("取消", null)
                            .show()
                    },
                ),
            )
            addView(packageCard)

            if (cur != null && !cur.allowOverride) {
                addView(
                    TextView(this@MainActivity).apply {
                        text = "当前：${cur.provider} · ${cur.model}\n${cur.docsHint}"
                        setTextColor(AppUi.MUTED)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                        setPadding(0, AppUi.dp(this@MainActivity, 8), 0, AppUi.dp(this@MainActivity, 8))
                    },
                )
            }

            if (cur?.allowOverride == true) {
                addView(AppUi.sectionLabel(this@MainActivity, "自定义端点"))
                val baseEt = EditText(this@MainActivity).apply {
                    setText(LlmByokPrefs.customBaseUrl(this@MainActivity))
                    hint = "Base URL"
                    setSingleLine()
                }
                val modelEt = EditText(this@MainActivity).apply {
                    setText(LlmByokPrefs.customModel(this@MainActivity))
                    hint = "Model（Ollama 模型名或 ep-xxx）"
                    setSingleLine()
                }
                addView(baseEt)
                addView(modelEt)
                addView(
                    AppUi.secondaryButton(this@MainActivity, "保存自定义地址 / 模型") {
                        LlmByokPrefs.setCustomBaseUrl(this@MainActivity, baseEt.text.toString())
                        LlmByokPrefs.setCustomModel(this@MainActivity, modelEt.text.toString())
                        Toast.makeText(this@MainActivity, "已保存自定义端点", Toast.LENGTH_SHORT).show()
                    },
                )
                addView(
                    TextView(this@MainActivity).apply {
                        text = cur.docsHint
                        setTextColor(AppUi.MUTED)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                        setPadding(0, AppUi.dp(this@MainActivity, 8), 0, 0)
                    },
                )
            }

            addView(AppUi.sectionLabel(this@MainActivity, "API Key（仅存本机）"))
            val keyEt = EditText(this@MainActivity).apply {
                hint = if (LlmByokPrefs.hasKey(this@MainActivity)) "已保存（输入新 Key 可覆盖）" else "粘贴你的 API Key"
                setSingleLine()
                inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD
            }
            addView(keyEt)
            addView(
                AppUi.primaryButton(this@MainActivity, "保存 Key") {
                    val k = keyEt.text?.toString().orEmpty()
                    if (k.isBlank()) {
                        Toast.makeText(this@MainActivity, "请输入 Key", Toast.LENGTH_SHORT).show()
                    } else {
                        LlmByokPrefs.setApiKey(this@MainActivity, k)
                        keyEt.setText("")
                        Toast.makeText(this@MainActivity, "Key 已保存到本机", Toast.LENGTH_SHORT).show()
                        showPage(PAGE_LLM)
                    }
                },
            )
            addView(
                LinearLayout(this@MainActivity).apply {
                    orientation = LinearLayout.HORIZONTAL
                    setPadding(0, AppUi.dp(this@MainActivity, 12), 0, 0)
                    addView(
                        AppUi.secondaryButton(this@MainActivity, "测试连接") {
                            Toast.makeText(this@MainActivity, "正在测试…", Toast.LENGTH_SHORT).show()
                            Thread {
                                val r = LlmAssistRouter.testConnection(this@MainActivity)
                                runOnUiThread {
                                    val msg = when {
                                        r.error != null && r.variants.isEmpty() -> r.error
                                        r.local -> "未就绪：${r.error ?: "请检查套餐与 Key"}"
                                        else -> "连接成功：${r.variants.firstOrNull()?.text?.take(40) ?: "ok"}"
                                    }
                                    Toast.makeText(this@MainActivity, msg, Toast.LENGTH_LONG).show()
                                }
                            }.start()
                        },
                        LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
                    )
                    addView(
                        AppUi.secondaryButton(this@MainActivity, "清除 Key") {
                            LlmByokPrefs.clearApiKey(this@MainActivity)
                            Toast.makeText(this@MainActivity, "已清除 Key", Toast.LENGTH_SHORT).show()
                            showPage(PAGE_LLM)
                        },
                        LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                            marginStart = AppUi.dp(this@MainActivity, 8)
                        },
                    )
                },
            )
            addView(
                TextView(this@MainActivity).apply {
                    text = "请求直连你所选厂商；产品不转发、不托管 Key。密码框禁用 AI。"
                    setTextColor(AppUi.MUTED)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(0, AppUi.dp(this@MainActivity, 16), 0, 0)
                },
            )
        }
        scroll.addView(inner)
        col.addView(scroll, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        return col
    }

    private fun buildLangsPage(): View {
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(AppUi.BG)
        }
        col.addView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 8), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 4))
                addView(
                    TextView(this@MainActivity).apply {
                        text = "‹ 设置"
                        setTextColor(AppUi.ACCENT)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                        setPadding(0, 0, 0, AppUi.dp(this@MainActivity, 8))
                        setOnClickListener { showPage(PAGE_SETTINGS) }
                    },
                )
                addView(AppUi.title(this@MainActivity, "语言与布局"))
                addView(AppUi.subtitle(this@MainActivity, "已装语言在此管理，不占用首页"))
            },
        )
        val scroll = ScrollView(this)
        val inner = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 8), AppUi.dp(this@MainActivity, 16), AppUi.dp(this@MainActivity, 24))
            addView(AppUi.sectionLabel(this@MainActivity, "已安装"))
            val card = AppUi.card(this@MainActivity)
            val current = LangPrefs.code(this@MainActivity)
            LangPrefs.OPTIONS.forEachIndexed { i, opt ->
                if (i > 0) card.addView(AppUi.divider(this@MainActivity))
                val on = opt.code == current
                card.addView(
                    AppUi.listRow(
                        this@MainActivity,
                        when (opt.code) {
                            "zh" -> "中"
                            "en" -> "En"
                            "vi" -> "Vi"
                            else -> "Th"
                        },
                        opt.name,
                        "${opt.packId} · ${opt.layoutHint}",
                        trailing = AppUi.badge(this@MainActivity, if (on) "启用中" else "已装", on),
                        iconBg = if (on) AppUi.ACCENT_SOFT else 0xFFE8EAED.toInt(),
                        iconFg = if (on) AppUi.ACCENT else 0xFF3C4043.toInt(),
                        onClick = {
                            LangPrefs.save(this@MainActivity, opt.code)
                            Toast.makeText(this@MainActivity, "已设为默认：${opt.name}（下次打开键盘生效）", Toast.LENGTH_SHORT).show()
                            showPage(PAGE_LANGS)
                        },
                    ),
                )
            }
            addView(card)
            addView(
                TextView(this@MainActivity).apply {
                    text = "语言包在 App 启动时后台安装；此处只展示状态与默认语言，不显示 install 日志。"
                    setTextColor(AppUi.MUTED)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                    setPadding(0, 0, 0, AppUi.dp(this@MainActivity, 16))
                },
            )
            addView(
                AppUi.secondaryButton(this@MainActivity, "获取更多语言") {
                    Toast.makeText(this@MainActivity, "后续对接 Catalog 语言包下载", Toast.LENGTH_SHORT).show()
                },
            )
        }
        scroll.addView(inner)
        col.addView(scroll, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        return col
    }

    private fun chevron(): TextView =
        TextView(this).apply {
            text = "›"
            setTextColor(AppUi.MUTED)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        }

    companion object {
        const val EXTRA_TAB = "tab"
        const val EXTRA_CHANNEL = "channel"
        const val PAGE_HOME = "home"
        const val PAGE_DISCOVER = "discover"
        const val PAGE_SETTINGS = "settings"
        const val PAGE_LANGS = "langs"
        const val PAGE_LLM = "llm"
        const val PAGE_PHRASE = "phrase"
    }
}