//! Hot-path arena (double-buffered snapshot for FFI readers).

use yc_types::{
    YcCandidateSlot, YcHotHeader, YcUiCommandSlot, YC_CMD_APPLY_THEME, YC_CMD_COMMIT,
    YC_CMD_DELETE_SURROUNDING, YC_CMD_FINISH_COMPOSING, YC_CMD_RELOAD_KEYBOARD,
    YC_CMD_SET_COMPOSING, MAX_ARENA_COMMANDS, MAX_CAND_TEXT_LEN, MAX_CANDIDATES,
    MAX_COMPOSING_LEN,
};
use yc_types::{ImmSnapshot, UiCommand};

const ARENA_SIZE: usize = 8192;

pub struct HotArena {
    buffers: [Vec<u8>; 2],
    write_index: usize,
    latest_seq: u64,
}

impl HotArena {
    pub fn new() -> Self {
        Self {
            buffers: [vec![0u8; ARENA_SIZE], vec![0u8; ARENA_SIZE]],
            write_index: 0,
            latest_seq: 0,
        }
    }

    pub fn ptr(&self) -> *const u8 {
        self.buffers[self.write_index].as_ptr()
    }

    pub fn size(&self) -> usize {
        ARENA_SIZE
    }

    pub fn latest_seq(&self) -> u64 {
        self.latest_seq
    }

    pub fn read_latest_buffer(&self) -> &[u8] {
        &self.buffers[self.write_index]
    }

    pub fn write_snapshot(&mut self, snapshot: &ImmSnapshot, commands: &[UiCommand]) {
        self.write_index = 1 - self.write_index;
        let buf = &mut self.buffers[self.write_index];
        buf.fill(0);

        let composing_bytes = snapshot.composing.text.as_bytes();
        let composing_len = composing_bytes.len().min(MAX_COMPOSING_LEN);

        let header = YcHotHeader {
            editor_id: snapshot.editor_id.raw(),
            seq: snapshot.seq,
            status_flags: snapshot.status_flags,
            composing_len: composing_len as u32,
            cand_count: snapshot.candidates.len().min(MAX_CANDIDATES) as u32,
            cmd_count: commands.len().min(4) as u32,
        };

        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const YcHotHeader as *const u8,
                std::mem::size_of::<YcHotHeader>(),
            )
        };
        buf[..header_bytes.len()].copy_from_slice(header_bytes);

        let composing_off = std::mem::size_of::<YcHotHeader>();
        buf[composing_off..composing_off + composing_len]
            .copy_from_slice(&composing_bytes[..composing_len]);

        let slots_off = composing_off + MAX_COMPOSING_LEN;
        for (i, cand) in snapshot.candidates.iter().take(MAX_CANDIDATES).enumerate() {
            let text_bytes = cand.text.as_bytes();
            let text_len = text_bytes.len().min(MAX_CAND_TEXT_LEN);
            let mut slot = YcCandidateSlot {
                id: cand.id,
                score_bits: cand.score.to_bits(),
                text_len: text_len as u32,
                reserved: 0,
                text: [0; MAX_CAND_TEXT_LEN],
            };
            slot.text[..text_len].copy_from_slice(&text_bytes[..text_len]);
            let off = slots_off + i * std::mem::size_of::<YcCandidateSlot>();
            let slot_bytes = unsafe {
                std::slice::from_raw_parts(
                    &slot as *const YcCandidateSlot as *const u8,
                    std::mem::size_of::<YcCandidateSlot>(),
                )
            };
            if off + slot_bytes.len() <= ARENA_SIZE {
                buf[off..off + slot_bytes.len()].copy_from_slice(slot_bytes);
            }
        }

        let cmds_off = slots_off + MAX_CANDIDATES * std::mem::size_of::<YcCandidateSlot>();
        for (i, cmd) in commands.iter().take(MAX_ARENA_COMMANDS).enumerate() {
            let mut slot = YcUiCommandSlot::default();
            match cmd {
                UiCommand::Commit { text } => {
                    slot.cmd_type = YC_CMD_COMMIT;
                    let bytes = text.as_bytes();
                    let len = bytes.len().min(MAX_CAND_TEXT_LEN);
                    slot.text_len = len as u32;
                    slot.text[..len].copy_from_slice(&bytes[..len]);
                }
                UiCommand::SetComposing { text } => {
                    slot.cmd_type = YC_CMD_SET_COMPOSING;
                    let bytes = text.as_bytes();
                    let len = bytes.len().min(MAX_CAND_TEXT_LEN);
                    slot.text_len = len as u32;
                    slot.text[..len].copy_from_slice(&bytes[..len]);
                }
                UiCommand::FinishComposing => {
                    slot.cmd_type = YC_CMD_FINISH_COMPOSING;
                }
                UiCommand::DeleteSurrounding { before, after } => {
                    slot.cmd_type = YC_CMD_DELETE_SURROUNDING;
                    slot.param0 = *before;
                    slot.param1 = *after;
                }
                UiCommand::ReloadKeyboard { layout, layout_id } => {
                    slot.cmd_type = YC_CMD_RELOAD_KEYBOARD;
                    slot.param0 = layout.raw();
                    let bytes = layout_id.as_bytes();
                    let len = bytes.len().min(MAX_CAND_TEXT_LEN);
                    slot.text_len = len as u32;
                    slot.text[..len].copy_from_slice(&bytes[..len]);
                }
                UiCommand::ApplyTheme { skin_id } => {
                    slot.cmd_type = YC_CMD_APPLY_THEME;
                    let bytes = skin_id.as_bytes();
                    let len = bytes.len().min(MAX_CAND_TEXT_LEN);
                    slot.text_len = len as u32;
                    slot.text[..len].copy_from_slice(&bytes[..len]);
                }
            }
            let off = cmds_off + i * std::mem::size_of::<YcUiCommandSlot>();
            let slot_bytes = unsafe {
                std::slice::from_raw_parts(
                    &slot as *const YcUiCommandSlot as *const u8,
                    std::mem::size_of::<YcUiCommandSlot>(),
                )
            };
            if off + slot_bytes.len() <= ARENA_SIZE {
                buf[off..off + slot_bytes.len()].copy_from_slice(slot_bytes);
            }
        }

        self.latest_seq = snapshot.seq;
    }
}

impl Default for HotArena {
    fn default() -> Self {
        Self::new()
    }
}
