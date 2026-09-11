package com.yc.input.ui

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Loads `layouts/{id}.bin` (YCLY) from installed langpacks. */
object LayoutLoader {
    private const val MAGIC = "YCLY"
    private const val MAX_LAYOUT_ID = 64
    private const val MAX_KEY_LABEL = 16
    private const val MAX_KEY_OUTPUT = 16

    fun load(dataDir: File, layoutId: String): List<List<KeyDef>> {
        val langpacks = File(dataDir, "langpacks")
        if (!langpacks.isDirectory) return Layout26Pinyin.rows
        for (pack in langpacks.listFiles() ?: emptyArray()) {
            val bin = File(pack, "layouts/$layoutId.bin")
            if (bin.isFile) {
                return parseBin(bin.readBytes())
            }
        }
        return Layout26Pinyin.rows
    }

    private fun parseBin(bytes: ByteArray): List<List<KeyDef>> {
        if (bytes.size < 8 + MAX_LAYOUT_ID + 4) return Layout26Pinyin.rows
        val magic = String(bytes, 0, 4, Charsets.US_ASCII)
        if (magic != MAGIC) return Layout26Pinyin.rows
        val keyCount = ByteBuffer.wrap(bytes, 8 + MAX_LAYOUT_ID, 4)
            .order(ByteOrder.LITTLE_ENDIAN)
            .int
            .coerceAtLeast(0)
        val slotSize = MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1 + 4
        val keysStart = 8 + MAX_LAYOUT_ID + 4
        if (bytes.size < keysStart + keyCount * slotSize) return Layout26Pinyin.rows

        val keys = mutableListOf<KeyDef>()
        for (i in 0 until keyCount) {
            val off = keysStart + i * slotSize
            val label = cstr(bytes, off, MAX_KEY_LABEL)
            val output = cstr(bytes, off + MAX_KEY_LABEL, MAX_KEY_OUTPUT)
            val action = bytes[off + MAX_KEY_LABEL + MAX_KEY_OUTPUT].toInt()
            val width = ByteBuffer.wrap(bytes, off + MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1, 4)
                .order(ByteOrder.LITTLE_ENDIAN)
                .float
                .coerceAtLeast(0.5f)
            keys.add(
                when (action) {
                    1 -> KeyDef(label.ifEmpty { "⌫" }, width, KeyStyle.Utility, action = KeyAction.Backspace)
                    else -> {
                        val ch = output.firstOrNull() ?: label.firstOrNull()
                        KeyDef(
                            label = label.ifEmpty { output },
                            widthWeight = width,
                            keyCode = ch?.code,
                            action = if (output == " ") KeyAction.Space else KeyAction.Letter,
                        )
                    }
                },
            )
        }
        if (keys.isEmpty()) return Layout26Pinyin.rows
        return listOf(keys)
    }

    private fun cstr(bytes: ByteArray, off: Int, max: Int): String {
        val end = (off until off + max).firstOrNull { bytes[it] == 0.toByte() } ?: (off + max)
        return String(bytes, off, end - off, Charsets.UTF_8)
    }
}
