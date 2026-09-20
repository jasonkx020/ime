package com.yc.input

import android.content.Context
import android.util.Log
import java.io.File

/**
 * 将 ContentPack 行业词合入 `user_words.tsv`（lang\\tquery_key\\tword\\tfreq），
 * 供 UserWordStore 抬权；热路径不加载贴纸。
 */
object IndustryPackInstaller {
    private const val TAG = "IndustryPack"

    fun enable(context: Context, packId: String): Boolean {
        context.getSharedPreferences("yc_content", Context.MODE_PRIVATE)
            .edit()
            .putString("active_industry", packId)
            .apply()
        return mergeLexicon(context, packId)
    }

    private fun mergeLexicon(context: Context, packId: String): Boolean {
        val assetPath = "content/$packId/lexicon/words.tsv"
        val out = File(context.filesDir, "user_words.tsv")
        return try {
            val lines = context.assets.open(assetPath).bufferedReader().readLines()
            val existing = if (out.exists()) out.readText() else "lang\tquery_key\tword\tfreq\n"
            val builder = StringBuilder(existing.trimEnd()).append('\n')
            var n = 0
            for (line in lines) {
                val t = line.trim()
                if (t.isEmpty() || t.startsWith("word")) continue
                val parts = t.split('\t')
                if (parts.size < 3) continue
                val word = parts[0].trim()
                val freq = parts.getOrNull(1)?.toIntOrNull() ?: 1000
                val pinyin = parts.getOrNull(2)?.trim()?.lowercase().orEmpty()
                if (word.isEmpty() || pinyin.isEmpty()) continue
                val row = "zh\t$pinyin\t$word\t$freq"
                if (!existing.contains("\t$word\t") && !builder.contains(row)) {
                    builder.append(row).append('\n')
                    n++
                }
            }
            out.writeText(builder.toString())
            Log.i(TAG, "merged $n industry words from $packId -> ${out.absolutePath}")
            true
        } catch (e: Exception) {
            Log.w(TAG, "merge lexicon $packId failed", e)
            false
        }
    }
}
