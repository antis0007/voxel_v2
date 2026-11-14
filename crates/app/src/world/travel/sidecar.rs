//! Checksummed travel file beside a world. A world without one starts with full stock and no boats.
use super::{Boat, MAX_BOATS, MAX_GUARDS, State};
use anyhow::{Context, Result, ensure};
use glam::Vec3;
use std::path::Path;

pub const FILE: &str = "travel.vxt";
const MAGIC: &[u8; 7] = b"VXTRAVL";
const VERSION: u8 = 6;
const MAX_ROWS: usize = 4096;

fn resident_id(id: u32) -> bool {
    // Authored settlement scenes reserve IDs 0..64, independently of active crew capacity.
    id < 64 || crate::world::farming::jobs::crew::is_constructor(id)
        || engine_worldgen::households::role(id).is_some()
}

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).context("Travel file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated travel file")?;
        self.at = end;
        Ok(out.try_into()?)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

/// Boat records are mirrored from their live actors every step, so they are current here.
pub fn encode(state: &State) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(VERSION);
    out.extend_from_slice(&state.day.to_le_bytes());
    let used: Vec<_> = state.used.iter().take(MAX_ROWS).collect();
    out.extend_from_slice(&(used.len() as u32).to_le_bytes());
    for (&(villager, row), &n) in used {
        out.extend_from_slice(&villager.to_le_bytes());
        out.extend_from_slice(&row.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
    }
    let away: Vec<_> = state.away.iter().take(MAX_ROWS).collect();
    out.extend_from_slice(&(away.len() as u32).to_le_bytes());
    for (&villager, &day) in away {
        out.extend_from_slice(&villager.to_le_bytes());
        out.extend_from_slice(&day.to_le_bytes());
    }
    out.extend_from_slice(&state.next_boat.to_le_bytes());
    let boats: Vec<_> = state.boats.iter().take(MAX_BOATS).collect();
    out.extend_from_slice(&(boats.len() as u32).to_le_bytes());
    for (&id, b) in boats {
        out.extend_from_slice(&id.to_le_bytes());
        for v in b.pos.to_array().into_iter().chain([b.yaw]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&(state.homes.len() as u32).to_le_bytes());
    for (&id,p) in &state.homes {
        out.extend_from_slice(&id.to_le_bytes());
        for v in p.to_array(){out.extend_from_slice(&v.to_le_bytes());}
    }
    out.extend_from_slice(&(state.guard_posts.len() as u32).to_le_bytes());
    for (&id, post) in &state.guard_posts {
        out.extend_from_slice(&id.to_le_bytes());
        for value in post.to_array() {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    let settlements = ron::to_string(&state.settlements).expect("Settlement ledgers contain plain data");
    out.extend_from_slice(&(settlements.len() as u32).to_le_bytes());
    out.extend_from_slice(settlements.as_bytes());
    let politics = ron::to_string(&state.politics).expect("Politics contains plain data");
    out.extend_from_slice(&(politics.len() as u32).to_le_bytes());
    out.extend_from_slice(politics.as_bytes());
    out.extend_from_slice(&state.civilization_scan.to_le_bytes());
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8]) -> Result<State> {
    ensure!(bytes.len() >= 36 && bytes.get(..7) == Some(MAGIC.as_slice()), "Unsupported travel file");
    ensure!((1..=VERSION).contains(&bytes[7]), "Unsupported travel file version {}", bytes[7]);
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Travel file checksum mismatch");
    let mut r = Reader { bytes: &bytes[..end], at: 8 };
    let mut state = State { day: r.u64()?, ..State::default() };
    let used = r.u32()? as usize;
    ensure!(used <= MAX_ROWS, "Too many trade records");
    for _ in 0..used {
        let key = (r.u32()?, r.u16()?);
        ensure!(state.used.insert(key, r.u16()?).is_none(), "Duplicate trade record");
    }
    let away = r.u32()? as usize;
    ensure!(away <= MAX_ROWS, "Too many villager records");
    for _ in 0..away {
        let villager = r.u32()?;
        ensure!(state.away.insert(villager, r.u64()?).is_none(), "Duplicate villager record");
    }
    state.next_boat = r.u32()?;
    let boats = r.u32()? as usize;
    ensure!(boats <= MAX_BOATS && state.next_boat < 1 << 24, "Too many boats");
    for _ in 0..boats {
        let id = r.u32()?;
        let pos = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
        let yaw = r.f32()?;
        ensure!(id < 1 << 24 && pos.is_finite() && pos.abs().max_element() < 50_000_000.0 && yaw.is_finite(), "Invalid boat");
        ensure!(state.boats.insert(id, Boat { pos, yaw }).is_none(), "Duplicate boat");
    }
    if bytes[7]>=2 {
        let n=r.u32()? as usize;
        ensure!(n<=64,"Too many resident homes");
        for _ in 0..n {
            let id=r.u32()?;
            let p=Vec3::new(r.f32()?,r.f32()?,r.f32()?);
            ensure!(resident_id(id) && p.is_finite()
                && p.abs().max_element()<50_000_000.0,"Invalid resident home");
            ensure!(state.homes.insert(id,p).is_none(),"Duplicate resident home");
        }
    }
    if bytes[7] >= 3 {
        let count = r.u32()? as usize;
        ensure!(count <= MAX_GUARDS, "Too many guard posts");
        for _ in 0..count {
            let id = r.u32()?;
            let post = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            ensure!(resident_id(id)
                && post.is_finite() && post.abs().max_element() < 50_000_000.0, "Invalid guard post");
            ensure!(state.guard_posts.insert(id, post).is_none(), "Duplicate guard post");
        }
    }
    if bytes[7] >= 4 {
        let length = r.u32()? as usize;
        let end = r.at.checked_add(length).context("Settlement ledger offset overflow")?;
        ensure!(length <= 2_000_000, "Settlement ledgers are too large");
        state.settlements = ron::de::from_bytes(r.bytes.get(r.at..end).context("Truncated settlement ledgers")?)?;
        ensure!(state.settlements.len() <= super::civilization::MAX_SETTLEMENTS
            && state.settlements.iter().all(|(&id, ledger)| {
                engine_worldgen::civilization::original_settlement(id) == Some(id) && ledger.valid()
            }) && super::civilization::valid_ownership(&state.settlements), "Invalid settlement ledgers");
        r.at = end;
    }
    if bytes[7] >= 5 {
        let length = r.u32()? as usize;
        let end = r.at.checked_add(length).context("Politics offset overflow")?;
        ensure!(length <= 128_000, "Politics record is too large");
        state.politics = ron::de::from_bytes(r.bytes.get(r.at..end).context("Truncated politics record")?)?;
        ensure!(state.politics.valid(&state.settlements), "Invalid politics record");
        r.at = end;
    }
    if bytes[7] >= 6 { state.civilization_scan=r.u32()?; }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in travel file");
    Ok(state)
}

#[cfg(test)]
mod resident_tests;

/// Loads the sidecar into `state`. Missing reads as defaults; a damaged one is reported and ignored.
pub fn load(state: &mut State, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok(loaded) => *state = loaded,
        Err(e) => eprintln!("[travel] Ignoring {}: {e:#}", path.display()),
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
