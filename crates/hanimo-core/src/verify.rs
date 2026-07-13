//! Capability-relative live verification with one whole-bundle mutation retry.

use std::{io, path::Path};

use serde::Serialize;
use thiserror::Error;

mod filesystem;
mod live;
#[cfg(test)]
mod mutation_tests;
mod policy;

pub use policy::MAX_VERIFY_BUNDLE_BYTES;

use crate::{
    evidence::{EvidenceBundle, bundle_sha256, decoded_bytes},
    identity::{BlockIdentityInput, IdentityError, block_id},
    model::{EvidenceBlock, SCHEMA_VERSION},
};
use filesystem::{open_root, path_from_bytes, read_nofollow};
use live::SourceReader;

/// Structured verification outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The live source reproduces every recorded identity field.
    Verified,
    /// The recorded source path no longer opens without following links.
    Stale,
    /// The bundle is internally inconsistent with its block identity.
    Forged,
    /// Live source bytes differ from the recorded exact-file digest.
    SourceDrift,
}

/// Verification outcome for one evidence block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockVerification {
    /// Recorded block identity.
    pub block_id: String,
    /// Live verification classification.
    pub status: VerificationStatus,
}

/// Whole-bundle live verification report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationReport {
    /// Aggregate status using forged, stale, drift, verified precedence.
    pub status: VerificationStatus,
    /// One normally, two when an in-flight mutation triggers the sole retry.
    pub attempts: u8,
    /// Per-block outcomes in bundle order.
    pub blocks: Vec<BlockVerification>,
}

/// Fail-closed verification setup errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VerifyError {
    /// Bundle schema version is unsupported.
    #[error("unsupported evidence schema version")]
    UnsupportedSchema,
    /// The supplied root cannot become a filesystem capability.
    #[error("cannot open verification root")]
    RootOpen(#[source] io::Error),
    /// Artifact arrays or numeric budgets exceed verifier-owned ceilings.
    #[error("invalid evidence bundle: {0}")]
    InvalidBundle(&'static str),
    /// A block locator is absolute, empty, or attempts root traversal.
    #[error("unsafe evidence path")]
    UnsafePath,
    /// Live verification exhausted its verifier-owned aggregate read allowance.
    #[error("verification source-read limit exceeded")]
    ResourceLimit,
    /// A recorded source exists but cannot be read under the verifier's policy.
    #[error("cannot read verification source")]
    SourceRead(#[source] io::Error),
    /// Block identity reconstruction failed.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

impl VerifyError {
    /// Returns the stable invalid-artifact reason used by CLI exit 3 mapping.
    pub const fn invalid_bundle_reason(&self) -> Option<&'static str> {
        match self {
            Self::UnsupportedSchema => Some("unsupported evidence schema version"),
            Self::InvalidBundle(reason) => Some(reason),
            Self::RootOpen(_)
            | Self::UnsafePath
            | Self::ResourceLimit
            | Self::SourceRead(_)
            | Self::Identity(_) => None,
        }
    }
}

/// Re-reads and cryptographically verifies a bundle beneath one root capability.
pub fn verify(root: &Path, bundle: &EvidenceBundle) -> Result<VerificationReport, VerifyError> {
    let root = open_root(root).map_err(VerifyError::RootOpen)?;
    verify_with_reader(bundle, |path, maximum| read_nofollow(&root, path, maximum))
}

fn verify_with_reader(
    bundle: &EvidenceBundle,
    mut read: impl FnMut(&Path, usize) -> io::Result<Vec<u8>>,
) -> Result<VerificationReport, VerifyError> {
    if bundle.schema_version != SCHEMA_VERSION {
        return Err(VerifyError::UnsupportedSchema);
    }
    policy::validate_limits(bundle).map_err(VerifyError::InvalidBundle)?;
    if !bundle_sha256(bundle).is_ok_and(|digest| digest == bundle.bundle_sha256) {
        return Ok(forged_report(bundle));
    }
    if !policy::critic_is_consistent(bundle) {
        return Ok(forged_report(bundle));
    }
    let preflight = bundle
        .blocks
        .iter()
        .map(|block| {
            preflight_status(block).map(|status| BlockVerification {
                block_id: block.block_id.clone(),
                status,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if preflight
        .iter()
        .any(|block| block.status == VerificationStatus::Forged)
    {
        return Ok(report(preflight, 1));
    }
    let mut reader = SourceReader::new(&mut read);
    let first = live::attempt(bundle, &mut reader)?;
    if !first.raced {
        return Ok(report(first.blocks, 1));
    }
    let mut second = live::attempt(bundle, &mut reader)?;
    if second.raced {
        for block in &mut second.blocks {
            if block.status == VerificationStatus::Verified {
                block.status = VerificationStatus::SourceDrift;
            }
        }
    }
    Ok(report(second.blocks, 2))
}

fn preflight_status(block: &EvidenceBlock) -> Result<VerificationStatus, VerifyError> {
    let Ok(path) = decoded_bytes(&block.path) else {
        return Ok(VerificationStatus::Forged);
    };
    let Ok(content) = decoded_bytes(&block.content) else {
        return Ok(VerificationStatus::Forged);
    };
    if path_from_bytes(&path).is_none() {
        return Err(VerifyError::UnsafePath);
    }
    if block.line_start > block.line_end
        || block.byte_start > block.byte_end
        || block.score_components.total() != Some(block.score)
    {
        return Ok(VerificationStatus::Forged);
    }
    let expected = block_id(BlockIdentityInput {
        path: &path,
        line_start: block.line_start,
        line_end: block.line_end,
        content: &content,
    })?;
    Ok(if expected == block.block_id {
        VerificationStatus::Verified
    } else {
        VerificationStatus::Forged
    })
}

fn block_report(block: &EvidenceBlock, status: VerificationStatus) -> BlockVerification {
    BlockVerification {
        block_id: block.block_id.clone(),
        status,
    }
}

fn forged_report(bundle: &EvidenceBundle) -> VerificationReport {
    VerificationReport {
        status: VerificationStatus::Forged,
        attempts: 1,
        blocks: bundle
            .blocks
            .iter()
            .map(|block| block_report(block, VerificationStatus::Forged))
            .collect(),
    }
}

fn report(blocks: Vec<BlockVerification>, attempts: u8) -> VerificationReport {
    let status = if blocks
        .iter()
        .any(|block| block.status == VerificationStatus::Forged)
    {
        VerificationStatus::Forged
    } else if blocks
        .iter()
        .any(|block| block.status == VerificationStatus::Stale)
        || blocks.is_empty()
    {
        VerificationStatus::Stale
    } else if blocks
        .iter()
        .any(|block| block.status == VerificationStatus::SourceDrift)
    {
        VerificationStatus::SourceDrift
    } else {
        VerificationStatus::Verified
    };
    VerificationReport {
        status,
        attempts,
        blocks,
    }
}
