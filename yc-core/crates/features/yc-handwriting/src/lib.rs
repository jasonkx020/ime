//! On-device handwriting recognition (M2.5 stub templates).

mod cloud;
mod recognizer;
mod service;
pub mod templates;

pub use cloud::{CloudHwRecognizer, StubCloudRecognizer};
pub use recognizer::OnDeviceRecognizer;
pub use service::HandwritingService;
