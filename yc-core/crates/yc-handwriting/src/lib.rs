//! On-device handwriting recognition (M2.5 stub templates).

mod recognizer;
mod service;
pub mod templates;

pub use recognizer::OnDeviceRecognizer;
pub use service::HandwritingService;
