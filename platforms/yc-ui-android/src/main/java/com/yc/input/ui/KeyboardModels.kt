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
    /** 引擎当前页内的页码，展开多页列表选词时用于先翻到对应页再 SELECT */
    val page: Int = 0,
)

enum class KeyStyle { Normal, Utility, Accent }

data class KeyDef(
    val label: String,
    val widthWeight: Float = 1f,
    val style: KeyStyle = KeyStyle.Normal,
    val keyCode: Int? = null,
    val action: KeyAction = KeyAction.Letter,
)

enum class KeyAction {
    Letter,
    Backspace,
    Space,
    Search,
    Symbol,
    Globe,
    Shift,
}

/**
 * 美式完整 QWERTY（拼音 26 键默认布局）：
 * 数字行 + QWERTY + ASDF + Shift/ZXCV/⌫ + 底栏。
 */
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
                KeyDef(
                    label = if (shifted) "⇧" else "⇧",
                    widthWeight = 1.4f,
                    style = if (shifted) KeyStyle.Accent else KeyStyle.Utility,
                    action = KeyAction.Shift,
                ),
            ) + z + listOf(
                KeyDef("⌫", 1.4f, KeyStyle.Utility, action = KeyAction.Backspace),
            ),
            listOf(
                KeyDef("!#1", 1.15f, KeyStyle.Utility, action = KeyAction.Symbol),
                KeyDef("🌐", 1.15f, KeyStyle.Utility, action = KeyAction.Globe),
                KeyDef(",", 0.9f, KeyStyle.Normal, keyCode = ','.code),
                KeyDef("空格", 3.4f, KeyStyle.Normal, keyCode = ' '.code, action = KeyAction.Space),
                KeyDef(".", 0.9f, KeyStyle.Normal, keyCode = '.'.code),
                KeyDef("搜索", 1.4f, KeyStyle.Accent, action = KeyAction.Search),
            ),
        )
    }

    private fun letter(ch: String, shifted: Boolean): KeyDef {
        val out = if (shifted && ch.length == 1 && ch[0].isLetter()) ch.uppercase() else ch
        return KeyDef(out, 1f, KeyStyle.Normal, keyCode = out[0].code, action = KeyAction.Letter)
    }
}
