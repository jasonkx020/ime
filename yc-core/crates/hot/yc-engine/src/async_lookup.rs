//! Single-flight background pinyin lookup with latest-wins cancel.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use yc_lexicon::{DatLexicon, LookupCancel, LookupOpts};
use yc_types::Candidate;

struct LookupJob {
    gen: u64,
    composing: String,
    syllables: Vec<String>,
    lex: Arc<DatLexicon>,
    opts: LookupOpts,
}

struct LookupResult {
    gen: u64,
    composing: String,
    cands: Vec<Candidate>,
}

struct SharedState {
    cancel: Arc<LookupCancel>,
    /// Latest completed result (gen + composing + candidates).
    result: Mutex<Option<LookupResult>>,
    result_cv: Condvar,
    /// Generation currently being searched (0 = idle).
    inflight: Mutex<u64>,
}

/// Process-wide single worker; jobs are coalesced to the newest gen.
pub struct AsyncLookupHub {
    tx: Sender<LookupJob>,
    state: Arc<SharedState>,
}

impl AsyncLookupHub {
    pub fn shared() -> Arc<Self> {
        static CELL: std::sync::OnceLock<Arc<AsyncLookupHub>> = std::sync::OnceLock::new();
        CELL.get_or_init(|| Arc::new(Self::spawn())).clone()
    }

    /// Private hub for unit tests (avoids racing the process-wide singleton).
    #[cfg(test)]
    fn isolated() -> Arc<Self> {
        Arc::new(Self::spawn())
    }

    fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<LookupJob>();
        let state = Arc::new(SharedState {
            cancel: LookupCancel::shared(),
            result: Mutex::new(None),
            result_cv: Condvar::new(),
            inflight: Mutex::new(0),
        });
        let st = state.clone();
        thread::Builder::new()
            .name("yc-pinyin-lookup".into())
            .spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    // Drain to newest job only.
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    if st.cancel.is_canceled(job.gen) {
                        continue;
                    }
                    *st.inflight.lock() = job.gen;
                    let composing = job.composing.clone();
                    let cands = match job.lex.lookup_pinyin_opts(
                        &job.composing,
                        &job.syllables,
                        Some(st.cancel.as_ref()),
                        job.gen,
                        job.opts,
                    ) {
                        // User boost / LightIntel rerank runs on the IME thread after poll.
                        Some(c) => c,
                        None => {
                            *st.inflight.lock() = 0;
                            st.result_cv.notify_all();
                            continue;
                        }
                    };
                    if !st.cancel.is_canceled(job.gen) {
                        *st.result.lock() = Some(LookupResult {
                            gen: job.gen,
                            composing,
                            cands,
                        });
                    }
                    *st.inflight.lock() = 0;
                    st.result_cv.notify_all();
                }
            })
            .expect("spawn yc-pinyin-lookup");
        Self { tx, state }
    }

    pub fn cancel_token(&self) -> Arc<LookupCancel> {
        self.state.cancel.clone()
    }

    /// Bump generation and enqueue a lookup for `composing`.
    pub fn submit(
        &self,
        composing: String,
        syllables: Vec<String>,
        lex: Arc<DatLexicon>,
    ) -> u64 {
        let gen = self.state.cancel.bump();
        // Mark inflight immediately so poll returns 2 while the job is still queued.
        *self.state.inflight.lock() = gen;
        let _ = self.tx.send(LookupJob {
            gen,
            composing,
            syllables,
            lex,
            opts: LookupOpts::lean(),
        });
        gen
    }

    /// Take result only when gen is current **and** composing matches (avoids cross-engine races).
    pub fn take_if_matches(&self, composing: &str) -> Option<(u64, Vec<Candidate>)> {
        let cur = self.state.cancel.current();
        let mut guard = self.state.result.lock();
        match guard.as_ref() {
            Some(r) if r.gen == cur && r.composing == composing => {
                let r = guard.take().unwrap();
                Some((r.gen, r.cands))
            }
            _ => None,
        }
    }

    /// Block until current gen completes for `composing`, or timeout.
    pub fn wait_for(&self, composing: &str, timeout: Duration) -> Option<Vec<Candidate>> {
        let cur = self.state.cancel.current();
        if cur == 0 {
            return None;
        }
        let deadline = Instant::now() + timeout;
        let mut guard = self.state.result.lock();
        loop {
            if let Some(r) = guard.as_ref() {
                if r.gen == cur && r.composing == composing {
                    return Some(r.cands.clone());
                }
            }
            // Gen advanced by another submit — this wait is stale.
            if self.state.cancel.current() != cur {
                return None;
            }
            let now = Instant::now();
            if now >= deadline {
                return guard.as_ref().and_then(|r| {
                    if r.gen == cur && r.composing == composing {
                        Some(r.cands.clone())
                    } else {
                        None
                    }
                });
            }
            let wait = deadline.saturating_duration_since(now);
            let notified = self.state.result_cv.wait_for(&mut guard, wait);
            if notified.timed_out() {
                return guard.as_ref().and_then(|r| {
                    if r.gen == cur && r.composing == composing {
                        Some(r.cands.clone())
                    } else {
                        None
                    }
                });
            }
        }
    }

    pub fn has_inflight_current(&self) -> bool {
        let cur = self.state.cancel.current();
        cur != 0 && *self.state.inflight.lock() == cur
    }
}

/// Test helper: cancel mid-lookup by bumping after start.
#[allow(dead_code)]
static TEST_FLAG: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
mod tests {
    use super::*;
    use yc_lexicon::compile_tsv_to_dat;

    #[test]
    fn latest_wins_discards_stale_gen() {
        let tmp = std::env::temp_dir().join("yc_async_lookup_latest.tsv");
        let tsv = "word\tfreq\tpinyin\n你好\t100\tnihao\n你们\t90\tnimen\n";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = Arc::new(DatLexicon::from_bytes(dat).unwrap());
        let syls = vec!["ni".into(), "hao".into(), "men".into()];
        let hub = AsyncLookupHub::isolated();

        let g1 = hub.submit("ni".into(), syls.clone(), lex.clone());
        let g2 = hub.submit("nihao".into(), syls.clone(), lex.clone());
        assert!(g2 > g1);

        let cands = hub
            .wait_for("nihao", Duration::from_millis(500))
            .expect("latest lookup should complete");
        assert!(
            cands.iter().any(|c| c.text == "你好"),
            "got {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        assert_eq!(hub.cancel_token().current(), g2);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn wait_rejects_mismatched_composing() {
        let tmp = std::env::temp_dir().join("yc_async_lookup_mismatch.tsv");
        let tsv = "word\tfreq\tpinyin\n你好\t100\tnihao\n";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = Arc::new(DatLexicon::from_bytes(dat).unwrap());
        let syls = vec!["ni".into(), "hao".into()];
        let hub = AsyncLookupHub::isolated();
        let _ = hub.submit("nihao".into(), syls, lex);
        let _ = hub.wait_for("nihao", Duration::from_millis(500));
        // Stale wait for a different string must not return this result.
        assert!(hub.wait_for("ta", Duration::from_millis(20)).is_none());
        let _ = std::fs::remove_file(tmp);
    }
}
