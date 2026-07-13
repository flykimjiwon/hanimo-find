//! Deterministic, source-cited static diagnosis for inspectable RAG patterns.

use std::fmt::Write as _;
use std::{num::NonZeroUsize, path::Path};

use serde::Serialize;
use thiserror::Error;

mod filesystem;
mod scanner;

const SCHEMA_VERSION: &str = "0.1.0";
const DEFAULT_MAX_CANDIDATE_FILES: usize = 4_096;
const DEFAULT_MAX_FILE_BYTES: usize = 1_048_576;
const DEFAULT_MAX_TOTAL_BYTES: usize = 16_777_216;
const DIRECT_RULES: &[(RuleId, &str)] = &[
    (
        RuleId::VectorStoreDependency,
        "chromadb|qdrant_client|weaviate|lancedb|pgvector|faiss",
    ),
    (
        RuleId::EmbeddingCall,
        "embeddings.create(|.embed_documents(|sentence_transformers|flagembedding",
    ),
    (
        RuleId::FixedChunking,
        "chunk_size=|chunk_size =|recursivecharactertextsplitter",
    ),
    (
        RuleId::TopKRetriever,
        "similarity_search(|search_kwargs={\"k\"|top_k=|top_k =",
    ),
    (RuleId::Reranker, "reranker|cross_encoder"),
];

/// Stable identifiers for deterministic diagnosis rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId {
    /// A vector-store dependency is declared or imported.
    VectorStoreDependency,
    /// Source invokes an embedding model.
    EmbeddingCall,
    /// Source configures fixed-size chunking.
    FixedChunking,
    /// Retrieval uses an explicit top-k vector query.
    TopKRetriever,
    /// Retrieval results pass through a reranker.
    Reranker,
    /// Citation output omits line information.
    MissingLineCitations,
    /// Indexed retrieval has no visible freshness check.
    MissingFreshnessValidation,
    /// Vector retrieval has no visible exact-search route.
    MissingExactSearchFallback,
}

impl RuleId {
    /// Returns the frozen machine-readable rule identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VectorStoreDependency => "RAG001_VECTOR_STORE_DEPENDENCY",
            Self::EmbeddingCall => "RAG002_EMBEDDING_CALL",
            Self::FixedChunking => "RAG003_FIXED_CHUNKING",
            Self::TopKRetriever => "RAG004_TOP_K_RETRIEVER",
            Self::Reranker => "RAG005_RERANKER",
            Self::MissingLineCitations => "RAG006_MISSING_LINE_CITATIONS",
            Self::MissingFreshnessValidation => "RAG007_MISSING_FRESHNESS_VALIDATION",
            Self::MissingExactSearchFallback => "RAG008_MISSING_EXACT_SEARCH_FALLBACK",
        }
    }
}

/// Severity attached to a diagnosis finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// An inspectable architectural risk.
    Warning,
}

impl Severity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
        }
    }
}

/// A source path and one-based line supporting a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Citation {
    /// Root-relative UTF-8 path.
    pub path: String,
    /// One-based source line.
    pub line: usize,
}

/// One deterministic, source-cited diagnosis result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Typed rule identity; the serialized message begins with this identifier.
    #[serde(skip)]
    pub rule_id: RuleId,
    /// Finding severity.
    pub severity: Severity,
    /// Stable human-readable explanation.
    pub message: String,
    /// Real source locations that triggered the rule.
    pub citations: Vec<Citation>,
}

/// Authoritative JSON diagnosis model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RagDiagnosis {
    /// Frozen schema version.
    pub schema_version: &'static str,
    /// Digest of sorted source paths and exact file digests.
    pub bundle_sha256: String,
    /// Deterministically ordered findings.
    pub findings: Vec<Finding>,
    /// Human-readable finding count.
    pub summary: String,
}

/// Deterministic resource limits for one diagnosis scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnoseBudget {
    /// Maximum regular files admitted as candidates.
    pub max_candidate_files: NonZeroUsize,
    /// Maximum bytes read from one source file.
    pub max_file_bytes: NonZeroUsize,
    /// Maximum source bytes read by the complete diagnosis.
    pub max_total_bytes: NonZeroUsize,
}

impl Default for DiagnoseBudget {
    fn default() -> Self {
        Self {
            max_candidate_files: nonzero(DEFAULT_MAX_CANDIDATE_FILES),
            max_file_bytes: nonzero(DEFAULT_MAX_FILE_BYTES),
            max_total_bytes: nonzero(DEFAULT_MAX_TOTAL_BYTES),
        }
    }
}

/// The deterministic diagnosis limit that rejected a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DiagnoseLimit {
    /// More regular candidates were discovered than allowed.
    #[error("candidate file limit exceeded")]
    CandidateFiles,
    /// One source exceeded the per-file byte allowance.
    #[error("per-file byte limit exceeded")]
    FileBytes,
    /// The scan exceeded its total source-byte allowance.
    #[error("total byte limit exceeded")]
    TotalBytes,
}

/// Fail-closed errors raised while reading a diagnosis target.
#[derive(Debug, Error)]
pub enum DiagnoseError {
    /// The supplied diagnosis root contains a symbolic-link component.
    #[error("diagnosis root must not contain symbolic links")]
    RootSymlink,
    /// Walking the target failed.
    #[error("failed to walk diagnosis target: {0}")]
    Walk(#[from] ignore::Error),
    /// Reading a source file failed.
    #[error("failed to read diagnosis source: {0}")]
    Read(#[from] std::io::Error),
    /// A walked path could not be represented by the public schema.
    #[error("diagnosis source path is not a valid root-relative UTF-8 path")]
    InvalidPath,
    /// A deterministic resource envelope was exhausted.
    #[error("diagnosis budget exceeded: {0}")]
    BudgetExceeded(DiagnoseLimit),
}

/// Statically diagnoses inspectable RAG patterns without importing target code.
pub fn diagnose(root: &Path) -> Result<RagDiagnosis, DiagnoseError> {
    diagnose_with_budget(root, DiagnoseBudget::default())
}

/// Diagnoses one root using an explicit deterministic resource envelope.
pub fn diagnose_with_budget(
    root: &Path,
    budget: DiagnoseBudget,
) -> Result<RagDiagnosis, DiagnoseError> {
    scanner::diagnose(root, budget)
}

/// Renders Markdown solely from the authoritative typed diagnosis.
#[must_use]
pub fn render_markdown(diagnosis: &RagDiagnosis) -> String {
    let mut output = format!(
        "# RagDiagnosis\n\n- Schema version: `{}`\n- Bundle SHA-256: `{}`\n\n## Findings\n",
        diagnosis.schema_version, diagnosis.bundle_sha256
    );
    if diagnosis.findings.is_empty() {
        output.push_str("\nNo findings.\n");
    }
    for finding in &diagnosis.findings {
        let _ = write!(
            output,
            "\n### {}\n\n- Severity: `{}`\n",
            finding.message,
            finding.severity.as_str()
        );
        for citation in &finding.citations {
            let _ = writeln!(output, "- Citation: `{}:{}`", citation.path, citation.line);
        }
    }
    let _ = write!(output, "\n## Summary\n\n{}\n", diagnosis.summary);
    output
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or(NonZeroUsize::MIN)
}
