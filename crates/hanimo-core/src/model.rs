use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::render_support::EncodedBytes;

/// Frozen protocol schema version.
pub const SCHEMA_VERSION: &str = "0.1.0";
/// Maximum UTF-8 bytes admitted in one query or typed literal set.
pub const MAX_QUERY_BYTES: usize = 4_096;
/// Maximum exact literals admitted in one query.
pub const MAX_QUERY_LITERALS: usize = 64;
/// Maximum eligible files examined by one search.
pub const MAX_CANDIDATE_FILES: usize = 64;
/// Maximum metadata entries examined before source content scanning.
pub const MAX_DISCOVERY_ENTRIES: usize = 256;
/// Maximum root-relative metadata depth examined by search.
pub const MAX_DISCOVERY_DEPTH: usize = 64;

/// Hard query-admission failures shared by direct and adapted search callers.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryLimitError {
    /// Query text or the typed literal bytes exceed the fixed ceiling.
    #[error("query exceeds {maximum} bytes")]
    Bytes {
        /// Fixed byte ceiling.
        maximum: usize,
    },
    /// The exact-literal count exceeds the fixed ceiling.
    #[error("query exceeds {maximum} literals")]
    Literals {
        /// Fixed literal-count ceiling.
        maximum: usize,
    },
}

/// Enforces the hard query byte ceiling.
pub const fn validate_query_bytes(bytes: usize) -> Result<(), QueryLimitError> {
    if bytes > MAX_QUERY_BYTES {
        Err(QueryLimitError::Bytes {
            maximum: MAX_QUERY_BYTES,
        })
    } else {
        Ok(())
    }
}

/// Enforces the hard exact-literal count ceiling.
pub const fn validate_query_literal_count(count: usize) -> Result<(), QueryLimitError> {
    if count > MAX_QUERY_LITERALS {
        Err(QueryLimitError::Literals {
            maximum: MAX_QUERY_LITERALS,
        })
    } else {
        Ok(())
    }
}

/// Bounded resource limits for one search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Maximum selected evidence blocks.
    pub max_blocks: NonZeroUsize,
    /// Context lines on either side of a literal hit.
    pub context_lines: usize,
    /// Maximum bytes read from one file.
    pub max_file_bytes: NonZeroUsize,
    /// Maximum bytes read by one search.
    pub max_total_bytes: NonZeroUsize,
    /// Maximum literal occurrences accepted by one search.
    pub max_matches: NonZeroUsize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_blocks: nonzero_or_minimum(8),
            context_lines: 3,
            max_file_bytes: nonzero_or_minimum(1_048_576),
            max_total_bytes: nonzero_or_minimum(16_777_216),
            max_matches: nonzero_or_minimum(1_000),
        }
    }
}

fn nonzero_or_minimum(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or(NonZeroUsize::MIN)
}

/// Typed literal-search request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryPlan {
    /// Protocol schema version.
    pub schema_version: String,
    /// Original query text.
    pub query: String,
    /// Display-only root identifier; filesystem access is supplied separately.
    pub root: String,
    /// Exact quoted UTF-8 byte sequences.
    pub quoted_phrases: Vec<String>,
    /// Exact identifier UTF-8 byte sequences.
    pub identifiers: Vec<String>,
    /// Exact ordinary-term UTF-8 byte sequences.
    pub terms: Vec<String>,
    /// Search resource limits.
    pub budget: Budget,
}

impl QueryPlan {
    /// Enforces hard query work ceilings for direct core callers.
    pub fn validate_limits(&self) -> Result<(), QueryLimitError> {
        validate_query_bytes(self.query.len())?;
        let literals = self
            .quoted_phrases
            .len()
            .saturating_add(self.identifiers.len())
            .saturating_add(self.terms.len());
        validate_query_literal_count(literals)?;
        let literal_bytes = self
            .quoted_phrases
            .iter()
            .chain(&self.identifiers)
            .chain(&self.terms)
            .fold(0_usize, |total, literal| {
                total.saturating_add(literal.len())
            });
        validate_query_bytes(literal_bytes)
    }
}

/// One-based inclusive source line number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineNumber(NonZeroUsize);

impl LineNumber {
    pub(crate) fn from_zero_based(index: usize) -> Option<Self> {
        index.checked_add(1).and_then(NonZeroUsize::new).map(Self)
    }

    /// Returns the one-based integer.
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

/// Zero-based source byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteOffset(usize);

impl ByteOffset {
    pub(crate) const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the raw offset.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Frozen explainable integer score components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreComponents {
    /// Exact quoted-phrase component.
    pub exact_phrase: u16,
    /// Identifier component.
    pub identifier: u16,
    /// All ordinary terms component.
    pub all_terms: u16,
    /// Markdown heading component.
    pub heading: u16,
    /// Eligible path component.
    pub path: u16,
    /// Required-literal proximity component.
    pub proximity: u16,
}

impl ScoreComponents {
    pub(crate) fn total(self) -> Option<u16> {
        [
            self.exact_phrase,
            self.identifier,
            self.all_terms,
            self.heading,
            self.path,
            self.proximity,
        ]
        .into_iter()
        .try_fold(0_u16, u16::checked_add)
    }
}

/// Canonical evidence block returned by literal search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBlock {
    /// Raw root-relative path bytes encoded without loss.
    pub path: EncodedBytes,
    /// First selected line.
    pub line_start: LineNumber,
    /// Last selected line.
    pub line_end: LineNumber,
    /// First selected content byte.
    pub byte_start: ByteOffset,
    /// One past the last selected content byte.
    pub byte_end: ByteOffset,
    /// LF-canonical block bytes encoded without loss.
    pub content: EncodedBytes,
    /// Query literals present in the block, in query-plan order.
    pub matched_terms: Vec<String>,
    /// Sum of the explainable components.
    pub score: u16,
    /// Explainable component values.
    pub score_components: ScoreComponents,
    /// Stable human-readable score reasons.
    pub why: Vec<String>,
    /// Domain-separated block digest.
    pub block_id: String,
    /// Digest of the exact complete source bytes.
    pub source_sha256: String,
}

/// Why a discovered source was not included in evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Ignore rules excluded the path before candidate discovery.
    Ignored,
    /// Hidden-path policy excluded the path before candidate discovery.
    Hidden,
    /// Secret-path policy excluded a discovered path.
    Secret,
    /// A symbolic link was not followed.
    Symlink,
    /// The entry was not a regular file.
    NonRegular,
    /// Per-file size policy excluded a discovered path.
    Oversized,
    /// A scan budget truncated the discovered path order.
    Budget,
    /// The path was not a safe root-relative path.
    InvalidPath,
    /// Source access failed.
    IoError,
}

/// One deterministically reported source gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkippedEvidence {
    /// Lossless root-relative path or truncation boundary.
    pub path: EncodedBytes,
    /// Stable gap classification.
    pub reason: SkipReason,
}

/// Todo 2 search output before verification and critic stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    /// Blocks sorted by the frozen total ordering and then truncated.
    pub blocks: Vec<EvidenceBlock>,
    /// Discovered gaps sorted by raw path bytes and reason.
    pub skipped: Vec<SkippedEvidence>,
}
