//! The one way this driver puts something under a name it looks it up by.
//!
//! A map's own `insert` answers a key written twice by keeping the second value and handing back
//! the first, and a caller that does not look at what came back has let the document name two
//! things one way and kept whichever was read last. That is how a helper written twice, or a value,
//! or an entry, was once read: the second copy was checked and the first was compiled. So `insert`
//! is refused by the lint configuration (`clippy.toml`), and a name goes into an index here or not
//! at all. A place that means to replace what a key held — a scope a binder shadows — says so where
//! it does it.

use anyhow::{Result, bail};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// A map keyed by what the document names things by.
pub(crate) trait Index<K, V> {
    /// What was under `key` before, if anything, with `value` under it now.
    fn put(&mut self, key: K, value: V) -> Option<V>;
}

impl<K: Eq + Hash, V> Index<K, V> for HashMap<K, V> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place a key is put, and `once` and `unique` read what it answers"
    )]
    fn put(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

impl<K: Ord, V> Index<K, V> for BTreeMap<K, V> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place a key is put, and `once` and `unique` read what it answers"
    )]
    fn put(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

/// `value` under `key`, and a document naming two things by `key` refused as the two halves
/// disagreeing, with `twice` saying what was named twice.
pub(crate) fn once<K, V>(
    index: &mut impl Index<K, V>,
    key: K,
    value: V,
    twice: impl FnOnce() -> String,
) -> Result<()> {
    if index.put(key, value).is_some() {
        bail!("{}", twice());
    }
    Ok(())
}

/// `value` under `key`, where every key was already held to be named once before this runs, so a
/// key twice is this compiler's own mistake and not something a document can say.
pub(crate) fn unique<K, V>(index: &mut impl Index<K, V>, key: K, value: V) {
    assert!(
        index.put(key, value).is_none(),
        "a key named twice after `coherent` held every one of them to be named once"
    );
}
