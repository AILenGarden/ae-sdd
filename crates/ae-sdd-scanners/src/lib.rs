#![forbid(unsafe_code)]

//! In-process authenticity, depth, flow, and plugin scanners.

mod engine;
mod parser;
mod registry;
mod report;
mod scope;

pub use engine::{ScanRequest, ScannerEngine};
pub use parser::{ParseError, ParserKind, SourceParserRegistry};
pub use registry::{SCANNER_COUNT, ScanScopeKind, ScannerId, ScannerRegistry, ScannerSpec};
pub use report::{FindingSeverity, ScanError, ScanReport, ScanStatus, ScannerFinding};
pub use scope::{
    ExcludedPath, RaClassification, ResolvedScanScope, ScopeError, classify_formal_ra,
    resolve_scan_scope,
};
