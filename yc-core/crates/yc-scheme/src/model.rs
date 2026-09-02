use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SchemeYaml {
    pub scheme_id: String,
    pub lang: String,
    #[serde(default)]
    pub normalization: Option<String>,
    #[serde(default)]
    pub alphabet: Option<String>,
    pub transform: TransformYaml,
    #[serde(default)]
    pub candidate: Option<CandidateYaml>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TransformYaml {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub rules: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CandidateYaml {
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformKind {
    LatinPredict,
    RuleChain,
    Table,
}

impl TransformKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "latin_predict" | "latin" => Some(Self::LatinPredict),
            "rule_chain" | "rules" => Some(Self::RuleChain),
            "table" | "pinyin_full" => Some(Self::Table),
            _ => None,
        }
    }

    pub fn as_byte(self) -> u8 {
        match self {
            Self::LatinPredict => crate::TRANSFORM_LATIN,
            Self::RuleChain => crate::TRANSFORM_RULE_CHAIN,
            Self::Table => crate::TRANSFORM_TABLE,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RuleEntry {
    pub r#match: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub output_tone: Option<String>,
    #[serde(default)]
    pub consume: Option<u32>,
}
