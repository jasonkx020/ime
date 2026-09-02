package com.yc.input.ui

data class KeyboardSnapshot(
    val editorId: Long,
    val seq: Long,
    val composing: String,
    val candidates: List<CandidateItem>,
)

data class CandidateItem(val id: Int, val text: String)

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
}

object Layout26Pinyin {
    val rows: List<List<KeyDef>> = listOf(
        listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p").map { letter(it) },
        listOf("a", "s", "d", "f", "g", "h", "j", "k", "l").map { letter(it) } +
            listOf(KeyDef("⌫", 1.35f, KeyStyle.Utility, action = KeyAction.Backspace)),
        listOf("z", "x", "c", "v", "b", "n", "m").map { letter(it) },
        listOf(
            KeyDef("!#1", 1.1f, KeyStyle.Utility, action = KeyAction.Symbol),
            KeyDef("🌐", 1.1f, KeyStyle.Utility, action = KeyAction.Globe),
            KeyDef(",", 0.85f, KeyStyle.Normal, keyCode = ','.code),
            KeyDef("空格", 3.6f, KeyStyle.Normal, keyCode = ' '.code, action = KeyAction.Space),
            KeyDef("。", 0.85f, KeyStyle.Normal, keyCode = '.'.code),
            KeyDef("搜索", 1.4f, KeyStyle.Accent, action = KeyAction.Search),
        ),
    )

    private fun letter(ch: String): KeyDef =
        KeyDef(ch, 1f, KeyStyle.Normal, keyCode = ch[0].code)
}
