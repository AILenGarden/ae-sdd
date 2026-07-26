//! Bounded in-memory source-read cache for the execution Hook fast path.
//!
//! Repeated reads of the same source range are one of the largest avoidable
//! context costs during a supervised slice.  This cache deduplicates them:
//! the key is `workspace + canonical path + content digest + range`, so the
//! same key hits while a changed content digest or a shifted range misses.
//!
//! Hard bounds (implementation plan Task 10):
//!
//! - entries live in one daemon-wide LRU; only the digest, the range and an
//!   excerpt of at most [`MAX_SOURCE_READ_EXCERPT_BYTES`] are retained;
//! - excerpts are returned under per-session visibility — an entry stored by
//!   one session is never visible to another session or another workspace;
//! - the cache is rebuildable runtime metadata held in memory only; source
//!   bodies are never persisted to SQLite, the project tree or durable events.
//!
//! The module is deliberately free of crate-internal imports so the
//! integration tests can include it verbatim and drive it directly.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use sha2::{Digest, Sha256};

/// Maximum retained excerpt bytes per cache entry (24 KiB).
pub const MAX_SOURCE_READ_EXCERPT_BYTES: usize = 24 * 1024;

/// Visibility scope of one cache caller.
///
/// The daemon never returns an entry outside the session that stored it, so
/// cross-session and cross-workspace leakage stays at zero even when the
/// underlying spec key is identical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceReadVisibility<'a> {
    workspace_id: &'a str,
    session_id: &'a str,
}

impl<'a> SourceReadVisibility<'a> {
    /// Creates the visibility scope for one authenticated session.
    pub const fn new(workspace_id: &'a str, session_id: &'a str) -> Self {
        Self {
            workspace_id,
            session_id,
        }
    }

    /// Returns the workspace the caller operates in.
    pub const fn workspace_id(&self) -> &'a str {
        self.workspace_id
    }

    /// Returns the authenticated session identity.
    pub const fn session_id(&self) -> &'a str {
        self.session_id
    }
}

/// Canonical source-read cache key: workspace + canonical path + content
/// digest + optional 1-based inclusive line range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReadKey {
    workspace_id: Box<str>,
    canonical_path: Box<str>,
    content_digest: Box<str>,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

impl SourceReadKey {
    /// Creates one key, canonicalizing the path separators so the slash and
    /// backslash spellings of one project-relative path share an entry.
    pub fn new(
        workspace_id: &str,
        path: &str,
        content_digest: &str,
        start_line: Option<u32>,
        end_line: Option<u32>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            canonical_path: path.replace('\\', "/").into(),
            content_digest: content_digest.into(),
            start_line,
            end_line,
        }
    }

    /// Deterministic storage identity of this key inside one session scope.
    ///
    /// Both workspace identities are mixed in so a session can never address
    /// an entry outside its own workspace, even with a spoofed key.
    fn storage_identity(&self, visibility: &SourceReadVisibility<'_>) -> Box<str> {
        let range_start = self
            .start_line
            .map_or("-".to_owned(), |line| line.to_string());
        let range_end = self
            .end_line
            .map_or("-".to_owned(), |line| line.to_string());
        format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            visibility.workspace_id(),
            visibility.session_id(),
            self.workspace_id,
            self.canonical_path,
            self.content_digest,
            range_start,
            range_end,
        )
        .into()
    }

    /// Stable, body-free reference a client can correlate with a cached read.
    fn reference(identity: &str) -> Box<str> {
        format!(
            "source-read:{}",
            hex::encode(Sha256::digest(identity.as_bytes()))
        )
        .into()
    }
}

/// Bounded excerpt of one cached source read.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceReadEntry {
    excerpt: Box<str>,
    reference: Box<str>,
    last_used: u64,
}

/// Cumulative, body-free cache counters (persisted as rebuildable metadata by
/// the runtime-metadata migration, never holding source content).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceReadCacheStats {
    /// Cache hits (same key, same session).
    pub hits: u64,
    /// Cache misses (unknown, invalidated or foreign key).
    pub misses: u64,
    /// Stored excerpts.
    pub stores: u64,
    /// Entries evicted by the bounded LRU.
    pub evictions: u64,
}

#[derive(Debug, Default)]
struct SourceReadCacheInner {
    entries: BTreeMap<Box<str>, SourceReadEntry>,
    tick: u64,
    stats: SourceReadCacheStats,
}

/// Daemon-wide bounded LRU of source-read excerpts.
///
/// All operations are deterministic and bounded: lookups and inserts are
/// `O(log n)`, eviction scans at most `capacity` entries, and the retained
/// memory is capped at `capacity * MAX_SOURCE_READ_EXCERPT_BYTES` plus key
/// material.  A poisoned mutex is recovered rather than panicking the Hook
/// fast path, because a cache can always be rebuilt.
#[derive(Debug, Default)]
pub struct SourceReadCache {
    inner: Mutex<SourceReadCacheInner>,
}

impl SourceReadCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the stable reference of the cached excerpt for this exact key,
    /// or `None` on a miss (unknown key, changed digest/range, or an entry
    /// owned by another session or workspace).
    pub fn get(
        &self,
        visibility: &SourceReadVisibility<'_>,
        key: &SourceReadKey,
    ) -> Option<Box<str>> {
        let identity = key.storage_identity(visibility);
        let mut inner = self.lock();
        inner.tick = inner.tick.saturating_add(1);
        let tick = inner.tick;
        if let Some(entry) = inner.entries.get_mut(&identity) {
            entry.last_used = tick;
            let reference = entry.reference.clone();
            inner.stats.hits = inner.stats.hits.saturating_add(1);
            Some(reference)
        } else {
            inner.stats.misses = inner.stats.misses.saturating_add(1);
            None
        }
    }

    /// Returns the retained excerpt for this exact key under the caller's
    /// session visibility, without affecting LRU recency or statistics.
    // Exercised by the integration tests that include this module verbatim and
    // by the excerpt retrieval rollout stage.
    #[allow(dead_code)]
    pub fn excerpt(
        &self,
        visibility: &SourceReadVisibility<'_>,
        key: &SourceReadKey,
    ) -> Option<Box<str>> {
        let identity = key.storage_identity(visibility);
        self.lock()
            .entries
            .get(&identity)
            .map(|entry| entry.excerpt.clone())
    }

    /// Stores the bounded excerpt for one read, truncating the body to
    /// [`MAX_SOURCE_READ_EXCERPT_BYTES`] at a UTF-8 boundary, evicting the
    /// least recently used entry when the cache is at `capacity`.
    ///
    /// Returns the stable reference regardless of capacity so callers can
    /// always correlate a read; with `capacity == 0` nothing is retained.
    pub fn put(
        &self,
        visibility: &SourceReadVisibility<'_>,
        key: &SourceReadKey,
        body: &str,
        capacity: usize,
    ) -> Box<str> {
        let identity = key.storage_identity(visibility);
        let reference = SourceReadKey::reference(&identity);
        if capacity == 0 {
            return reference;
        }
        let mut inner = self.lock();
        inner.stats.stores = inner.stats.stores.saturating_add(1);
        if !inner.entries.contains_key(&identity) {
            while inner.entries.len() >= capacity {
                let Some(oldest) = inner
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(identity, _)| identity.clone())
                else {
                    break;
                };
                inner.entries.remove(&oldest);
                inner.stats.evictions = inner.stats.evictions.saturating_add(1);
            }
        }
        inner.tick = inner.tick.saturating_add(1);
        let last_used = inner.tick;
        inner.entries.insert(
            identity,
            SourceReadEntry {
                excerpt: truncate_excerpt(body).into(),
                reference: reference.clone(),
                last_used,
            },
        );
        reference
    }

    /// Returns the cumulative body-free cache counters.
    // Exercised by the integration tests that include this module verbatim and
    // persisted as rebuildable metadata by the runtime-metadata migration.
    #[allow(dead_code)]
    pub fn stats(&self) -> SourceReadCacheStats {
        self.lock().stats
    }

    /// Returns the number of retained entries.
    // Exercised by the integration tests that include this module verbatim.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    fn lock(&self) -> MutexGuard<'_, SourceReadCacheInner> {
        // A poisoned cache mutex still holds rebuildable data; the Hook fast
        // path must not panic on it.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Truncates `body` to at most [`MAX_SOURCE_READ_EXCERPT_BYTES`] bytes without
/// splitting a UTF-8 character.
fn truncate_excerpt(body: &str) -> &str {
    let mut end = body.len().min(MAX_SOURCE_READ_EXCERPT_BYTES);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    &body[..end]
}
