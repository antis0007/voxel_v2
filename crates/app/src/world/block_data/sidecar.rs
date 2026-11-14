//! Checksummed block data used by checkpoints and network snapshots.
use super::{CHEST_SLOTS, Entry, Key, MAX_ENTRIES, Stand, State};
use crate::world::inventory::{Item, ItemStack};
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;

pub const FILE: &str = "blocks.vxb";
const MAGIC: &[u8; 7] = b"VXBLOCK";
/// Version 2 adds brewing stand records; version 1 files still load.
const VERSION: u8 = 6;
const COORD_LIMIT: i32 = 1 << 28;
/// Dungeon complexes whose keys one save remembers.
const MAX_DUNGEONS: usize = 4096;
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
        ensure!([k.0, k.1, k.2].iter().all(|v| v.unsigned_abs() < COORD_LIMIT as u32), "Block record outside the world");
        Ok(k)
    }
}

fn put_stack(out: &mut Vec<u8>, slot: Option<ItemStack>) {
    let (kind, payload, count) = slot.map_or((0, 0, 0), |s| {
        let (k, p) = s.item.encode_u32();
        (k, p, s.count)
    });
    for v in [kind, payload, count] {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn read_stack(r: &mut Reader) -> Result<Option<ItemStack>> {
    let (kind, payload, count) = (r.u32()?, r.u32()?, r.u32()?);
    if kind == 0 {
        ensure!(payload == 0 && count == 0, "Invalid empty slot");
        return Ok(None);
    }
    ensure!((1..=MAX_STACK).contains(&count), "Invalid stack");
    Ok(Some(ItemStack { item: Item::decode_u32(kind, payload)?, count }))
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
                out.push(slots.len() as u8);
                for slot in slots.iter() {
                    put_stack(&mut out, *slot);
                }
            }
            Entry::Stand(stand) => {
                out.push(3);
                for slot in [stand.base, stand.modifier, stand.output] {
                    put_stack(&mut out, slot);
                }
                out.extend_from_slice(&stand.steps.to_le_bytes());
            }
        }
    }
    out.extend_from_slice(&(state.fuses.len() as u32).to_le_bytes());
    for (&k, &steps) in &state.fuses {
        put_key(&mut out, k);
        out.extend_from_slice(&steps.to_le_bytes());
    }
    let policies = ron::to_string(&state.storage_policies).expect("storage policy serialization");
    out.extend_from_slice(&(policies.len() as u32).to_le_bytes());
    out.extend_from_slice(policies.as_bytes());
    out.extend_from_slice(&(state.placed_sediment.len() as u32).to_le_bytes());
    for &cell in &state.placed_sediment {
        put_key(&mut out, cell);
    }
    out.extend_from_slice(&(state.dungeon_keys.len().min(MAX_DUNGEONS) as u32).to_le_bytes());
    for (&(x, z), ring) in state.dungeon_keys.iter().take(MAX_DUNGEONS) {
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
        out.push(ring.small);
        out.push(u8::from(ring.great));
        out.extend_from_slice(&ring.looted.to_le_bytes());
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8]) -> Result<State> {
    ensure!(bytes.len() >= 24 && bytes.get(..7) == Some(MAGIC.as_slice()), "Unsupported block file");
    ensure!((1..=VERSION).contains(&bytes[7]), "Unsupported block file version {}", bytes[7]);
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
                let count = if bytes[7] >= 4 { r.u8()? as usize } else { CHEST_SLOTS };
                ensure!(matches!(count, 27 | 36 | 45 | 54), "Invalid chest capacity");
                let mut slots = Box::new(vec![None; count]);
                for slot in slots.iter_mut() {
                    *slot = read_stack(&mut r)?;
                }
                Entry::Chest(slots)
            }
            3 if bytes[7] >= 2 => {
                let (base, modifier, output) = (read_stack(&mut r)?, read_stack(&mut r)?, read_stack(&mut r)?);
                state.stands.insert(k);
                Entry::Stand(Stand { base, modifier, output, steps: r.u16()? })
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
    if bytes[7] >= 3 {
        let len = r.u32()? as usize;
        ensure!(len <= 16 * 1024 * 1024, "Storage policies too large");
        state.storage_policies = ron::de::from_bytes(r.take(len)?)?;
        ensure!(state.storage_policies.len() <= MAX_ENTRIES, "Too many storage policies");
    }
    if bytes[7] >= 5 {
        let count = r.u32()? as usize;
        ensure!(count <= super::MAX_PLACED_SEDIMENT, "Too many placed sediment cells");
        for _ in 0..count {
            ensure!(state.placed_sediment.insert(r.key()?), "Duplicate sediment cell");
        }
    }
    if bytes[7] >= 6 {
        let count = r.u32()? as usize;
        ensure!(count <= MAX_DUNGEONS, "Too many dungeon key rings");
        for _ in 0..count {
            let x = r.u32()? as i32;
            let z = r.u32()? as i32;
            let small = r.u8()?;
            let great = r.u8()? != 0;
            let looted = u64::from(r.u32()?) | (u64::from(r.u32()?) << 32);
            let ring = super::KeyRing { small, great, looted };
            ensure!(state.dungeon_keys.insert((x, z), ring).is_none(), "Duplicate dungeon key ring");
        }
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
