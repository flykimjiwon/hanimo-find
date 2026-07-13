//! Authoritative evidence bundle, deterministic critic, and Markdown projection.

use std::borrow::Cow;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    bytes::contains,
    model::{
        Budget, EvidenceBlock, QueryPlan, SCHEMA_VERSION, SearchResult, SkipReason, SkippedEvidence,
    },
    render_support::EncodedBytes,
};

/// Deterministic critic decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CriticVerdict {
    /// At least one block covers every required literal.
    Accepted,
    /// Evidence is absent or required literal coverage has gaps.
    Rejected,
}

/// Coverage details produced by the deterministic critic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticReport {
    /// Critic decision.
    pub verdict: CriticVerdict,
    /// Quoted phrases covered in query order.
    pub covered_quoted_phrases: Vec<String>,
    /// Identifiers covered in query order.
    pub covered_identifiers: Vec<String>,
    /// Missing phrases followed by missing identifiers, in query order.
    pub uncovered: Vec<String>,
}

/// Authoritative evidence JSON model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBundle {
    /// Frozen schema version.
    pub schema_version: String,
    /// Self-consistency digest of the immutable evidence payload.
    pub bundle_sha256: String,
    /// Original query text.
    pub query: String,
    /// Display-only root identifier.
    pub root: String,
    /// Search budgets used to produce the evidence.
    pub budget: Budget,
    /// Canonical evidence blocks in frozen rank order.
    pub blocks: Vec<EvidenceBlock>,
    /// Skips sorted by raw path bytes and reason.
    pub skipped: Vec<SkippedEvidence>,
    /// Deterministic sufficiency decision and gaps.
    pub critic: CriticReport,
}

/// Bundle assembly and rendering failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvidenceError {
    /// Query schema version is unsupported.
    #[error("unsupported evidence schema version")]
    UnsupportedSchema,
    /// An encoded byte field is not canonical base64.
    #[error("invalid encoded evidence bytes")]
    InvalidBytes,
    /// A bundle attestation frame cannot be represented by the required u64 length.
    #[error("bundle attestation frame exceeds u64 length")]
    FrameTooLarge,
    /// Authoritative JSON serialization failed.
    #[error("cannot serialize authoritative evidence JSON")]
    Json(#[from] serde_json::Error),
}

/// Builds the authoritative bundle and deterministic critic report.
pub fn assemble_bundle(
    plan: &QueryPlan,
    result: SearchResult,
) -> Result<EvidenceBundle, EvidenceError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(EvidenceError::UnsupportedSchema);
    }
    let skipped = sort_skips(result.skipped)?;
    let (covered_quoted_phrases, mut uncovered) =
        classify_required(&result.blocks, &plan.quoted_phrases)?;
    let (covered_identifiers, missing_identifiers) =
        classify_required(&result.blocks, &plan.identifiers)?;
    uncovered.extend(missing_identifiers);
    let budget_incomplete = skipped.iter().any(|gap| gap.reason == SkipReason::Budget);
    let verdict = if result.blocks.is_empty() || !uncovered.is_empty() || budget_incomplete {
        CriticVerdict::Rejected
    } else {
        CriticVerdict::Accepted
    };
    let mut bundle = EvidenceBundle {
        schema_version: SCHEMA_VERSION.to_owned(),
        bundle_sha256: String::new(),
        query: plan.query.clone(),
        root: plan.root.clone(),
        budget: plan.budget,
        blocks: result.blocks,
        skipped,
        critic: CriticReport {
            verdict,
            covered_quoted_phrases,
            covered_identifiers,
            uncovered,
        },
    };
    bundle.bundle_sha256 = bundle_sha256(&bundle)?;
    Ok(bundle)
}

/// Computes the deterministic self-consistency digest, excluding the digest field itself.
pub fn bundle_sha256(bundle: &EvidenceBundle) -> Result<String, EvidenceError> {
    crate::attestation::compute_bundle_sha256(bundle)
}

/// Renders Markdown as a pure view containing the exact authoritative JSON.
pub fn render_markdown(bundle: &EvidenceBundle) -> Result<String, EvidenceError> {
    let json = serde_json::to_string_pretty(bundle)?;
    Ok(format!("# Hanimo Find Evidence\n\n```json\n{json}\n```\n"))
}

pub(crate) fn decoded_bytes(value: &EncodedBytes) -> Result<Cow<'_, [u8]>, EvidenceError> {
    match value {
        EncodedBytes::Utf8 { text } => Ok(Cow::Borrowed(text.as_bytes())),
        EncodedBytes::Base64 { bytes } => STANDARD
            .decode(bytes)
            .map(Cow::Owned)
            .map_err(|_| EvidenceError::InvalidBytes),
    }
}

fn classify_required(
    blocks: &[EvidenceBlock],
    required: &[String],
) -> Result<(Vec<String>, Vec<String>), EvidenceError> {
    let mut covered = Vec::new();
    let mut uncovered = Vec::new();
    for item in required {
        let mut found = false;
        for block in blocks {
            if contains(&decoded_bytes(&block.content)?, item.as_bytes()) {
                found = true;
                break;
            }
        }
        if found {
            covered.push(item.clone());
        } else {
            uncovered.push(item.clone());
        }
    }
    Ok((covered, uncovered))
}

fn sort_skips(skipped: Vec<SkippedEvidence>) -> Result<Vec<SkippedEvidence>, EvidenceError> {
    let mut keyed = skipped
        .into_iter()
        .map(|skip| {
            let path = decoded_bytes(&skip.path)?.into_owned();
            Ok::<_, EvidenceError>((path, skip))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.reason.cmp(&right.1.reason))
    });
    Ok(keyed.into_iter().map(|(_, skip)| skip).collect())
}
