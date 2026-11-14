//! Checksummed goal file beside a world: the ids of goals already met.
//! A sidecar keeps the save format unchanged; a world without one starts with none met.
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const FILE: &str = "goals.vxg";
const MAGIC: &[u8; 8] = b"VXGOL001";
const MAX_ENTRIES: usize = 255;
const MAX_ID: usize = 64;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("Goal file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated goal file")?;
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
}

/// The whole sidecar for `completed`. Ids the format cannot hold are left out.
pub fn encode(completed: &[String]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let ids: Vec<_> = completed.iter().filter(|id| valid_id(id)).take(MAX_ENTRIES).collect();
    out.push(ids.len() as u8);
    for id in ids {
        out.push(id.len() as u8);
        out.extend_from_slice(id.as_bytes());
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8]) -> Result<Vec<String>> {
    ensure!(bytes.len() >= 17 && bytes.get(..8) == Some(MAGIC.as_slice()), "Unsupported goal file");
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Goal file checksum mismatch");
    let mut r = Reader { bytes: &bytes[..end], at: 8 };
    let count = r.u8()? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let len = r.u8()? as usize;
        let id = std::str::from_utf8(r.take(len)?).context("Goal id is not UTF-8")?;
        ensure!(valid_id(id), "Invalid goal id");
        ensure!(!out.iter().any(|o: &String| o == id), "Duplicate goal id");
        out.push(id.to_owned());
    }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in goal file");
    Ok(out)
}

/// Loads the sidecar into `state`. Missing reads as defaults; a damaged one is reported and ignored.
pub fn load(state: &mut super::State, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok(completed) => state.completed = completed,
        Err(e) => eprintln!("[goals] Ignoring {}: {e:#}", path.display()),
    }
}

/// Replaces the file through a synced temporary so a crash keeps the old one.
pub fn save(dir: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let path = dir.join(FILE);
    let tmp = dir.join(format!("{FILE}.tmp"));
    let mut file = std::fs::File::create(&tmp).with_context(|| format!("Cannot create {}", tmp.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, &path).with_context(|| format!("Cannot publish {}", path.display()))
}
