#![forbid(unsafe_code)]
//! Deterministic, raw-byte evidence search for Hanimo Find.

mod attestation;
pub(crate) mod bytes;
mod evidence;
pub(crate) mod rank;
mod render_support;
mod root;
mod verify;

/// Domain-separated source and block digests.
pub mod identity;
/// Typed query, budget, and evidence values.
pub mod model;
/// Bounded capability-relative literal search.
pub mod search;

pub use evidence::{
    CriticReport, CriticVerdict, EvidenceBundle, EvidenceError, assemble_bundle, bundle_sha256,
    render_markdown,
};
pub use model::{EvidenceBlock, QueryPlan, SearchResult, SkipReason, SkippedEvidence};
pub use render_support::EncodedBytes;
pub use search::{SearchError, search};
pub use verify::{
    BlockVerification, MAX_VERIFY_BUNDLE_BYTES, VerificationReport, VerificationStatus,
    VerifyError, verify,
};

pub mod diagnose;
