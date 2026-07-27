//! Deterministic rendering of ae-sdd managed L2 instruction blocks.
//!
//! Each supported Agent host owns a global instruction file that carries an
//! `ae-sdd-l2-ssot` anchor pair. The released Rust distribution chain replaces
//! only the bytes between those anchors and must leave every other byte of the
//! user-owned file untouched. This module performs that replacement as a pure
//! function: it never touches the filesystem, the wall clock, the process
//! environment, or Git, so the same inputs always render the same bytes and can
//! be asserted without a home directory.

use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Anchor name present in every host file, including those written before the
/// cutover. Changing it orphans the block it was meant to replace.
pub const MANAGED_ANCHOR: &str = "ae-sdd-l2-ssot";

/// Adapter identity recorded in the audit header. Bump when the rendered block
/// layout changes so every host receives a full refresh on the next commit.
pub const MANAGED_ADAPTER_VERSION: &str = "1.0.0";

/// Adapter label recorded in the audit header in place of the legacy wall-clock
/// timestamp. Deterministic rendering is required for replay-safe receipts.
pub const MANAGED_ADAPTER_LABEL: &str = "rust-ae-sdd-build";

/// Language slice selected from the L2 discipline SSOT.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstructionLanguage {
    /// Chinese discipline body, used by Claude and ZCode.
    Zh,
    /// English discipline body, used by Codex.
    En,
}

impl InstructionLanguage {
    /// Returns the SSOT section marker suffix for this language.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
        }
    }
}

/// One managed global instruction file owned by a specific Agent host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedInstructionTarget {
    /// Stable host name used for ordering, idempotency keys, and reporting.
    pub host: String,
    /// Language slice this host expects.
    pub language: InstructionLanguage,
    /// Absolute path of the host's global instruction file.
    pub target_file: PathBuf,
}

/// Pure rendering input. Every volatile value is supplied by the caller.
#[derive(Clone, Copy, Debug)]
pub struct ManagedInstructionRenderRequest<'a> {
    /// Full text of the L2 discipline SSOT read from the compiled package.
    pub source: &'a str,
    /// Current full text of the host's global instruction file.
    pub target: &'a str,
    /// Language slice to render.
    pub language: InstructionLanguage,
    /// Git revision recorded in the audit header; never resolved internally.
    pub revision: &'a str,
}

/// Deterministic rendering outcome for a single managed target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedInstructionPlan {
    /// The anchor range differs and the target must be rewritten.
    Updated {
        /// Full rendered file contents, byte-identical outside the anchor span.
        contents: String,
        /// Short digest of the rendered language body.
        content_hash: String,
    },
    /// The rendered file is byte-identical to the current target.
    Unchanged {
        /// Short digest of the rendered language body.
        content_hash: String,
    },
    /// The target carries no complete anchor pair; bootstrapping is forbidden.
    MissingAnchor,
}

/// Rendering failure. Every variant leaves the target file untouched because
/// this module never writes.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ManagedInstructionError {
    /// The SSOT has no opening marker for the requested language.
    #[error("the L2 SSOT has no SECTION:{0} marker")]
    SectionMissing(&'static str),
    /// The SSOT opening marker has no matching closing marker.
    #[error("the L2 SSOT SECTION:{0} marker is not closed")]
    SectionUnterminated(&'static str),
    /// The SSOT declares the same language section more than once.
    #[error("the L2 SSOT declares SECTION:{0} more than once")]
    SectionDuplicated(&'static str),
    /// The SSOT language body is empty after trimming.
    #[error("the L2 SSOT SECTION:{0} body is empty")]
    SectionEmpty(&'static str),
    /// The target has a BEGIN anchor without a matching END anchor.
    #[error("the managed anchor is opened but never closed")]
    AnchorUnterminated,
    /// The target declares more than one BEGIN or END anchor.
    #[error("the managed anchor is declared more than once")]
    AnchorDuplicated,
    /// The target closes the managed anchor before opening it.
    #[error("the managed anchor END precedes BEGIN")]
    AnchorReversed,
}

/// Renders the managed anchor range for a single host without touching disk.
///
/// Returns [`ManagedInstructionPlan::MissingAnchor`] when the target has no
/// anchor at all: automatic bootstrap is intentionally not ported, so a host
/// without anchors must receive a visible skip instead of a silent new block.
///
/// # Errors
///
/// Returns [`ManagedInstructionError`] when the SSOT language markers or the
/// target anchors are missing a counterpart, duplicated, or reversed. The
/// caller must fail closed and leave the target file unchanged.
pub fn render_managed_instruction(
    request: &ManagedInstructionRenderRequest<'_>,
) -> Result<ManagedInstructionPlan, ManagedInstructionError> {
    let body = language_body(request.source, request.language)?;
    let content_hash = short_digest(&body);

    let Some(span) = anchor_span(request.target)? else {
        return Ok(ManagedInstructionPlan::MissingAnchor);
    };

    let newline = if request.target.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let block = anchor_block(&body, request.revision, &content_hash, newline);
    let mut contents = String::with_capacity(request.target.len() + block.len());
    contents.push_str(&request.target[..span.start]);
    contents.push_str(&block);
    contents.push_str(&request.target[span.end..]);

    if contents == request.target {
        Ok(ManagedInstructionPlan::Unchanged { content_hash })
    } else {
        Ok(ManagedInstructionPlan::Updated {
            contents,
            content_hash,
        })
    }
}

/// Byte range of the managed anchor span, including both marker lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AnchorSpan {
    start: usize,
    end: usize,
}

/// Extracts exactly one language body from the SSOT.
///
/// Mirrors the legacy Python slice: the remainder of the opening marker line is
/// discarded, trailing blank lines collapse into a single newline, and the
/// closing marker itself is excluded.
fn language_body(
    source: &str,
    language: InstructionLanguage,
) -> Result<String, ManagedInstructionError> {
    let language = language.as_str();
    let open = format!("<!-- SECTION:{language} -->");
    let close = format!("<!-- /SECTION:{language} -->");
    let mut opens = source.match_indices(&open);
    let (open_index, _) = opens
        .next()
        .ok_or(ManagedInstructionError::SectionMissing(language))?;
    if opens.next().is_some() {
        return Err(ManagedInstructionError::SectionDuplicated(language));
    }
    let body_start = source[open_index..]
        .find('\n')
        .map(|offset| open_index + offset + 1)
        .ok_or(ManagedInstructionError::SectionUnterminated(language))?;
    let mut closes = source[body_start..].match_indices(&close);
    let (close_offset, _) = closes
        .next()
        .ok_or(ManagedInstructionError::SectionUnterminated(language))?;
    if closes.next().is_some() {
        return Err(ManagedInstructionError::SectionDuplicated(language));
    }
    let body = source[body_start..body_start + close_offset].trim_end();
    if body.is_empty() {
        return Err(ManagedInstructionError::SectionEmpty(language));
    }
    Ok(format!("{body}\n"))
}

/// Locates exactly one complete anchor span in the target file.
fn anchor_span(target: &str) -> Result<Option<AnchorSpan>, ManagedInstructionError> {
    let mut begin: Option<(usize, usize)> = None;
    let mut end: Option<(usize, usize)> = None;
    let mut offset = 0_usize;
    for line in target.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        if is_anchor_line(line, "BEGIN") {
            if begin.is_some() {
                return Err(ManagedInstructionError::AnchorDuplicated);
            }
            begin = Some((line_start, offset));
        } else if is_anchor_line(line, "END") {
            if end.is_some() {
                return Err(ManagedInstructionError::AnchorDuplicated);
            }
            end = Some((line_start, offset));
        }
    }
    match (begin, end) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(ManagedInstructionError::AnchorUnterminated),
        (None, Some(_)) => Err(ManagedInstructionError::AnchorUnterminated),
        (Some(begin), Some(end)) => {
            if end.0 < begin.1 {
                return Err(ManagedInstructionError::AnchorReversed);
            }
            Ok(Some(AnchorSpan {
                start: begin.0,
                end: end.1,
            }))
        }
    }
}

/// Recognizes an HTML-comment anchor marker line without a regex dependency.
///
/// Accepts the same shapes as the legacy Python patterns: optional whitespace
/// inside the comment, and arbitrary audit metadata after `BEGIN <anchor>`.
fn is_anchor_line(line: &str, keyword: &str) -> bool {
    let trimmed = line.trim();
    let Some(inner) = trimmed
        .strip_prefix("<!--")
        .and_then(|rest| rest.strip_suffix("-->"))
    else {
        return false;
    };
    let inner = inner.trim();
    let Some(rest) = inner.strip_prefix(keyword) else {
        return false;
    };
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix(MANAGED_ANCHOR) else {
        return false;
    };
    // BEGIN carries audit metadata; END must be bare.
    if keyword == "END" {
        rest.trim().is_empty()
    } else {
        rest.is_empty() || rest.starts_with(|value: char| !value.is_alphanumeric())
    }
}

/// Renders the full anchor span, audit header included.
fn anchor_block(body: &str, revision: &str, content_hash: &str, newline: &str) -> String {
    let mut block = String::with_capacity(body.len() + 160);
    // String writes are infallible; the Result is discarded deliberately.
    let _ = write!(
        block,
        "<!-- BEGIN {MANAGED_ANCHOR} @ {revision} @ {MANAGED_ADAPTER_LABEL} (hash={content_hash} v={MANAGED_ADAPTER_VERSION}) -->"
    );
    block.push_str(newline);
    for line in body.lines() {
        block.push_str(line);
        block.push_str(newline);
    }
    // The legacy Python injector renders `{begin}\n{body}\n{end}\n` where `body`
    // already ends in a newline, so a blank line separates the body from END.
    // Keeping it preserves byte-level parity with previously injected files and
    // avoids a spurious rewrite on the first Rust-driven run.
    block.push_str(newline);
    let _ = write!(block, "<!-- END {MANAGED_ANCHOR} -->");
    block.push_str(newline);
    block
}

/// Short content digest matching the legacy Python `_content_hash` contract.
fn short_digest(body: &str) -> String {
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    digest[..12].to_owned()
}

#[cfg(test)]
mod tests;
