package com.yc.input.phrase

import android.content.Context
import android.util.Log
import com.yc.input.llm.AiAssistResult
import com.yc.input.llm.AiAssistVariant
import com.yc.input.llm.LlmByokPrefs
import com.yc.input.llm.LlmProviderCatalog
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors

data class PhraseOptimizeRequest(
    val cardLabel: String,
    val cardText: String,
    val peerMessage: String = "",
    val intentHint: String = "",
)

/**
 * 行业话术专用 BYOK 路由：强制 scene_profile，可选知识库。
 */
object PhraseLlmRouter {
    private const val TAG = "PhraseLlm"
    private val executor = Executors.newSingleThreadExecutor()

    fun optimizeAsync(
        ctx: Context,
        req: PhraseOptimizeRequest,
        onDone: (AiAssistResult) -> Unit,
    ) {
        executor.execute {
            val result = try {
                optimize(ctx, req)
            } catch (e: Exception) {
                Log.w(TAG, "optimize failed", e)
                AiAssistResult(emptyList(), local = false, error = e.message ?: "请求失败")
            }
            android.os.Handler(android.os.Looper.getMainLooper()).post { onDone(result) }
        }
    }

    fun optimize(ctx: Context, req: PhraseOptimizeRequest): AiAssistResult {
        val scene = SceneProfileStore.current(ctx)
        val profiles = LlmProviderCatalog.load(ctx)
        val endpoint = LlmByokPrefs.resolveEndpoint(ctx, profiles)
            ?: return AiAssistResult(
                variants = listOf(
                    AiAssistVariant(req.cardText, "local"),
                    AiAssistVariant("（示意）" + req.cardText, "local"),
                ),
                local = true,
                error = "未配置 API Key，请到 App 设置 → AI 大模型",
            )

        val bucket = scene.industryId
        val rules = PhraseKbStore.ruleBlock(ctx, bucket)
        val refs = PhraseKbStore.retrieveRefs(
            ctx,
            bucket,
            "${req.cardLabel} ${req.cardText} ${req.peerMessage}",
        )

        val system = buildString {
            append("你是行业话术助手，只服务当前使用场景，输出 2～3 条可直接发送的短回复，每条一行，不要编号。\n")
            append(scene.systemAnchor()).append('\n')
            append("严格服务【使用场景】；禁止输出与场景无关的话术；不要编造未提供的具体数据、单号、价格或承诺时效。\n")
            if (rules.isNotBlank()) {
                append("【用户硬约束】\n").append(rules).append('\n')
            }
        }
        val user = buildString {
            append("【场景卡·").append(req.cardLabel).append("】\n").append(req.cardText).append("\n\n")
            if (req.intentHint.isNotBlank()) append("【意图】").append(req.intentHint).append("\n\n")
            if (req.peerMessage.isNotBlank()) {
                append("【对方消息】\n").append(req.peerMessage).append("\n\n")
            }
            if (refs.isNotBlank()) {
                append("【知识库参考】\n").append(refs).append("\n\n")
            }
            append("请基于模板改写出 2～3 条更贴合当前场景的变体。")
        }

        val body = JSONObject().apply {
            put("model", endpoint.model)
            put(
                "messages",
                JSONArray()
                    .put(JSONObject().put("role", "system").put("content", system))
                    .put(JSONObject().put("role", "user").put("content", user)),
            )
            put("temperature", 0.7)
            put("n", 1)
        }
        val url = URL(endpoint.baseUrl.trimEnd('/') + endpoint.apiPath)
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            connectTimeout = 20_000
            readTimeout = 60_000
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
                return AiAssistResult(emptyList(), local = false, error = "HTTP $code：${text.take(120)}")
            }
            return parseVariants(text)
        } finally {
            conn.disconnect()
        }
    }

    private fun parseVariants(raw: String): AiAssistResult {
        val obj = JSONObject(raw)
        val choices = obj.optJSONArray("choices")
            ?: return AiAssistResult(emptyList(), local = false, error = "响应无 choices")
        val variants = mutableListOf<AiAssistVariant>()
        for (i in 0 until choices.length()) {
            val content = choices.getJSONObject(i)
                .optJSONObject("message")
                ?.optString("content")
                ?.trim()
                .orEmpty()
            if (content.isEmpty()) continue
            val lines = content.lines()
                .map { stripIndex(it.trim()) }
                .filter { it.isNotEmpty() }
            if (lines.size >= 2) {
                lines.take(3).forEach { variants.add(AiAssistVariant(it)) }
            } else {
                variants.add(AiAssistVariant(stripIndex(content)))
            }
        }
        if (variants.isEmpty()) {
            return AiAssistResult(emptyList(), local = false, error = "空响应")
        }
        return AiAssistResult(variants.take(3), local = false)
    }

    private fun stripIndex(text: String): String {
        val t = text.trim()
        if (t.isEmpty()) return t
        val stripped = t
            .replace(Regex("""^[\(（]?\d+[\)）]?[\.．、\)）:\s]+"""), "")
            .replace(Regex("""^[①②③④⑤⑥⑦⑧⑨⑩]\s*"""), "")
            .trim()
        return stripped.ifEmpty { t }
    }
}
