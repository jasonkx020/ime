package com.yc.input.llm

import android.content.Context
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

data class HabitPersonaPack(
    val personaTags: List<String> = emptyList(),
    val preferPairs: JSONArray = JSONArray(),
    val demote: JSONArray = JSONArray(),
    val boosts: JSONArray = JSONArray(),
    val local: Boolean = false,
    val error: String? = null,
) {
    fun pairsJson(): String = preferPairs.toString()
    fun deltasJson(): String {
        // demote (negative) + boosts as positive score deltas for LightIntel
        val out = JSONArray()
        for (i in 0 until demote.length()) {
            out.put(demote.getJSONObject(i))
        }
        for (i in 0 until boosts.length()) {
            val b = boosts.getJSONObject(i)
            val delta = JSONObject()
                .put("query_key", b.optString("query_key"))
                .put("word", b.optString("word"))
                .put("boost", b.optDouble("boost", 1.0).coerceIn(0.1, 5.0))
            out.put(delta)
        }
        return out.toString()
    }

    fun boostsJson(): String {
        val out = JSONArray()
        for (i in 0 until boosts.length()) {
            val b = boosts.getJSONObject(i)
            val freq = when {
                b.has("freq") -> b.optInt("freq", 2)
                else -> (b.optDouble("boost", 1.0) * 2).toInt().coerceIn(2, 50)
            }
            out.put(
                JSONObject()
                    .put("lang", b.optString("lang"))
                    .put("query_key", b.optString("query_key"))
                    .put("word", b.optString("word"))
                    .put("freq", freq),
            )
        }
        return out.toString()
    }

    fun summary(): String {
        val tags = personaTags.take(3).joinToString(",")
        return "tags=$tags pairs=${preferPairs.length()} demote=${demote.length()} boosts=${boosts.length()}"
    }
}

data class HabitSnapshot(
    val lang: String,
    val selectCount: Int,
    val avgPos: Double,
    val topWords: List<Triple<String, String, Int>>, // query_key, word, count
    val topKeys: List<Pair<String, Int>>,
) {
    fun allowedKeys(): Set<String> = topKeys.map { it.first }.toSet() +
        topWords.map { it.first }.toSet()

    fun allowedWords(): Set<String> = topWords.map { it.second }.toSet()
}

/**
 * 选词画像 BYOK：输出 PersonalizationPack 子集 JSON；无 Key / 失败走规则兜底。
 */
object HabitPersonaLlmRouter {
    private const val TAG = "HabitPersonaLlm"

    fun analyze(ctx: Context, snap: HabitSnapshot): HabitPersonaPack {
        val profiles = LlmProviderCatalog.load(ctx)
        val endpoint = LlmByokPrefs.resolveEndpoint(ctx, profiles)
        if (endpoint == null) {
            return ruleFallback(snap).copy(local = true, error = "未配置 API Key")
        }
        return try {
            val pack = callLlm(endpoint, snap)
            sanitize(pack, snap)
        } catch (e: Exception) {
            Log.w(TAG, "analyze failed", e)
            ruleFallback(snap).copy(local = true, error = e.message)
        }
    }

    fun buildSnapshot(ctx: Context, langHint: String): HabitSnapshot {
        val events = HabitJournal.readRecent(ctx, 800)
        val filtered = if (langHint.isNotBlank()) {
            events.filter { it.lang == langHint || it.lang.isBlank() }
        } else {
            events
        }
        val wordCount = linkedMapOf<String, Int>()
        val keyCount = linkedMapOf<String, Int>()
        var posSum = 0.0
        var posN = 0
        for (e in filtered) {
            val qk = e.queryKey.lowercase()
            val w = e.selectedWord
            if (qk.isBlank() || w.isBlank()) continue
            val wk = "$qk\t$w"
            wordCount[wk] = (wordCount[wk] ?: 0) + 1
            keyCount[qk] = (keyCount[qk] ?: 0) + 1
            posSum += e.candidatePos
            posN++
        }
        // Merge light stats from user_words.tsv (read-only, cold path)
        mergeUserWordsFile(ctx, langHint, wordCount, keyCount)

        val topWords = wordCount.entries
            .sortedByDescending { it.value }
            .take(40)
            .map {
                val parts = it.key.split('\t', limit = 2)
                Triple(parts.getOrElse(0) { "" }, parts.getOrElse(1) { "" }, it.value)
            }
        val topKeys = keyCount.entries
            .sortedByDescending { it.value }
            .take(30)
            .map { it.key to it.value }
        val lang = langHint.ifBlank {
            filtered.map { it.lang }.firstOrNull { it.isNotBlank() } ?: "zh"
        }
        return HabitSnapshot(
            lang = lang,
            selectCount = filtered.size.coerceAtLeast(wordCount.values.sum()),
            avgPos = if (posN > 0) posSum / posN else 0.0,
            topWords = topWords,
            topKeys = topKeys,
        )
    }

    private fun mergeUserWordsFile(
        ctx: Context,
        langHint: String,
        wordCount: MutableMap<String, Int>,
        keyCount: MutableMap<String, Int>,
    ) {
        val file = File(ctx.filesDir, "user_words.tsv")
        if (!file.exists()) return
        try {
            file.bufferedReader().useLines { lines ->
                lines.forEach { line ->
                    val t = line.trim()
                    if (t.isEmpty() || t.startsWith("lang") || t.startsWith("pinyin") || t.startsWith("query_key")) {
                        return@forEach
                    }
                    val p = t.split('\t')
                    if (p.size < 4) return@forEach
                    val lang = p[0].trim().lowercase()
                    if (langHint.isNotBlank() && lang.isNotBlank() && lang != langHint) return@forEach
                    val qk = p[1].trim().lowercase()
                    val word = p[2].trim()
                    val freq = p[3].toIntOrNull() ?: 1
                    if (qk.isEmpty() || word.isEmpty()) return@forEach
                    val wk = "$qk\t$word"
                    wordCount[wk] = maxOf(wordCount[wk] ?: 0, freq)
                    keyCount[qk] = maxOf(keyCount[qk] ?: 0, freq)
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "mergeUserWordsFile", e)
        }
    }

    private fun callLlm(endpoint: LlmEndpoint, snap: HabitSnapshot): HabitPersonaPack {
        val system = """
            你是输入法选词个性化助手。根据用户选词统计，输出严格 JSON（不要 markdown）：
            {"persona_tags":[],"prefer_pairs":[{"prev":"","next":"","delta":1.0}],"demote":[{"query_key":"","word":"","boost":-1.0}],"boosts":[{"lang":"","query_key":"","word":"","boost":1.5,"freq":3}]}
            规则：只使用输入里出现过的 query_key/word；pairs/demote/boosts 各最多 40 条；delta/boost 在 -5~5；不要编造。
        """.trimIndent()
        val user = buildString {
            append("lang=").append(snap.lang)
            append(" selects=").append(snap.selectCount)
            append(" avg_pos=").append("%.2f".format(snap.avgPos)).append('\n')
            append("top_words:\n")
            snap.topWords.take(30).forEach { (qk, w, c) ->
                append("- ").append(qk).append('\t').append(w).append('\t').append(c).append('\n')
            }
            append("top_keys:\n")
            snap.topKeys.take(20).forEach { (k, c) ->
                append("- ").append(k).append('\t').append(c).append('\n')
            }
        }
        val body = JSONObject().apply {
            put("model", endpoint.model)
            put(
                "messages",
                JSONArray()
                    .put(JSONObject().put("role", "system").put("content", system))
                    .put(JSONObject().put("role", "user").put("content", user)),
            )
            put("temperature", 0.2)
        }
        val url = URL(endpoint.baseUrl.trimEnd('/') + endpoint.apiPath)
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            connectTimeout = 15_000
            readTimeout = 45_000
            doOutput = true
            setRequestProperty("Content-Type", "application/json")
            if (endpoint.apiKey.isNotBlank() &&
                (endpoint.auth == "bearer" || endpoint.auth == "bearer_optional")
            ) {
                setRequestProperty("Authorization", "Bearer ${endpoint.apiKey}")
            }
        }
        try {
            OutputStreamWriter(conn.outputStream, Charsets.UTF_8).use { it.write(body.toString()) }
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val text = BufferedReader(InputStreamReader(stream, Charsets.UTF_8)).readText()
            if (code !in 200..299) {
                return ruleFallback(snap).copy(local = true, error = "HTTP $code")
            }
            return parsePackFromChat(text) ?: ruleFallback(snap).copy(local = true, error = "parse")
        } finally {
            conn.disconnect()
        }
    }

    private fun parsePackFromChat(raw: String): HabitPersonaPack? {
        val obj = JSONObject(raw)
        val choices = obj.optJSONArray("choices") ?: return null
        val content = choices.optJSONObject(0)
            ?.optJSONObject("message")
            ?.optString("content")
            ?.trim()
            .orEmpty()
        if (content.isEmpty()) return null
        val jsonText = extractJsonObject(content) ?: return null
        val pack = JSONObject(jsonText)
        val tags = mutableListOf<String>()
        val tagsArr = pack.optJSONArray("persona_tags")
        if (tagsArr != null) {
            for (i in 0 until tagsArr.length()) {
                tags.add(tagsArr.optString(i))
            }
        }
        return HabitPersonaPack(
            personaTags = tags,
            preferPairs = pack.optJSONArray("prefer_pairs") ?: JSONArray(),
            demote = pack.optJSONArray("demote") ?: JSONArray(),
            boosts = pack.optJSONArray("boosts") ?: JSONArray(),
            local = false,
        )
    }

    private fun extractJsonObject(text: String): String? {
        val start = text.indexOf('{')
        val end = text.lastIndexOf('}')
        if (start < 0 || end <= start) return null
        return text.substring(start, end + 1)
    }

    private fun sanitize(pack: HabitPersonaPack, snap: HabitSnapshot): HabitPersonaPack {
        val keys = snap.allowedKeys()
        val words = snap.allowedWords()
        val pairs = JSONArray()
        for (i in 0 until pack.preferPairs.length().coerceAtMost(40)) {
            val o = pack.preferPairs.optJSONObject(i) ?: continue
            val prev = o.optString("prev")
            val next = o.optString("next")
            if (prev.isBlank() || next.isBlank()) continue
            if (words.isNotEmpty() && next !in words && prev !in words) continue
            val delta = o.optDouble("delta", 1.0).coerceIn(-5.0, 5.0)
            pairs.put(JSONObject().put("prev", prev).put("next", next).put("delta", delta))
        }
        val demote = JSONArray()
        for (i in 0 until pack.demote.length().coerceAtMost(40)) {
            val o = pack.demote.optJSONObject(i) ?: continue
            val qk = o.optString("query_key").lowercase()
            val word = o.optString("word")
            if (qk.isBlank() || word.isBlank()) continue
            if (keys.isNotEmpty() && qk !in keys) continue
            if (words.isNotEmpty() && word !in words) continue
            val boost = o.optDouble("boost", -1.0).coerceIn(-5.0, -0.1)
            demote.put(JSONObject().put("query_key", qk).put("word", word).put("boost", boost))
        }
        val boosts = JSONArray()
        for (i in 0 until pack.boosts.length().coerceAtMost(40)) {
            val o = pack.boosts.optJSONObject(i) ?: continue
            val qk = o.optString("query_key").lowercase()
            val word = o.optString("word")
            if (qk.isBlank() || word.isBlank()) continue
            if (keys.isNotEmpty() && qk !in keys) continue
            if (words.isNotEmpty() && word !in words) continue
            val boost = o.optDouble("boost", 1.0).coerceIn(0.1, 5.0)
            val freq = o.optInt("freq", (boost * 2).toInt().coerceIn(2, 50))
            boosts.put(
                JSONObject()
                    .put("lang", o.optString("lang", snap.lang))
                    .put("query_key", qk)
                    .put("word", word)
                    .put("boost", boost)
                    .put("freq", freq),
            )
        }
        val tags = pack.personaTags.toMutableList()
        if (tags.none { it.startsWith("lang_") }) tags.add("lang_${snap.lang}")
        if (snap.selectCount < 20) tags.add("new_user")
        if (snap.avgPos >= 3) tags.add("needs_rerank")
        return HabitPersonaPack(
            personaTags = tags.distinct().take(12),
            preferPairs = pairs,
            demote = demote,
            boosts = boosts,
            local = pack.local,
            error = pack.error,
        )
    }

    /** 移植 HabitSummarizer 规则兜底。 */
    fun ruleFallback(snap: HabitSnapshot): HabitPersonaPack {
        val tags = mutableListOf("lang_${snap.lang}")
        when {
            snap.selectCount < 20 -> tags.add("new_user")
            snap.selectCount > 500 -> tags.add("power_user")
        }
        if (snap.avgPos >= 3) tags.add("needs_rerank")

        val byKey = snap.topWords.groupBy { it.first }
        val demote = JSONArray()
        for ((key, rows) in byKey) {
            if (rows.size < 2 || key.isBlank()) continue
            val best = rows.maxByOrNull { it.third } ?: continue
            for (r in rows) {
                if (r.second == best.second) continue
                if (r.third * 2 >= best.third) continue
                demote.put(
                    JSONObject()
                        .put("query_key", key)
                        .put("word", r.second)
                        .put("boost", -1.0),
                )
                if (demote.length() >= 40) break
            }
            if (demote.length() >= 40) break
        }

        val pairs = JSONArray()
        fun addPair(prev: String, next: String, delta: Double) {
            if (prev.isBlank() || next.isBlank() || pairs.length() >= 40) return
            pairs.put(JSONObject().put("prev", prev).put("next", next).put("delta", delta))
        }
        when (snap.lang) {
            "en" -> {
                addPair("thank", "you", 1.5)
                addPair("good", "morning", 1.2)
            }
            "zh" -> {
                addPair("你", "好", 1.5)
                addPair("我", "们", 1.2)
            }
            "vi" -> {
                addPair("xin", "chào", 1.5)
            }
            "th" -> {
                addPair("สวัสดี", "ครับ", 1.2)
            }
        }
        for ((_, word, _) in snap.topWords) {
            val parts = word.split(Regex("\\s+"))
            if (parts.size == 2) addPair(parts[0], parts[1], 1.0)
        }

        val boosts = JSONArray()
        for ((qk, word, count) in snap.topWords.take(20)) {
            if (count < 2) continue
            boosts.put(
                JSONObject()
                    .put("lang", snap.lang)
                    .put("query_key", qk)
                    .put("word", word)
                    .put("boost", (1.0 + count / 10.0).coerceAtMost(3.0))
                    .put("freq", count.coerceIn(2, 100)),
            )
        }
        return HabitPersonaPack(
            personaTags = tags.distinct(),
            preferPairs = pairs,
            demote = demote,
            boosts = boosts,
            local = true,
        )
    }
}
