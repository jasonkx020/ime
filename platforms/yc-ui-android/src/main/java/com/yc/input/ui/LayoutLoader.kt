package com.yc.input.ui

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.ZipFile

/** Loads `layouts/{id}.bin` (YCLY) from installed langpacks; falls back to US QWERTY. */
object LayoutLoader {
    private const val MAGIC = "YCLY"
    private const val MAX_LAYOUT_ID = 64
    private const val MAX_KEY_LABEL = 16
    private const val MAX_KEY_OUTPUT = 16
    private const val ACTION_BACKSPACE = 1
    private const val ACTION_SWITCH_LAYOUT = 2
    private const val ACTION_SWITCH_LANG = 3
    private const val ACTION_ROW_BREAK = 5
    private const val ACTION_SHIFT = 6
    private val cache = java.util.concurrent.ConcurrentHashMap<String, List<List<KeyDef>>>()

    fun load(dataDir: File, layoutId: String, preferredPackId: String? = null): List<List<KeyDef>> {
        val cacheKey = if (preferredPackId.isNullOrBlank()) layoutId else "$preferredPackId/$layoutId"
        cache[cacheKey]?.let { return it }
        val rows = if (isPinyinLayout(layoutId)) {
            val fromPack = loadFromPack(dataDir, layoutId, preferredPackId)
            if (fromPack != null && fromPack.size >= 4) fromPack else Layout26Pinyin.rows
        } else {
            loadFromPack(dataDir, layoutId, preferredPackId) ?: Layout26Pinyin.rows
        }
        cache[cacheKey] = rows
        return rows
    }

    /** 仅从语言包加载；找不到返回 null（不做 QWERTY 兜底）。 */
    fun loadOrNull(dataDir: File, layoutId: String, preferredPackId: String? = null): List<List<KeyDef>>? =
        loadFromPack(dataDir, layoutId, preferredPackId)

    private fun isPinyinLayout(layoutId: String): Boolean =
        layoutId == "layout_pinyin26" || layoutId == "layout_26_pinyin"

    private fun loadFromPack(
        dataDir: File,
        layoutId: String,
        preferredPackId: String? = null,
    ): List<List<KeyDef>>? {
        val preferredPack = preferredPackId ?: preferredPackForLayout(layoutId)
        // 1) 已解压目录：优先匹配语言相关 pack，再扫其余
        val langpacks = File(dataDir, "langpacks")
        if (langpacks.isDirectory) {
            val dirs = (langpacks.listFiles() ?: emptyArray())
                .filter { it.isDirectory }
                .sortedBy { dir ->
                    when {
                        preferredPack != null && dir.name == preferredPack -> 0
                        dir.name.startsWith("en") && layoutId.startsWith("layout_en") -> 1
                        else -> 2
                    }
                }
            for (pack in dirs) {
                val bin = File(pack, "layouts/$layoutId.bin")
                if (bin.isFile) {
                    parseBin(bin.readBytes())?.let { return it }
                }
            }
        }
        // 2) 回退：直接读 {dataDir}/{packId}.imepack ZIP 内 layouts/{id}.bin
        val packs = buildList {
            if (preferredPack != null) add(preferredPack)
            addAll(listOf("en-v1", "zh-pack-v1", "vi-v1", "th-v1"))
        }.distinct()
        for (packId in packs) {
            val imepack = File(dataDir, "$packId.imepack")
            if (!imepack.isFile) continue
            readBinFromZip(imepack, "layouts/$layoutId.bin")?.let { bytes ->
                parseBin(bytes)?.let { return it }
            }
        }
        return null
    }

    /** layout_en_* → en-v1；避免与 vi/th 的 layout_qwerty stub 撞名误加载。 */
    private fun preferredPackForLayout(layoutId: String): String? = when {
        layoutId.startsWith("layout_en") -> "en-v1"
        layoutId.contains("vietnamese") -> "vi-v1"
        layoutId.contains("thai") -> "th-v1"
        layoutId.contains("pinyin") || layoutId == "layout_symbol" -> "zh-pack-v1"
        else -> null
    }

    private fun readBinFromZip(zipFile: File, entryName: String): ByteArray? {
        return try {
            ZipFile(zipFile).use { zip ->
                val entry = zip.getEntry(entryName) ?: return null
                zip.getInputStream(entry).use { it.readBytes() }
            }
        } catch (_: Exception) {
            null
        }
    }

    private fun parseBin(bytes: ByteArray): List<List<KeyDef>>? {
        if (bytes.size < 8 + MAX_LAYOUT_ID + 4) return null
        val magic = String(bytes, 0, 4, Charsets.US_ASCII)
        if (magic != MAGIC) return null
        val keyCount = ByteBuffer.wrap(bytes, 8 + MAX_LAYOUT_ID, 4)
            .order(ByteOrder.LITTLE_ENDIAN)
            .int
            .coerceAtLeast(0)
        val slotSize = MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1 + 4
        val keysStart = 8 + MAX_LAYOUT_ID + 4
        if (bytes.size < keysStart + keyCount * slotSize) return null

        val rows = mutableListOf<MutableList<KeyDef>>()
        var row = mutableListOf<KeyDef>()
        for (i in 0 until keyCount) {
            val off = keysStart + i * slotSize
            val label = cstr(bytes, off, MAX_KEY_LABEL)
            val output = cstr(bytes, off + MAX_KEY_LABEL, MAX_KEY_OUTPUT)
            val action = bytes[off + MAX_KEY_LABEL + MAX_KEY_OUTPUT].toInt() and 0xff
            val width = ByteBuffer.wrap(bytes, off + MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1, 4)
                .order(ByteOrder.LITTLE_ENDIAN)
                .float
                .coerceAtLeast(0.5f)

            if (action == ACTION_ROW_BREAK) {
                if (row.isNotEmpty()) {
                    rows.add(row)
                    row = mutableListOf()
                }
                continue
            }

            row.add(
                when {
                    action == ACTION_BACKSPACE ->
                        KeyDef(label.ifEmpty { "⌫" }, width, KeyStyle.Utility, action = KeyAction.Backspace)
                    action == ACTION_SHIFT ->
                        KeyDef(label.ifEmpty { "⇧" }, width, KeyStyle.Utility, action = KeyAction.Shift)
                    action == ACTION_SWITCH_LAYOUT -> {
                        val isBack = label.contains("ABC") || label.contains("手写") ||
                            label.equals("ABC", true)
                        if (isBack) {
                            KeyDef(label.ifEmpty { "ABC" }, width, KeyStyle.Utility, action = KeyAction.Letters)
                        } else {
                            KeyDef(label.ifEmpty { "123" }, width, KeyStyle.Utility, action = KeyAction.Symbol)
                        }
                    }
                    action == ACTION_SWITCH_LANG ->
                        KeyDef(label.ifEmpty { "🌐" }, width, KeyStyle.Utility, action = KeyAction.Globe)
                    output.startsWith("tone:") ->
                        KeyDef(
                            label = label.ifEmpty { output.removePrefix("tone:") },
                            widthWeight = width,
                            style = KeyStyle.Utility,
                            action = KeyAction.Tone,
                            output = output,
                        )
                    label == "🎤" || output == "voice" ->
                        KeyDef(label.ifEmpty { "🎤" }, width, KeyStyle.Utility, action = KeyAction.Mic)
                    else -> {
                        val isSpace = output == " " || label.contains("空格") ||
                            label.equals("space", true) || label == "cách" || label == "วรรค"
                        val ch = output.firstOrNull() ?: label.firstOrNull()
                        KeyDef(
                            label = label.ifEmpty { output },
                            widthWeight = width,
                            keyCode = ch?.code,
                            action = when {
                                isSpace -> KeyAction.Space
                                label == "搜索" || label == "换行" || label == "回车" ||
                                    label == "下一步" || label.equals("go", true) ||
                                    output == "\n" -> KeyAction.Search
                                else -> KeyAction.Letter
                            },
                            style = when {
                                label == "搜索" || label == "换行" || label == "下一步" ||
                                    label.equals("go", true) -> KeyStyle.Accent
                                label == "回车" || isSpace -> KeyStyle.Utility
                                else -> KeyStyle.Normal
                            },
                            output = output.ifEmpty { null },
                        )
                    }
                },
            )
        }
        if (row.isNotEmpty()) rows.add(row)
        return rows.takeIf { it.isNotEmpty() }
    }

    private fun cstr(bytes: ByteArray, off: Int, max: Int): String {
        val end = (off until off + max).firstOrNull { bytes[it] == 0.toByte() } ?: (off + max)
        return String(bytes, off, end - off, Charsets.UTF_8)
    }
}
