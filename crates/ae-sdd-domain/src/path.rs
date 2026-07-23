use std::{fmt, str::FromStr};

use thiserror::Error;

pub const MAX_PROJECT_RELATIVE_PATH_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectRelativePath(Box<str>);

impl ProjectRelativePath {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ProjectRelativePathError> {
        let value = value.into();
        validate_path(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn contains(&self, candidate: &Self) -> bool {
        self == candidate
            || candidate
                .as_str()
                .strip_prefix(self.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

impl fmt::Display for ProjectRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ProjectRelativePath {
    type Err = ProjectRelativePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for ProjectRelativePath {
    type Error = ProjectRelativePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectRelativePathError {
    #[error("project-relative path must not be empty")]
    Empty,
    #[error("project-relative path exceeds the {max_bytes}-byte limit (actual: {actual_bytes})")]
    TooLong {
        max_bytes: usize,
        actual_bytes: usize,
    },
    #[error("path is not in canonical project-relative form")]
    NotRelative,
    #[error("path contains an empty, current-directory, or parent-directory segment")]
    InvalidSegment,
    #[error("path contains a character that is not portable across supported platforms")]
    NonPortableCharacter,
    #[error("path segment must not end with a dot or space")]
    NonPortableSuffix,
}

fn validate_path(value: &str) -> Result<(), ProjectRelativePathError> {
    if value.is_empty() {
        return Err(ProjectRelativePathError::Empty);
    }
    if value.len() > MAX_PROJECT_RELATIVE_PATH_BYTES {
        return Err(ProjectRelativePathError::TooLong {
            max_bytes: MAX_PROJECT_RELATIVE_PATH_BYTES,
            actual_bytes: value.len(),
        });
    }
    if value.starts_with('/') || value.contains('\\') {
        return Err(ProjectRelativePathError::NotRelative);
    }
    if value.chars().any(|character| {
        character.is_control() || matches!(character, ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err(ProjectRelativePathError::NonPortableCharacter);
    }
    for segment in value.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return Err(ProjectRelativePathError::InvalidSegment);
        }
        if segment.ends_with('.') || segment.ends_with(' ') {
            return Err(ProjectRelativePathError::NonPortableSuffix);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_relative_path_rejects_escape_and_absolute_paths() {
        for invalid in [
            "../secret",
            "source/../secret",
            "/absolute",
            "C:/absolute",
            "source\\windows",
            "source//empty",
        ] {
            assert!(
                ProjectRelativePath::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn project_relative_path_containment_is_segment_aware() {
        let scope = ProjectRelativePath::new("crates/ae-sdd-domain").expect("valid scope");
        let child =
            ProjectRelativePath::new("crates/ae-sdd-domain/src/lib.rs").expect("valid child");
        let sibling =
            ProjectRelativePath::new("crates/ae-sdd-domain-old/src/lib.rs").expect("valid sibling");

        assert!(scope.contains(&child));
        assert!(!scope.contains(&sibling));
    }
}
