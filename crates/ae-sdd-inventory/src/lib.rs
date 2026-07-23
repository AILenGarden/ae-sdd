#![forbid(unsafe_code)]

//! Deterministic project inventory, selector fingerprints, and watch repair.

mod cache;
mod filesystem;
mod inventory;
mod selector;
mod yaml;

pub use cache::SelectorCacheIndex;
pub use filesystem::{FilesystemError, FilesystemInventory, FilesystemLimits, FilesystemScan};
pub use inventory::{
    ApplyWatchResult, FileRecord, Inventory, InventoryDelta, InventoryError, ReconcileReason,
    WatchBatch, WatchEvent,
};
pub use selector::{PathSelector, SelectorError, SelectorId};
pub use yaml::{YamlDocument, YamlError, YamlValue};
