package com.yc.input.ui

import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.zip.ZipFile

/**
 * Pack-level layout ids & keyboard height from installed langpack `manifest.fb` (JSON).
 * Falls back to scheme defaults when fields are missing.
 */
data class PackLayoutConfig(
    val packId: String,
    val letterLayoutId: String,
    val symbolLayoutId: String?,
    val shiftLayoutId: String?,
    val keyboardHeightDp: Int?,
)

object PackLayoutCatalog {
    private val cache = ConcurrentHashMap<String, PackLayoutConfig>()

    fun clear() {
        cache.clear()
    }

    fun forLang(dataDir: File, langCode: String): PackLayoutConfig {
        val packId = packIdForLang(langCode)
        return forPack(dataDir, packId, langCode)
    }

    fun forPack(dataDir: File, packId: String, langCode: String = langFromPack(packId)): PackLayoutConfig {
        cache[packId]?.let { return it }
        val parsed = readManifest(dataDir, packId) ?: fallback(packId, langCode)
        cache[packId] = parsed
        return parsed
    }

    fun invalidate(packId: String) {
        cache.remove(packId)
    }

    fun packIdForLang(code: String): String = when (code) {
        "en" -> "en-v1"
        "vi" -> "vi-v1"
        "th" -> "th-v1"
        else -> "zh-pack-v1"
    }

    private fun langFromPack(packId: String): String = when {
        packId.startsWith("en") -> "en"
        packId.startsWith("vi") -> "vi"
        packId.startsWith("th") -> "th"
        else -> "zh"
    }

    private fun fallback(packId: String, langCode: String): PackLayoutConfig = when (langCode) {
        "en" -> PackLayoutConfig(
            packId = packId,
            letterLayoutId = "layout_en_qwerty",
            symbolLayoutId = "layout_en_symbol",
            shiftLayoutId = "layout_en_qwerty_shift",
            keyboardHeightDp = 267,
        )
        "vi" -> PackLayoutConfig(
            packId = packId,
            letterLayoutId = "layout_vietnamese",
            symbolLayoutId = "layout_symbol",
            shiftLayoutId = "layout_vietnamese_shift",
            keyboardHeightDp = 324,
        )
        "th" -> PackLayoutConfig(
            packId = packId,
            letterLayoutId = "layout_thai",
            symbolLayoutId = "layout_symbol",
            shiftLayoutId = "layout_thai_shift",
            keyboardHeightDp = 228,
        )
        else -> PackLayoutConfig(
            packId = packId,
            letterLayoutId = "layout_pinyin26",
            symbolLayoutId = "layout_symbol",
            shiftLayoutId = "layout_pinyin26_shift",
            keyboardHeightDp = 267,
        )
    }

    private fun readManifest(dataDir: File, packId: String): PackLayoutConfig? {
        val bytes = readManifestBytes(dataDir, packId) ?: return null
        return try {
            val root = JSONObject(String(bytes, Charsets.UTF_8))
            val schemes = root.optJSONArray("schemes")
            val letter = schemes?.optJSONObject(0)?.optString("default_layout_id")
                ?.takeIf { it.isNotBlank() }
                ?: fallback(packId, langFromPack(packId)).letterLayoutId
            val layouts = root.optJSONObject("layouts")
            val symbol = layouts?.optString("symbol_layout_id")?.takeIf { it.isNotBlank() }
            val shift = layouts?.optString("shift_layout_id")?.takeIf { it.isNotBlank() }
            val height = layouts?.let { o ->
                if (o.has("keyboard_height_dp") && !o.isNull("keyboard_height_dp")) {
                    o.optInt("keyboard_height_dp").takeIf { it > 0 }
                } else {
                    null
                }
            }
            PackLayoutConfig(
                packId = packId,
                letterLayoutId = letter,
                symbolLayoutId = symbol,
                shiftLayoutId = shift,
                keyboardHeightDp = height,
            )
        } catch (_: Exception) {
            null
        }
    }

    private fun readManifestBytes(dataDir: File, packId: String): ByteArray? {
        val extracted = File(dataDir, "langpacks/$packId/manifest.fb")
        if (extracted.isFile) {
            return try {
                extracted.readBytes()
            } catch (_: Exception) {
                null
            }
        }
        val imepack = File(dataDir, "$packId.imepack")
        if (!imepack.isFile) return null
        return try {
            ZipFile(imepack).use { zip ->
                val entry = zip.getEntry("manifest.fb") ?: return null
                zip.getInputStream(entry).use { it.readBytes() }
            }
        } catch (_: Exception) {
            null
        }
    }
}
