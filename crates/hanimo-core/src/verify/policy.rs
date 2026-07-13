use crate::{
    evidence::{CriticVerdict, EvidenceBundle},
    model::SkipReason,
};

/// Maximum serialized evidence-bundle bytes accepted by verification.
pub const MAX_VERIFY_BUNDLE_BYTES: usize = 16_777_216;
pub(super) const MAX_VERIFY_SOURCE_BYTES: usize = 134_217_728;

const MAX_BLOCKS: usize = 64;
const MAX_SKIPPED: usize = 65_536;
const MAX_LIST_ITEMS: usize = 4_096;
const MAX_CONTEXT_LINES: usize = 4_096;
const MAX_FILE_BYTES: usize = 16_777_216;
const MAX_TOTAL_BYTES: usize = 67_108_864;
const MAX_MATCHES: usize = 1_000_000;

pub(super) fn validate_limits(bundle: &EvidenceBundle) -> Result<(), &'static str> {
    if bundle.blocks.len() > MAX_BLOCKS {
        return Err("blocks exceed verification limit");
    }
    if bundle.skipped.len() > MAX_SKIPPED {
        return Err("skipped entries exceed verification limit");
    }
    if bundle.critic.covered_quoted_phrases.len() > MAX_LIST_ITEMS
        || bundle.critic.covered_identifiers.len() > MAX_LIST_ITEMS
        || bundle.critic.uncovered.len() > MAX_LIST_ITEMS
        || bundle.blocks.iter().any(|block| {
            block.matched_terms.len() > MAX_LIST_ITEMS || block.why.len() > MAX_LIST_ITEMS
        })
    {
        return Err("nested arrays exceed verification limit");
    }
    let budget = bundle.budget;
    if budget.max_blocks.get() > MAX_BLOCKS {
        return Err("max_blocks exceeds verification limit");
    }
    if budget.context_lines > MAX_CONTEXT_LINES {
        return Err("context_lines exceeds verification limit");
    }
    if budget.max_file_bytes.get() > MAX_FILE_BYTES {
        return Err("max_file_bytes exceeds verification limit");
    }
    if budget.max_total_bytes.get() > MAX_TOTAL_BYTES {
        return Err("max_total_bytes exceeds verification limit");
    }
    if budget.max_matches.get() > MAX_MATCHES {
        return Err("max_matches exceeds verification limit");
    }
    Ok(())
}

pub(super) fn critic_is_consistent(bundle: &EvidenceBundle) -> bool {
    let incomplete = bundle.blocks.is_empty()
        || !bundle.critic.uncovered.is_empty()
        || bundle
            .skipped
            .iter()
            .any(|skip| skip.reason == SkipReason::Budget);
    bundle.critic.verdict
        == if incomplete {
            CriticVerdict::Rejected
        } else {
            CriticVerdict::Accepted
        }
}
