package com.yc.input

import android.content.Context
import org.json.JSONObject

data class CatalogItem(
    val kind: String,
    val packId: String,
    val displayName: String,
    val tags: List<String>,
)

data class CampaignItem(
    val id: String,
    val title: String,
    val skinId: String,
    val phraseDeck: String,
    val expires: String,
)

data class LocalCatalog(
    val version: Int,
    val entries: List<CatalogItem>,
    val campaigns: List<CampaignItem>,
) {
    companion object {
        fun parse(raw: String): LocalCatalog {
            val obj = JSONObject(raw)
            val entries = mutableListOf<CatalogItem>()
            val arr = obj.optJSONArray("entries")
            if (arr != null) {
                for (i in 0 until arr.length()) {
                    val e = arr.getJSONObject(i)
                    val tags = mutableListOf<String>()
                    val t = e.optJSONArray("tags")
                    if (t != null) {
                        for (j in 0 until t.length()) tags.add(t.getString(j))
                    }
                    entries.add(
                        CatalogItem(
                            kind = e.optString("kind"),
                            packId = e.optString("pack_id"),
                            displayName = e.optString("display_name"),
                            tags = tags,
                        ),
                    )
                }
            }
            val campaigns = mutableListOf<CampaignItem>()
            val ca = obj.optJSONArray("campaigns")
            if (ca != null) {
                for (i in 0 until ca.length()) {
                    val c = ca.getJSONObject(i)
                    campaigns.add(
                        CampaignItem(
                            id = c.optString("id"),
                            title = c.optString("title"),
                            skinId = c.optString("skin_id"),
                            phraseDeck = c.optString("phrase_deck"),
                            expires = c.optString("expires"),
                        ),
                    )
                }
            }
            return LocalCatalog(obj.optInt("catalog_version", 1), entries, campaigns)
        }

        fun load(ctx: Context): LocalCatalog =
            try {
                ctx.assets.open("catalog/discover_catalog.json").bufferedReader().use {
                    parse(it.readText())
                }
            } catch (_: Exception) {
                LocalCatalog(1, emptyList(), emptyList())
            }
    }
}

object LangPrefs {
    private const val PREF = "yc_lang"
    private const val KEY = "preferred_lang"

    data class LangOption(
        val code: String,
        val name: String,
        val packFile: String,
        val packId: String,
        val layoutHint: String,
    )

    val OPTIONS = listOf(
        LangOption("zh", "中文", "zh-pack-v1.imepack", "zh-pack-v1", "拼音 26 键"),
        LangOption("en", "English", "en-v1.imepack", "en-v1", "US QWERTY"),
        LangOption("vi", "Tiếng Việt", "vi-v1.imepack", "vi-v1", "Telex"),
        LangOption("th", "ไทย", "th-v1.imepack", "th-v1", "Thai"),
    )

    fun code(ctx: Context): String =
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE).getString(KEY, "zh") ?: "zh"

    fun name(ctx: Context): String =
        OPTIONS.firstOrNull { it.code == code(ctx) }?.name ?: "中文"

    fun save(ctx: Context, code: String) {
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE).edit().putString(KEY, code).apply()
    }
}

object SettingsPrefs {
    private const val PREF = "yc_settings"
    private const val KEY_PERSONALIZATION = "personalization"

    fun personalizationEnabled(ctx: Context): Boolean =
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE).getBoolean(KEY_PERSONALIZATION, true)

    fun setPersonalization(ctx: Context, enabled: Boolean) {
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_PERSONALIZATION, enabled)
            .apply()
    }
}
