//! AiAssistService: privacy gate, scrubbing, local templates, cloud client trait (P2).

use std::sync::atomic::{AtomicU64, Ordering};

use yc_types::{
    AiMode, AiOutput, AiVariant, EditorId, EngineError, HotResult, PrivacyLevel, RedactedField,
    RedactedPreview, TaskId, TaskReq,
};

/// Cloud LLM transport — never call from hot path.
pub trait CloudLlmClient: Send + Sync {
    fn generate(&self, req: &TaskReq) -> HotResult<AiOutput>;
}

/// Default stub: returns echo-style variants without network.
#[derive(Debug, Default)]
pub struct EchoStubClient;

impl CloudLlmClient for EchoStubClient {
    fn generate(&self, req: &TaskReq) -> HotResult<AiOutput> {
        let seed = if !req.selection_text.is_empty() {
            req.selection_text.clone()
        } else if !req.peer_message.is_empty() {
            req.peer_message.clone()
        } else {
            req.user_intent.clone()
        };
        let base = if seed.is_empty() {
            "好的，收到。".to_string()
        } else {
            seed
        };
        Ok(AiOutput {
            variants: vec![
                AiVariant {
                    text: format!("{base}"),
                    tone: "neutral".into(),
                },
                AiVariant {
                    text: format!("（润色）{base}"),
                    tone: "polite".into(),
                },
                AiVariant {
                    text: format!("（高情商）理解你的意思：{base}"),
                    tone: "higheq".into(),
                },
            ],
            local: false,
        })
    }
}

pub struct AiAssistService {
    client: Box<dyn CloudLlmClient>,
    next_task: AtomicU64,
}

impl Default for AiAssistService {
    fn default() -> Self {
        Self::new()
    }
}

impl AiAssistService {
    pub fn new() -> Self {
        Self {
            client: Box::new(EchoStubClient),
            next_task: AtomicU64::new(1),
        }
    }

    pub fn with_client(client: Box<dyn CloudLlmClient>) -> Self {
        Self {
            client,
            next_task: AtomicU64::new(1),
        }
    }

    pub fn is_allowed(&self, privacy: PrivacyLevel, mode: AiMode) -> bool {
        match privacy {
            PrivacyLevel::ForbiddenCloud => false,
            PrivacyLevel::Sensitive => matches!(mode, AiMode::Polish | AiMode::Compose),
            PrivacyLevel::Normal => true,
        }
    }

    pub fn preview_payload(&self, req: &TaskReq) -> RedactedPreview {
        let mode = req.ai_mode().unwrap_or(AiMode::Polish);
        let will_cloud = !can_handle_locally(req) && mode != AiMode::Polish;
        RedactedPreview {
            summary: format!(
                "scene={} mode={} will_use_cloud={}",
                req.scene_id,
                mode.raw(),
                will_cloud
            ),
            fields: vec![
                RedactedField {
                    name: "peer_message".into(),
                    value: redact(&req.peer_message),
                },
                RedactedField {
                    name: "background_note".into(),
                    value: redact(&req.background_note),
                },
                RedactedField {
                    name: "selection_text".into(),
                    value: redact(&req.selection_text),
                },
                RedactedField {
                    name: "user_intent".into(),
                    value: redact(&req.user_intent),
                },
            ],
            will_use_cloud: will_cloud,
        }
    }

    /// Synchronous suggest: local templates first; else EchoStub/cloud client.
    pub fn suggest(&self, privacy: PrivacyLevel, req: &TaskReq) -> HotResult<AiOutput> {
        let mode = req.ai_mode().unwrap_or(AiMode::SmartReply);
        if !self.is_allowed(privacy, mode) {
            return Err(EngineError::Unsupported);
        }
        if privacy == PrivacyLevel::Sensitive || can_handle_locally(req) {
            return Ok(local_template(req));
        }
        self.client.generate(req)
    }

    pub fn polish(&self, privacy: PrivacyLevel, req: &TaskReq) -> HotResult<AiOutput> {
        let mut req = req.clone();
        req.mode = AiMode::Polish.raw();
        self.suggest(privacy, &req)
    }

    pub fn allocate_task(&self) -> TaskId {
        TaskId(self.next_task.fetch_add(1, Ordering::Relaxed))
    }

    pub fn cancel(&self, _task_id: TaskId) -> HotResult<()> {
        Ok(())
    }

    /// Compatibility stub used by early M0 callers.
    pub fn generate(&self, _editor_id: EditorId, scene: &str) -> HotResult<AiOutput> {
        let req = TaskReq {
            editor_id: 0,
            mode: AiMode::Compose.raw(),
            scene_id: scene.to_string(),
            peer_message: String::new(),
            background_note: String::new(),
            selection_text: String::new(),
            user_intent: "打个招呼".into(),
        };
        self.suggest(PrivacyLevel::Normal, &req)
    }
}

fn can_handle_locally(req: &TaskReq) -> bool {
    let total = req.peer_message.chars().count() + req.background_note.chars().count();
    total < 80
        && matches!(
            req.scene_id.as_str(),
            "greeting" | "thanks" | "apology" | "" | "polish"
        )
}

fn local_template(req: &TaskReq) -> AiOutput {
    let text = match req.scene_id.as_str() {
        "greeting" => "你好，很高兴认识你。".to_string(),
        "thanks" => "非常感谢，辛苦了！".to_string(),
        "apology" => "抱歉给你带来不便，我们马上处理。".to_string(),
        _ if !req.selection_text.is_empty() => format!("润色：{}", req.selection_text),
        _ if !req.user_intent.is_empty() => req.user_intent.clone(),
        _ => "好的。".to_string(),
    };
    AiOutput {
        variants: vec![
            AiVariant {
                text: text.clone(),
                tone: "local".into(),
            },
            AiVariant {
                text: format!("{text}（简短）"),
                tone: "concise".into(),
            },
            AiVariant {
                text: format!("{text}（更礼貌）"),
                tone: "polite".into(),
            },
        ],
        local: true,
    }
}

fn redact(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let count = s.chars().count();
    if count <= 4 {
        return "***".into();
    }
    let visible: String = s.chars().take(count.saturating_sub(3)).collect();
    format!("{visible}***")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_cloud_blocked() {
        let svc = AiAssistService::new();
        assert!(!svc.is_allowed(PrivacyLevel::ForbiddenCloud, AiMode::Polish));
        assert!(svc.is_allowed(PrivacyLevel::Normal, AiMode::Polish));
        assert!(svc.is_allowed(PrivacyLevel::Sensitive, AiMode::Polish));
        assert!(!svc.is_allowed(PrivacyLevel::Sensitive, AiMode::SmartReply));
    }

    #[test]
    fn local_greeting_template() {
        let svc = AiAssistService::new();
        let req = TaskReq {
            editor_id: 1,
            mode: AiMode::Compose.raw(),
            scene_id: "greeting".into(),
            peer_message: String::new(),
            background_note: String::new(),
            selection_text: String::new(),
            user_intent: String::new(),
        };
        let out = svc.suggest(PrivacyLevel::Normal, &req).unwrap();
        assert!(out.local);
        assert_eq!(out.variants.len(), 3);
    }

    #[test]
    fn preview_redacts() {
        let svc = AiAssistService::new();
        let req = TaskReq {
            editor_id: 1,
            mode: AiMode::SmartReply.raw(),
            scene_id: "dating".into(),
            peer_message: "你们报价太高了12345".into(),
            background_note: "首次合作".into(),
            selection_text: String::new(),
            user_intent: String::new(),
        };
        let preview = svc.preview_payload(&req);
        assert!(preview.fields[0].value.ends_with("***"));
    }
}
