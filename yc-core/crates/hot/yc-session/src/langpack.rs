//! Enabled lang pack metadata for hot-path switch_lang (no yc-plugin dependency).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnabledLangPack {
    pub pack_id: String,
    pub lang_tag: String,
    pub default_scheme_id: String,
    pub default_layout_id: String,
}

impl EnabledLangPack {
    pub fn pack_id_hash(&self) -> u32 {
        hash_pack_id(&self.pack_id)
    }
}

pub fn hash_pack_id(id: &str) -> u32 {
    id.bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
}
