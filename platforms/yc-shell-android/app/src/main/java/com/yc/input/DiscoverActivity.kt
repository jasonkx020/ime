package com.yc.input

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * 兼容入口：键盘「设置 / 更多皮肤」仍可能跳到本 Activity，
 * 统一转发到 [MainActivity] 底栏壳。
 */
class DiscoverActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val tab = intent.getStringExtra(EXTRA_TAB) ?: "skins"
        val target = when (tab) {
            "settings" -> MainActivity.PAGE_SETTINGS
            "skins", "content", "campaigns" -> MainActivity.PAGE_DISCOVER
            else -> MainActivity.PAGE_DISCOVER
        }
        startActivity(
            Intent(this, MainActivity::class.java).apply {
                putExtra(MainActivity.EXTRA_TAB, target)
                if (tab in listOf("skins", "content", "campaigns")) {
                    putExtra(MainActivity.EXTRA_CHANNEL, tab)
                }
                addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            },
        )
        finish()
    }

    companion object {
        const val EXTRA_TAB = "tab"
    }
}
