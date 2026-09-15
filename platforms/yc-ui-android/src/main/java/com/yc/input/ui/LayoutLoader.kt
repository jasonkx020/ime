package com.yc.input.ui

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder

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

    fun load(dataDir: File, layoutId: String): List<List<KeyDef>> {
        if (isPinyinLayout(layoutId)) {
            // Prefer built-in full US QWERTY until pack bins are rebuilt with row_break.
            val fromPack = loadFromPack(dataDir, layoutId)
            if (fromPack != null && fromPack.size >= 4) return fromPack
            return Layout26Pinyin.rows
        }
        return loadFromPack(dataDir, layoutId) ?: Layout26Pinyin.rows
    }

    private fun isPinyinLayout(layoutId: String): Boolean =
        layoutId == "layout_pinyin26" || layoutId == "layout_26_pinyin"

    private fun loadFromPack(dataDir: File, layoutId: String): List<List<KeyDef>>? {
        val langpacks = File(dataDir, "langpacks")
        if (!langpacks.isDirectory) return null
        for (pack in langpacks.listFiles() ?: emptyArray()) {
            val bin = File(pack, "layouts/$layoutId.bin")
            if (bin.isFile) {
                return parseBin(bin.readBytes())
            }
        }
        return null
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
                when (action) {
                    ACTION_BACKSPACE ->
                        KeyDef(label.ifEmpty { "⌫" }, width, KeyStyle.Utility, action = KeyAction.Backspace)
                    ACTION_SHIFT ->
                        KeyDef(label.ifEmpty { "⇧" }, width, KeyStyle.Utility, action = KeyAction.Shift)
                    ACTION_SWITCH_LAYOUT ->
                        KeyDef(label.ifEmpty { "!#1" }, width, KeyStyle.Utility, action = KeyAction.Symbol)
                    ACTION_SWITCH_LANG ->
                        KeyDef(label.ifEmpty { "🌐" }, width, KeyStyle.Utility, action = KeyAction.Globe)
                    else -> {
                        val ch = output.firstOrNull() ?: label.firstOrNull()
                        val isSpace = output == " " || label.contains("空格")
                        KeyDef(
                            label = label.ifEmpty { output },
                            widthWeight = width,
                            keyCode = ch?.code,
                            action = if (isSpace) KeyAction.Space else KeyAction.Letter,
                            style = if (label == "搜索") KeyStyle.Accent else KeyStyle.Normal,
                        ).let { def ->
                            if (label == "搜索") def.copy(action = KeyAction.Search) else def
                        }
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
