//! Checksummed block-data file beside a world. A world without one has no block records.
use super::{CHEST_SLOTS, Entry, Key, MAX_ENTRIES, State};
use crate::world::inventory::{Item, ItemStack};
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;

pub const FILE: &str = "blocks.vxb";
const MAGIC: &[u8; 7] = b"VXBLOCK";
const VERSION: u8 = 1;
const COORD_LIMIT: i32 = 1 << 28;
const MAX_LINES: usize = 16;
const MAX_LINE_BYTES: usize = 256;
const MAX_STACK: u32 = 1 << 16;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("Block file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated block file")?;
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn key(&mut self) -> Result<Key> {
        let k = (self.u32()? as i32, self.u32()? as i32, self.u32()? as i32);
        ensure!([k.0, k.1, k.2].iter().all(|v| v.abs() < COORD_LIMIT), "Block record outside the world");
        Ok(k)
    }
}

fn put_key(out: &mut Vec<u8>, k: Key) {
    for v in [k.0, k.1, k.2] {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

pub fn encode(state: &State) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(VERSION);
    let entries: Vec<_> = state.entries.iter().take(MAX_ENTRIES).collect();
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (&k, entry) in entries {
        put_key(&mut out, k);
        match entry {
            Entry::Text(lines) => {
                out.push(1);
                let lines: Vec<_> = lines.iter().take(MAX_LINES).collect();
                out.push(lines.len() as u8);
                for line in lines {
                    let mut end = line.len().min(MAX_LINE_BYTES);
                    while !line.is_char_boundary(end) {
                        end -= 1;
                    }
                    out.extend_from_slice(&(end as u16).to_le_bytes());
                    out.extend_from_slice(&line.as_bytes()[..end]);
                }
            }
            Entry::Chest(slots) => {
                out.push(2);
                for slot in slots.iter() {
                    let (kind, payload, count) = slot.map_or((0, 0, 0), |s| {
                        let (k, p) = s.item.encode_u32();
                        (k, p, s.count)
                    });
                    for v in [kind, payload, count] {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
    }
    out.extend_from_slice(&(state.fuses.len() as u32).to_le_bytes());
    for (&k, &steps) in &state.fuses {
        put_key(&mut out, k);
        out.extend_from_slice(&steps.to_le_bytes());
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8]) -> Result<State> {
    ensure!(bytes.len() >= 24 && bytes.get(..7) == Some(MAGIC.as_slice()), "Unsupported block file");
    ensure!(bytes[7] == VERSION, "Unsupported block file version {}", bytes[7]);
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Block file checksum mismatch");
    let mut r = Reader { bytes: &bytes[..end], at: 8 };
    let mut state = State::default();
    let count = r.u32()? as usize;
    ensure!(count <= MAX_ENTRIES, "Too many block records");
    for _ in 0..count {
        let k = r.key()?;
        let entry = match r.u8()? {
            1 => {
                let n = r.u8()? as usize;
                ensure!(n <= MAX_LINES, "Too many lines");
                let mut lines = Vec::with_capacity(n);
                for _ in 0..n {
                    let len = r.u16()? as usize;
                    ensure!(len <= MAX_LINE_BYTES, "Line too long");
                    lines.push(String::from_utf8(r.take(len)?.to_vec()).context("Line is not UTF-8")?);
                }
                Entry::Text(lines)
            }
            2 => {
                let mut slots = Box::new([None; CHEST_SLOTS]);
                for slot in slots.iter_mut() {
                    let (kind, payload, count) = (r.u32()?, r.u32()?, r.u32()?);
                    if kind == 0 {
                        ensure!(payload == 0 && count == 0, "Invalid empty chest slot");
                        continue;
                    }
                    ensure!((1..=MAX_STACK).contains(&count), "Invalid chest stack");
                    *slot = Some(ItemStack::new(Item::decode_u32(kind, payload)?, count));
                }
                Entry::Chest(slots)
            }
            tag => bail!("Unknown block record {tag}"),
        };
        ensure!(state.entries.insert(k, entry).is_none(), "Duplicate block record");
    }
    let fuses = r.u32()? as usize;
    ensure!(fuses <= super::tnt::MAX_FUSES, "Too many fuses");
    for _ in 0..fuses {
        let k = r.key()?;
        state.fuses.insert(k, r.u16()?);
    }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in block file");
    Ok(state)
}

/// Loads the sidecar into `state`. Missing reads as empty; a damaged one is reported and ignored.
pub fn load(state: &mut State, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok(loaded) => *state = loaded,
        Err(e) => eprintln!("[blocks] Ignoring {}: {e:#}", path.display()),
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
