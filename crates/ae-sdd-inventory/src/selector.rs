use std::fmt;

use ae_sdd_domain::ProjectRelativePath;
use thiserror::Error;

const MAX_SELECTOR_ID_BYTES: usize = 128;

/// Stable identity for a selector used by reverse indexes and Gate caches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectorId(Box<str>);

impl SelectorId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, SelectorError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SelectorError::EmptyId);
        }
        if value.len() > MAX_SELECTOR_ID_BYTES {
            return Err(SelectorError::IdTooLong {
                actual: value.len(),
                maximum: MAX_SELECTOR_ID_BYTES,
            });
        }
        if value
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !b"._:-".contains(&byte))
        {
            return Err(SelectorError::InvalidId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SelectorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Declarative, portable project-path selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathSelector {
    All,
    Exact(ProjectRelativePath),
    Prefix(ProjectRelativePath),
    Extension(Box<str>),
    FileName(Box<str>),
    Any(Vec<PathSelector>),
    AllOf(Vec<PathSelector>),
}

impl PathSelector {
    pub fn extension(value: impl Into<Box<str>>) -> Result<Self, SelectorError> {
        let value = value.into();
        let extension = value.strip_prefix('.').unwrap_or(&value);
        if extension.is_empty() || extension.bytes().any(|byte| !byte.is_ascii_alphanumeric()) {
            return Err(SelectorError::InvalidExtension);
        }
        Ok(Self::Extension(extension.to_ascii_lowercase().into()))
    }

    pub fn file_name(value: impl Into<Box<str>>) -> Result<Self, SelectorError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.contains('\\') {
            return Err(SelectorError::InvalidFileName);
        }
        Ok(Self::FileName(value))
    }

    pub fn matches(&self, path: &ProjectRelativePath) -> bool {
        match self {
            Self::All => true,
            Self::Exact(expected) => expected == path,
            Self::Prefix(prefix) => prefix.contains(path),
            Self::Extension(extension) => path
                .as_str()
                .rsplit_once('.')
                .is_some_and(|(_, actual)| actual.eq_ignore_ascii_case(extension)),
            Self::FileName(expected) => path
                .as_str()
                .rsplit('/')
                .next()
                .is_some_and(|actual| actual == expected.as_ref()),
            Self::Any(selectors) => selectors.iter().any(|selector| selector.matches(path)),
            Self::AllOf(selectors) => {
                !selectors.is_empty() && selectors.iter().all(|selector| selector.matches(path))
            }
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SelectorError {
    #[error("selector ID must not be empty")]
    EmptyId,
    #[error("selector ID exceeds {maximum} bytes (actual: {actual})")]
    IdTooLong { actual: usize, maximum: usize },
    #[error("selector ID contains a non-portable character")]
    InvalidId,
    #[error("extension must contain only ASCII letters and digits")]
    InvalidExtension,
    #[error("file name must be one non-empty path segment")]
    InvalidFileName,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> ProjectRelativePath {
        ProjectRelativePath::new(value).expect("valid path")
    }

    #[test]
    fn selector_matching_is_segment_and_case_aware() {
        let prefix = PathSelector::Prefix(path("crates/ae-sdd-gates"));
        let rust = PathSelector::extension(".RS").expect("valid extension");

        assert!(prefix.matches(&path("crates/ae-sdd-gates/src/lib.rs")));
        assert!(!prefix.matches(&path("crates/ae-sdd-gates-old/src/lib.rs")));
        assert!(rust.matches(&path("src/lib.rs")));
    }
}
