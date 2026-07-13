use std::{
    ffi::OsString,
    io::{self, Read as _},
    path::{Component, Path, PathBuf},
};

use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::fs::Dir;

pub(super) fn open_root(root: &Path) -> io::Result<Dir> {
    if root.as_os_str().is_empty()
        || root
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe verification root",
        ));
    }
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    let absolute: PathBuf = absolute.components().collect();
    let Some(name) = absolute.file_name() else {
        return Dir::open_ambient_dir(absolute, cap_std::ambient_authority());
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unsafe verification root"))?;
    Dir::open_ambient_dir(parent, cap_std::ambient_authority())?.open_dir_nofollow(name)
}

pub(super) fn path_from_bytes(raw: &[u8]) -> Option<PathBuf> {
    if raw.is_empty()
        || raw.contains(&0)
        || raw.starts_with(b"/")
        || raw.ends_with(b"/")
        || raw
            .split(|byte| *byte == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return None;
    }
    #[cfg(unix)]
    let candidate = Some(platform_path(raw));
    #[cfg(not(unix))]
    let candidate = platform_path(raw);
    candidate.filter(|path| {
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    })
}

pub(super) fn read_nofollow(root: &Dir, relative: &Path, maximum: usize) -> io::Result<Vec<u8>> {
    let mut directory = root.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsafe path"));
        };
        if components.peek().is_some() {
            directory = directory.open_dir_nofollow(name)?;
            continue;
        }
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = directory.open_with(name, &options)?;
        let metadata = file.metadata()?;
        let maximum_u64 = u64::try_from(maximum)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid size limit"))?;
        if !metadata.is_file() || metadata.len() > maximum_u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file policy"));
        }
        let mut bytes = Vec::new();
        file.take(maximum_u64.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() > maximum {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file policy"));
        }
        return Ok(bytes);
    }
    Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"))
}

#[cfg(unix)]
fn platform_path(raw: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStringExt as _;

    PathBuf::from(OsString::from_vec(raw.to_vec()))
}

#[cfg(not(unix))]
fn platform_path(raw: &[u8]) -> Option<PathBuf> {
    String::from_utf8(raw.to_vec()).ok().map(PathBuf::from)
}
