//! Parsed snapshot from the hot-path arena (for shells / yc-cli).

use yc_types::{
    YcCandidateSlot, YcHotHeader, MAX_CAND_TEXT_LEN, MAX_CANDIDATES, MAX_COMPOSING_LEN,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaSnapshot {
    pub editor_id: u64,
    pub seq: u64,
    pub status_flags: u32,
    pub composing: String,
    pub candidates: Vec<ArenaCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaCandidate {
    pub id: u32,
    pub text: String,
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

    Some(ArenaSnapshot {
        editor_id: header.editor_id,
        seq: header.seq,
        status_flags: header.status_flags,
        composing,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::{Candidate, ComposingText, EditorId, ImmSnapshot};

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
                id: 0,
                text: "你".into(),
                score: 1.0,
                source: yc_types::CandidateSource::Lexicon,
            }],
            status_flags: 0,
        };
        arena.write_snapshot(&snapshot, &[]);
        let buf = arena.read_latest_buffer();
        let parsed = parse_arena(buf).unwrap();
        assert_eq!(parsed.editor_id, 7);
        assert_eq!(parsed.composing, "ni");
        assert_eq!(parsed.candidates[0].text, "你");
    }
}
