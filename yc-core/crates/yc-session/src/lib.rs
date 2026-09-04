//! Session lifecycle and hot-path scheduling.

mod langpack;
mod manager;
mod scheduler;

pub use langpack::{hash_pack_id, EnabledLangPack};
pub use manager::SessionManager;
pub use scheduler::Scheduler;

use yc_engine::EngineFactory;
use yc_handwriting::HandwritingService;
use yc_lexicon::UserWordStore;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct CoreServices {
    pub sessions: SessionManager,
    pub scheduler: Scheduler,
    pub handwriting: HandwritingService,
}

impl CoreServices {
    pub fn new() -> Self {
        let factory = EngineFactory::new();
        let sessions = SessionManager::new();
        let handwriting = HandwritingService::new();
        let scheduler = Scheduler::new(factory);
        Self {
            sessions,
            scheduler,
            handwriting,
        }
    }

    pub fn with_user_words(store: Arc<Mutex<UserWordStore>>) -> Self {
        let factory = EngineFactory::with_user_words(store);
        let sessions = SessionManager::new();
        let handwriting = HandwritingService::new();
        let scheduler = Scheduler::new(factory);
        Self {
            sessions,
            scheduler,
            handwriting,
        }
    }
}

impl Default for CoreServices {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CoreServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreServices").finish_non_exhaustive()
    }
}
