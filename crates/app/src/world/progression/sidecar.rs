//! Checksummed progression file beside a world: XP, saturation, effects and item wear.
//! A sidecar keeps the save format unchanged; a world without one starts at the defaults.
use super::effects::{Active, Kind, MAX_LEVEL, MAX_SECONDS};
use super::wear::{Entry, Slot};
use super::State;
use crate::world::inventory::Item;
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const FILE: &str = "progression.vxp";
const MAGIC: &[u8; 8] = b"VXPRG004";
const MAX_ENTRIES: usize = 64;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &v| (h ^ v as u64).wrapping_mul(0x0100_0000_01b3))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("Progression file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated progression file")?;
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
}

/// The whole sidecar for `state`. Wear entries whose slot no longer holds their item are left out.
pub fn encode(state: &State, inv: &crate::world::inventory::SurvivalInventory) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&state.xp.to_le_bytes());
    out.extend_from_slice(&state.lore.to_le_bytes());
    out.extend_from_slice(&[state.lessons, u8::from(state.horticulture)]);
    out.extend_from_slice(&state.saturation.to_bits().to_le_bytes());
    let effects: Vec<_> = state.effects.list.iter().take(Kind::ALL.len()).collect();
    out.push(effects.len() as u8);
    for a in effects {
        out.extend_from_slice(&[a.kind as u8, a.level]);
        out.extend_from_slice(&a.seconds.to_bits().to_le_bytes());
    }
    let wear: Vec<_> = state
        .wear
        .entries
        .iter()
        .filter(|e| inv_item(inv, e.slot) == Some(e.item))
        .take(MAX_ENTRIES)
        .collect();
    out.push(wear.len() as u8);
    for e in wear {
        let (kind, payload) = e.item.encode_u32();
        out.push(e.slot.code());
        for v in [kind, payload, e.used] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let sum = hash(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

fn inv_item(inv: &crate::world::inventory::SurvivalInventory, slot: Slot) -> Option<Item> {
    match slot {
        Slot::Hotbar(i) => inv.hotbar.get(i as usize).copied().flatten(),
        Slot::Pack(i) => inv.pack.get(i as usize).copied().flatten(),
        Slot::Equipment(i) => inv.equipment.get(i as usize).copied().flatten(),
        Slot::Offhand => inv.offhand,
        Slot::Cursor => inv.cursor,
        Slot::CraftGrid(i) => inv.craft_grid.get(i as usize).copied().flatten(),
    }
    .map(|s| s.item)
}

pub fn decode(bytes: &[u8]) -> Result<State> {
    let current = bytes.get(..8) == Some(MAGIC.as_slice());
    let ranked_study = bytes.get(..8) == Some(b"VXPRG003");
    let has_lore = current || ranked_study || bytes.get(..8) == Some(b"VXPRG002");
    ensure!(bytes.len() >= 26 && (has_lore || bytes.get(..8) == Some(b"VXPRG001")), "Unsupported progression file");
    let end = bytes.len() - 8;
    ensure!(hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?), "Progression file checksum mismatch");
    let mut r = Reader { bytes: &bytes[..end], at: 8 };
    let mut state = State { xp: r.u32()?, ..State::default() };
    if has_lore { state.lore = r.u32()? & (super::super::adventure::chronicle::MASK | super::thresholds::Milestone::MASK); }
    if current || ranked_study {
        let study = r.u8()?;
        state.lessons = if ranked_study {
            ensure!(study <= super::study::LEGACY_MAX_AFFINITY, "Invalid magic affinity");
            (1 << study) - 1
        } else {
            ensure!(study & !super::study::LESSON_MASK == 0, "Invalid study lessons");
            study
        };
        let horticulture = r.u8()?;
        ensure!(horticulture <= 1, "Invalid horticulture study");
        state.horticulture = horticulture == 1;
    }
    let saturation = r.f32()?;
    ensure!(saturation.is_finite() && (0.0..=100.0).contains(&saturation), "Invalid saturation");
    state.saturation = saturation;
    let count = r.u8()? as usize;
    ensure!(count <= Kind::ALL.len(), "Too many effects");
    for _ in 0..count {
        let kind = Kind::from_u8(r.u8()?).context("Unknown effect")?;
        let level = r.u8()?;
        let seconds = r.f32()?;
        ensure!((1..=MAX_LEVEL).contains(&level), "Invalid effect level");
        ensure!(seconds.is_finite() && seconds > 0.0 && seconds <= MAX_SECONDS, "Invalid effect time");
        ensure!(state.effects.level(kind) == 0, "Duplicate effect");
        state.effects.list.push(Active { kind, seconds, level, total: seconds });
    }
    let count = r.u8()? as usize;
    ensure!(count <= MAX_ENTRIES, "Too many wear entries");
    for _ in 0..count {
        let slot = Slot::from_code(r.u8()?).context("Unknown wear slot")?;
        let item = Item::decode_u32(r.u32()?, r.u32()?)?;
        let used = r.u32()?;
        ensure!(!state.wear.entries.iter().any(|e| e.slot == slot), "Duplicate wear slot");
        state.wear.entries.push(Entry { slot, item, used });
    }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in progression file");
    Ok(state)
}

/// Loads the sidecar into `world`. Missing reads as defaults; a damaged one is reported and ignored.
pub fn load(world: &mut crate::world::World, dir: &Path) {
    let path = dir.join(FILE);
    let Ok(bytes) = std::fs::read(&path) else { return };
    match decode(&bytes) {
        Ok(state) => {
            world.progression = state;
            super::study::restore_spells(world);
        }
        Err(e) => eprintln!("[progression] Ignoring {}: {e:#}", path.display()),
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
