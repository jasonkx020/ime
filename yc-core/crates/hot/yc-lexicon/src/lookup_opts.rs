//! Cooperative cancel + lean lookup options for pinyin candidate search.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::seg_hypotheses::SegmentMode;

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
///
/// Instant / Lean / Expanded share one Gen pipeline; only budgets differ
/// (`segment_mode`, fuzzy/typo caps, `pool_limit`).
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
    /// Enable 1-edit insert/delete/transpose (漏字等).
    pub enable_edit1: bool,
    /// Cap for edit1 variants.
    pub edit1_variant_cap: usize,
    /// Structural Seg depth (same Gen, different budget).
    pub segment_mode: SegmentMode,
}

impl LookupOpts {
    /// Backward-compat: true when lattice / multi-span Gen runs.
    pub fn enable_segment(&self) -> bool {
        matches!(
            self.segment_mode,
            SegmentMode::Full | SegmentMode::FullFine
        )
    }

    /// Backward-compat: fine letter splits under first syllable.
    pub fn enable_fine_segment(&self) -> bool {
        self.segment_mode == SegmentMode::FullFine
    }

    /// Same-frame fill on the UI thread: full pinyin + jianpin only.
    /// Fuzzy / typo wait for the background lean upgrade; first-syl Gen still runs.
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
            enable_edit1: false,
            edit1_variant_cap: 0,
            segment_mode: SegmentMode::FirstSyllable,
        }
    }

    /// Background path: same pool as instant, plus fuzzy/typo/full lattice.
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
            enable_edit1: true,
            edit1_variant_cap: 48,
            segment_mode: SegmentMode::Full,
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
            enable_edit1: true,
            edit1_variant_cap: 64,
            segment_mode: SegmentMode::FullFine,
        }
    }
}

impl Default for LookupOpts {
    fn default() -> Self {
        Self::lean()
    }
}
