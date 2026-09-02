//! Scheme YAML compile + mmap runtime (latin_predict / rule_chain / table).

mod compile;
mod model;
mod runtime;

pub use compile::compile_scheme_yaml;
pub use model::{RuleEntry, SchemeYaml, TransformKind};
pub use runtime::SchemeDesc;

pub const SCHEME_MAGIC: &[u8; 4] = b"YCSH";
pub const SCHEME_VERSION: u32 = 1;

pub const TRANSFORM_LATIN: u8 = 0;
pub const TRANSFORM_RULE_CHAIN: u8 = 1;
pub const TRANSFORM_TABLE: u8 = 2;
