package com.yc.input.native

import java.nio.ByteBuffer
import java.nio.ByteOrder

object YcNative {
    const val OK = 0
    const val ERR_SESSION = -1
    const val ERR_BUSY = -2
    const val ERR_INTERNAL = -3

    const val ACTION_INIT = 0
    const val ACTION_KEY_PRESS = 1
    const val ACTION_BACKSPACE = 2
    const val ACTION_SELECT_CANDIDATE = 3

    private const val ACTION_SIZE = 40

    init {
        try {
            System.loadLibrary("yc_ffi")
        } catch (_: UnsatisfiedLinkError) {
            // Stub build: FFI symbols are linked into libyc_jni.so.
        }
        System.loadLibrary("yc_jni")
    }

    @JvmStatic external fun ycCoreInit(dataDir: String): Int

    @JvmStatic external fun ycCoreShutdown()

    @JvmStatic external fun ycSessionBegin(fieldId: Long): Long

    @JvmStatic external fun ycSessionBeginWithInput(fieldId: Long, inputType: Int): Long

    @JvmStatic external fun ycSessionValidate(editorId: Long): Int

    @JvmStatic external fun ycSessionStop(editorId: Long, reason: Int)

    @JvmStatic external fun ycHotSubmit(action: ByteArray): Int

    @JvmStatic external fun ycHotArenaPtr(): Long

    @JvmStatic external fun ycHotArenaSize(): Int

    @JvmStatic external fun ycHotLatestSeq(editorId: Long): Long

    @JvmStatic external fun ycColdSubmit(editorId: Long, kind: Int, payload: ByteArray): Int

    @JvmStatic external fun ycCoreSyncLangPacks(): Int

    fun coldSubmit(editorId: Long, kind: Int, payload: ByteArray): Int =
        ycColdSubmit(editorId, kind, payload)

    fun readArena(): ArenaSnapshot? {
        val ptr = ycHotArenaPtr()
        val size = ycHotArenaSize()
        if (ptr == 0L || size <= 0) return null
        val bytes = ByteArray(size)
        nativeReadBytes(ptr, bytes, size)
        return YcArena.parse(bytes)
    }

    @JvmStatic
    private external fun nativeReadBytes(ptr: Long, dest: ByteArray, size: Int)

    fun buildAction(
        editorId: Long,
        clientSeq: Long,
        actionType: Int,
        keyCode: Int = 0,
        candidateId: Int = 0,
    ): ByteArray = ByteBuffer.allocate(ACTION_SIZE)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putLong(editorId)
        .putLong(clientSeq)
        .putInt(actionType)
        .putInt(keyCode)
        .putInt(candidateId)
        .putInt(0)
        .put(ByteArray(8))
        .array()

    /** M0 smoke: init core, begin session, submit INIT action. */
    fun smoke(dataDir: String = "/data/local/tmp/yc"): Int {
        val initRc = ycCoreInit(dataDir)
        if (initRc != OK) return initRc
        val editorId = ycSessionBegin(1L)
        return ycHotSubmit(buildAction(editorId, 1L, ACTION_INIT))
    }
}
