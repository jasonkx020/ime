//! AI assist service (M0 stub).

use yc_types::{EditorId, EngineError, HotResult, PrivacyLevel};

#[derive(Debug, Default)]
pub struct AiAssistService;

impl AiAssistService {
    pub fn new() -> Self {
        Self
    }

    pub fn is_allowed(&self, privacy: PrivacyLevel) -> bool {
        privacy == PrivacyLevel::Normal
    }

    pub fn generate(&self, _editor_id: EditorId, _scene: &str) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_cloud_blocked() {
        let svc = AiAssistService::new();
        assert!(!svc.is_allowed(PrivacyLevel::ForbiddenCloud));
        assert!(svc.is_allowed(PrivacyLevel::Normal));
    }
}
