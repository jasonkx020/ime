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

enum class AiAssistMode(val raw: Int, val label: String) {
    SmartReply(0, "智能回复"),
    HighEqReply(1, "高情商"),
    Compose(2, "撰写"),
    Rewrite(3, "改写"),
    Polish(4, "润色"),
    Translate(5, "翻译"),
}

data class AiAssistRequest(
    val mode: AiAssistMode,
    val selectionText: String = "",
    val peerMessage: String = "",
    val backgroundNote: String = "",
    val userIntent: String = "",
    val targetLang: String = "en",
    val sceneId: String = "",
)

data class AiAssistVariant(val text: String, val tone: String = "")

data class AiAssistResult(
    val variants: List<AiAssistVariant>,
    val local: Boolean,
    val error: String? = null,
)

/**
 * 统一 BYOK 路由：润色 / 高情商 / 翻译 / 智能回复等共用同一 endpoint。
 */
object LlmAssistRouter {
    private const val TAG = "LlmAssist"
    private val executor = Executors.newSingleThreadExecutor()

    fun suggestAsync(
        ctx: Context,
        req: AiAssistRequest,
        onDone: (AiAssistResult) -> Unit,
    ) {
        executor.execute {
            val result = try {
                suggest(ctx, req)
            } catch (e: Exception) {
                Log.w(TAG, "suggest failed", e)
                AiAssistResult(emptyList(), local = false, error = e.message ?: "请求失败")
            }
            android.os.Handler(android.os.Looper.getMainLooper()).post { onDone(result) }
        }
    }

    fun testConnection(ctx: Context): AiAssistResult {
        return suggest(
            ctx,
            AiAssistRequest(
                mode = AiAssistMode.Polish,
                selectionText = "你好",
                userIntent = "ping",
            ),
        )
    }

    fun suggest(ctx: Context, req: AiAssistRequest): AiAssistResult {
        val profiles = LlmProviderCatalog.load(ctx)
        val endpoint = LlmByokPrefs.resolveEndpoint(ctx, profiles)
            ?: return AiAssistResult(
                variants = localFallback(req),
                local = true,
                error = "未配置 API Key，请到 App 设置 → AI 大模型",
            )

        val (system, user) = buildPrompts(req)
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
                return AiAssistResult(
                    emptyList(),
                    local = false,
                    error = "HTTP $code：${text.take(120)}",
                )
            }
            return parseChatResponse(text)
        } finally {
            conn.disconnect()
        }
    }

    private fun parseChatResponse(raw: String): AiAssistResult {
        val obj = JSONObject(raw)
        val choices = obj.optJSONArray("choices") ?: return AiAssistResult(
            emptyList(),
            local = false,
            error = "响应无 choices",
        )
        val variants = mutableListOf<AiAssistVariant>()
        for (i in 0 until choices.length()) {
            val c = choices.getJSONObject(i)
            val msg = c.optJSONObject("message")
            val content = msg?.optString("content")?.trim().orEmpty()
            if (content.isNotEmpty()) {
                // 若模型用换行给出多条，拆开；去掉「1. / 1、」等序号再上屏
                val lines = content.lines()
                    .map { stripLeadingIndex(it.trim()) }
                    .filter { it.isNotEmpty() }
                if (lines.size >= 2 && lines.all { it.length < 200 }) {
                    lines.take(3).forEach { variants.add(AiAssistVariant(it)) }
                } else {
                    variants.add(AiAssistVariant(stripLeadingIndex(content)))
                }
            }
        }
        if (variants.isEmpty()) {
            return AiAssistResult(emptyList(), local = false, error = "空响应")
        }
        return AiAssistResult(variants.take(3), local = false)
    }

    private fun buildPrompts(req: AiAssistRequest): Pair<String, String> {
        val sceneHint = when (req.sceneId) {
            "dating" -> "场景：恋爱/亲密沟通。"
            "customer_followup" -> "场景：客户跟进或客服沟通。"
            "work_chat" -> "场景：职场沟通。"
            else -> ""
        }
        val system = when (req.mode) {
            AiAssistMode.SmartReply ->
                "你是输入法智能回复助手。$sceneHint 根据对方消息与背景给出 3 条得体短回复，每条一行，不要编号。"
            AiAssistMode.HighEqReply ->
                "你是高情商沟通助手。$sceneHint 语气真诚、留有余地。输出 3 条不同风格回复，每条一行，不要编号。"
            AiAssistMode.Compose ->
                "你是写作助手。$sceneHint 按用户意图撰写短文案，给出 3 个版本，每条一行，不要编号。"
            AiAssistMode.Rewrite ->
                "你是改写助手。保留原意，给出 3 种不同语气的改写，每条一行，不要编号。"
            AiAssistMode.Polish ->
                "你是中文润色助手。保留原意，使表达更清晰自然。可给 1～3 个版本，每条一行，不要编号。"
            AiAssistMode.Translate ->
                "你是翻译助手。将用户文本翻译成目标语言，自然流畅。可给 1～2 个译法，每条一行，不要编号。"
        }
        val user = buildString {
            when (req.mode) {
                AiAssistMode.Translate -> {
                    append("目标语言：").append(langName(req.targetLang)).append('\n')
                    append("原文：\n").append(req.selectionText.ifBlank { req.userIntent })
                }
                AiAssistMode.Polish, AiAssistMode.Rewrite -> {
                    append("原文：\n").append(req.selectionText.ifBlank { req.userIntent })
                }
                else -> {
                    if (req.backgroundNote.isNotBlank()) append("【背景】\n").append(req.backgroundNote).append("\n\n")
                    if (req.peerMessage.isNotBlank()) append("【对方消息】\n").append(req.peerMessage).append("\n\n")
                    else if (req.selectionText.isNotBlank()) append("【对方消息/草稿】\n").append(req.selectionText).append("\n\n")
                    if (req.userIntent.isNotBlank()) append("【意图】\n").append(req.userIntent)
                    if (isEmpty()) append("请打个简短招呼。")
                }
            }
        }
        return system to user
    }

    /** 去掉模型常加的列表序号，避免上屏带「1.」「1、」等前缀。 */
    private fun stripLeadingIndex(text: String): String {
        val t = text.trim()
        if (t.isEmpty()) return t
        // 1. / 1) / 1、 / 1． / (1) / （1） / ①～⑩
        val stripped = t
            .replace(Regex("""^[\(（]?\d+[\)）]?[\.．、\)）:\s]+"""), "")
            .replace(Regex("""^[①②③④⑤⑥⑦⑧⑨⑩]\s*"""), "")
            .trim()
        return stripped.ifEmpty { t }
    }

    private fun langName(code: String): String = when (code) {
        "zh" -> "中文"
        "en" -> "English"
        "vi" -> "Tiếng Việt"
        "th" -> "ไทย"
        else -> code.ifBlank { "English" }
    }

    private fun localFallback(req: AiAssistRequest): List<AiAssistVariant> {
        val seed = req.selectionText.ifBlank { req.peerMessage.ifBlank { req.userIntent.ifBlank { "你好" } } }
        return when (req.mode) {
            AiAssistMode.Translate -> listOf(AiAssistVariant("（本地示意）$seed", "local"))
            AiAssistMode.Polish -> listOf(
                AiAssistVariant(seed, "local"),
                AiAssistVariant("（润色示意）$seed", "local"),
            )
            else -> listOf(
                AiAssistVariant(seed, "local"),
                AiAssistVariant("好的，收到。", "local"),
                AiAssistVariant("理解你的意思：$seed", "local"),
            )
        }
    }
}
