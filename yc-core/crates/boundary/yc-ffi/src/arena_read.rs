//! Parsed snapshot from the hot-path arena (for shells / yc-cli).

use yc_types::{
    YcCandidateSlot, YcHotHeader, YcUiCommandSlot, YC_CMD_APPLY_THEME, YC_CMD_COMMIT,
    YC_CMD_DELETE_SURROUNDING, YC_CMD_FINISH_COMPOSING, YC_CMD_RELOAD_KEYBOARD,
    YC_CMD_SET_COMPOSING, MAX_ARENA_COMMANDS, MAX_CAND_TEXT_LEN, MAX_CANDIDATES,
    MAX_COMPOSING_LEN,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaSnapshot {
    pub editor_id: u64,
    pub seq: u64,
    pub status_flags: u32,
    pub composing: String,
    pub candidates: Vec<ArenaCandidate>,
    pub commands: Vec<ArenaCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaCandidate {
    pub id: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaCommand {
    Commit { text: String },
    SetComposing { text: String },
    FinishComposing,
    DeleteSurrounding { before: u32, after: u32 },
    ReloadKeyboard { layout: u32, layout_id: String },
    ApplyTheme { skin_id: String },
}

pub fn parse_arena(data: &[u8]) -> Option<ArenaSnapshot> {
    if data.len() < std::mem::size_of::<YcHotHeader>() {
        return None;
    }
    let header = unsafe { &*(data.as_ptr() as *const YcHotHeader) };
    let composing_off = std::mem::size_of::<YcHotHeader>();
    let composing_len = header.composing_len as usize;
    if composing_off + MAX_COMPOSING_LEN > data.len() {
        return None;
    }
    let composing = String::from_utf8_lossy(
        &data[composing_off..composing_off + composing_len.min(MAX_COMPOSING_LEN)],
    )
    .into_owned();

    let slots_off = composing_off + MAX_COMPOSING_LEN;
    let slot_size = std::mem::size_of::<YcCandidateSlot>();
    let cand_count = header.cand_count as usize;
    let mut candidates = Vec::with_capacity(cand_count);
    for i in 0..cand_count.min(MAX_CANDIDATES) {
        let off = slots_off + i * slot_size;
        if off + slot_size > data.len() {
            break;
        }
        let slot = unsafe { &*(data.as_ptr().add(off) as *const YcCandidateSlot) };
        let text_len = slot.text_len as usize;
        let text = String::from_utf8_lossy(&slot.text[..text_len.min(MAX_CAND_TEXT_LEN)]).into_owned();
        candidates.push(ArenaCandidate {
            id: slot.id,
            text,
        });
    }

    let cmds_off = slots_off + MAX_CANDIDATES * slot_size;
    let cmd_slot_size = std::mem::size_of::<YcUiCommandSlot>();
    let cmd_count = header.cmd_count as usize;
    let mut commands = Vec::with_capacity(cmd_count);
    for i in 0..cmd_count.min(MAX_ARENA_COMMANDS) {
        let off = cmds_off + i * cmd_slot_size;
        if off + cmd_slot_size > data.len() {
            break;
        }
        let slot = unsafe { &*(data.as_ptr().add(off) as *const YcUiCommandSlot) };
        let text_len = slot.text_len as usize;
        let text = String::from_utf8_lossy(&slot.text[..text_len.min(MAX_CAND_TEXT_LEN)]).into_owned();
        let cmd = match slot.cmd_type {
            YC_CMD_COMMIT => ArenaCommand::Commit { text },
            YC_CMD_SET_COMPOSING => ArenaCommand::SetComposing { text },
            YC_CMD_FINISH_COMPOSING => ArenaCommand::FinishComposing,
            YC_CMD_DELETE_SURROUNDING => ArenaCommand::DeleteSurrounding {
                before: slot.param0,
                after: slot.param1,
            },
            YC_CMD_RELOAD_KEYBOARD => ArenaCommand::ReloadKeyboard {
                layout: slot.param0,
                layout_id: text,
            },
            YC_CMD_APPLY_THEME => ArenaCommand::ApplyTheme { skin_id: text },
            _ => continue,
        };
        commands.push(cmd);
    }

    Some(ArenaSnapshot {
        editor_id: header.editor_id,
        seq: header.seq,
        status_flags: header.status_flags,
        composing,
        candidates,
        commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::{Candidate, ComposingText, EditorId, ImmSnapshot, UiCommand};

    #[test]
    fn roundtrip_via_arena_write() {
        use crate::arena::HotArena;

        let mut arena = HotArena::new();
        let snapshot = ImmSnapshot {
            editor_id: EditorId(7),
            seq: 3,
            input_mode: Default::default(),
            composing: ComposingText {
                text: "ni".into(),
                cursor: 2,
            },
            candidates: vec![Candidate {
                id: 1,
                text: "你".into(),
                source: yc_types::CandidateSource::Lexicon,
                score: 1.0,
            }],
            status_flags: 0,
        };
        let commands = vec![UiCommand::Commit {
            text: "你好".into(),
        }];
        arena.write_snapshot(&snapshot, &commands);

        let parsed = parse_arena(arena.read_latest_buffer()).expect("parse");
        assert_eq!(parsed.editor_id, 7);
        assert_eq!(parsed.composing, "ni");
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(parsed.commands.len(), 1);
        assert_eq!(
            parsed.commands[0],
            ArenaCommand::Commit {
                text: "你好".into()
            }
        );
    }
}
