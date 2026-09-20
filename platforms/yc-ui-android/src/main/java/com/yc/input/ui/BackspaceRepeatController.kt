package com.yc.input.ui

import android.os.Handler
import android.os.Looper
import android.view.View

/**
 * 微信式退格：按下约 400ms 后加速连删；抬手 / 滑出 / CANCEL 停止。
 * 单击由调用方在 UP 且 [consumedByRepeat] 为 false 时触发一次。
 */
internal class BackspaceRepeatController(
    private val host: View,
    private val fire: (KeyDef) -> Unit,
) {
    private val handler = Handler(Looper.getMainLooper())
    private var activeKey: KeyDef? = null
    private var repeating = false
    private var intervalMs = INITIAL_INTERVAL_MS

    private val startRunnable = Runnable { beginRepeat() }
    private val tickRunnable = Runnable { tick() }

    /** 是否已进入连删（UP 时不应再点删一次）。 */
    fun consumedByRepeat(): Boolean = repeating

    fun onDown(key: KeyDef) {
        cancel()
        if (key.action != KeyAction.Backspace) return
        activeKey = key
        repeating = false
        intervalMs = INITIAL_INTERVAL_MS
        handler.postDelayed(startRunnable, LONG_PRESS_MS)
    }

    /** 手指仍在退格键上时保持；滑出则停止。 */
    fun onMoveStay(stillOnBackspace: Boolean) {
        if (activeKey == null) return
        if (!stillOnBackspace) cancel()
    }

    fun onUpOrCancel() {
        cancel()
    }

    fun cancel() {
        handler.removeCallbacks(startRunnable)
        handler.removeCallbacks(tickRunnable)
        activeKey = null
        repeating = false
        intervalMs = INITIAL_INTERVAL_MS
    }

    private fun beginRepeat() {
        val key = activeKey ?: return
        repeating = true
        fire(key)
        host.performHapticFeedback(android.view.HapticFeedbackConstants.KEYBOARD_TAP)
        scheduleNext()
    }

    private fun tick() {
        val key = activeKey ?: return
        fire(key)
        intervalMs = when {
            intervalMs > 80L -> 80L
            intervalMs > 50L -> 50L
            else -> MIN_INTERVAL_MS
        }
        scheduleNext()
    }

    private fun scheduleNext() {
        handler.postDelayed(tickRunnable, intervalMs)
    }

    companion object {
        const val LONG_PRESS_MS = 400L
        const val INITIAL_INTERVAL_MS = 120L
        const val MIN_INTERVAL_MS = 40L
    }
}
