use super::{format, fs, Limits, WriteOutcome};
use std::collections::BTreeMap;
use std::fs::{self as disk, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NONCE: AtomicU64 = AtomicU64::new(0);
struct Entry { bytes: u64, modified: SystemTime, owned: bool }
pub(super) struct Writer {
    _lease: File,
    entries: BTreeMap<String, Entry>,
    bytes: u64,
}

impl Writer {
    pub(super) fn open(root: &Path, limits: Limits) -> io::Result<Option<Self>> {
        let Some(lease) = fs::writer_lease(root)? else { return Ok(None); };
        let mut entries = BTreeMap::new();
        let mut bytes = 0_u64;
        for (index, item) in disk::read_dir(root)?.enumerate() {
            if index >= limits.entries { return Err(format::invalid("cache entry limit")); }
            let item = item?;
            let name = item.file_name().into_string().map_err(|_| format::invalid("non-UTF8 cache entry"))?;
            if name == fs::LOCK { continue; }
            let mut file = fs::regular_file(&item.path())?;
            let meta = file.metadata()?;
            let ownership = if let Some((hash, temporary)) = format::owned_name(&name) {
                match format::read_header(&mut file) {
                    Ok(h) => (format::digest_name(&h.key) == hash).then_some(temporary),
                    Err(e) if matches!(e.kind(), ErrorKind::InvalidData | ErrorKind::UnexpectedEof) => None,
                    Err(e) => return Err(e),
                }
            } else { None };
            if ownership == Some(true) {
                drop(file);
                disk::remove_file(item.path())?;
                continue;
            }
            bytes = bytes.checked_add(meta.len()).ok_or_else(|| format::invalid("cache byte overflow"))?;
            entries.insert(name, Entry { bytes: meta.len(), modified: meta.modified()?, owned: ownership == Some(false) });
        }
        Ok(Some(Self { _lease: lease, entries, bytes }))
    }

    fn checked(&self, root: &Path, name: &str) -> io::Result<Option<File>> {
        let expected = self.entries.get(name);
        let file = match fs::regular_file(&root.join(name)) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound && expected.is_none() => return Ok(None),
            Err(e) => return Err(e),
        };
        let meta = file.metadata()?;
        let Some(entry) = expected else { return Err(format::invalid("cache changed outside writer")); };
        if meta.len() != entry.bytes || meta.modified()? != entry.modified {
            return Err(format::invalid("cache entry changed outside writer"));
        }
        Ok(Some(file))
    }

    fn owned_header(file: &mut File, name: &str) -> io::Result<bool> {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        let Some((hash, temporary)) = format::owned_name(name) else { return Ok(false); };
        if temporary { return Ok(false); }
        match format::read_header(file) {
            Ok(h) => Ok(format::digest_name(&h.key) == hash),
            Err(e) if matches!(e.kind(), ErrorKind::InvalidData | ErrorKind::UnexpectedEof) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn evict(&mut self, root: &Path, target: &str) -> io::Result<bool> {
        let oldest = self.entries.iter().filter(|(name, entry)| name.as_str() != target && entry.owned)
            .min_by_key(|(name, entry)| (entry.modified, *name)).map(|(name, _)| name.clone());
        let Some(name) = oldest else { return Ok(false); };
        let Some(mut file) = self.checked(root, &name)? else { return Err(format::invalid("missing eviction entry")); };
        if !Self::owned_header(&mut file, &name)? { return Err(format::invalid("eviction ownership changed")); }
        drop(file);
        disk::remove_file(root.join(&name))?;
        if let Some(entry) = self.entries.remove(&name) { self.bytes -= entry.bytes; }
        Ok(true)
    }

    pub(super) fn write(&mut self, root: &Path, key: &[u8], payload: &[u8], replace_invalid: bool, limits: Limits) -> io::Result<WriteOutcome> {
        use std::io::{Seek, SeekFrom};
        fs::check_root(root)?;
        let name = format::filename(key);
        let needed = format::entry_len(key, payload.len())?;
        if let Some(mut file) = self.checked(root, &name)? {
            if !Self::owned_header(&mut file, &name)? { return Ok(WriteOutcome::ProtectedSlot); }
            file.seek(SeekFrom::Start(0))?;
            let header = format::read_header(&mut file)?;
            if header.key != key { return Ok(WriteOutcome::ProtectedSlot); }
            if !replace_invalid {
                file.seek(SeekFrom::Start(0))?;
                let len = file.metadata()?.len();
                match format::read_entry(&mut file, len, key) {
                    Ok(_) if self.bytes <= limits.bytes => return Ok(WriteOutcome::AlreadyPresent),
                    Ok(_) => (),
                    Err(e) if matches!(e.kind(), ErrorKind::InvalidData | ErrorKind::UnexpectedEof) => (),
                    Err(e) => return Err(e),
                }
            }
        }
        // The lease permits only one temporary; old and new bytes both count until publication.
        while self.bytes.checked_add(needed).is_none_or(|n| n > limits.bytes)
            || self.entries.len().checked_add(2).is_none_or(|n| n > limits.entries)
        {
            if !self.evict(root, &name)? { return Ok(WriteOutcome::QuotaBlocked); }
        }
        let (mut file, mut temporary) = temporary(root, key)?;
        format::write_entry_buffered(&mut file, key, payload)?;
        if file.metadata()?.len() != needed { return Err(format::invalid("short cache write")); }
        drop(file);
        // Revalidate the destination, then replace atomically without removing a valid old entry.
        drop(self.checked(root, &name)?);
        disk::rename(&temporary.path, root.join(&name))?;
        temporary.published = true;
        let file = fs::regular_file(&root.join(&name))?;
        let meta = file.metadata()?;
        if meta.len() != needed { return Err(format::invalid("published cache length changed")); }
        if let Some(old) = self.entries.remove(&name) { self.bytes -= old.bytes; }
        self.bytes = self.bytes.checked_add(needed).ok_or_else(|| format::invalid("cache accounting overflow"))?;
        self.entries.insert(name, Entry { bytes: needed, modified: meta.modified()?, owned: true });
        Ok(WriteOutcome::Stored)
    }
}

struct Temporary { path: PathBuf, published: bool }
impl Drop for Temporary {
    fn drop(&mut self) { if !self.published { let _ = disk::remove_file(&self.path); } }
}
fn temporary(root: &Path, key: &[u8]) -> io::Result<(File, Temporary)> {
    for _ in 0..16 {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        let nonce = time ^ NONCE.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!("{}{}.{:x}.{nonce:016x}.tmp", format::PREFIX, format::digest_name(key), std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, Temporary { path, published: false })),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(ErrorKind::AlreadyExists, "cache temporary name collision"))
}
