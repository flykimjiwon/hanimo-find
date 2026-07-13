use std::{
    io::Read as _,
    path::{Component, Path, PathBuf},
};

use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::fs::Dir;
use ignore::WalkBuilder;

use crate::model::{MAX_CANDIDATE_FILES, MAX_DISCOVERY_DEPTH, MAX_DISCOVERY_ENTRIES, SkipReason};

use super::SearchError;

#[derive(Debug)]
pub(super) struct CandidatePath {
    pub(super) relative: PathBuf,
    pub(super) raw: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct SkippedPath {
    pub(super) raw: Vec<u8>,
    pub(super) reason: SkipReason,
}

#[derive(Debug)]
pub(super) struct Discovery {
    pub(super) candidates: Vec<CandidatePath>,
    pub(super) skipped: Vec<SkippedPath>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReadBudget {
    pub(super) maximum_file_bytes: usize,
    pub(super) remaining_total_bytes: usize,
}

#[derive(Debug)]
pub(super) enum CandidateRead {
    Content(Vec<u8>),
    Skipped(SkipReason),
}

pub(super) fn extend_budget_suffix(
    skipped: &mut Vec<SkippedPath>,
    current: CandidatePath,
    remaining: impl IntoIterator<Item = CandidatePath>,
) {
    skipped.extend(
        std::iter::once(current)
            .chain(remaining)
            .map(|candidate| SkippedPath {
                raw: candidate.raw,
                reason: SkipReason::Budget,
            }),
    );
}

pub(super) fn discover(root: &Path) -> Result<Discovery, SearchError> {
    let mut builder = WalkBuilder::new(root);
    let overflow_depth = MAX_DISCOVERY_DEPTH
        .checked_add(1)
        .ok_or(SearchError::BudgetExceeded)?;
    builder
        .follow_links(false)
        .hidden(true)
        .ignore(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(false)
        .require_git(false)
        .max_depth(Some(overflow_depth));
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let mut entries_seen = 0_usize;
    for item in builder.build() {
        let entry = item.map_err(SearchError::Walk)?;
        if entry.depth() == 0 {
            continue;
        }
        entries_seen = entries_seen
            .checked_add(1)
            .ok_or(SearchError::BudgetExceeded)?;
        if entries_seen > MAX_DISCOVERY_ENTRIES || entry.depth() > MAX_DISCOVERY_DEPTH {
            return Ok(generic_budget_gap());
        }
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| SearchError::UnsafePath)?
            .to_path_buf();
        let raw = relative_path_bytes(&relative)?;
        if is_secret_like(&raw) {
            skipped.push(SkippedPath {
                raw,
                reason: SkipReason::Secret,
            });
            continue;
        }
        candidates.push(CandidatePath { relative, raw });
    }
    candidates.sort_by(|left, right| left.raw.cmp(&right.raw));
    if candidates.len() > MAX_CANDIDATE_FILES {
        let omitted = candidates.split_off(MAX_CANDIDATE_FILES);
        if let Some(boundary) = omitted.into_iter().next() {
            skipped.push(SkippedPath {
                raw: boundary.raw,
                reason: SkipReason::Budget,
            });
        }
    }
    Ok(Discovery {
        candidates,
        skipped,
    })
}

fn generic_budget_gap() -> Discovery {
    Discovery {
        candidates: Vec::new(),
        skipped: vec![SkippedPath {
            raw: Vec::new(),
            reason: SkipReason::Budget,
        }],
    }
}

pub(super) fn read_candidate(
    root: &Dir,
    relative: &Path,
    budget: ReadBudget,
) -> Result<CandidateRead, SearchError> {
    let mut directory = root.try_clone().map_err(SearchError::FileIo)?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(SearchError::UnsafePath);
        };
        if components.peek().is_some() {
            directory = directory
                .open_dir_nofollow(name)
                .map_err(SearchError::FileIo)?;
            continue;
        }
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = directory
            .open_with(name, &options)
            .map_err(SearchError::FileIo)?;
        let metadata = file.metadata().map_err(SearchError::FileIo)?;
        if !metadata.is_file() {
            return Ok(CandidateRead::Skipped(SkipReason::NonRegular));
        }
        let maximum_file =
            u64::try_from(budget.maximum_file_bytes).map_err(|_| SearchError::FilePolicy)?;
        if metadata.len() > maximum_file {
            return Ok(CandidateRead::Skipped(SkipReason::Oversized));
        }
        let remaining_total =
            u64::try_from(budget.remaining_total_bytes).map_err(|_| SearchError::BudgetExceeded)?;
        if metadata.len() > remaining_total {
            return Ok(CandidateRead::Skipped(SkipReason::Budget));
        }
        let read_limit = maximum_file
            .min(remaining_total)
            .checked_add(1)
            .ok_or(SearchError::FilePolicy)?;
        let mut bytes = Vec::new();
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(SearchError::FileIo)?;
        if bytes.len() > budget.maximum_file_bytes {
            return Ok(CandidateRead::Skipped(SkipReason::Oversized));
        }
        if bytes.len() > budget.remaining_total_bytes {
            return Ok(CandidateRead::Skipped(SkipReason::Budget));
        }
        return Ok(CandidateRead::Content(bytes));
    }
    Err(SearchError::UnsafePath)
}

fn is_secret_like(path: &[u8]) -> bool {
    path.split(|byte| *byte == b'/').any(|component| {
        let lower = component.to_ascii_lowercase();
        lower.starts_with(b".")
            || lower == b"secrets"
            || lower.starts_with(b".env")
            || lower.ends_with(b".key")
            || lower.ends_with(b".pem")
            || lower.windows(6).any(|window| window == b"secret")
            || lower.windows(5).any(|window| window == b"token")
            || lower.windows(10).any(|window| window == b"credential")
    })
}

#[cfg(unix)]
fn relative_path_bytes(path: &Path) -> Result<Vec<u8>, SearchError> {
    use std::os::unix::ffi::OsStrExt as _;

    validate_relative(path)?;
    Ok(path.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn relative_path_bytes(path: &Path) -> Result<Vec<u8>, SearchError> {
    validate_relative(path)?;
    let text = path.to_str().ok_or(SearchError::UnsafePath)?;
    Ok(text.replace('\\', "/").into_bytes())
}

fn validate_relative(path: &Path) -> Result<(), SearchError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SearchError::UnsafePath);
    }
    Ok(())
}
