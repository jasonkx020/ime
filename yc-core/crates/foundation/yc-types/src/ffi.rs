/// C ABI error codes (exported to `yc_hot.h`).
pub const YC_OK: i32 = 0;
pub const YC_ERR_SESSION: i32 = -1;
pub const YC_ERR_BUSY: i32 = -2;
pub const YC_ERR_INTERNAL: i32 = -3;

pub const MAX_CANDIDATES: usize = 9;
pub const MAX_COMPOSING_LEN: usize = 64;
pub const MAX_CAND_TEXT_LEN: usize = 64;
pub const MAX_HW_POINTS: usize = 256;
pub const MAX_HW_STROKES: usize = 16;

/// Normalized stroke point for FFI (`yc_hw_push_stroke`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YcStrokePoint {
    pub x: f32,
    pub y: f32,
    pub t: u64,
    pub pressure: f32,
}

impl Default for YcStrokePoint {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            t: 0,
            pressure: 1.0,
        }
    }
}

/// Fixed-size hot-path action (40 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YcHotAction {
    pub editor_id: u64,
    pub client_seq: u64,
    pub action_type: u32,
    pub key_code: u32,
    pub candidate_id: u32,
    pub flags: u32,
    pub reserved: [u8; 8],
}

impl Default for YcHotAction {
    fn default() -> Self {
        Self {
            editor_id: 0,
            client_seq: 0,
            action_type: 0,
            key_code: 0,
            candidate_id: 0,
            flags: 0,
            reserved: [0; 8],
        }
    }
}

/// Arena header written before composing + candidate slots.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YcHotHeader {
    pub editor_id: u64,
    pub seq: u64,
    pub status_flags: u32,
    pub composing_len: u32,
    pub cand_count: u32,
    pub cmd_count: u32,
}

impl Default for YcHotHeader {
    fn default() -> Self {
        Self {
            editor_id: 0,
            seq: 0,
            status_flags: 0,
            composing_len: 0,
            cand_count: 0,
            cmd_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YcCandidateSlot {
    pub id: u32,
    pub score_bits: u32,
    pub text_len: u32,
    pub reserved: u32,
    pub text: [u8; MAX_CAND_TEXT_LEN],
}

impl Default for YcCandidateSlot {
    fn default() -> Self {
        Self {
            id: 0,
            score_bits: 0,
            text_len: 0,
            reserved: 0,
            text: [0; MAX_CAND_TEXT_LEN],
        }
    }
}

/// UiCommand types written after candidate slots in the hot arena (M1).
pub const YC_CMD_COMMIT: u32 = 0;
pub const YC_CMD_SET_COMPOSING: u32 = 1;
pub const YC_CMD_FINISH_COMPOSING: u32 = 2;
pub const YC_CMD_DELETE_SURROUNDING: u32 = 3;
pub const YC_CMD_RELOAD_KEYBOARD: u32 = 4;
pub const YC_CMD_APPLY_THEME: u32 = 5;

pub const MAX_ARENA_COMMANDS: usize = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YcUiCommandSlot {
    pub cmd_type: u32,
    pub param0: u32,
    pub param1: u32,
    pub text_len: u32,
    pub text: [u8; MAX_CAND_TEXT_LEN],
}

impl Default for YcUiCommandSlot {
    fn default() -> Self {
        Self {
            cmd_type: 0,
            param0: 0,
            param1: 0,
            text_len: 0,
            text: [0; MAX_CAND_TEXT_LEN],
        }
    }
}
