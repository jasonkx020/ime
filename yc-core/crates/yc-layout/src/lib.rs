//! Layout YAML compile + mmap runtime.

mod compile;
mod model;
mod runtime;

pub use compile::compile_layout_yaml;
pub use model::LayoutYaml;
pub use runtime::{KeySlot, LayoutView, LAYOUT_MAGIC, LAYOUT_VERSION};

pub const MAX_LAYOUT_ID: usize = 64;
pub const MAX_KEY_LABEL: usize = 16;
pub const MAX_KEY_OUTPUT: usize = 16;
