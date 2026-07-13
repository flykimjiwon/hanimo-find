use std::{
    io::{self, Read as _},
    path::{Component, Path},
};

use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::fs::Dir;
use ignore::WalkBuilder;

use super::{DiagnoseBudget, DiagnoseError, DiagnoseLimit};

pub(super) fn scan_sources(
    root: &Path,
    budget: DiagnoseBudget,
    visit: impl FnMut(&str, &[u8]),
) -> Result<(), DiagnoseError> {
    scan_sources_with_hook(
        root,
        budget,
        ScanCallbacks {
            visit,
            before_open: no_before_open,
        },
    )
}

const fn no_before_open(_: &Path) {}

struct ScanCallbacks<V, H> {
    visit: V,
    before_open: H,
}

struct ReadBudget {
    maximum_file: usize,
    remaining_total: usize,
}

fn scan_sources_with_hook<V, H>(
    root: &Path,
    budget: DiagnoseBudget,
    mut callbacks: ScanCallbacks<V, H>,
) -> Result<(), DiagnoseError>
where
    V: FnMut(&str, &[u8]),
    H: FnMut(&Path),
{
    let opened_root = crate::root::open(root).map_err(diagnose_root_error)?;
    let absolute = opened_root.absolute;
    let root = opened_root.directory;
    let mut builder = WalkBuilder::new(&absolute);
    builder
        .follow_links(false)
        .sort_by_file_path(std::cmp::Ord::cmp);
    let mut candidate_files = 0_usize;
    let mut total_bytes = 0_usize;
    for entry in builder.build() {
        let entry = entry?;
        if entry.depth() == 0 || !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        if candidate_files == budget.max_candidate_files.get() {
            return Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::CandidateFiles));
        }
        candidate_files = candidate_files.saturating_add(1);
        let relative = entry
            .path()
            .strip_prefix(&absolute)
            .map_err(|_| DiagnoseError::InvalidPath)?;
        let display = relative_utf8(relative)?;
        (callbacks.before_open)(relative);
        let remaining = budget
            .max_total_bytes
            .get()
            .checked_sub(total_bytes)
            .ok_or(DiagnoseError::BudgetExceeded(DiagnoseLimit::TotalBytes))?;
        let bytes = read_nofollow(
            &root,
            relative,
            &ReadBudget {
                maximum_file: budget.max_file_bytes.get(),
                remaining_total: remaining,
            },
        )?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or(DiagnoseError::BudgetExceeded(DiagnoseLimit::TotalBytes))?;
        (callbacks.visit)(&display, &bytes);
    }
    Ok(())
}

fn read_nofollow(
    root: &Dir,
    relative: &Path,
    budget: &ReadBudget,
) -> Result<Vec<u8>, DiagnoseError> {
    let mut directory = root.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(DiagnoseError::InvalidPath);
        };
        if components.peek().is_some() {
            directory = directory.open_dir_nofollow(name)?;
            continue;
        }
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = directory.open_with(name, &options)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "source is not regular").into());
        }
        if metadata.len() > u64::try_from(budget.maximum_file).map_err(io::Error::other)? {
            return Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::FileBytes));
        }
        if metadata.len() > u64::try_from(budget.remaining_total).map_err(io::Error::other)? {
            return Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::TotalBytes));
        }
        let read_limit = budget
            .maximum_file
            .min(budget.remaining_total)
            .checked_add(1)
            .ok_or(DiagnoseError::BudgetExceeded(DiagnoseLimit::FileBytes))?;
        let mut bytes = Vec::new();
        file.take(u64::try_from(read_limit).map_err(io::Error::other)?)
            .read_to_end(&mut bytes)?;
        if bytes.len() > budget.maximum_file {
            return Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::FileBytes));
        }
        if bytes.len() > budget.remaining_total {
            return Err(DiagnoseError::BudgetExceeded(DiagnoseLimit::TotalBytes));
        }
        return Ok(bytes);
    }
    Err(DiagnoseError::InvalidPath)
}

fn diagnose_root_error(error: crate::root::OpenRootError) -> DiagnoseError {
    match error {
        crate::root::OpenRootError::Symlink => DiagnoseError::RootSymlink,
        crate::root::OpenRootError::InvalidPath => DiagnoseError::InvalidPath,
        crate::root::OpenRootError::Io(source) => DiagnoseError::Read(source),
    }
}

fn relative_utf8(path: &Path) -> Result<String, DiagnoseError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str().ok_or(DiagnoseError::InvalidPath)?),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return Err(DiagnoseError::InvalidPath),
        }
    }
    if parts.is_empty() {
        return Err(DiagnoseError::InvalidPath);
    }
    Ok(parts.join("/"))
}

#[cfg(all(test, unix))]
#[path = "filesystem_tests.rs"]
mod tests;
