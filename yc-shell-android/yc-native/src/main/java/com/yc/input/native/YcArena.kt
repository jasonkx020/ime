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
)

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

        val composing = String(data, HEADER_SIZE, composingLen, Charsets.UTF_8)

        val candidates = mutableListOf<ArenaCandidate>()
        var slotsOff = HEADER_SIZE + COMPOSING_LEN
        repeat(candCount) { i ->
            val off = slotsOff + i * CAND_SLOT_SIZE
            if (off + CAND_SLOT_SIZE > data.size) return@repeat
            val slot = ByteBuffer.wrap(data, off, CAND_SLOT_SIZE).order(ByteOrder.LITTLE_ENDIAN)
            val id = slot.getInt(0)
            val textLen = slot.getInt(8).coerceIn(0, MAX_CAND_TEXT_LEN)
            val text = String(data, off + 16, textLen, Charsets.UTF_8)
            candidates.add(ArenaCandidate(id, text))
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
            val text = String(data, off + 16, textLen, Charsets.UTF_8)
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
}
