package com.yc.input.llm

import android.content.Context
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicLong

/**
 * BYOK pinyin candidate top-up when the local lexicon pool is empty / thin / at last page.
 * Never blocks the keypress path; results are injected via [YcNative.ycHotInjectAiCandidates].
 */
object PinyinLlmFallbackRouter {
    private const val TAG = "PinyinLlmFallback"
    private val executor = Executors.newSingleThreadExecutor()
    private val generation = AtomicLong(0)

    fun cancel() {
        generation.incrementAndGet()
    }

    /**
     * @param query current pinyin composing (transformed)
     * @param existing already-shown candidate texts (avoid duplicates)
     * @param onDone main-thread callback with Chinese phrases; empty on skip/error
     */
    fun requestAsync(
        ctx: Context,
        query: String,
        existing: List<String>,
        onDone: (List<String>) -> Unit,
    ) {
        val q = query.trim().lowercase()
        if (q.length < 2) {
            onDone(emptyList())
            return
        }
        val gen = generation.incrementAndGet()
        executor.execute {
            val texts = try {
                request(ctx, q, existing)
            } catch (e: Exception) {
                Log.w(TAG, "request failed q=$q", e)
                emptyList()
            }
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                if (gen != generation.get()) {
                    return@post
                }
                onDone(texts)
            }
        }
    }

    fun request(ctx: Context, query: String, existing: List<String>): List<String> {
        val profiles = LlmProviderCatalog.load(ctx)
        val endpoint = LlmByokPrefs.resolveEndpoint(ctx, profiles) ?: return emptyList()
        if (endpoint.apiKey.isBlank() && endpoint.auth == "bearer") {
            return emptyList()
        }

        val need = 5
        val existingHint = existing.take(12).joinToString("、")
        val system =
            "你是中文输入法候选补全助手。根据用户输入的拼音，给出简体中文词语或短短语（优先多字词，少给单字）。" +
                "只输出一个 JSON 字符串数组，最多 5 个，例如 [\"你好\",\"您好\"]，不要解释、不要 markdown。"
        val user = buildString {
            append("拼音：").append(query).append('\n')
            if (existingHint.isNotEmpty()) {
                append("已有候选（勿重复）：").append(existingHint).append('\n')
            }
            append("请再给出 ").append(need).append(" 个合适的简体词语候选。")
        }

        val body = JSONObject().apply {
            put("model", endpoint.model)
            put(
                "messages",
                JSONArray()
                    .put(JSONObject().put("role", "system").put("content", system))
                    .put(JSONObject().put("role", "user").put("content", user)),
            )
            put("temperature", 0.4)
            put("n", 1)
        }

        val url = URL(endpoint.baseUrl.trimEnd('/') + endpoint.apiPath)
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            connectTimeout = 12_000
            readTimeout = 25_000
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
                Log.w(TAG, "HTTP $code ${text.take(80)}")
                return emptyList()
            }
            return parseCandidates(text, existing.toSet()).take(5)
        } finally {
            conn.disconnect()
        }
    }

    private fun parseCandidates(raw: String, exclude: Set<String>): List<String> {
        val obj = JSONObject(raw)
        val choices = obj.optJSONArray("choices") ?: return emptyList()
        val content = choices.optJSONObject(0)
            ?.optJSONObject("message")
            ?.optString("content")
            ?.trim()
            .orEmpty()
        if (content.isEmpty()) return emptyList()

        val jsonSlice = extractJsonArray(content) ?: content
        val out = mutableListOf<String>()
        try {
            val arr = JSONArray(jsonSlice)
            for (i in 0 until arr.length()) {
                val t = arr.optString(i).trim()
                if (t.isNotEmpty() && t !in exclude && t.length <= 32) {
                    out.add(t)
                }
            }
        } catch (_: Exception) {
            content.lines()
                .map { it.trim().removePrefix("-").removePrefix("•").trim() }
                .map { it.replace(Regex("""^[\(（]?\d+[\)）]?[\.．、\)）:\s]+"""), "").trim() }
                .filter { it.isNotEmpty() && it !in exclude && it.length <= 32 }
                .take(5)
                .forEach { out.add(it) }
        }
        return out
    }

    private fun extractJsonArray(content: String): String? {
        val start = content.indexOf('[')
        val end = content.lastIndexOf(']')
        if (start < 0 || end <= start) return null
        return content.substring(start, end + 1)
    }
}
