/// The exact native scanner identities migrated from `scripts/*_scan.py`.
///
/// Task 6 (RA generality refactor) adds `RaCore`, `RaApplicability`, and
/// `RaClosure`, which all share the single `parse_ra_specification` parser and
/// filter its findings by rule-id prefix. The legacy `RaAuthenticity`,
/// `RaFlowViolation`, `RaDepth`, and `RaImplementation` ids remain as
/// deprecated direct diagnostics; the Gate registry no longer references them
/// after Task 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScannerId {
    CodingAuthenticity,
    TestAuthenticity,
    RaAuthenticity,
    RaFlowViolation,
    RaDepth,
    RaImplementation,
    RaCore,
    RaApplicability,
    RaClosure,
    PluginContent,
}

impl ScannerId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodingAuthenticity => "coding-authenticity",
            Self::TestAuthenticity => "test-authenticity",
            Self::RaAuthenticity => "ra-authenticity",
            Self::RaFlowViolation => "ra-flow-violation",
            Self::RaDepth => "ra-depth",
            Self::RaImplementation => "ra-implementation",
            Self::RaCore => "ra-core",
            Self::RaApplicability => "ra-applicability",
            Self::RaClosure => "ra-closure",
            Self::PluginContent => "plugin-content",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanScopeKind {
    Production,
    TestsAndEvidence,
    FormalRa,
    Plugin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScannerSpec {
    pub id: ScannerId,
    pub legacy_source: &'static str,
    pub scope: ScanScopeKind,
}

pub const SCANNER_COUNT: usize = 10;

const SCANNERS: [ScannerSpec; SCANNER_COUNT] = [
    ScannerSpec {
        id: ScannerId::CodingAuthenticity,
        legacy_source: "scripts/coding_authenticity_scan.py",
        scope: ScanScopeKind::Production,
    },
    ScannerSpec {
        id: ScannerId::TestAuthenticity,
        legacy_source: "scripts/test_authenticity_scan.py",
        scope: ScanScopeKind::TestsAndEvidence,
    },
    ScannerSpec {
        id: ScannerId::RaAuthenticity,
        legacy_source: "scripts/ra_authenticity_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaFlowViolation,
        legacy_source: "scripts/flow_violation_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaDepth,
        legacy_source: "scripts/ra_depth_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaImplementation,
        legacy_source: "scripts/ra_implementation_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaCore,
        legacy_source: "scripts/ra_core_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaApplicability,
        legacy_source: "scripts/ra_applicability_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::RaClosure,
        legacy_source: "scripts/ra_closure_scan.py",
        scope: ScanScopeKind::FormalRa,
    },
    ScannerSpec {
        id: ScannerId::PluginContent,
        legacy_source: "scripts/plugin_content_scan.py",
        scope: ScanScopeKind::Plugin,
    },
];

pub struct ScannerRegistry;

impl ScannerRegistry {
    pub const fn all() -> &'static [ScannerSpec; SCANNER_COUNT] {
        &SCANNERS
    }

    pub fn get(id: ScannerId) -> &'static ScannerSpec {
        SCANNERS
            .iter()
            .find(|scanner| scanner.id == id)
            .expect("all ScannerId variants are registered")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn registry_has_exactly_ten_unique_native_scanners() {
        assert_eq!(ScannerRegistry::all().len(), SCANNER_COUNT);
        assert_eq!(
            ScannerRegistry::all()
                .iter()
                .map(|scanner| scanner.id)
                .collect::<BTreeSet<_>>()
                .len(),
            SCANNER_COUNT
        );
        assert!(
            ScannerRegistry::all()
                .iter()
                .all(|scanner| scanner.legacy_source.starts_with("scripts/"))
        );
    }
}
