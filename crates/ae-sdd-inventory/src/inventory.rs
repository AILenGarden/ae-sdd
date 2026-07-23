use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_domain::{
    ArtifactDigest, CounterError, InputFingerprint, InventoryGeneration, ProjectRelativePath,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PathSelector, SelectorId};

/// Content-addressed inventory record for one regular file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    path: ProjectRelativePath,
    digest: ArtifactDigest,
    byte_length: u64,
}

impl FileRecord {
    pub const fn new(path: ProjectRelativePath, digest: ArtifactDigest, byte_length: u64) -> Self {
        Self {
            path,
            digest,
            byte_length,
        }
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    Upsert(FileRecord),
    Remove(ProjectRelativePath),
}

impl WatchEvent {
    fn path(&self) -> &ProjectRelativePath {
        match self {
            Self::Upsert(record) => record.path(),
            Self::Remove(path) => path,
        }
    }
}

/// One ordered watcher delivery. Sequence numbers must be contiguous.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchBatch {
    pub sequence: u64,
    pub overflowed: bool,
    pub events: Vec<WatchEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconcileReason {
    WatchOverflow,
    SequenceGap { expected: u64, actual: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryDelta {
    pub previous_generation: InventoryGeneration,
    pub generation: InventoryGeneration,
    pub changed_paths: BTreeSet<ProjectRelativePath>,
    pub invalidated_selectors: BTreeSet<SelectorId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyWatchResult {
    Applied(InventoryDelta),
    FullReconcileRequired(ReconcileReason),
}

/// Authoritative in-memory inventory projection with deterministic fingerprints.
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    generation: InventoryGeneration,
    records: BTreeMap<ProjectRelativePath, FileRecord>,
    selectors: BTreeMap<SelectorId, PathSelector>,
    reverse: BTreeMap<ProjectRelativePath, BTreeSet<SelectorId>>,
    last_watch_sequence: Option<u64>,
}

impl Inventory {
    pub const fn generation(&self) -> InventoryGeneration {
        self.generation
    }

    pub fn records(&self) -> impl Iterator<Item = &FileRecord> {
        self.records.values()
    }

    pub fn register_selector(&mut self, id: SelectorId, selector: PathSelector) {
        if self.selectors.insert(id.clone(), selector).is_some() {
            for ids in self.reverse.values_mut() {
                ids.remove(&id);
            }
        }
        self.reindex_selector(&id);
    }

    pub fn selector(&self, id: &SelectorId) -> Option<&PathSelector> {
        self.selectors.get(id)
    }

    pub fn fingerprint(&self, id: &SelectorId) -> Result<InputFingerprint, InventoryError> {
        let selector = self
            .selectors
            .get(id)
            .ok_or_else(|| InventoryError::UnknownSelector(id.clone()))?;
        Ok(self.fingerprint_for(selector))
    }

    pub fn fingerprint_for(&self, selector: &PathSelector) -> InputFingerprint {
        let mut hasher = Sha256::new();
        hasher.update(b"ae-sdd-selector-fingerprint/v1\0");
        for record in self
            .records
            .values()
            .filter(|record| selector.matches(record.path()))
        {
            hash_bytes(&mut hasher, record.path().as_str().as_bytes());
            hasher.update(record.digest().as_bytes());
            hasher.update(record.byte_length().to_be_bytes());
        }
        InputFingerprint::from_array(hasher.finalize().into())
    }

    pub fn apply_watch_batch(
        &mut self,
        batch: WatchBatch,
    ) -> Result<ApplyWatchResult, InventoryError> {
        if batch.overflowed {
            return Ok(ApplyWatchResult::FullReconcileRequired(
                ReconcileReason::WatchOverflow,
            ));
        }
        if let Some(previous) = self.last_watch_sequence {
            let expected = previous
                .checked_add(1)
                .ok_or(InventoryError::WatchSequenceOverflow)?;
            if batch.sequence != expected {
                return Ok(ApplyWatchResult::FullReconcileRequired(
                    ReconcileReason::SequenceGap {
                        expected,
                        actual: batch.sequence,
                    },
                ));
            }
        }

        let mut events = BTreeMap::new();
        for event in batch.events {
            events.insert(event.path().clone(), event);
        }
        self.last_watch_sequence = Some(batch.sequence);

        let previous_generation = self.generation;
        let mut changed_paths = BTreeSet::new();
        let mut invalidated_selectors = BTreeSet::new();
        for (path, event) in events {
            let previous = self.records.get(&path).cloned();
            let changed = match event {
                WatchEvent::Upsert(record) => {
                    if previous.as_ref() == Some(&record) {
                        false
                    } else {
                        self.records.insert(path.clone(), record);
                        true
                    }
                }
                WatchEvent::Remove(_) => self.records.remove(&path).is_some(),
            };
            if !changed {
                continue;
            }

            changed_paths.insert(path.clone());
            if let Some(previous_ids) = self.reverse.remove(&path) {
                invalidated_selectors.extend(previous_ids);
            }
            let current_ids = self.matching_selectors(&path);
            invalidated_selectors.extend(current_ids.iter().cloned());
            if self.records.contains_key(&path) && !current_ids.is_empty() {
                self.reverse.insert(path, current_ids);
            }
        }

        if !changed_paths.is_empty() {
            self.generation = self.generation.checked_next()?;
        }
        Ok(ApplyWatchResult::Applied(InventoryDelta {
            previous_generation,
            generation: self.generation,
            changed_paths,
            invalidated_selectors,
        }))
    }

    /// Replaces the complete inventory after watcher overflow or a sequence gap.
    ///
    /// A full reconcile always advances generation and invalidates all registered
    /// selectors, even when the resulting file set is identical.
    pub fn full_reconcile(
        &mut self,
        records: impl IntoIterator<Item = FileRecord>,
    ) -> Result<InventoryDelta, InventoryError> {
        let next_records: BTreeMap<_, _> = records
            .into_iter()
            .map(|record| (record.path().clone(), record))
            .collect();
        let mut changed_paths: BTreeSet<_> = self
            .records
            .keys()
            .chain(next_records.keys())
            .filter(|path| self.records.get(*path) != next_records.get(*path))
            .cloned()
            .collect();
        if changed_paths.is_empty() {
            changed_paths.extend(next_records.keys().cloned());
        }

        let previous_generation = self.generation;
        self.generation = self.generation.checked_next()?;
        self.records = next_records;
        self.last_watch_sequence = None;
        self.rebuild_reverse();
        Ok(InventoryDelta {
            previous_generation,
            generation: self.generation,
            changed_paths,
            invalidated_selectors: self.selectors.keys().cloned().collect(),
        })
    }

    fn matching_selectors(&self, path: &ProjectRelativePath) -> BTreeSet<SelectorId> {
        self.selectors
            .iter()
            .filter(|(_, selector)| selector.matches(path))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn reindex_selector(&mut self, id: &SelectorId) {
        let Some(selector) = self.selectors.get(id) else {
            return;
        };
        for path in self.records.keys() {
            if selector.matches(path) {
                self.reverse
                    .entry(path.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }
    }

    fn rebuild_reverse(&mut self) {
        self.reverse.clear();
        let ids: Vec<_> = self.selectors.keys().cloned().collect();
        for id in ids {
            self.reindex_selector(&id);
        }
    }
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("unknown selector {0}")]
    UnknownSelector(SelectorId),
    #[error("watch sequence overflowed")]
    WatchSequenceOverflow,
    #[error(transparent)]
    Counter(#[from] CounterError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, content: &[u8]) -> FileRecord {
        FileRecord::new(
            ProjectRelativePath::new(path).expect("valid path"),
            ArtifactDigest::digest(content),
            content.len() as u64,
        )
    }

    #[test]
    fn selector_fingerprint_is_order_independent_and_content_sensitive() {
        let selector = SelectorId::new("rust-source").expect("valid selector");
        let mut first = Inventory::default();
        first.register_selector(
            selector.clone(),
            PathSelector::extension("rs").expect("valid extension"),
        );
        first
            .full_reconcile([record("src/b.rs", b"b"), record("src/a.rs", b"a")])
            .expect("reconcile");

        let mut second = Inventory::default();
        second.register_selector(
            selector.clone(),
            PathSelector::extension("rs").expect("valid extension"),
        );
        second
            .full_reconcile([record("src/a.rs", b"a"), record("src/b.rs", b"b")])
            .expect("reconcile");
        assert_eq!(
            first.fingerprint(&selector).expect("fingerprint"),
            second.fingerprint(&selector).expect("fingerprint")
        );

        let result = second
            .apply_watch_batch(WatchBatch {
                sequence: 1,
                overflowed: false,
                events: vec![WatchEvent::Upsert(record("src/a.rs", b"changed"))],
            })
            .expect("watch applied");
        assert!(matches!(result, ApplyWatchResult::Applied(_)));
        assert_ne!(
            first.fingerprint(&selector).expect("fingerprint"),
            second.fingerprint(&selector).expect("fingerprint")
        );
    }

    #[test]
    fn overflow_and_sequence_gap_require_full_reconcile_without_mutation() {
        let mut inventory = Inventory::default();
        inventory
            .full_reconcile([record("src/lib.rs", b"v1")])
            .expect("reconcile");
        let generation = inventory.generation();

        assert_eq!(
            inventory
                .apply_watch_batch(WatchBatch {
                    sequence: 1,
                    overflowed: true,
                    events: vec![WatchEvent::Remove(
                        ProjectRelativePath::new("src/lib.rs").expect("valid path"),
                    )],
                })
                .expect("overflow reported"),
            ApplyWatchResult::FullReconcileRequired(ReconcileReason::WatchOverflow)
        );
        assert_eq!(inventory.generation(), generation);

        inventory
            .apply_watch_batch(WatchBatch {
                sequence: 4,
                overflowed: false,
                events: vec![],
            })
            .expect("first sequence establishes cursor");
        assert_eq!(
            inventory
                .apply_watch_batch(WatchBatch {
                    sequence: 6,
                    overflowed: false,
                    events: vec![],
                })
                .expect("gap reported"),
            ApplyWatchResult::FullReconcileRequired(ReconcileReason::SequenceGap {
                expected: 5,
                actual: 6,
            })
        );
    }
}
