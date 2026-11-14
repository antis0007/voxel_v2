//! Filesystem checks retained from the audited geology store.
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Component, Path};
use super::format::invalid;

pub(super) fn plain(meta: &Metadata) -> bool {
    if !meta.is_file() || meta.file_type().is_symlink() { return false; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return false; }
    }
    true
}

pub(super) fn regular_file(path: &Path) -> io::Result<File> {
    if !plain(&fs::symlink_metadata(path)?) { return Err(invalid("cache entry is not a regular file")); }
    let mut options = OpenOptions::new(); options.read(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // OPEN_REPARSE_POINT: inspect, never follow the final link.
    }
    let file = options.open(path)?;
    if !plain(&file.metadata()?) { return Err(invalid("cache entry changed type")); }
    Ok(file)
}

pub(super) fn check_ancestors(root: &Path) -> io::Result<()> {
    if !root.is_absolute() || root.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(invalid("cache root must be an absolute path without parent components"));
    }
    for path in root.ancestors() {
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                if !meta.is_dir() || meta.file_type().is_symlink() { return Err(invalid("cache ancestor is not a plain directory")); }
                #[cfg(windows)] {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 { return Err(invalid("cache ancestor is a reparse point")); }
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub(super) fn check_root(root: &Path) -> io::Result<()> {
    check_ancestors(root)?;
    if !fs::symlink_metadata(root)?.is_dir() { return Err(invalid("cache root is not a directory")); }
    Ok(())
}


pub(super) const LOCK: &str = "axiom-stage-v1.lock";

pub(super) fn writer_lease(root: &Path) -> io::Result<Option<File>> {
    check_ancestors(root)?;
    fs::create_dir_all(root)?;
    check_root(root)?;
    let path = root.join(LOCK);
    match fs::symlink_metadata(&path) {
        Ok(meta) if !plain(&meta) || meta.len() != 0 => return Err(invalid("unexpected cache lock")),
        Ok(_) => (),
        Err(e) if e.kind() == ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !plain(&meta) || meta.len() != 0 { return Err(invalid("cache lock is not regular")); }
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}
