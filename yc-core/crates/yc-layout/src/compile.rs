use std::fs;
use std::path::Path;

use crate::model::LayoutYaml;
use crate::runtime::{KeySlot, LAYOUT_MAGIC, LAYOUT_VERSION, MAX_KEY_LABEL, MAX_KEY_OUTPUT, MAX_LAYOUT_ID};

pub fn compile_layout_yaml(path: &Path) -> std::io::Result<Vec<u8>> {
    let text = fs::read_to_string(path)?;
    let layout: LayoutYaml = serde_yaml::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut keys = Vec::new();
    for row in &layout.rows {
        for key in row {
            let label = key.label.clone().unwrap_or_default();
            let output = key.output.clone().unwrap_or_default();
            let action = key.action.as_deref().unwrap_or("");
            keys.push(KeySlot {
                label: pad_str(&label, MAX_KEY_LABEL),
                output: pad_str(&output, MAX_KEY_OUTPUT),
                action: action_to_byte(action),
                width: key.width,
            });
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(LAYOUT_MAGIC);
    out.extend_from_slice(&LAYOUT_VERSION.to_le_bytes());
    let mut id_buf = [0u8; MAX_LAYOUT_ID];
    let id_bytes = layout.layout_id.as_bytes();
    let copy = id_bytes.len().min(MAX_LAYOUT_ID - 1);
    id_buf[..copy].copy_from_slice(&id_bytes[..copy]);
    out.extend_from_slice(&id_buf);
    out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
    for key in keys {
        out.extend_from_slice(&key.label);
        out.extend_from_slice(&key.output);
        out.push(key.action);
        out.extend_from_slice(&key.width.to_le_bytes());
    }
    Ok(out)
}

fn pad_str(s: &str, max: usize) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let b = s.as_bytes();
    let n = b.len().min(max - 1);
    buf[..n].copy_from_slice(&b[..n]);
    buf
}

fn action_to_byte(action: &str) -> u8 {
    match action {
        "backspace" => 1,
        "switch_layout" => 2,
        "switch_lang" => 3,
        "separator" => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compile_qwerty_fixture() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/vi-v1/layouts/layout_qwerty.yaml");
        if !path.exists() {
            return;
        }
        let bin = compile_layout_yaml(&path).expect("compile");
        assert!(bin.starts_with(b"YCLY"));
    }
}
