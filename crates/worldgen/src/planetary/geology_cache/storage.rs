//! Disposable geology entries, separate from saves and player edits.
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod format;
use format::{digest_name, filename, invalid, owned_name, read_entry, read_header, write_entry};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 2;
const LOCK: &str = "axiom-geology-v1.lock";
static NONCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Store { root: PathBuf }

impl Store {
    pub(super) fn new(root: PathBuf) -> Self { Self { root } }

    pub(super) fn read(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        format::key_valid(key)?;
        match check_root(&self.root) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            result => result?,
        }
        let path = self.root.join(filename(key));
        let mut file = match regular_file(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let length = file.metadata()?.len();
        if length > format::HEADER + format::MAX_KEY as u64 + format::MAX_PAYLOAD { return Ok(None); }
        match read_entry(&mut file, length, key) {
            Ok(payload) => Ok(Some(payload)),
            Err(e) if matches!(e.kind(), ErrorKind::InvalidData | ErrorKind::UnexpectedEof) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(super) fn write(&self, key: &[u8], payload: &[u8]) -> io::Result<()> {
        format::key_valid(key)?;
        if payload.len() as u64 > format::MAX_PAYLOAD { return Err(invalid("cache payload too large")); }
        check_ancestors(&self.root)?;
        fs::create_dir_all(&self.root)?;
        check_root(&self.root)?;
        let lock_path = self.root.join(LOCK);
        if let Ok(meta) = fs::symlink_metadata(&lock_path) {
            if !plain(&meta) || meta.len() != 0 { return Err(invalid("unexpected cache lock")); }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)] {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x0020_0000);
        }
        let lock = options.open(&lock_path)?;
        if !plain(&lock.metadata()?) || lock.metadata()?.len() != 0 { return Err(invalid("cache lock is not regular")); }
        lock.try_lock().map_err(|e| io::Error::new(ErrorKind::WouldBlock, e.to_string()))?;
        let target = self.root.join(filename(key));
        let mut entries = self.inventory()?;
        for entry in entries.iter().filter(|e| e.temporary && e.deletable) { fs::remove_file(&entry.path)?; }
        entries.retain(|e| !e.temporary || !e.deletable);
        let already_valid = self.read(key)?.is_some();
        let replacing = entries.iter().any(|e| e.path == target);
        if replacing && !already_valid && !entries.iter().any(|e| e.path == target && e.deletable) {
            return Err(invalid("unrecognized file at cache destination"));
        }
        let needed = if already_valid { 0 } else { format::HEADER + key.len() as u64 + payload.len() as u64 };
        let added = usize::from(!already_valid && !replacing);
        entries.sort_by_key(|e| (e.modified, e.path.clone()));
        loop {
            let count = entries.iter().filter(|e| !e.temporary).count();
            let bytes = entries.iter().try_fold(needed, |n, e| n.checked_add(e.length))
                .ok_or_else(|| invalid("cache quota overflow"))?;
            if count + added <= MAX_ENTRIES && bytes <= MAX_BYTES { break; }
            let index = entries.iter().position(|e| e.path != target && e.deletable)
                .ok_or_else(|| invalid("cache quota blocked by unrecognized entries"))?;
            fs::remove_file(&entries[index].path)?;
            entries.remove(index);
        }
        if already_valid { return Ok(()); }
        let (mut file, mut temporary) = self.temporary(key)?;
        write_entry(&mut file, key, payload)?;
        file.sync_all()?;
        drop(file);
        // A corrupt recognized entry is replaceable; valid entries are immutable.
        if replacing { fs::remove_file(&target)?; }
        fs::rename(&temporary.path, &target)?;
        temporary.published = true;
        // The same-directory rename publishes either a complete file or no hit.
        // A power loss may discard this disposable entry; it never changes a save.
        drop(lock);
        Ok(())
    }

    fn temporary(&self, key: &[u8]) -> io::Result<(File, Temporary)> {
        for _ in 0..16 {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
            let nonce = time ^ NONCE.fetch_add(1, Ordering::Relaxed);
            let path = self.root.join(format!("{}{}.{:x}.{nonce:016x}.tmp", format::PREFIX, digest_name(key), std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((file, Temporary { path, published: false })),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(ErrorKind::AlreadyExists, "cache temporary name collision"))
    }

    fn inventory(&self) -> io::Result<Vec<Entry>> {
        let mut result = Vec::new();
        for (index, item) in fs::read_dir(&self.root)?.enumerate() {
            if index >= 4096 { return Err(invalid("cache directory has too many entries")); }
            let item = item?;
            let name = item.file_name();
            let Some(name) = name.to_str() else { continue; };
            let Some((digest, temporary)) = owned_name(name) else { continue; };
            let path = item.path();
            let mut file = regular_file(&path)?;
            let meta = file.metadata()?;
            let recognized = read_header(&mut file).is_ok_and(|h| {
                digest_name(&h.key) == digest
            });
            result.push(Entry { path, length: meta.len(), temporary, deletable: recognized,
                modified: meta.modified().unwrap_or(UNIX_EPOCH) });
        }
        Ok(result)
    }
}

struct Entry { path: PathBuf, length: u64, temporary: bool, deletable: bool, modified: SystemTime }
struct Temporary { path: PathBuf, published: bool }
impl Drop for Temporary {
    fn drop(&mut self) { if !self.published { let _ = fs::remove_file(&self.path); } }
}

fn plain(meta: &Metadata) -> bool {
    if !meta.is_file() || meta.file_type().is_symlink() { return false; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return false; }
    }
    true
}

fn regular_file(path: &Path) -> io::Result<File> {
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

fn check_ancestors(root: &Path) -> io::Result<()> {
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

fn check_root(root: &Path) -> io::Result<()> {
    check_ancestors(root)?;
    if !fs::symlink_metadata(root)?.is_dir() { return Err(invalid("cache root is not a directory")); }
    Ok(())
}

#[cfg(test)]
#[path = "storage/tests.rs"]
mod tests;
