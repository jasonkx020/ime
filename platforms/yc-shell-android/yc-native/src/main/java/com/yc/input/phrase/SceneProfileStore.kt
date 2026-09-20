package com.yc.input.phrase

import android.content.Context
import org.json.JSONObject

/**
 * 当前话术场景：内置行业 ContentPack 或用户自定义。
 * 与通用 AiAssist 分离；每次 Phrase LLM 必注入。
 */
data class SceneProfile(
    val type: String, // builtin | custom
    val industryId: String,
    val packId: String,
    val displayName: String,
    val hint: String,
    val taboos: String,
) {
    fun systemAnchor(): String = buildString {
        append("【使用场景】").append(displayName).append('\n')
        if (hint.isNotBlank()) append("【场景说明】").append(hint).append('\n')
        append("【禁止】输出与该场景无关的话术。")
        if (taboos.isNotBlank()) append(taboos)
    }
}

object SceneProfileStore {
    private const val PREF = "yc_content"
    private const val KEY_ACTIVE_PACK = "active_industry"
    private const val KEY_MODE = "scene_mode" // builtin | custom
    private const val KEY_CUSTOM = "custom_scene"

    data class BuiltinIndustry(
        val industryId: String,
        val packId: String,
        val displayName: String,
        val hint: String,
        val taboos: String,
    )

    val BUILTINS = listOf(
        BuiltinIndustry(
            industryId = "ecommerce",
            packId = "industry-ecommerce-v1",
            displayName = "电商客服",
            hint = "面向网购买家的店铺客服，语气亲切专业，可称「亲」。",
            taboos = "不贬低竞品；不承诺未核实的发货时效/价格/库存；不编造单号。",
        ),
        BuiltinIndustry(
            industryId = "gaming",
            packId = "industry-gaming-v1",
            displayName = "游戏开黑",
            hint = "游戏组队与开黑沟通，语气轻松，可用常用黑话。",
            taboos = "不人身攻击；不泄露账号密码；不引导未成年人充值。",
        ),
    )

    fun mode(ctx: Context): String =
        prefs(ctx).getString(KEY_MODE, "builtin") ?: "builtin"

    fun setMode(ctx: Context, mode: String) {
        prefs(ctx).edit().putString(KEY_MODE, mode).apply()
    }

    fun activePackId(ctx: Context): String =
        prefs(ctx).getString(KEY_ACTIVE_PACK, "industry-ecommerce-v1")
            ?: "industry-ecommerce-v1"

    fun setActivePackId(ctx: Context, packId: String) {
        prefs(ctx).edit().putString(KEY_ACTIVE_PACK, packId).apply()
        setMode(ctx, "builtin")
    }

    fun saveCustom(ctx: Context, name: String, hint: String, taboos: String) {
        val obj = JSONObject()
            .put("name", name.trim())
            .put("hint", hint.trim())
            .put("taboos", taboos.trim())
        prefs(ctx).edit()
            .putString(KEY_CUSTOM, obj.toString())
            .putString(KEY_MODE, "custom")
            .apply()
    }

    fun customRaw(ctx: Context): Triple<String, String, String>? {
        val raw = prefs(ctx).getString(KEY_CUSTOM, null) ?: return null
        return try {
            val o = JSONObject(raw)
            Triple(
                o.optString("name"),
                o.optString("hint"),
                o.optString("taboos"),
            )
        } catch (_: Exception) {
            null
        }
    }

    fun current(ctx: Context): SceneProfile {
        if (mode(ctx) == "custom") {
            val c = customRaw(ctx)
            if (c != null && c.first.isNotBlank() && c.second.isNotBlank()) {
                return SceneProfile(
                    type = "custom",
                    industryId = "custom",
                    packId = "",
                    displayName = c.first,
                    hint = c.second,
                    taboos = c.third,
                )
            }
        }
        val pack = activePackId(ctx)
        val b = BUILTINS.firstOrNull { it.packId == pack } ?: BUILTINS.first()
        return SceneProfile(
            type = "builtin",
            industryId = b.industryId,
            packId = b.packId,
            displayName = b.displayName,
            hint = b.hint,
            taboos = b.taboos,
        )
    }

    /** 自定义行业时的通用骨架卡。 */
    fun customSkeletonCards(displayName: String): List<Triple<String, String, String>> {
        val n = displayName.ifBlank { "本行业" }
        return listOf(
            Triple("greet", "欢迎", "您好，这里是$n，请问有什么可以帮您？"),
            Triple("answer", "解答", "您的问题我已了解，我来为您说明一下。"),
            Triple("schedule", "约时间", "方便的话我们可以约个时间细聊，您看什么时候合适？"),
            Triple("close", "收尾", "好的，有需要随时联系我。祝您顺利！"),
            Triple("sorry", "道歉", "非常抱歉给您带来不便，我们会尽快协助处理。"),
        )
    }

    private fun prefs(ctx: Context) =
        ctx.applicationContext.getSharedPreferences(PREF, Context.MODE_PRIVATE)
}
