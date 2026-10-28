//! Interval-indexed growth rolls. Each roll is a pure function of seed, cell and
//! interval index, so evaluating late (after an unload) equals evaluating every tick.
use super::table::Crop;
use glam::IVec3;

/// Growth inputs read at the crop cell when a roll is evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conditions {
    pub light: u8,
    pub wet: bool,
    pub soil: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plot {
    /// Row index into `table::CROPS`.
    pub crop: u8,
    pub planted: u64,
    /// Roll intervals already evaluated since planting.
    pub rolled: u32,
    pub stage: u8,
}

impl Plot {
    pub fn new(crop: u8, planted: u64) -> Self {
        Self { crop, planted, rolled: 0, stage: 0 }
    }
    /// Tick at which the next unevaluated interval ends.
    pub fn next_roll(&self, crop: &Crop) -> u64 {
        self.planted + (u64::from(self.rolled) + 1) * u64::from(crop.roll_ticks)
    }
}

pub fn chance(crop: &Crop, c: Conditions) -> u32 {
    if !c.soil || c.light < crop.light_min || c.light > crop.light_max {
        0
    } else if c.wet {
        crop.wet_per_mille
    } else {
        crop.dry_per_mille
    }
}

pub fn roll(seed: u32, p: IVec3, salt: u64) -> u32 {
    use engine_core::hash::hash_u32;
    let cell = hash_u32((p.x as u32).wrapping_mul(0x9e37_79b9)
        ^ (p.y as u32).wrapping_mul(0x85eb_ca6b) ^ (p.z as u32).wrapping_mul(0xc2b2_ae35));
    hash_u32(seed ^ cell ^ hash_u32(salt as u32 ^ hash_u32((salt >> 32) as u32 ^ 0x27d4_eb2f)))
}

/// Applies every roll whose interval ended by `now`, at most `limit` of them.
/// Returns true when the stage changed.
pub fn advance(plot: &mut Plot, crop: &Crop, seed: u32, p: IVec3, now: u64, c: Conditions, limit: u32) -> bool {
    let due = u32::try_from(now.saturating_sub(plot.planted) / u64::from(crop.roll_ticks.max(1)))
        .unwrap_or(u32::MAX);
    let before = plot.stage;
    let chance = chance(crop, c);
    // A mature crop keeps the interval it ripened in; a zero chance consumes intervals without rolling.
    if plot.stage >= crop.mature() {
        return false;
    }
    if chance == 0 {
        plot.rolled = plot.rolled.max(due);
        return false;
    }
    let mut spent = 0;
    while plot.rolled < due && plot.stage < crop.mature() && spent < limit {
        plot.rolled += 1;
        spent += 1;
        if roll(seed, p, u64::from(plot.rolled)) % 1000 < chance {
            plot.stage += 1;
        }
    }
    plot.stage != before
}
