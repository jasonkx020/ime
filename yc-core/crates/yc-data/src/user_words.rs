//! User-word persistence helpers for yc-data.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::UserWordStore;

/// Open (or create) the on-disk user word table under `{data_dir}/user_words.tsv`.
pub fn open_user_words(data_dir: impl AsRef<Path>) -> Arc<Mutex<UserWordStore>> {
    let path = data_dir.as_ref().join("user_words.tsv");
    UserWordStore::open_or_create(path)
}
