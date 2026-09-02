use crate::model::TransformKind;
use crate::{SCHEME_MAGIC, SCHEME_VERSION, TRANSFORM_LATIN, TRANSFORM_RULE_CHAIN, TRANSFORM_TABLE};

#[derive(Debug, Clone)]
pub struct RuleChainRule {
    pub pattern: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct SchemeDesc {
    pub scheme_id: String,
    pub lang: String,
    pub transform: TransformKind,
    pub rules: Vec<RuleChainRule>,
    pub syllables: Vec<String>,
    pub alphabet: String,
}

impl SchemeDesc {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 13 || &bytes[0..4] != SCHEME_MAGIC {
            return Err("bad magic");
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != SCHEME_VERSION {
            return Err("bad version");
        }
        let kind_byte = bytes[8];
        let payload_len = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
        if bytes.len() < 13 + payload_len {
            return Err("truncated");
        }
        let payload = &bytes[13..13 + payload_len];
        let transform = match kind_byte {
            TRANSFORM_LATIN => TransformKind::LatinPredict,
            TRANSFORM_RULE_CHAIN => TransformKind::RuleChain,
            TRANSFORM_TABLE => TransformKind::Table,
            _ => return Err("unknown transform"),
        };

        let (scheme_id, rest) = read_cstr(payload)?;
        let (lang, rest) = read_cstr(rest)?;

        let mut rules = Vec::new();
        let mut syllables = Vec::new();
        let mut alphabet = String::new();
        let mut rest = rest;

        match transform {
            TransformKind::LatinPredict => {
                if !rest.is_empty() {
                    let (a, _) = read_cstr(rest)?;
                    alphabet = a;
                }
            }
            TransformKind::RuleChain => {
                if rest.len() < 4 {
                    return Err("bad rules");
                }
                let count = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
                rest = &rest[4..];
                for _ in 0..count {
                    let (pat, r) = read_len_str(rest)?;
                    rest = r;
                    let (out, r) = read_len_str(rest)?;
                    rest = r;
                    rules.push(RuleChainRule {
                        pattern: pat,
                        output: out,
                    });
                }
            }
            TransformKind::Table => {
                if rest.len() < 4 {
                    return Err("bad table");
                }
                let count = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
                rest = &rest[4..];
                for _ in 0..count {
                    let (syl, r) = read_len_str(rest)?;
                    rest = r;
                    syllables.push(syl);
                }
            }
        }

        Ok(Self {
            scheme_id,
            lang,
            transform,
            rules,
            syllables,
            alphabet,
        })
    }

    pub fn apply_rule_chain(&self, input: &str) -> String {
        let mut out = input.to_string();
        for rule in &self.rules {
            if out.contains(&rule.pattern) {
                out = out.replace(&rule.pattern, &rule.output);
            }
        }
        out
    }

    pub fn is_valid_syllable(&self, syl: &str) -> bool {
        self.syllables.iter().any(|s| s == syl)
    }
}

fn read_cstr(data: &[u8]) -> Result<(String, &[u8]), &'static str> {
    let end = data.iter().position(|&b| b == 0).ok_or("bad cstr")?;
    let s = String::from_utf8_lossy(&data[..end]).into_owned();
    Ok((s, &data[end + 1..]))
}

fn read_len_str(data: &[u8]) -> Result<(String, &[u8]), &'static str> {
    if data.len() < 2 {
        return Err("bad str");
    }
    let len = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
    if data.len() < 2 + len {
        return Err("bad str len");
    }
    let s = String::from_utf8_lossy(&data[2..2 + len]).into_owned();
    Ok((s, &data[2 + len..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile_scheme_yaml;
    use std::path::PathBuf;

    #[test]
    fn telex_aw_rule() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/vi-v1");
        let scheme = root.join("schemes/vi_telex.yaml");
        if !scheme.exists() {
            return;
        }
        let bin = compile_scheme_yaml(&scheme, &root).unwrap();
        let desc = SchemeDesc::from_bytes(&bin).unwrap();
        assert_eq!(desc.apply_rule_chain("aw"), "ă");
    }
}
