package com.yc.input.native

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Parsed hot-path arena snapshot (mirrors yc-ffi arena_read). */
data class ArenaSnapshot(
    val editorId: Long,
    val seq: Long,
    val statusFlags: Int,
    val composing: String,
    val candidates: List<ArenaCandidate>,
    val commands: List<ArenaCommand>,
) {
    /** status_flags bits 8..15 = cand_page */
    val candPage: Int get() = (statusFlags ushr 8) and 0xff

    /** status_flags bits 16..31 = total_pages */
    val totalPages: Int get() = (statusFlags ushr 16) and 0xffff

    /** status_flags bit0 = ascii / English mode */
    val asciiMode: Boolean get() = (statusFlags and 0x1) != 0

    /** status_flags bit1 = pending cloud handwriting confirm */
    val pendingCloudHw: Boolean get() = (statusFlags and 0x2) != 0

    /** status_flags bit2 = need LLM pinyin candidate top-up */
    val needsLlmFallback: Boolean get() = (statusFlags and 0x4) != 0
}

data class ArenaCandidate(val id: Int, val text: String)

sealed class ArenaCommand {
    data class Commit(val text: String) : ArenaCommand()
    data class SetComposing(val text: String) : ArenaCommand()
    object FinishComposing : ArenaCommand()
    data class DeleteSurrounding(val before: Int, val after: Int) : ArenaCommand()
    data class ReloadKeyboard(val layout: Int, val layoutId: String = "") : ArenaCommand()
}

object YcArena {
    private const val HEADER_SIZE = 32
    private const val COMPOSING_LEN = 64
    private const val CAND_SLOT_SIZE = 80
    private const val CMD_SLOT_SIZE = 80
    private const val MAX_CANDIDATES = 9
    private const val MAX_ARENA_COMMANDS = 4
    private const val MAX_CAND_TEXT_LEN = 64

    const val CMD_COMMIT = 0
    const val CMD_SET_COMPOSING = 1
    const val CMD_FINISH_COMPOSING = 2
    const val CMD_DELETE_SURROUNDING = 3
    const val CMD_RELOAD_KEYBOARD = 4

    fun parse(data: ByteArray): ArenaSnapshot? {
        if (data.size < HEADER_SIZE) return null
        val buf = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
        val editorId = buf.getLong(0)
        val seq = buf.getLong(8)
        val statusFlags = buf.getInt(16)
        val composingLen = buf.getInt(20).coerceIn(0, COMPOSING_LEN)
        val candCount = buf.getInt(24).coerceIn(0, MAX_CANDIDATES)
        val cmdCount = buf.getInt(28).coerceIn(0, MAX_ARENA_COMMANDS)

        val composing = decodeArenaText(data, HEADER_SIZE, composingLen)

        val candidates = mutableListOf<ArenaCandidate>()
        val slotsOff = HEADER_SIZE + COMPOSING_LEN
        repeat(candCount) { i ->
            val off = slotsOff + i * CAND_SLOT_SIZE
            if (off + CAND_SLOT_SIZE > data.size) return@repeat
            val slot = ByteBuffer.wrap(data, off, CAND_SLOT_SIZE).order(ByteOrder.LITTLE_ENDIAN)
            val id = slot.getInt(0)
            val textLen = slot.getInt(8).coerceIn(0, MAX_CAND_TEXT_LEN)
            val text = decodeArenaText(data, off + 16, textLen)
            if (text.isNotEmpty()) {
                candidates.add(ArenaCandidate(id, text))
            }
        }

        val commands = mutableListOf<ArenaCommand>()
        val cmdsOff = slotsOff + MAX_CANDIDATES * CAND_SLOT_SIZE
        repeat(cmdCount) { i ->
            val off = cmdsOff + i * CMD_SLOT_SIZE
            if (off + CMD_SLOT_SIZE > data.size) return@repeat
            val slot = ByteBuffer.wrap(data, off, CMD_SLOT_SIZE).order(ByteOrder.LITTLE_ENDIAN)
            val cmdType = slot.getInt(0)
            val param0 = slot.getInt(4)
            val param1 = slot.getInt(8)
            val textLen = slot.getInt(12).coerceIn(0, MAX_CAND_TEXT_LEN)
            val text = decodeArenaText(data, off + 16, textLen)
            when (cmdType) {
                CMD_COMMIT -> commands.add(ArenaCommand.Commit(text))
                CMD_SET_COMPOSING -> commands.add(ArenaCommand.SetComposing(text))
                CMD_FINISH_COMPOSING -> commands.add(ArenaCommand.FinishComposing)
                CMD_DELETE_SURROUNDING -> commands.add(ArenaCommand.DeleteSurrounding(param0, param1))
                CMD_RELOAD_KEYBOARD -> commands.add(ArenaCommand.ReloadKeyboard(param0, text))
            }
        }

        return ArenaSnapshot(editorId, seq, statusFlags, composing, candidates, commands)
    }

    /**
     * 定长槽位可能在有效 UTF-8 后仍有 NUL/脏字节；按首个 0 截断，并去掉 FFFD/控制符。
     */
    fun decodeArenaText(data: ByteArray, offset: Int, claimedLen: Int): String {
        if (claimedLen <= 0 || offset < 0 || offset >= data.size) return ""
        val max = minOf(claimedLen, data.size - offset)
        var end = max
        for (i in 0 until max) {
            if (data[offset + i] == 0.toByte()) {
                end = i
                break
            }
        }
        if (end <= 0) return ""
        return String(data, offset, end, Charsets.UTF_8)
            .filter { ch ->
                ch != '\u0000' && ch != '\uFFFD' && !ch.isISOControl()
            }
    }
}
