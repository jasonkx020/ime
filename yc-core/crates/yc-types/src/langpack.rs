//! Engine registration spec (decouples yc-engine from yc-plugin).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangPackEngineSpec {
    pub pack_id: String,
    pub lexicon_path: String,
    pub install_path: String,
    pub engine_kind: String,
    pub default_scheme_id: String,
}
