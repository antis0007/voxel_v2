//! Checksummed farming file beside a world: clock, plots, the tended herd and paid wild cells.
//! The same checksummed payload is embedded in world checkpoints from version 71.
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
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.at
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
    if f.nursery_stock != 0 {
        out.extend_from_slice(b"NUR1");
        out.extend_from_slice(&f.nursery_stock.to_le_bytes());
    }
    if f.freight.market != 0 || f.freight.cargo != 0 || f.freight.last_crossing.is_some() {
        out.extend_from_slice(b"FRT2");
        out.extend_from_slice(&f.freight.market.to_le_bytes());
        out.extend_from_slice(&f.freight.cargo.to_le_bytes());
        out.push(f.freight.progress);
        out.push(f.freight.last_crossing.map_or(0, |route| route as u8));
        out.push(f.freight.cargo_lane.map_or(0, |route| route as u8));
    }
    if let Some(pantry) = &f.pantry {
        out.extend_from_slice(b"PNT1");
        for stock in [pantry.meals, pantry.biomass, pantry.purse] {
            out.extend_from_slice(&stock.to_le_bytes());
        }
        out.extend_from_slice(&pantry.day.to_le_bytes());
        out.extend_from_slice(&[pantry.demand, pantry.served]);
    }
    if let Some(freight) = &f.planet_freight {
        out.extend_from_slice(b"PFR2");
        for stock in [freight.stock[0], freight.stock[1], freight.market, freight.cargo] {
            out.extend_from_slice(&stock.to_le_bytes());
        }
        out.extend_from_slice(&[freight.progress, freight.holding, freight.lane]);
        for damage in freight.wounds {
            out.extend_from_slice(&damage.to_le_bytes());
        }
        out.extend_from_slice(&freight.pause_until.to_le_bytes());
    }
    if f.jobs.used() {
        let text = ron::to_string(&f.jobs.checkpoint(&world.adventure.enemies, f.clock)).expect("serializable settler jobs");
        let bytes = text.as_bytes();
        out.extend_from_slice(b"JOB1");
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    if let Some(bounds) = f.base.bounds {
        let text = ron::to_string(&bounds).expect("serializable base bounds");
        out.extend_from_slice(b"BAS1");
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }
    if f.base.stakes.used() {
        let text = ron::to_string(&f.base.stakes).expect("serializable stake claims");
        out.extend_from_slice(b"STK1");
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }
    if f.communities.used() {
        let text = ron::to_string(&f.communities.checkpoint(&world.adventure.enemies, f.clock))
            .expect("serializable settlement communities");
        use std::io::Write;
        let mut encoder=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::fast());
        encoder.write_all(text.as_bytes()).expect("memory community encoder");
        let packed=encoder.finish().expect("memory community encoder");
        out.extend_from_slice(b"COM2");
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&packed);
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
        ensure!([k.0, k.1, k.2].iter().all(|v| v.unsigned_abs() < COORD_LIMIT as u32), "Plot outside the world");
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
            ensure!([k.0, k.1, k.2].iter().all(|v| v.unsigned_abs() < COORD_LIMIT as u32), "Wild cell outside the world");
            state.wild_paid.insert(k);
        }
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"NUR1") {
        r.take(4)?;
        state.nursery_stock = r.u32()?;
        ensure!(state.nursery_stock <= super::watershed::CAPACITY, "Invalid nursery stock");
    }
    let tag = r.bytes.get(r.at..r.at + 4);
    if tag == Some(b"FRT1") || tag == Some(b"FRT2") {
        use super::freight::{LOAD, MARKET_CAPACITY, Route, TRIP_STEPS};
        let format = r.take(4)?;
        let f = &mut state.freight;
        f.market = r.u32()?;
        f.cargo = r.u32()?;
        f.progress = r.u8()?;
        if format == b"FRT2" {
            f.last_crossing = match r.u8()? {
                0 => None,
                1 => Some(Route::North),
                2 => Some(Route::South),
                3 => Some(Route::Blocked),
                _ => anyhow::bail!("Invalid bridge report"),
            };
            f.cargo_lane = match r.u8()? {
                0 => None,
                1 if f.cargo != 0 => Some(Route::North),
                2 if f.cargo != 0 => Some(Route::South),
                _ => anyhow::bail!("Invalid cargo lane"),
            };
        }
        ensure!(
            f.cargo <= LOAD && f.market <= MARKET_CAPACITY - f.cargo
                && f.progress < TRIP_STEPS && (f.cargo != 0 || f.progress == 0),
            "Invalid Fernwake freight"
        );
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"PNT1") {
        use super::pantry::{BIOMASS_CAPACITY, FOOD_CAPACITY, PURSE_CAPACITY, RESIDENTS, State};
        ensure!(r.take(4)? == b"PNT1", "Unknown farming extension");
        let pantry = State {
            meals: r.u32()?,
            biomass: r.u32()?,
            purse: r.u32()?,
            day: r.u64()?,
            demand: r.u8()?,
            served: r.u8()?,
        };
        ensure!(
            pantry.meals <= FOOD_CAPACITY && pantry.biomass <= BIOMASS_CAPACITY
                && pantry.purse <= PURSE_CAPACITY
                && pantry.served <= pantry.demand && pantry.demand <= RESIDENTS
                && pantry.day <= state.clock / crate::world::travel::DAY_TICKS,
            "Invalid Fernwake pantry"
        );
        state.pantry = Some(pantry);
    }
    let crew_saved = r.bytes.get(r.at..r.at + 4) == Some(b"PFR2");
    if crew_saved || r.bytes.get(r.at..r.at + 4) == Some(b"PFR1") {
        r.take(4)?;
        let mut freight = super::planet_freight::State::default();
        freight.stock = [r.u32()?, r.u32()?];
        freight.market = r.u32()?;
        freight.cargo = r.u32()?;
        freight.progress = r.u8()?;
        freight.holding = r.u8()?;
        freight.lane = r.u8()?;
        if crew_saved {
            freight.wounds = [r.f32()?, r.f32()?];
            freight.pause_until = r.u64()?;
            ensure!(freight.pause_until <= state.clock.saturating_add(super::planet_freight::DRIVER_PAUSE), "Invalid freight interruption");
        }
        ensure!(super::planet_freight::valid(&freight), "Invalid Planet freight");
        state.planet_freight = Some(freight);
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"JOB1") {
        r.take(4)?;
        let len = r.u32()? as usize;
        ensure!(len <= 256*1024, "Oversized settler jobs");
        state.jobs = ron::de::from_bytes(r.take(len)?)?;
        ensure!(state.jobs.valid(), "Invalid settler jobs");
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"BAS1") {
        r.take(4)?;
        let len = r.u32()? as usize;
        ensure!(len <= 256, "Oversized base bounds");
        let bounds: super::base::Bounds = ron::de::from_bytes(r.take(len)?)?;
        ensure!(bounds.valid(), "Invalid base bounds");
        state.base.bounds = Some(bounds);
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"STK1") {
        r.take(4)?;
        let len = r.u32()? as usize;
        ensure!(len <= 4096, "Oversized stake claims");
        state.base.stakes = ron::de::from_bytes(r.take(len)?)?;
        ensure!(state.base.stakes.valid(), "Invalid stake claims");
    }
    if r.bytes.get(r.at..r.at + 4) == Some(b"COM2") {
        use std::io::Read;
        r.take(4)?;
        let raw_len=r.u32()? as usize;
        let packed_len=r.u32()? as usize;
        ensure!(raw_len<=256*1024*1024 && packed_len<=32*1024*1024,"Oversized settlement communities");
        let mut raw=Vec::new();
        flate2::read::ZlibDecoder::new(r.take(packed_len)?).take(raw_len as u64+1).read_to_end(&mut raw)?;
        ensure!(raw.len()==raw_len,"Invalid community expansion length");
        state.communities=ron::de::from_bytes(&raw)?;
        ensure!(state.communities.valid(&state.jobs,state.clock),"Invalid settlement communities");
    } else if r.bytes.get(r.at..r.at + 4) == Some(b"COM1") {
        r.take(4)?;
        let len = r.u32()? as usize;
        ensure!(len <= 32 * 1024 * 1024, "Oversized settlement communities");
        state.communities = ron::de::from_bytes(r.take(len)?)?;
        ensure!(state.communities.valid(&state.jobs, state.clock), "Invalid settlement communities");
    }
    ensure!(r.at == r.bytes.len(), "Trailing bytes in farming file");
    super::jobs::crew::migrate_legacy_commitments(&mut state.jobs, state.clock);
    state.communities.migrate_legacy_commitments(state.clock);
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
