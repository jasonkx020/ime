package com.yc.input

import android.app.Activity
import android.os.Bundle
import android.widget.TextView
import com.yc.input.native.YcNative
import java.io.File

class MainActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val msg = StringBuilder("YC Input (M3/M3.5/P3)\n")
        val dataDir = filesDir.apply { mkdirs() }

        YcNative.ycCoreInit(dataDir.absolutePath)

        installLangPack(dataDir, msg, "vi-v1.imepack")
        installLangPack(dataDir, msg, "th-v1.imepack")
        installLangPack(dataDir, msg, "zh-pack-v1.imepack", enable = true)

        setContentView(
            TextView(this).apply {
                text = msg.toString()
                textSize = 16f
                setPadding(48, 48, 48, 48)
            },
        )
    }

    private fun installLangPack(
        dataDir: File,
        msg: StringBuilder,
        fileName: String,
        enable: Boolean = false,
    ) {
        val assetPath = "langpacks/$fileName"
        val out = File(dataDir, fileName)
        try {
            assets.open(assetPath).use { input ->
                out.outputStream().use { output -> input.copyTo(output) }
            }
            if (enable) {
                // Sync path: install+enable+scheduler register (avoids cold async race).
                val rc = YcNative.ycCoreInstallLangpack(out.absolutePath)
                msg.append("install+enable $fileName: rc=$rc\n")
            } else {
                val rc = YcNative.coldSubmit(
                    editorId = 0,
                    kind = 1, // YC_COLD_LANGPACK_INSTALL
                    payload = out.absolutePath.toByteArray(),
                )
                msg.append("install $fileName: rc=$rc (queued)\n")
            }
        } catch (_: Exception) {
            msg.append("$fileName 未打包进 assets；运行 scripts/build-all.ps1\n")
        }
    }
}
