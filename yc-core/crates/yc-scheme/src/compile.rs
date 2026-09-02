use std::fs;
use std::path::Path;

use crate::model::{RuleEntry, SchemeYaml, TransformKind};
use crate::{SCHEME_MAGIC, SCHEME_VERSION};

pub fn compile_scheme_yaml(scheme_path: &Path, pack_root: &Path) -> std::io::Result<Vec<u8>> {
    let text = fs::read_to_string(scheme_path)?;
    let scheme: SchemeYaml = serde_yaml::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let kind = TransformKind::from_str(&scheme.transform.kind).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown transform type {}", scheme.transform.kind),
        )
    })?;

    let mut payload = Vec::new();
    payload.extend_from_slice(scheme.scheme_id.as_bytes());
    payload.push(0);
    payload.extend_from_slice(scheme.lang.as_bytes());
    payload.push(0);

    match kind {
        TransformKind::LatinPredict => {
            if let Some(a) = &scheme.alphabet {
                payload.extend_from_slice(a.as_bytes());
                payload.push(0);
            }
        }
        TransformKind::RuleChain => {
            let rules_rel = scheme.transform.rules.as_ref().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "rule_chain missing rules")
            })?;
            let rules_path = pack_root.join(rules_rel);
            let rules_text = fs::read_to_string(rules_path)?;
            let rules: Vec<RuleEntry> = serde_yaml::from_str(&rules_text)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            payload.extend_from_slice(&(rules.len() as u32).to_le_bytes());
            for rule in rules {
                let m = rule.r#match.as_bytes();
                payload.extend_from_slice(&(m.len() as u16).to_le_bytes());
                payload.extend_from_slice(m);
                let out = rule.output.unwrap_or_default();
                let o = out.as_bytes();
                payload.extend_from_slice(&(o.len() as u16).to_le_bytes());
                payload.extend_from_slice(o);
            }
        }
        TransformKind::Table => {
            let rules_rel = scheme.transform.rules.as_ref().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "table missing rules")
            })?;
            let rules_path = pack_root.join(rules_rel);
            let rules_text = fs::read_to_string(rules_path)?;
            let syllables: Vec<String> = serde_yaml::from_str(&rules_text)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            payload.extend_from_slice(&(syllables.len() as u32).to_le_bytes());
            for syl in syllables {
                let b = syl.as_bytes();
                payload.extend_from_slice(&(b.len() as u16).to_le_bytes());
                payload.extend_from_slice(b);
            }
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(SCHEME_MAGIC);
    out.extend_from_slice(&SCHEME_VERSION.to_le_bytes());
    out.push(kind.as_byte());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compile_latin_fixture() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/vi-v1");
        let scheme = root.join("schemes/vi_latin.yaml");
        if !scheme.exists() {
            return;
        }
        let bin = compile_scheme_yaml(&scheme, &root).expect("compile");
        assert!(bin.starts_with(b"YCSH"));
    }
}
