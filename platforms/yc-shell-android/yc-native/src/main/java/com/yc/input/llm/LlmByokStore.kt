package com.yc.input.llm

import android.content.Context
import org.json.JSONObject

data class LlmProfile(
    val id: String,
    val region: String,
    val provider: String,
    val displayName: String,
    val baseUrl: String,
    val apiPath: String,
    val model: String,
    val auth: String,
    val allowOverride: Boolean,
    val docsHint: String,
)

data class LlmEndpoint(
    val profileId: String,
    val baseUrl: String,
    val apiPath: String,
    val model: String,
    val auth: String,
    val apiKey: String,
)

object LlmProviderCatalog {
    fun load(ctx: Context): List<LlmProfile> {
        return try {
            ctx.assets.open("catalog/llm_providers.json").bufferedReader().use { reader ->
                parse(reader.readText())
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    fun parse(raw: String): List<LlmProfile> {
        val obj = JSONObject(raw)
        val arr = obj.optJSONArray("profiles") ?: return emptyList()
        val out = mutableListOf<LlmProfile>()
        for (i in 0 until arr.length()) {
            val e = arr.getJSONObject(i)
            out.add(
                LlmProfile(
                    id = e.optString("id"),
                    region = e.optString("region", "intl"),
                    provider = e.optString("provider"),
                    displayName = e.optString("display_name"),
                    baseUrl = e.optString("base_url").trimEnd('/'),
                    apiPath = e.optString("api_path"),
                    model = e.optString("model"),
                    auth = e.optString("auth", "bearer"),
                    allowOverride = e.optBoolean("allow_override", false),
                    docsHint = e.optString("docs_hint"),
                ),
            )
        }
        return out
    }

    fun find(profiles: List<LlmProfile>, id: String): LlmProfile? =
        profiles.firstOrNull { it.id == id }
}

/**
 * BYOK 本地配置。Key 存普通私有偏好（不写日志）；生产可再升 EncryptedSharedPreferences。
 */
object LlmByokPrefs {
    private const val PREF = "yc_llm_byok"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_PROFILE = "profile_id"
    private const val KEY_API_KEY = "api_key"
    private const val KEY_CUSTOM_BASE = "custom_base_url"
    private const val KEY_CUSTOM_MODEL = "custom_model"

    fun enabled(ctx: Context): Boolean =
        prefs(ctx).getBoolean(KEY_ENABLED, true)

    fun setEnabled(ctx: Context, v: Boolean) {
        prefs(ctx).edit().putBoolean(KEY_ENABLED, v).apply()
    }

    fun selectedProfileId(ctx: Context): String =
        prefs(ctx).getString(KEY_PROFILE, "deepseek-chat") ?: "deepseek-chat"

    fun setSelectedProfileId(ctx: Context, id: String) {
        prefs(ctx).edit().putString(KEY_PROFILE, id).apply()
    }

    fun apiKey(ctx: Context): String =
        prefs(ctx).getString(KEY_API_KEY, "") ?: ""

    fun setApiKey(ctx: Context, key: String) {
        prefs(ctx).edit().putString(KEY_API_KEY, key.trim()).apply()
    }

    fun clearApiKey(ctx: Context) {
        prefs(ctx).edit().remove(KEY_API_KEY).apply()
    }

    fun hasKey(ctx: Context): Boolean = apiKey(ctx).isNotBlank()

    fun customBaseUrl(ctx: Context): String =
        prefs(ctx).getString(KEY_CUSTOM_BASE, "http://127.0.0.1:11434") ?: "http://127.0.0.1:11434"

    fun setCustomBaseUrl(ctx: Context, url: String) {
        prefs(ctx).edit().putString(KEY_CUSTOM_BASE, url.trim().trimEnd('/')).apply()
    }

    fun customModel(ctx: Context): String =
        prefs(ctx).getString(KEY_CUSTOM_MODEL, "llama3.2") ?: "llama3.2"

    fun setCustomModel(ctx: Context, model: String) {
        prefs(ctx).edit().putString(KEY_CUSTOM_MODEL, model.trim()).apply()
    }

    fun resolveEndpoint(ctx: Context, profiles: List<LlmProfile>): LlmEndpoint? {
        if (!enabled(ctx)) return null
        val id = selectedProfileId(ctx)
        val profile = LlmProviderCatalog.find(profiles, id) ?: return null
        val key = apiKey(ctx)
        return if (profile.allowOverride) {
            val base = customBaseUrl(ctx)
            val model = customModel(ctx)
            if (base.isBlank() || model.isBlank()) return null
            if (profile.auth == "bearer" && key.isBlank()) return null
            LlmEndpoint(id, base, profile.apiPath, model, profile.auth, key)
        } else {
            if (key.isBlank()) return null
            LlmEndpoint(id, profile.baseUrl, profile.apiPath, profile.model, profile.auth, key)
        }
    }

    fun isReady(ctx: Context, profiles: List<LlmProfile>): Boolean =
        resolveEndpoint(ctx, profiles) != null

    private fun prefs(ctx: Context) =
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE)
}
