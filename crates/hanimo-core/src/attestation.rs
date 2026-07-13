//! Deterministic self-consistency digest for immutable evidence payload fields.

use std::fmt::Display;

use sha2::{Digest as _, Sha256};

use crate::{
    evidence::{CriticVerdict, EvidenceBundle, EvidenceError, decoded_bytes},
    model::{EvidenceBlock, ScoreComponents, SkipReason},
};

const BUNDLE_DOMAIN: &[u8] = b"hanimo:evidence-bundle:v1\0";

pub(crate) fn compute_bundle_sha256(bundle: &EvidenceBundle) -> Result<String, EvidenceError> {
    let mut digest = Sha256::new();
    digest.update(BUNDLE_DOMAIN);
    update_frame(&mut digest, bundle.schema_version.as_bytes())?;
    update_frame(&mut digest, bundle.query.as_bytes())?;
    update_number(&mut digest, bundle.blocks.len())?;
    for block in &bundle.blocks {
        update_block(&mut digest, block)?;
    }
    update_number(&mut digest, bundle.skipped.len())?;
    for skip in &bundle.skipped {
        update_frame(&mut digest, &decoded_bytes(&skip.path)?)?;
        update_frame(&mut digest, skip_reason(skip.reason))?;
    }
    update_frame(&mut digest, critic_verdict(bundle.critic.verdict))?;
    update_strings(&mut digest, &bundle.critic.covered_quoted_phrases)?;
    update_strings(&mut digest, &bundle.critic.covered_identifiers)?;
    update_strings(&mut digest, &bundle.critic.uncovered)?;
    Ok(hex::encode(digest.finalize()))
}

fn update_block(digest: &mut Sha256, block: &EvidenceBlock) -> Result<(), EvidenceError> {
    update_frame(digest, &decoded_bytes(&block.path)?)?;
    update_number(digest, block.line_start.get())?;
    update_number(digest, block.line_end.get())?;
    update_number(digest, block.byte_start.get())?;
    update_number(digest, block.byte_end.get())?;
    update_frame(digest, &decoded_bytes(&block.content)?)?;
    update_strings(digest, &block.matched_terms)?;
    update_number(digest, block.score)?;
    update_score(digest, block.score_components)?;
    update_strings(digest, &block.why)?;
    update_frame(digest, block.block_id.as_bytes())?;
    update_frame(digest, block.source_sha256.as_bytes())
}

fn update_score(digest: &mut Sha256, score: ScoreComponents) -> Result<(), EvidenceError> {
    for value in [
        score.exact_phrase,
        score.identifier,
        score.all_terms,
        score.heading,
        score.path,
        score.proximity,
    ] {
        update_number(digest, value)?;
    }
    Ok(())
}

fn update_strings(digest: &mut Sha256, values: &[String]) -> Result<(), EvidenceError> {
    update_number(digest, values.len())?;
    for value in values {
        update_frame(digest, value.as_bytes())?;
    }
    Ok(())
}

fn update_number(digest: &mut Sha256, value: impl Display) -> Result<(), EvidenceError> {
    update_frame(digest, value.to_string().as_bytes())
}

fn update_frame(digest: &mut Sha256, value: &[u8]) -> Result<(), EvidenceError> {
    let length = u64::try_from(value.len()).map_err(|_| EvidenceError::FrameTooLarge)?;
    digest.update(length.to_be_bytes());
    digest.update(value);
    Ok(())
}

const fn critic_verdict(verdict: CriticVerdict) -> &'static [u8] {
    match verdict {
        CriticVerdict::Accepted => b"accepted",
        CriticVerdict::Rejected => b"rejected",
    }
}

const fn skip_reason(reason: SkipReason) -> &'static [u8] {
    match reason {
        SkipReason::Ignored => b"ignored",
        SkipReason::Hidden => b"hidden",
        SkipReason::Secret => b"secret",
        SkipReason::Symlink => b"symlink",
        SkipReason::NonRegular => b"non_regular",
        SkipReason::Oversized => b"oversized",
        SkipReason::Budget => b"budget",
        SkipReason::InvalidPath => b"invalid_path",
        SkipReason::IoError => b"io_error",
    }
}
