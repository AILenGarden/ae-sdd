use std::{io, path::PathBuf};

use ae_sdd_domain::ProjectRelativePath;
use thiserror::Error;

use crate::{ParseError, ScannerId, ScopeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    Blocker,
    Warn,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannerFinding {
    pub severity: FindingSeverity,
    pub rule: Box<str>,
    pub path: ProjectRelativePath,
    pub line: usize,
    pub message: Box<str>,
}

impl ScannerFinding {
    pub fn new(
        severity: FindingSeverity,
        rule: impl Into<Box<str>>,
        path: ProjectRelativePath,
        line: usize,
        message: impl Into<Box<str>>,
    ) -> Self {
        Self {
            severity,
            rule: rule.into(),
            path,
            line,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanStatus {
    Pass,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanReport {
    scanner: ScannerId,
    status: ScanStatus,
    scanned_paths: Vec<ProjectRelativePath>,
    findings: Vec<ScannerFinding>,
}

impl ScanReport {
    pub fn new(
        scanner: ScannerId,
        mut scanned_paths: Vec<ProjectRelativePath>,
        mut findings: Vec<ScannerFinding>,
    ) -> Self {
        scanned_paths.sort();
        scanned_paths.dedup();
        findings.sort_by(|left, right| {
            left.severity
                .cmp(&right.severity)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.rule.cmp(&right.rule))
        });
        let status = if findings
            .iter()
            .any(|finding| finding.severity == FindingSeverity::Blocker)
        {
            ScanStatus::Fail
        } else {
            ScanStatus::Pass
        };
        Self {
            scanner,
            status,
            scanned_paths,
            findings,
        }
    }

    pub fn pass(scanner: ScannerId, scanned_paths: Vec<ProjectRelativePath>) -> Self {
        Self::new(scanner, scanned_paths, Vec::new())
    }

    pub const fn scanner(&self) -> ScannerId {
        self.scanner
    }

    pub const fn status(&self) -> ScanStatus {
        self.status
    }

    pub fn permits_gate(&self) -> bool {
        self.status == ScanStatus::Pass
    }

    pub fn scanned_paths(&self) -> &[ProjectRelativePath] {
        &self.scanned_paths
    }

    pub fn findings(&self) -> &[ScannerFinding] {
        &self.findings
    }
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error(transparent)]
    Scope(#[from] ScopeError),
    #[error("scanner selected no authoritative input files")]
    EmptyScope,
    #[error("scanner selected {actual} files, exceeding {maximum}")]
    FileCountLimit { actual: usize, maximum: usize },
    #[error("scanner input {path} has {actual} bytes, exceeding {maximum}")]
    FileByteLimit {
        path: ProjectRelativePath,
        actual: u64,
        maximum: u64,
    },
    #[error("failed to read scanner input {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Parse(#[from] ParseError),
}
