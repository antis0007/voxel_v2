//! Checksummed farming file beside a world: clock, plots, the tended herd and paid wild cells.
//! A sidecar keeps the save format unchanged; a world without one has no plots.
use super::{Key, MAX_PLOTS, MAX_WILD_PAID, Plot, State, table};
use crate::world::World;
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const FILE: &str = "farming.vxf";
const MAGIC: &[u8; 8] = b"VXFRM001";
const COORD_LIMIT: i32 = 1 << 28;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("Farming file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated farming file")?;
        self.at = end;
        Ok(out)
    }
    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }
    pub(crate) fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
}

/// The whole sidecar for `world` as it stands now.
pub fn encode(world: &World) -> Vec<u8> {
    let f = &world.farming;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&f.clock.to_le_bytes());
    let plots: Vec<_> = f.plots.iter().take(MAX_PLOTS).collect();
    out.extend_from_slice(&(plots.len() as u32).to_le_bytes());
    for (k, plot) in plots {
        for v in [k.0, k.1, k.2] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&[plot.crop, plot.stage]);
        out.extend_from_slice(&plot.planted.to_le_bytes());
        out.extend_from_slice(&plot.rolled.to_le_bytes());
    }
    let herd = herd_bytes(world);
    out.extend_from_slice(&(herd.len() as u32).to_le_bytes());
    out.extend_from_slice(&herd);
    let paid: Vec<_> = f.wild_paid.iter().take(MAX_WILD_PAID).collect();
    out.extend_from_slice(&(paid.len() as u32).to_le_bytes());
    for k in paid {
        for v in [k.0, k.1, k.2] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// Plots and clock, plus the herd section as raw bytes.
pub fn decode(bytes: &[u8]) -> Result<(State, &[u8])> {
    ensure!(bytes.len() >= 24 && bytes.get(..8) == Some(MAGIC.as_slice()), "Unsupported farming file");
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Farming file checksum mismatch");
    let mut r = Reader { bytes: &bytes[..end], at: 8 };
    let mut state = State { clock: r.u64()?, ..State::default() };
    let count = r.u32()? as usize;
    ensure!(count <= MAX_PLOTS, "Too many plots");
    for _ in 0..count {
        let k: Key = (r.u32()? as i32, r.u32()? as i32, r.u32()? as i32);
        ensure!([k.0, k.1, k.2].iter().all(|v| v.abs() < COORD_LIMIT), "Plot outside the world");
        let (crop, stage) = (r.u8()?, r.u8()?);
        let plot = Plot { crop, stage, planted: r.u64()?, rolled: r.u32()? };
        let row = table::get(crop).context("Unknown crop")?;
        ensure!(stage <= row.mature() && plot.planted <= state.clock, "Invalid plot");
        ensure!(state.plots.insert(k, plot).is_none(), "Duplicate plot");
    }
    let len = r.u32()? as usize;
    let herd = r.take(len)?;
    // Files written before paid wild cells were recorded end here.
    if r.at < r.bytes.len() {
        let count = r.u32()? as usize;
        ensure!(count <= MAX_WILD_PAID, "Too many paid wild cells");
        for _ in 0..count {
            let k: Key = (r.u32()? as i32, r.u32()? as i32, r.u32()? as i32);
            ensure!([k.0, k.1, k.2].iter().all(|v| v.abs() < COORD_LIMIT), "Wild cell outside the world");
            state.wild_paid.insert(k);
        }
    }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in farming file");
    Ok((state, herd))
}

/// Loads the sidecar into `world`. Missing reads as empty; a damaged one is reported and ignored.
pub fn load(world: &mut World, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok((state, herd)) => {
            let clock = state.clock;
            world.farming = state;
            if let Err(e) = apply_herd(world, herd, clock) {
                eprintln!("[farming] Ignoring the herd in {}: {e:#}", path.display());
            }
        }
        Err(e) => eprintln!("[farming] Ignoring {}: {e:#}", path.display()),
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

pub(super) fn herd_bytes(world: &World) -> Vec<u8> {
    let mut out = Vec::new();
    world.adventure.wildlife.herd_encode(&world.adventure.enemies, &mut out);
    out
}

pub(crate) fn apply_herd(world: &mut World, bytes: &[u8], clock: u64) -> Result<()> {
    let mut r = Reader { bytes, at: 0 };
    let herd = crate::world::adventure::wildlife::Herd::decode(&mut r, clock)?;
    ensure!(r.at == bytes.len(), "Trailing bytes in herd section");
    world.adventure.wildlife.herd = herd;
    Ok(())
}
