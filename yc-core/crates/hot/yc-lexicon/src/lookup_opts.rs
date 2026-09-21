//! Cooperative cancel + lean lookup options for pinyin candidate search.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Shared generation token: bump on each new composing update to cancel older lookups.
#[derive(Debug, Default)]
pub struct LookupCancel {
    gen: AtomicU64,
}

impl LookupCancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Advance generation; returns the new generation id for the caller to own.
    pub fn bump(&self) -> u64 {
        self.gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current(&self) -> u64 {
        self.gen.load(Ordering::SeqCst)
    }

    pub fn is_canceled(&self, mine: u64) -> bool {
        self.current() != mine
    }
}

/// Limits / feature flags for a single lookup pass.
#[derive(Debug, Clone, Copy)]
pub struct LookupOpts {
    /// Max candidates retained after ranking.
    pub pool_limit: usize,
    /// Neighbor-key typo expansion.
    pub enable_typo: bool,
    /// How many adjacent-key variants to try (instant stays tiny).
    pub typo_variant_cap: usize,
    /// Skip typo when this many hits are already collected.
    pub typo_max_hits: usize,
    /// zh/z, an/ang style fuzzy rewrites.
    pub enable_fuzzy: bool,
    /// How many fuzzy variants to try. Instant uses 1.
    pub fuzzy_variant_cap: usize,
    /// Jianpin scan when exact hits are scarce.
    pub enable_jianpin: bool,
    /// Skip jianpin if exact (non-jianpin) hits already reach this count.
    pub skip_jianpin_if_exact_ge: usize,
    /// Minimum composing length before typo expansion runs.
    pub typo_min_len: usize,
    /// Sentence lattice merge.
    pub enable_segment: bool,
}

impl LookupOpts {
    /// Same-frame fill on the UI thread: full pinyin + jianpin only.
    /// Segment / fuzzy / typo wait for the background lean upgrade.
    pub fn instant() -> Self {
        Self {
            pool_limit: 10,
            enable_typo: false,
            typo_variant_cap: 0,
            typo_max_hits: 0,
            enable_fuzzy: false,
            fuzzy_variant_cap: 0,
            enable_jianpin: true,
            skip_jianpin_if_exact_ge: usize::MAX,
            typo_min_len: 2,
            enable_segment: false,
        }
    }

    /// Background path: same pool as instant, plus fuzzy/typo/segment.
    pub fn lean() -> Self {
        Self {
            pool_limit: 10,
            enable_typo: true,
            typo_variant_cap: 24,
            typo_max_hits: 8,
            enable_fuzzy: true,
            fuzzy_variant_cap: 4,
            enable_jianpin: true,
            skip_jianpin_if_exact_ge: usize::MAX,
            typo_min_len: 2,
            enable_segment: true,
        }
    }

    /// Expand path when user pages / scrolls for more candidates.
    pub fn expanded() -> Self {
        Self {
            pool_limit: 256,
            enable_typo: true,
            typo_variant_cap: 24,
            typo_max_hits: 8,
            enable_fuzzy: true,
            fuzzy_variant_cap: 4,
            enable_jianpin: true,
            skip_jianpin_if_exact_ge: usize::MAX,
            typo_min_len: 2,
            enable_segment: true,
        }
    }
}

impl Default for LookupOpts {
    fn default() -> Self {
        Self::lean()
    }
}
