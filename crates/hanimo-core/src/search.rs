use std::{io, path::Path};

use thiserror::Error;

mod filesystem;
mod ranking;

use crate::{
    bytes::{
        BytesError, LineRange, SourceLine, canonical_block, is_heading, occurrence_count,
        parse_lines,
    },
    identity::{BlockIdentityInput, IdentityError, block_id, source_sha256},
    model::{
        ByteOffset, EvidenceBlock, LineNumber, QueryLimitError, QueryPlan, SCHEMA_VERSION,
        SearchResult, SkipReason, SkippedEvidence,
    },
    rank::{matched_terms, reasons, score},
    render_support::EncodedBytes,
};

use filesystem::{
    CandidatePath, CandidateRead, Discovery, ReadBudget, SkippedPath, discover,
    extend_budget_suffix, read_candidate,
};
use ranking::{RankedBlock, compare_ranked};

/// Fail-closed search errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SearchError {
    /// Query schema version is unsupported.
    #[error("unsupported query schema version")]
    UnsupportedSchema,
    /// Query work exceeds a fixed admission ceiling.
    #[error(transparent)]
    Query(#[from] QueryLimitError),
    /// The supplied search root is itself a symbolic link.
    #[error("search root must not be a symbolic link")]
    RootSymlink,
    /// The supplied root cannot become a capability directory.
    #[error("cannot open search root")]
    RootOpen(#[source] io::Error),
    /// Ignore-aware traversal failed.
    #[error("ignore-aware traversal failed")]
    Walk(#[source] ignore::Error),
    /// A discovered path is not a safe root-relative path.
    #[error("unsafe root-relative path")]
    UnsafePath,
    /// A capability-relative file operation failed.
    #[error("capability-relative file operation failed")]
    FileIo(#[source] io::Error),
    /// A file changed type or exceeded its declared per-file budget.
    #[error("candidate violates file policy")]
    FilePolicy,
    /// The deterministic scan budget was exhausted.
    #[error("deterministic scan budget exhausted")]
    BudgetExceeded,
    /// Raw-byte line canonicalization failed.
    #[error("raw-byte canonicalization failed")]
    Canonicalization,
    /// Block identity construction failed.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

struct FileContext<'a> {
    plan: &'a QueryPlan,
    candidate: &'a CandidatePath,
    lines: &'a [SourceLine<'a>],
    source_digest: &'a str,
}

/// Searches one filesystem capability using exact literal bytes.
pub fn search(root: &Path, plan: &QueryPlan) -> Result<SearchResult, SearchError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(SearchError::UnsupportedSchema);
    }
    plan.validate_limits()?;
    let opened_root = crate::root::open(root).map_err(search_root_error)?;
    let Discovery {
        candidates,
        mut skipped,
    } = discover(&opened_root.absolute)?;
    let mut total_bytes = 0_usize;
    let mut total_matches = 0_usize;
    let mut ranked = Vec::new();
    let mut candidates = candidates.into_iter();
    while let Some(candidate) = candidates.next() {
        let remaining_total_bytes = plan
            .budget
            .max_total_bytes
            .get()
            .checked_sub(total_bytes)
            .ok_or(SearchError::BudgetExceeded)?;
        let read = read_candidate(
            &opened_root.directory,
            &candidate.relative,
            ReadBudget {
                maximum_file_bytes: plan.budget.max_file_bytes.get(),
                remaining_total_bytes,
            },
        )?;
        let raw = match read {
            CandidateRead::Content(raw) => raw,
            CandidateRead::Skipped(SkipReason::Budget) => {
                extend_budget_suffix(&mut skipped, candidate, candidates);
                break;
            }
            CandidateRead::Skipped(reason) => {
                skipped.push(SkippedPath {
                    raw: candidate.raw,
                    reason,
                });
                continue;
            }
        };
        total_bytes = total_bytes
            .checked_add(raw.len())
            .ok_or(SearchError::BudgetExceeded)?;
        let (blocks, file_matches) = blocks_for_file(plan, &candidate, &raw)?;
        let next_total_matches = total_matches
            .checked_add(file_matches)
            .ok_or(SearchError::BudgetExceeded)?;
        if next_total_matches > plan.budget.max_matches.get() {
            extend_budget_suffix(&mut skipped, candidate, candidates);
            break;
        }
        total_matches = next_total_matches;
        ranked.extend(blocks);
    }
    ranked.sort_by(compare_ranked);
    ranked.truncate(plan.budget.max_blocks.get());
    skipped.sort_by(|left, right| {
        left.raw
            .cmp(&right.raw)
            .then(left.reason.cmp(&right.reason))
    });
    Ok(SearchResult {
        blocks: ranked.into_iter().map(|entry| entry.block).collect(),
        skipped: skipped
            .into_iter()
            .map(|gap| SkippedEvidence {
                path: EncodedBytes::from_bytes(&gap.raw),
                reason: gap.reason,
            })
            .collect(),
    })
}

fn search_root_error(error: crate::root::OpenRootError) -> SearchError {
    match error {
        crate::root::OpenRootError::Symlink => SearchError::RootSymlink,
        crate::root::OpenRootError::InvalidPath => SearchError::UnsafePath,
        crate::root::OpenRootError::Io(source) => SearchError::RootOpen(source),
    }
}

fn blocks_for_file(
    plan: &QueryPlan,
    candidate: &CandidatePath,
    raw: &[u8],
) -> Result<(Vec<RankedBlock>, usize), SearchError> {
    let lines = parse_lines(raw)?;
    let (ranges, file_matches) = matching_ranges(plan, &lines)?;
    let source_digest = source_sha256(raw);
    let context = FileContext {
        plan,
        candidate,
        lines: &lines,
        source_digest: &source_digest,
    };
    let blocks = ranges
        .into_iter()
        .map(|range| build_block(&context, range))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((blocks, file_matches))
}

fn matching_ranges(
    plan: &QueryPlan,
    lines: &[SourceLine<'_>],
) -> Result<(Vec<LineRange>, usize), SearchError> {
    let needles: Vec<&[u8]> = plan
        .quoted_phrases
        .iter()
        .chain(&plan.identifiers)
        .chain(&plan.terms)
        .map(String::as_bytes)
        .filter(|value| !value.is_empty())
        .collect();
    let last = lines.len().saturating_sub(1);
    let mut ranges = Vec::new();
    let mut file_matches = 0_usize;
    for (index, line) in lines.iter().enumerate() {
        let line_matches = needles.iter().try_fold(0_usize, |count, needle| {
            count
                .checked_add(occurrence_count(line.content, needle))
                .ok_or(SearchError::BudgetExceeded)
        })?;
        if line_matches == 0 {
            continue;
        }
        file_matches = file_matches
            .checked_add(line_matches)
            .ok_or(SearchError::BudgetExceeded)?;
        let context_end = index.saturating_add(plan.budget.context_lines).min(last);
        let context_start = index.saturating_sub(plan.budget.context_lines);
        let heading_start = lines
            .get(..=index)
            .and_then(|prior| prior.iter().rposition(|item| is_heading(item.content)))
            .unwrap_or(context_start);
        ranges.push(LineRange {
            start: heading_start.min(context_start),
            end: context_end,
        });
    }
    Ok((merge_ranges(ranges), file_matches))
}

fn merge_ranges(ranges: Vec<LineRange>) -> Vec<LineRange> {
    let mut merged: Vec<LineRange> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(previous) if range.start <= previous.end.saturating_add(1) => {
                previous.end = previous.end.max(range.end);
            }
            Some(_) | None => merged.push(range),
        }
    }
    merged
}

fn build_block(context: &FileContext<'_>, range: LineRange) -> Result<RankedBlock, SearchError> {
    let canonical = canonical_block(context.lines, range)?;
    let selected = context
        .lines
        .get(range.start..=range.end)
        .ok_or(BytesError::InvalidRange)?;
    let line_start = LineNumber::from_zero_based(range.start).ok_or(BytesError::OffsetOverflow)?;
    let line_end = LineNumber::from_zero_based(range.end).ok_or(BytesError::OffsetOverflow)?;
    let components = score(context.plan, selected, &canonical.content);
    let total = components.total().ok_or(SearchError::BudgetExceeded)?;
    let identifier = block_id(BlockIdentityInput {
        path: &context.candidate.raw,
        line_start,
        line_end,
        content: &canonical.content,
    })?;
    let block = EvidenceBlock {
        path: EncodedBytes::from_bytes(&context.candidate.raw),
        line_start,
        line_end,
        byte_start: ByteOffset::new(canonical.byte_start),
        byte_end: ByteOffset::new(canonical.byte_end),
        content: EncodedBytes::from_bytes(&canonical.content),
        matched_terms: matched_terms(context.plan, &canonical.content),
        score: total,
        score_components: components,
        why: reasons(components),
        block_id: identifier,
        source_sha256: context.source_digest.to_owned(),
    };
    Ok(RankedBlock {
        block,
        raw_path: context.candidate.raw.clone(),
    })
}

impl From<BytesError> for SearchError {
    fn from(_: BytesError) -> Self {
        Self::Canonicalization
    }
}
