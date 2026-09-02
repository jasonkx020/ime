//! Extension host: emoji / skin / speech panels (M0 stub).

use yc_types::{EngineError, HotResult};

#[derive(Debug, Default)]
pub struct ExtensionHost;

impl ExtensionHost {
    pub fn new() -> Self {
        Self
    }

    pub fn open_panel(&self, _panel_id: &str) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }

    pub fn close_panel(&self) -> HotResult<()> {
        Ok(())
    }
}
