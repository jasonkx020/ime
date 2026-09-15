//! Handwriting session + offline recognition.
//!
//! Android production path uses Ismantic/Handwritten (NCNN) via shell
//! `yc_hw_apply_result`. Template matcher remains a fallback for tests / missing model.

mod cloud;
mod recognizer;
mod segment;
mod service;
pub mod templates;

pub use cloud::{CloudHwRecognizer, StubCloudRecognizer};
pub use recognizer::OnDeviceRecognizer;
pub use segment::{segment_strokes, DEFAULT_GAP_MS, DEFAULT_GAP_NORM};
pub use service::{HandwritingService, HW_MAX_CANDIDATES, HW_PAGE_SIZE};
