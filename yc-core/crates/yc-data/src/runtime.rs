use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use yc_pack::install_pack_to_dir;
use yc_theme::ThemeRuntime;
use yc_types::{ColdKind, EditorId, EngineError, HotResult, TaskId};

use crate::repository::Repository;

pub type ColdCallback = Box<dyn Fn(TaskId, EditorId, Vec<u8>, i32) + Send + Sync>;

struct ColdJob {
    task_id: TaskId,
    editor_id: EditorId,
    kind: ColdKind,
    payload: Vec<u8>,
}

pub struct ColdPathRuntime {
    repo: Repository,
    theme: Arc<ThemeRuntime>,
    tx: Sender<ColdJob>,
    _worker: JoinHandle<()>,
    next_task: AtomicU64,
    callback: Arc<parking_lot::RwLock<Option<ColdCallback>>>,
    #[cfg(feature = "plugin")]
    plugin: Arc<parking_lot::Mutex<yc_plugin::PluginHost>>,
}
impl ColdPathRuntime {
    pub fn new(data_dir: PathBuf) -> Self {
        let repo = Repository::new(data_dir.clone());
        let theme = Arc::new(ThemeRuntime::new());
        let theme_worker = theme.clone();
        let (tx, rx) = mpsc::channel::<ColdJob>();
        let mut repo_worker = Repository::new(data_dir.clone());
        let callback_slot: Arc<parking_lot::RwLock<Option<ColdCallback>>> =
            Arc::new(parking_lot::RwLock::new(None));
        let cb_for_thread = callback_slot.clone();
        #[cfg(feature = "plugin")]
        let plugin = Arc::new(parking_lot::Mutex::new(yc_plugin::PluginHost::new(
            data_dir.clone(),
        )));
        #[cfg(feature = "plugin")]
        let plugin_worker = plugin.clone();

        let worker = thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                let (payload, err) = Self::run_job(
                    &mut repo_worker,
                    &theme_worker,
                    #[cfg(feature = "plugin")]
                    &plugin_worker,
                    job.kind,
                    &job.payload,
                );
                if let Some(cb) = cb_for_thread.read().as_ref() {
                    cb(job.task_id, job.editor_id, payload, err);
                }
            }
        });

        Self {
            repo,
            theme,
            tx,
            _worker: worker,
            next_task: AtomicU64::new(1),
            callback: callback_slot,
            #[cfg(feature = "plugin")]
            plugin,
        }
    }

    pub fn repository(&self) -> &Repository {
        &self.repo
    }

    pub fn theme(&self) -> &ThemeRuntime {
        &self.theme
    }

    #[cfg(feature = "plugin")]
    pub fn plugin(&self) -> parking_lot::MutexGuard<'_, yc_plugin::PluginHost> {
        self.plugin.lock()
    }

    pub fn set_callback(&self, cb: ColdCallback) {
        *self.callback.write() = Some(cb);
    }

    pub fn submit(
        &self,
        editor_id: EditorId,
        kind: u32,
        payload: &[u8],
    ) -> HotResult<TaskId> {
        let kind = ColdKind::from_raw(kind).ok_or(EngineError::Unsupported)?;
        let task_id = TaskId(self.next_task.fetch_add(1, Ordering::Relaxed));
        self.tx
            .send(ColdJob {
                task_id,
                editor_id,
                kind,
                payload: payload.to_vec(),
            })
            .map_err(|_| EngineError::Busy)?;
        Ok(task_id)
    }

    pub fn cancel(&self, _task_id: TaskId) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }

    fn run_job(
        repo: &mut Repository,
        theme: &ThemeRuntime,
        #[cfg(feature = "plugin")] plugin: &Arc<parking_lot::Mutex<yc_plugin::PluginHost>>,
        kind: ColdKind,
        payload: &[u8],
    ) -> (Vec<u8>, i32) {
        match kind {
            ColdKind::Skin => {
                let path = String::from_utf8_lossy(payload);
                match theme.load_pack(std::path::Path::new(path.trim())) {
                    Ok(tokens) => {
                        let _ = repo.set_config("skin_id", tokens.skin_id.as_bytes());
                        (tokens.to_json_bytes(), 0)
                    }
                    Err(_) => (Vec::new(), -1),
                }
            }
            ColdKind::LangPackInstall => {
                #[cfg(feature = "plugin")]
                {
                    let path = String::from_utf8_lossy(payload);
                    match install_pack_to_dir(
                        std::path::Path::new(path.trim()),
                        &repo.langpacks_dir(),
                    ) {
                        Ok(m) => {
                            let mut host = plugin.lock();
                            let _ = host.register_installed(&m);
                            (serde_json::to_vec(&m.id).unwrap_or_default(), 0)
                        }
                        Err(_) => (Vec::new(), -1),
                    }
                }
                #[cfg(not(feature = "plugin"))]
                {
                    let _ = (repo, payload);
                    (Vec::new(), -1)
                }
            }
            ColdKind::LangPackEnable => {
                #[cfg(feature = "plugin")]
                {
                    let pack_id = String::from_utf8_lossy(payload);
                    match plugin.lock().enable(&pack_id) {
                        Ok(_slot) => (pack_id.as_bytes().to_vec(), 0),
                        Err(_) => (Vec::new(), -1),
                    }
                }
                #[cfg(not(feature = "plugin"))]
                (Vec::new(), -1)
            }
            ColdKind::LangPackDisable => {
                #[cfg(feature = "plugin")]
                {
                    let pack_id = String::from_utf8_lossy(payload);
                    match plugin.lock().disable(&pack_id) {
                        Ok(()) => (pack_id.as_bytes().to_vec(), 0),
                        Err(_) => (Vec::new(), -1),
                    }
                }
                #[cfg(not(feature = "plugin"))]
                (Vec::new(), -1)
            }
            ColdKind::LangPackCatalog => {
                #[cfg(feature = "plugin")]
                {
                    let payload_str = String::from_utf8_lossy(payload);
                    let trimmed = payload_str.trim();
                    let mut host = plugin.lock();
                    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        match host.fetch_catalog(trimmed) {
                            Ok(cat) => (serde_json::to_vec(&cat).unwrap_or_default(), 0),
                            Err(_) => (Vec::new(), -1),
                        }
                    } else {
                        let entries = host.list_catalog();
                        (serde_json::to_vec(&entries).unwrap_or_default(), 0)
                    }
                }
                #[cfg(not(feature = "plugin"))]
                (Vec::new(), -1)
            }
            ColdKind::HandwritingCloud => {
                // Stub cloud: echo enhanced candidate JSON
                let preview = serde_json::json!({
                    "candidates": ["云识别", "你好"],
                    "confidence": 0.85
                });
                (serde_json::to_vec(&preview).unwrap_or_default(), 0)
            }
            ColdKind::AiPolish | ColdKind::AiAssist => {
                match serde_json::from_slice::<yc_types::TaskReq>(payload) {
                    Ok(req) => {
                        let svc = yc_ai::AiAssistService::new();
                        let privacy = yc_types::PrivacyLevel::Normal;
                        let result = if kind == ColdKind::AiPolish {
                            svc.polish(privacy, &req)
                        } else {
                            svc.suggest(privacy, &req)
                        };
                        match result {
                            Ok(out) => (serde_json::to_vec(&out).unwrap_or_default(), 0),
                            Err(_) => (Vec::new(), -1),
                        }
                    }
                    Err(_) => (Vec::new(), -1),
                }
            }
        }
    }
}
