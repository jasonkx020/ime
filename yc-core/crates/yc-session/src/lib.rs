//! Session lifecycle and hot-path scheduling.

mod manager;
mod scheduler;

pub use manager::SessionManager;
pub use scheduler::Scheduler;

use yc_engine::EngineFactory;

#[derive(Debug)]
pub struct CoreServices {
    pub sessions: SessionManager,
    pub scheduler: Scheduler,
}

impl CoreServices {
    pub fn new() -> Self {
        let factory = EngineFactory::new();
        let sessions = SessionManager::new();
        let scheduler = Scheduler::new(factory);
        Self {
            sessions,
            scheduler,
        }
    }
}

impl Default for CoreServices {
    fn default() -> Self {
        Self::new()
    }
}
