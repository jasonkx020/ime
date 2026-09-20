package com.yc.input.ui

import android.content.Context
import android.content.SharedPreferences
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

data class SkinOption(
    val id: String,
    val name: String,
    val tokens: ThemeTokens,
)

object SkinRegistry {
    const val PREF = "yc_skin"
    const val KEY_ID = "skin_id"

    fun builtins(): List<SkinOption> = listOf(
        SkinOption("samsung-light", "浅色 One UI", ThemeTokens.samsungLight()),
        SkinOption("samsung-dark", "深色 One UI", ThemeTokens.samsungDark()),
        SkinOption("minimal-white", "极简白", ThemeTokens.light().copy(keyRadiusDp = 4f)),
        SkinOption("minimal-ink", "极简墨", ThemeTokens.dark().copy(keyRadiusDp = 4f)),
        SkinOption("blue-accent", "蓝调", ThemeTokens.samsungLight().copy(keyAccent = 0xFF1565C0.toInt())),
        SkinOption("teal-fresh", "青绿", ThemeTokens.samsungLight().copy(
            keyAccent = 0xFF00897B.toInt(),
            composingText = 0xFF00897B.toInt(),
        )),
        SkinOption("rose", "玫瑰粉", ThemeTokens.samsungLight().copy(
            keyAccent = 0xFFC2185B.toInt(),
            composingText = 0xFFC2185B.toInt(),
            keyboardBg = 0xFFFCE4EC.toInt(),
            toolbarBg = 0xFFFCE4EC.toInt(),
        )),
        SkinOption("amber", "琥珀", ThemeTokens.samsungLight().copy(
            keyAccent = 0xFFF57C00.toInt(),
            composingText = 0xFFE65100.toInt(),
            keyboardBg = 0xFFFFF8E1.toInt(),
            toolbarBg = 0xFFFFF8E1.toInt(),
        )),
        SkinOption("midnight", "午夜蓝", ThemeTokens.samsungDark().copy(
            keyAccent = 0xFF5C6BC0.toInt(),
            keyboardBg = 0xFF0D1B2A.toInt(),
            toolbarBg = 0xFF0D1B2A.toInt(),
            panelBg = 0xFF0D1B2A.toInt(),
        )),
        SkinOption("graphite", "石墨灰", ThemeTokens.samsungDark().copy(
            keyAccent = 0xFF90A4AE.toInt(),
            keyNormal = 0xFF37474F.toInt(),
        )),
        SkinOption("system", "跟随系统", ThemeTokens.light()),
    )

    fun prefs(ctx: Context): SharedPreferences =
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE)

    fun currentId(ctx: Context): String =
        prefs(ctx).getString(KEY_ID, "system") ?: "system"

    fun save(ctx: Context, id: String) {
        prefs(ctx).edit().putString(KEY_ID, id).apply()
    }

    fun resolve(ctx: Context): ThemeTokens {
        val id = currentId(ctx)
        if (id == "system") return ThemeTokens.from(ctx)
        return builtins().firstOrNull { it.id == id }?.tokens ?: ThemeTokens.from(ctx)
    }
}

/** 键盘内皮肤快切面板（最近/已装）。 */
class SkinPicker(context: Context) : ScrollView(context) {
    private val column = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(14), dp(12), dp(14), dp(12))
    }
    private var tokens = ThemeTokens.light()
    private var onPick: ((SkinOption) -> Unit)? = null
    private var onMore: (() -> Unit)? = null

    init {
        visibility = GONE
        addView(column)
        elevation = dp(8).toFloat()
    }

    fun setOnPick(listener: (SkinOption) -> Unit) {
        onPick = listener
    }

    fun setOnMore(listener: () -> Unit) {
        onMore = listener
    }

    fun show(selectedId: String, t: ThemeTokens) {
        tokens = t
        background = GradientDrawable().apply {
            setColor(t.panelBg)
            cornerRadius = dp(12).toFloat()
            setStroke(dp(1), t.keyBorder)
        }
        column.removeAllViews()
        column.addView(header("皮肤"))
        SkinRegistry.builtins().forEach { opt ->
            column.addView(row(opt, selected = opt.id == selectedId))
        }
        column.addView(
            TextView(context).apply {
                text = "更多皮肤 / 发现 →"
                setTextColor(t.keyAccent)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                setPadding(dp(8), dp(14), dp(8), dp(8))
                setOnClickListener { onMore?.invoke() }
            },
        )
        visibility = VISIBLE
    }

    fun hide() {
        visibility = GONE
    }

    fun isShowing(): Boolean = visibility == VISIBLE

    private fun header(title: String): TextView =
        TextView(context).apply {
            text = title
            setTextColor(tokens.candText)
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(4), dp(4), dp(4), dp(10))
        }

    private fun row(opt: SkinOption, selected: Boolean): TextView =
        TextView(context).apply {
            text = if (selected) "✓ ${opt.name}" else opt.name
            setTextColor(if (selected) tokens.keyAccent else tokens.candText)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(12), dp(12), dp(12), dp(12))
            background = GradientDrawable().apply {
                setColor(if (selected) tokens.candSelectedBg else tokens.keyNormal)
                cornerRadius = dp(8).toFloat()
            }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            lp.bottomMargin = dp(8)
            layoutParams = lp
            setOnClickListener { onPick?.invoke(opt) }
        }

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).toInt()
}
