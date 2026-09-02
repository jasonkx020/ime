pub const LAYOUT_MAGIC: &[u8; 4] = b"YCLY";
pub const LAYOUT_VERSION: u32 = 1;

pub const MAX_LAYOUT_ID: usize = 64;
pub const MAX_KEY_LABEL: usize = 16;
pub const MAX_KEY_OUTPUT: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeySlot {
    pub label: [u8; MAX_KEY_LABEL],
    pub output: [u8; MAX_KEY_OUTPUT],
    pub action: u8,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutView {
    pub layout_id: String,
    pub keys: Vec<KeySlot>,
}

impl LayoutView {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 4 + 4 + MAX_LAYOUT_ID + 4 {
            return Err("truncated header");
        }
        if &bytes[0..4] != LAYOUT_MAGIC {
            return Err("bad magic");
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != LAYOUT_VERSION {
            return Err("bad version");
        }
        let id_end = bytes[8..8 + MAX_LAYOUT_ID]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(MAX_LAYOUT_ID);
        let layout_id = String::from_utf8_lossy(&bytes[8..8 + id_end]).into_owned();
        let key_count =
            u32::from_le_bytes(bytes[8 + MAX_LAYOUT_ID..8 + MAX_LAYOUT_ID + 4].try_into().unwrap())
                as usize;
        let slot_size = MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1 + 4;
        let keys_start = 8 + MAX_LAYOUT_ID + 4;
        if bytes.len() < keys_start + key_count * slot_size {
            return Err("truncated keys");
        }
        let mut keys = Vec::with_capacity(key_count);
        for i in 0..key_count {
            let off = keys_start + i * slot_size;
            let mut label = [0u8; MAX_KEY_LABEL];
            let mut output = [0u8; MAX_KEY_OUTPUT];
            label.copy_from_slice(&bytes[off..off + MAX_KEY_LABEL]);
            output.copy_from_slice(&bytes[off + MAX_KEY_LABEL..off + MAX_KEY_LABEL + MAX_KEY_OUTPUT]);
            let action = bytes[off + MAX_KEY_LABEL + MAX_KEY_OUTPUT];
            let width = f32::from_le_bytes(
                bytes[off + MAX_KEY_LABEL + MAX_KEY_OUTPUT + 1..off + MAX_KEY_LABEL + MAX_KEY_OUTPUT + 5]
                    .try_into()
                    .unwrap(),
            );
            keys.push(KeySlot {
                label,
                output,
                action,
                width,
            });
        }
        Ok(Self { layout_id, keys })
    }

    pub fn label_str(slot: &KeySlot) -> String {
        cstr(&slot.label)
    }

    pub fn output_str(slot: &KeySlot) -> String {
        cstr(&slot.output)
    }
}

fn cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile_layout_yaml;
    use std::path::PathBuf;

    #[test]
    fn roundtrip_fixture() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/vi-v1/layouts/layout_qwerty.yaml");
        if !path.exists() {
            return;
        }
        let bin = compile_layout_yaml(&path).unwrap();
        let view = LayoutView::from_bytes(&bin).unwrap();
        assert_eq!(view.layout_id, "layout_qwerty");
        assert!(!view.keys.is_empty());
    }
}
