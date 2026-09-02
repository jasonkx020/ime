use std::path::PathBuf;

use yc_session::CoreServices;
use yc_types::{EditorFingerprint, EditorId, SessionStopReason, UserAction};

use crate::arena::HotArena;

pub struct CoreState {
    pub data_dir: PathBuf,
    pub services: CoreServices,
    pub arena: HotArena,
    pub initialized: bool,
}

impl CoreState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            services: CoreServices::new(),
            arena: HotArena::new(),
            initialized: true,
        }
    }

    pub fn begin_session(&mut self, field_id: u64, input_type: u32) -> EditorId {
        let fp = EditorFingerprint {
            package_name: String::new(),
            field_id,
            input_type,
            ime_options: 0,
            hint_hash: 0,
        };
        let id = self.services.sessions.create(fp);
        self.services.sessions.activate(id);
        self.services.scheduler.on_session_created(id);
        let _ = self.services.scheduler.handle(
            &mut self.services.sessions,
            id,
            UserAction::Init,
        );
        id
    }

    pub fn stop_session(&mut self, editor_id: EditorId, reason: SessionStopReason) {
        self.services.scheduler.on_session_stopped(editor_id);
        self.services.sessions.stop(editor_id, reason);
    }

    pub fn submit_action(&mut self, editor_id: EditorId, action: UserAction) -> i32 {
        use yc_types::{YC_ERR_BUSY, YC_ERR_SESSION, YC_OK};

        match self
            .services
            .scheduler
            .handle(&mut self.services.sessions, editor_id, action)
        {
            Ok(outcome) => {
                self.arena
                    .write_snapshot(&outcome.snapshot, &outcome.commands);
                YC_OK
            }
            Err(yc_types::EngineError::SessionInvalid) => YC_ERR_SESSION,
            Err(_) => YC_ERR_BUSY,
        }
    }
}
