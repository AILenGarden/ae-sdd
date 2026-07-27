use ae_sdd_domain::{ArtifactRef, ProjectKey, ProjectRelativePath, ProjectRelativePathError};
use thiserror::Error;

/// Maximum bytes returned by one assets read.
pub const MAX_ASSETS_READ_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum matches returned by one assets query.
pub const MAX_ASSETS_QUERY_MATCHES: usize = 100;
/// Maximum aggregate snippet bytes returned by one assets query.
pub const MAX_ASSETS_QUERY_BYTES: usize = 64 * 1024;

/// Canonical project assets Markdown path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsPath(ProjectRelativePath);

impl AssetsPath {
    /// Derives `.ae-sdd/assets/<project>.assets.md` from a typed project key.
    pub fn for_project(project_key: &ProjectKey) -> Result<Self, AssetsRequestError> {
        let path = format!(".ae-sdd/assets/{}.assets.md", project_key.as_str());
        Ok(Self(ProjectRelativePath::new(path)?))
    }

    /// Wraps an explicitly typed compatibility path.
    pub const fn from_path(path: ProjectRelativePath) -> Self {
        Self(path)
    }

    /// Returns the project-relative path.
    pub const fn as_path(&self) -> &ProjectRelativePath {
        &self.0
    }
}

/// Bounded assets read request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsReadRequest {
    path: AssetsPath,
    section: Option<Box<str>>,
    max_bytes: u64,
}

impl AssetsReadRequest {
    /// Constructs a whole-document or named-section read.
    pub fn new(
        path: AssetsPath,
        section: Option<impl Into<Box<str>>>,
        max_bytes: u64,
    ) -> Result<Self, AssetsRequestError> {
        if max_bytes == 0 || max_bytes > MAX_ASSETS_READ_BYTES {
            return Err(AssetsRequestError::InvalidReadLimit);
        }
        let section = section.map(Into::into);
        if section.as_deref().is_some_and(|value| value.is_empty()) {
            return Err(AssetsRequestError::EmptySection);
        }
        Ok(Self {
            path,
            section,
            max_bytes,
        })
    }

    /// Returns the requested assets path.
    pub const fn path(&self) -> &AssetsPath {
        &self.path
    }

    /// Returns the optional section selector.
    pub fn section(&self) -> Option<&str> {
        self.section.as_deref()
    }

    /// Returns the output byte limit.
    pub const fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
}

/// Verified bounded assets Markdown content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsDocument {
    reference: ArtifactRef,
    content: Box<str>,
    truncated: bool,
}

impl AssetsDocument {
    /// Constructs an assets read result.
    pub fn new(reference: ArtifactRef, content: impl Into<Box<str>>, truncated: bool) -> Self {
        Self {
            reference,
            content: content.into(),
            truncated,
        }
    }

    /// Returns the content-addressed source reference.
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    /// Returns bounded Markdown content.
    pub const fn content(&self) -> &str {
        &self.content
    }

    /// Reports whether output was explicitly truncated.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Assets validation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsCheckResult {
    reference: ArtifactRef,
    missing_sections: Vec<Box<str>>,
}

impl AssetsCheckResult {
    /// Constructs a deterministic check result.
    pub const fn new(reference: ArtifactRef, missing_sections: Vec<Box<str>>) -> Self {
        Self {
            reference,
            missing_sections,
        }
    }

    /// Returns the checked artifact reference.
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    /// Returns required sections that were absent.
    pub fn missing_sections(&self) -> &[Box<str>] {
        &self.missing_sections
    }

    /// Reports whether all required sections were present.
    pub fn is_valid(&self) -> bool {
        self.missing_sections.is_empty()
    }
}

/// Bounded assets query request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsQueryRequest {
    path: AssetsPath,
    query: Box<str>,
    max_matches: usize,
    max_bytes: usize,
}

impl AssetsQueryRequest {
    /// Constructs a bounded query request.
    pub fn new(
        path: AssetsPath,
        query: impl Into<Box<str>>,
        max_matches: usize,
        max_bytes: usize,
    ) -> Result<Self, AssetsRequestError> {
        let query = query.into();
        if query.is_empty() {
            return Err(AssetsRequestError::EmptyQuery);
        }
        if max_matches == 0
            || max_matches > MAX_ASSETS_QUERY_MATCHES
            || max_bytes == 0
            || max_bytes > MAX_ASSETS_QUERY_BYTES
        {
            return Err(AssetsRequestError::InvalidQueryBudget);
        }
        Ok(Self {
            path,
            query,
            max_matches,
            max_bytes,
        })
    }

    /// Returns the requested assets path.
    pub const fn path(&self) -> &AssetsPath {
        &self.path
    }

    /// Returns the query text.
    pub const fn query(&self) -> &str {
        &self.query
    }

    /// Returns the match-count limit.
    pub const fn max_matches(&self) -> usize {
        self.max_matches
    }

    /// Returns the aggregate output byte limit.
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

/// One ordered assets query match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsMatch {
    section: Box<str>,
    line: usize,
    snippet: Box<str>,
}

impl AssetsMatch {
    /// Constructs a bounded query match.
    pub fn new(section: impl Into<Box<str>>, line: usize, snippet: impl Into<Box<str>>) -> Self {
        Self {
            section: section.into(),
            line,
            snippet: snippet.into(),
        }
    }

    /// Returns the section identity.
    pub const fn section(&self) -> &str {
        &self.section
    }

    /// Returns the one-based source line.
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the bounded source snippet.
    pub const fn snippet(&self) -> &str {
        &self.snippet
    }
}

/// Ordered bounded assets query output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetsQueryResult {
    reference: ArtifactRef,
    matches: Vec<AssetsMatch>,
    truncated: bool,
}

impl AssetsQueryResult {
    /// Constructs a query result.
    pub const fn new(reference: ArtifactRef, matches: Vec<AssetsMatch>, truncated: bool) -> Self {
        Self {
            reference,
            matches,
            truncated,
        }
    }

    /// Returns the queried artifact reference.
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    /// Returns ordered matches.
    pub fn matches(&self) -> &[AssetsMatch] {
        &self.matches
    }

    /// Reports whether output was explicitly truncated.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Application port for bounded assets operations.
pub trait AssetsPort {
    /// Adapter-specific failure type.
    type Error;

    /// Reads whole assets Markdown or one named section.
    fn read(&self, request: &AssetsReadRequest) -> Result<AssetsDocument, Self::Error>;

    /// Validates canonical metadata and required sections.
    fn check(&self, path: &AssetsPath) -> Result<AssetsCheckResult, Self::Error>;

    /// Runs a bounded deterministic query.
    fn query(&self, request: &AssetsQueryRequest) -> Result<AssetsQueryResult, Self::Error>;
}

/// Invalid assets path, selector, or budget.
#[derive(Debug, Error)]
pub enum AssetsRequestError {
    /// The project key could not form a portable project-relative path.
    #[error("assets path is not portable")]
    InvalidPath(#[from] ProjectRelativePathError),
    /// Read limit was outside the bounded contract.
    #[error("assets read limit is outside the bounded contract")]
    InvalidReadLimit,
    /// Section selector was empty.
    #[error("assets section selector cannot be empty")]
    EmptySection,
    /// Query text was empty.
    #[error("assets query cannot be empty")]
    EmptyQuery,
    /// Query count or byte budget was outside the bounded contract.
    #[error("assets query budget is outside the bounded contract")]
    InvalidQueryBudget,
}
