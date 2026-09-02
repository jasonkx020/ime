#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputScheme {
    PinyinFull = 0,
    Qwerty = 1,
    Handwriting = 2,
}

impl InputScheme {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::PinyinFull),
            1 => Some(Self::Qwerty),
            2 => Some(Self::Handwriting),
            _ => None,
        }
    }

    pub fn raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    Pinyin26 = 0,
    Qwerty = 1,
    Numeric = 2,
    Symbol = 3,
    HandwritingPad = 4,
}

impl KeyboardLayout {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Pinyin26),
            1 => Some(Self::Qwerty),
            2 => Some(Self::Numeric),
            3 => Some(Self::Symbol),
            4 => Some(Self::HandwritingPad),
            _ => None,
        }
    }

    pub fn raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputMode {
    pub layout: KeyboardLayout,
    pub scheme: InputScheme,
    pub lang: Language,
    /// BCP-47 tag from LangPack manifest (e.g. `vi`, `th`).
    pub lang_tag: String,
    pub active_pack_id: String,
    pub scheme_id: String,
    pub layout_id: String,
    pub ascii_mode: bool,
    pub forced_by_editor: bool,
}

impl Default for InputMode {
    fn default() -> Self {
        Self {
            layout: KeyboardLayout::Pinyin26,
            scheme: InputScheme::PinyinFull,
            lang: Language::Zh,
            lang_tag: "zh".into(),
            active_pack_id: String::new(),
            scheme_id: "pinyin".into(),
            layout_id: "layout_pinyin26".into(),
            ascii_mode: false,
            forced_by_editor: false,
        }
    }
}
