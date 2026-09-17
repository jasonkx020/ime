package com.yc.input.ui

data class KeyboardSnapshot(
    val editorId: Long,
    val seq: Long,
    val composing: String,
    val candidates: List<CandidateItem>,
    val candPage: Int = 0,
    val totalPages: Int = 0,
    val expanded: Boolean = false,
    val asciiMode: Boolean = false,
)

data class CandidateItem(
    val id: Int,
    val text: String,
    val page: Int = 0,
)

enum class KeyStyle { Normal, Utility, Accent }

enum class ShiftState { Off, Once, Locked }

data class KeyDef(
    val label: String,
    val widthWeight: Float = 1f,
    val style: KeyStyle = KeyStyle.Normal,
    val keyCode: Int? = null,
    val action: KeyAction = KeyAction.Letter,
    val output: String? = null,
    /** 中文标点；非空时随语言切换标签 */
    val punctZh: String? = null,
    val punctEn: String? = null,
)

enum class KeyAction {
    Letter,
    Backspace,
    Space,
    Search,
    Symbol,
    Globe,
    Shift,
    Tone,
    Mic,
    Letters, // 从符号层返回字母层
}

data class LangOption(
    val code: String,
    val name: String,
    val native: String,
    val packId: String?,
    val ascii: Boolean = false,
)

data class ModeOption(
    val id: String,
    val name: String,
    val icon: String,
)

object Layout26Pinyin {
    private val numberUnshifted = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")
    private val numberShifted = listOf("!", "@", "#", "$", "%", "^", "&", "*", "(", ")")
    private val rowQ = listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p")
    private val rowA = listOf("a", "s", "d", "f", "g", "h", "j", "k", "l")
    private val rowZ = listOf("z", "x", "c", "v", "b", "n", "m")

    val rows: List<List<KeyDef>> = rows(shifted = false)

    fun rows(shifted: Boolean): List<List<KeyDef>> {
        val nums = if (shifted) numberShifted else numberUnshifted
        val q = rowQ.map { letter(it, shifted) }
        val a = rowA.map { letter(it, shifted) }
        val z = rowZ.map { letter(it, shifted) }
        return listOf(
            nums.map { letter(it, shifted = false) },
            q,
            a,
            listOf(
                KeyDef("⇧", 1.4f, KeyStyle.Utility, action = KeyAction.Shift),
            ) + z + listOf(
                KeyDef("⌫", 1.4f, KeyStyle.Utility, action = KeyAction.Backspace),
            ),
            listOf(
                KeyDef("123", 1.15f, KeyStyle.Utility, action = KeyAction.Symbol),
                KeyDef("🌐", 1.15f, KeyStyle.Utility, action = KeyAction.Globe),
                KeyDef("🎤", 1.1f, KeyStyle.Utility, action = KeyAction.Mic),
                KeyDef("空格", 3.4f, KeyStyle.Normal, keyCode = ' '.code, action = KeyAction.Space),
                KeyDef(
                    "。", 0.9f, KeyStyle.Normal, keyCode = '。'.code,
                    punctZh = "。", punctEn = ".",
                ),
                KeyDef("回车", 1.4f, KeyStyle.Utility, action = KeyAction.Search),
            ),
        )
    }

    private fun letter(ch: String, shifted: Boolean): KeyDef {
        val out = if (shifted && ch.length == 1 && ch[0].isLetter()) ch.uppercase() else ch
        return KeyDef(out, 1f, KeyStyle.Normal, keyCode = out[0].code, action = KeyAction.Letter)
    }
}

/** 数字/符号层（对齐参考效果 layerNumbers）；本地 fallback。 */
object LayoutSymbol {
    fun rows(useZhPunct: Boolean, backLabel: String = "ABC"): List<List<KeyDef>> {
        fun punct(zh: String, en: String) = KeyDef(
            label = if (useZhPunct) zh else en,
            widthWeight = 1f,
            keyCode = (if (useZhPunct) zh else en).first().code,
            action = KeyAction.Letter,
            output = if (useZhPunct) zh else en,
            punctZh = zh,
            punctEn = en,
        )
        fun k(ch: String, w: Float = 1f) = KeyDef(
            ch, w, KeyStyle.Normal, keyCode = ch.firstOrNull()?.code, action = KeyAction.Letter, output = ch,
        )
        return listOf(
            listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0").map { k(it) },
            listOf(
                k("+"), k("×"), k("÷"), k("="), k("/"), k("_"), k("<"), k(">"),
                punct("【", "["), punct("】", "]"),
            ),
            listOf(
                punct("！", "!"), k("@"), k("#"), punct("￥", "$"), k("%"), k("^"), k("&"), k("*"),
                punct("（", "("), punct("）", ")"),
            ),
            listOf(
                KeyDef("1/2", 1.15f, KeyStyle.Utility, action = KeyAction.Letter, output = "half"),
                k("-"),
                punct("“", "\""), punct("”", "'"),
                punct("：", ":"), punct("；", ";"),
                punct("，", ","), punct("？", "?"),
                KeyDef("⌫", 1.4f, KeyStyle.Utility, action = KeyAction.Backspace),
            ),
            listOf(
                KeyDef(backLabel, 1.3f, KeyStyle.Utility, action = KeyAction.Letters),
                KeyDef("🌐", 1.15f, KeyStyle.Utility, action = KeyAction.Globe),
                punct("，", ","),
                KeyDef("空格", 3.2f, KeyStyle.Normal, keyCode = ' '.code, action = KeyAction.Space),
                punct("。", "."),
                KeyDef("下一步", 1.4f, KeyStyle.Accent, action = KeyAction.Search),
            ),
        )
    }
}

object LayoutCaseShift {
    private val viUpper = mapOf(
        'ă' to 'Ă', 'â' to 'Â', 'ê' to 'Ê', 'ô' to 'Ô', 'ơ' to 'Ơ', 'ư' to 'Ư', 'đ' to 'Đ',
        'á' to 'Á', 'à' to 'À', 'ả' to 'Ả', 'ã' to 'Ã', 'ạ' to 'Ạ',
    )
    private val viLower = viUpper.entries.associate { (k, v) -> v to k }

    fun apply(rows: List<List<KeyDef>>, shifted: Boolean): List<List<KeyDef>> =
        rows.map { row ->
            row.map { key ->
                when {
                    key.action == KeyAction.Shift -> key
                    key.action == KeyAction.Tone -> key
                    key.action != KeyAction.Letter -> key
                    else -> shiftLetter(key, shifted)
                }
            }
        }

    fun applyPunctuation(rows: List<List<KeyDef>>, useZh: Boolean): List<List<KeyDef>> =
        rows.map { row ->
            row.map { key ->
                val zh = key.punctZh
                val en = key.punctEn
                if (zh != null && en != null) {
                    val label = if (useZh) zh else en
                    key.copy(label = label, output = label, keyCode = label.firstOrNull()?.code)
                } else {
                    key
                }
            }
        }

    private fun shiftLetter(key: KeyDef, shifted: Boolean): KeyDef {
        val src = key.output ?: key.label
        if (src.isEmpty() || src.startsWith("tone:")) return key
        val mapped = src.map { ch ->
            when {
                shifted && ch in viUpper -> viUpper.getValue(ch)
                !shifted && ch in viLower -> viLower.getValue(ch)
                shifted && ch.isLowerCase() -> ch.uppercaseChar()
                !shifted && ch.isUpperCase() && ch !in viUpper.values -> ch.lowercaseChar()
                else -> ch
            }
        }.joinToString("")
        val code = mapped.firstOrNull()?.code
        return key.copy(label = mapped, output = mapped, keyCode = code)
    }
}
