//! Input engines driven by LangPack schemes and lexicons

mod data_driven;

mod factory;

mod latin;

mod pinyin_seg;



pub use data_driven::DataDrivenEngine;

pub use factory::EngineFactory;

pub use latin::LatinPredictEngine;

pub use pinyin_seg::{is_valid_prefix, key_matches_composing, normalize_query, split_syllables};

use yc_types::{EditorId, EngineError, HotResult};



pub trait InputEngine {

    fn reset(&mut self, editor_id: EditorId);

    fn feed(

        &mut self,

        editor_id: EditorId,

        key_code: u32,

        input_mode: &yc_types::InputMode,

    ) -> HotResult<yc_types::EngineStep>;

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<yc_types::EngineStep>;

    fn backspace(&mut self, editor_id: EditorId) -> HotResult<yc_types::EngineStep>;

}



pub(crate) fn key_code_to_char(key_code: u32) -> Option<char> {

    if (b'a'..=b'z').contains(&(key_code as u8)) {

        return Some(key_code as u8 as char);

    }

    if (b'A'..=b'Z').contains(&(key_code as u8)) {

        return Some((key_code as u8).to_ascii_lowercase() as char);

    }

    None

}



pub(crate) fn invalid_session(editor_id: EditorId, active: EditorId) -> bool {

    editor_id != active

}



pub(crate) fn session_invalid<T>() -> HotResult<T> {

    Err(EngineError::SessionInvalid)

}

