use std::{
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
};

use cap_fs_ext::DirExt as _;
use cap_std::fs::Dir;

pub(crate) enum OpenRootError {
    Symlink,
    InvalidPath,
    Io(io::Error),
}

pub(crate) struct OpenedRoot {
    pub(crate) absolute: PathBuf,
    pub(crate) directory: Dir,
}

pub(crate) fn open(root: &Path) -> Result<OpenedRoot, OpenRootError> {
    if root.is_absolute() {
        open_absolute(root)
    } else {
        open_relative(root)
    }
}

fn open_absolute(root: &Path) -> Result<OpenedRoot, OpenRootError> {
    let absolute = normalize_absolute(root)?;
    let anchor = absolute
        .ancestors()
        .last()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(OpenRootError::InvalidPath)?;
    let relative = absolute
        .strip_prefix(anchor)
        .map_err(|_| OpenRootError::InvalidPath)?;
    let mut directory =
        Dir::open_ambient_dir(anchor, cap_std::ambient_authority()).map_err(OpenRootError::Io)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(OpenRootError::InvalidPath);
        };
        directory = open_component(&directory, name)?;
    }
    Ok(OpenedRoot {
        absolute,
        directory,
    })
}

fn open_relative(root: &Path) -> Result<OpenedRoot, OpenRootError> {
    if root.as_os_str().is_empty() {
        return Err(OpenRootError::InvalidPath);
    }
    let current = std::env::current_dir().map_err(OpenRootError::Io)?;
    let mut absolute = std::fs::canonicalize(current).map_err(OpenRootError::Io)?;
    let mut directory = Dir::open_ambient_dir(&absolute, cap_std::ambient_authority())
        .map_err(OpenRootError::Io)?;
    for component in root.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => {
                directory = open_component(&directory, name)?;
                absolute.push(name);
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(OpenRootError::InvalidPath);
            }
        }
    }
    Ok(OpenedRoot {
        absolute,
        directory,
    })
}

fn normalize_absolute(root: &Path) -> Result<PathBuf, OpenRootError> {
    let mut normalized = PathBuf::new();
    for component in root.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => return Err(OpenRootError::InvalidPath),
            Component::Normal(name) => normalized.push(name),
        }
    }
    if normalized.is_absolute() {
        Ok(normalized)
    } else {
        Err(OpenRootError::InvalidPath)
    }
}

fn open_component(parent: &Dir, name: &OsStr) -> Result<Dir, OpenRootError> {
    match parent.open_dir_nofollow(name) {
        Ok(directory) => Ok(directory),
        Err(source) => match parent.symlink_metadata(name) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(OpenRootError::Symlink),
            Ok(_) | Err(_) => Err(OpenRootError::Io(source)),
        },
    }
}
