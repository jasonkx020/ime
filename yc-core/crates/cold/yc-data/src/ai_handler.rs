//! Pluggable AI cold-path handler (no hard dependency on yc-ai).

use yc_types::ColdKind;

pub trait ColdAiHandler: Send + Sync {
    fn handle(&self, kind: ColdKind, payload: &[u8]) -> (Vec<u8>, i32);
}

#[derive(Debug, Default)]
pub struct NoOpColdAiHandler;

impl ColdAiHandler for NoOpColdAiHandler {
    fn handle(&self, _kind: ColdKind, _payload: &[u8]) -> (Vec<u8>, i32) {
        (Vec::new(), -1)
    }
}
