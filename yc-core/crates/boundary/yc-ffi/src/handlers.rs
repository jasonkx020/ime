//! Adapters that wire optional crates into cold-path runtime.

#[cfg(all(feature = "data", feature = "ai"))]
mod ai_cold {
    use std::sync::Arc;

    use yc_data::ColdAiHandler;
    use yc_types::{AiMode, ColdKind, PrivacyLevel, TaskReq};

    pub struct AiAssistColdHandler {
        svc: yc_ai::AiAssistService,
    }

    impl AiAssistColdHandler {
        pub fn new() -> Self {
            Self {
                svc: yc_ai::AiAssistService::new(),
            }
        }
    }

    impl ColdAiHandler for AiAssistColdHandler {
        fn handle(&self, kind: ColdKind, payload: &[u8]) -> (Vec<u8>, i32) {
            let Ok(mut req) = serde_json::from_slice::<TaskReq>(payload) else {
                return (Vec::new(), -1);
            };
            if kind == ColdKind::AiPolish {
                req.mode = AiMode::Polish.raw();
            }
            let privacy = PrivacyLevel::Normal;
            let result = if kind == ColdKind::AiPolish {
                self.svc.polish(privacy, &req)
            } else {
                self.svc.suggest(privacy, &req)
            };
            match result {
                Ok(out) => (serde_json::to_vec(&out).unwrap_or_default(), 0),
                Err(_) => (Vec::new(), -1),
            }
        }
    }

    pub fn install(runtime: &yc_data::ColdPathRuntime) {
        runtime.set_ai_handler(Arc::new(AiAssistColdHandler::new()));
    }
}

#[cfg(all(feature = "data", feature = "ai"))]
pub fn install_optional_handlers(runtime: &yc_data::ColdPathRuntime) {
    ai_cold::install(runtime);
}

#[cfg(all(feature = "data", not(feature = "ai")))]
pub fn install_optional_handlers(_runtime: &yc_data::ColdPathRuntime) {}

#[cfg(not(feature = "data"))]
pub fn install_optional_handlers() {}
