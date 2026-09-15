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
    const val ACTION_TOGGLE_ASCII = 6
    const val ACTION_OPEN_HANDWRITING = 7
    const val ACTION_DISMISS_HANDWRITING = 8
    const val ACTION_RECOGNIZE_HANDWRITING = 9
    const val ACTION_CLEAR_HANDWRITING = 10
    const val ACTION_UNDO_HANDWRITING = 11
    const val ACTION_CONFIRM_CLOUD_HW = 12
    const val ACTION_DISMISS_CLOUD_HW = 13
    const val ACTION_PAGE_NEXT = 15
    const val ACTION_PAGE_PREV = 16

    const val WRITING_SINGLE_CHAR = 0
    const val WRITING_CONTINUOUS = 1

    /** Arena ReloadKeyboard.layout = HandwritingPad */
    const val LAYOUT_HANDWRITING_PAD = 4

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

    @JvmStatic external fun ycCoreInstallLangpack(packPath: String): Int

    /**
     * Push one handwriting stroke.
     * @param xyPressure interleaved [x, y, pressure] * N (normalized 0..1)
     * @param timesMs timestamp ms per point (length N)
     */
    @JvmStatic
    external fun ycHwPushStroke(
        editorId: Long,
        xyPressure: FloatArray,
        timesMs: LongArray,
        sessionStrokeId: Long,
        canvasW: Int,
        canvasH: Int,
        writingMode: Int,
    ): Int

    /**
     * Inject shell handwriting recognition candidates into the hot arena.
     * @param flags bit0 = needs_cloud_confirm
     */
    @JvmStatic
    external fun ycHwApplyResult(
        editorId: Long,
        texts: Array<String>,
        scores: FloatArray,
        flags: Int,
    ): Int

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
