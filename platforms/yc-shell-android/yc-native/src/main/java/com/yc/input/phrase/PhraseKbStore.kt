package com.yc.input.phrase

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

data class PhraseKbEntry(
    val id: String,
    val name: String,
    val role: String, // rule | ref | product
    val text: String,
)

/**
 * 用户行业知识库：本地文件，按 bucket（industryId）分桶。
 */
object PhraseKbStore {
    private const val MAX_TOTAL_CHARS = 200_000
    private const val MAX_SNIPPET_CHARS = 1_500

    fun bucketDir(ctx: Context, bucket: String): File {
        val dir = File(ctx.applicationContext.filesDir, "phrase_kb/${bucket.ifBlank { "custom" }}")
        if (!dir.exists()) dir.mkdirs()
        return dir
    }

    fun manifestFile(ctx: Context, bucket: String): File =
        File(bucketDir(ctx, bucket), "manifest.json")

    fun list(ctx: Context, bucket: String): List<PhraseKbEntry> {
        val mf = manifestFile(ctx, bucket)
        if (!mf.exists()) return emptyList()
        return try {
            val arr = JSONObject(mf.readText()).optJSONArray("entries") ?: return emptyList()
            val out = mutableListOf<PhraseKbEntry>()
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                out.add(
                    PhraseKbEntry(
                        id = o.optString("id"),
                        name = o.optString("name"),
                        role = o.optString("role", "ref"),
                        text = o.optString("text"),
                    ),
                )
            }
            out
        } catch (_: Exception) {
            emptyList()
        }
    }

    fun totalChars(ctx: Context, bucket: String): Int =
        list(ctx, bucket).sumOf { it.text.length }

    fun importText(
        ctx: Context,
        bucket: String,
        name: String,
        text: String,
        role: String = "ref",
    ): Boolean {
        val body = text.trim()
        if (body.isEmpty() || name.isBlank()) return false
        if (totalChars(ctx, bucket) + body.length > MAX_TOTAL_CHARS) return false
        val id = "kb_${System.currentTimeMillis()}"
        val entries = list(ctx, bucket).toMutableList()
        entries.add(PhraseKbEntry(id, name.trim(), role, body))
        persist(ctx, bucket, entries)
        return true
    }

    fun delete(ctx: Context, bucket: String, id: String) {
        persist(ctx, bucket, list(ctx, bucket).filter { it.id != id })
    }

    /** 约束类全文（截断）拼进 system。 */
    fun ruleBlock(ctx: Context, bucket: String): String {
        val rules = list(ctx, bucket).filter { it.role == "rule" }
        if (rules.isEmpty()) return ""
        return rules.joinToString("\n") { "· ${it.name}：${it.text.take(800)}" }
            .take(2_000)
    }

    /** 简单字面重叠检索参考片段。 */
    fun retrieveRefs(ctx: Context, bucket: String, query: String): String {
        val refs = list(ctx, bucket).filter { it.role != "rule" }
        if (refs.isEmpty()) return ""
        val q = query.lowercase()
        val scored = refs.map { e ->
            val t = e.text.lowercase()
            var score = 0
            q.split(Regex("\\s+|，|。|、")).filter { it.length >= 2 }.forEach { w ->
                if (t.contains(w)) score += w.length
            }
            if (score == 0) score = 1
            e to score
        }.sortedByDescending { it.second }
        val sb = StringBuilder()
        for ((e, _) in scored) {
            if (sb.length >= MAX_SNIPPET_CHARS) break
            val chunk = "【${e.name}】\n${e.text.take(600)}\n"
            if (sb.length + chunk.length > MAX_SNIPPET_CHARS) break
            sb.append(chunk)
        }
        return sb.toString().trim()
    }

    private fun persist(ctx: Context, bucket: String, entries: List<PhraseKbEntry>) {
        val arr = JSONArray()
        entries.forEach { e ->
            arr.put(
                JSONObject()
                    .put("id", e.id)
                    .put("name", e.name)
                    .put("role", e.role)
                    .put("text", e.text),
            )
        }
        manifestFile(ctx, bucket).writeText(JSONObject().put("entries", arr).toString())
    }
}
