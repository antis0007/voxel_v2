//! Checksummed statistics file beside a world: counters and unlocked advancement ids.
//! A sidecar keeps the save format unchanged; a world without one starts at zero.
use super::{State, Stat, advancements};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const FILE: &str = "stats.vxs";
const MAGIC: &[u8; 8] = b"VXSTA001";
const MAX_IDS: usize = 64;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Statistics are keyed by their `Stat` code so new ones can be appended.
pub fn encode(state: &State) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(Stat::COUNT as u8);
    for v in state.stats {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let ids: Vec<&str> = advancements::table()
        .iter()
        .enumerate()
        .filter(|&(i, _)| state.is_unlocked(i))
        .map(|(_, a)| a.id.as_str())
        .filter(|id| id.len() <= u8::MAX as usize)
        .take(MAX_IDS)
        .collect();
    out.push(ids.len() as u8);
    for id in ids {
        out.push(id.len() as u8);
        out.extend_from_slice(id.as_bytes());
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// Unknown advancement ids (rows since removed) are skipped; later stat codes are ignored.
pub fn decode(bytes: &[u8]) -> Result<State> {
    ensure!(bytes.len() >= 18 && bytes.get(..8) == Some(MAGIC.as_slice()), "Unsupported statistics file");
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Statistics file checksum mismatch");
    let body = &bytes[8..end];
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8]> {
        let stop = at.checked_add(n).context("Statistics offset overflow")?;
        let out = body.get(at..stop).context("Truncated statistics file")?;
        at = stop;
        Ok(out)
    };
    let mut state = State::default();
    let count = take(1)?[0] as usize;
    for i in 0..count {
        let v = u64::from_le_bytes(take(8)?.try_into()?);
        if let Some(slot) = state.stats.get_mut(i) {
            *slot = v;
        }
    }
    let ids = take(1)?[0] as usize;
    ensure!(ids <= MAX_IDS, "Too many advancement ids");
    for _ in 0..ids {
        let len = take(1)?[0] as usize;
        let id = std::str::from_utf8(take(len)?).context("Advancement id is not UTF-8")?;
        if let Some(i) = advancements::index_of(id).filter(|&i| i < 64) {
            state.unlocked |= 1 << i;
        }
    }
    ensure!(at == body.len(),"Trailing bytes in statistics file");
    Ok(state)
}

/// Loads the sidecar into `world`. Missing reads as zero; a damaged one is reported and ignored.
pub fn load(world: &mut crate::world::World, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok(state) => world.stats = state,
        Err(e) => eprintln!("[stats] Ignoring {}: {e:#}", path.display()),
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
