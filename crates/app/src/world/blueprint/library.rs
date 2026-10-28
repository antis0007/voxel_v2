//! Named blueprint files in the `blueprints` folder of the save directory.
use super::Blueprint;
use super::format::{self, Decoded, HEADER, MAX_FILE_BYTES};
use anyhow::{Context, Result, bail, ensure};
use glam::IVec3;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FOLDER: &str = "blueprints";
pub const EXTENSION: &str = "vxbp";
pub const MAX_NAME: usize = 40;
/// Files listed at most; a folder filled by hand stays cheap to draw.
const MAX_ENTRIES: usize = 512;

pub fn directory() -> PathBuf {
    crate::world::storage::save_directory().join(FOLDER)
}

/// A file-safe name: letters, digits, spaces, `-` and `_`, trimmed and short.
/// Windows device names are refused because they cannot be file names there.
pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_');
    if name.is_empty() || name.chars().count() > MAX_NAME || !name.chars().all(allowed) {
        return None;
    }
    let upper = name.to_ascii_uppercase();
    let device = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit());
    (!device).then(|| name.to_owned())
}

fn path(dir: &Path, name: &str) -> Result<PathBuf> {
    let Some(name) = clean_name(name) else { bail!("Name Invalid") };
    Ok(dir.join(format!("{name}.{EXTENSION}")))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// `None` when the header is unreadable.
    pub size: Option<IVec3>,
}

/// Blueprint files by name; a missing folder is an empty list.
pub fn list(dir: &Path) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).context("Blueprint Folder Unreadable"),
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
            continue;
        }
        let Some(name) = p.file_stem().and_then(|s| s.to_str()).and_then(clean_name) else { continue };
        let mut header = [0u8; HEADER];
        let size = fs::File::open(&p).and_then(|mut f| f.read_exact(&mut header)).ok().and_then(|()| format::peek_size(&header));
        out.push(Entry { name, size });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.sort_by_key(|e| e.name.to_lowercase());
    Ok(out)
}

/// Writes through a temporary file so a failed save never leaves half a file.
pub fn save(dir: &Path, name: &str, bp: &Blueprint) -> Result<()> {
    let target = path(dir, name)?;
    fs::create_dir_all(dir).context("Blueprint Folder Unwritable")?;
    let temp = target.with_extension(format!("{EXTENSION}.{}.tmp", std::process::id()));
    fs::write(&temp, format::encode(bp)).context("Blueprint Not Saved")?;
    fs::rename(&temp, &target).context("Blueprint Not Saved")?;
    Ok(())
}

pub fn load(dir: &Path, name: &str) -> Result<Decoded> {
    let mut bytes = Vec::new();
    fs::File::open(path(dir, name)?)
        .and_then(|f| f.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes))
        .context("Blueprint Unreadable")?;
    format::decode(&bytes)
}

pub fn rename(dir: &Path, from: &str, to: &str) -> Result<()> {
    let (old, new) = (path(dir, from)?, path(dir, to)?);
    // A case-only rename is the same file on Windows; anything else must be free.
    ensure!(from.eq_ignore_ascii_case(to) || !new.exists(), "Name Taken");
    fs::rename(old, new).context("Blueprint Not Renamed")
}

pub fn delete(dir: &Path, name: &str) -> Result<()> {
    fs::remove_file(path(dir, name)?).context("Blueprint Not Deleted")
}
