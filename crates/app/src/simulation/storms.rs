//! Weather in play near the player: snow cover and still-water ice in cold
//! precipitation, and lightning in thunderstorms.
//!
//! Every roll hashes the world seed with the farming clock, the fixed-step
//! counter the save keeps, so replaying the same steps makes the same edits.
//! Nothing here is saved: snow and ice are ordinary blocks once placed.
use super::{Budget, STEP_SECONDS, sandbox::ThermalCell};
use crate::audio::{Cue, Event};
use crate::player::Player;
use crate::world::{
    BlockId as B, ChunkData, DayNightCycle, LightingState, World,
    chunk_coord_from_world_block as chunk_of, effects,
};
use engine_core::hash::hash_u32;
use glam::{IVec3, Vec2, Vec3};

/// Columns rolled per fixed step, about 480 a second.
const COLUMNS_PER_STEP: u32 = 4;
const RADIUS_M: f32 = 48.0;
/// Columns are scanned down from this far above the player; a roof higher up is not seen.
const SCAN_UP_M: f32 = 24.0;
const SCAN_DOWN_M: f32 = 32.0;
const SNOW_BELOW_C: f32 = 0.0;
const MELT_ABOVE_C: f32 = 0.0;
const FREEZE_BELOW_C: f32 = -4.0;
const THAW_ABOVE_C: f32 = 0.0;
/// Torch light at which snow melts, Minecraft's block light 12 on a 0..15 scale.
const TORCH_MELT: u8 = 11;
/// Chances per sampled column, per mille. Settling scales with precipitation.
const SETTLE_PER_MILLE: f32 = 300.0;
const MELT_PER_MILLE: u32 = 250;
const FREEZE_PER_MILLE: u32 = 120;
const THAW_PER_MILLE: u32 = 120;
/// Cumulonimbus share from which a storm throws lightning.
pub const THUNDER_CUMULONIMBUS: f32 = 0.7;
/// One strike per this many fixed steps on average: 20 s at 120 Hz.
const STRIKE_STEPS: u32 = 2400;
const STRIKE_RANGE_M: f32 = 64.0;
const STRIKE_HIT_M: f32 = 2.5;
const STRIKE_DAMAGE: f32 = 25.0;
const FLASH_SECONDS: f32 = 0.4;
const DIRTY_LIMIT: usize = 2048;
/// Wall time per fixed step for the column scan.
const SLICE: std::time::Duration = std::time::Duration::from_micros(400);

/// Runtime storm state kept beside the day clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pulse {
    flash_s: f32,
    /// Farming clock of the last weather step. A peer whose clock does not
    /// advance never repeats a strike.
    last_clock: Option<u64>,
}

impl Pulse {
    /// Lightning flash strength, 0..1.
    pub fn flash(self) -> f32 {
        (self.flash_s / FLASH_SECONDS).clamp(0.0, 1.0)
    }
}

/// Rain or snow amount, 0..1, from the cumulonimbus share. Matches the rain bed.
pub fn precipitation(cumulonimbus: f32) -> f32 {
    ((cumulonimbus - 0.10) / 0.65).clamp(0.0, 1.0)
}

fn roll(seed: u32, clock: u64, salt: u32) -> u32 {
    hash_u32(seed ^ hash_u32(clock as u32 ^ hash_u32((clock >> 32) as u32 ^ salt)))
}

fn cells(metres: f32, scale: f32) -> i32 {
    (metres / scale).round().max(1.0) as i32
}

/// Runs one fixed step of weather near the player.
pub(crate) fn tick(world: &mut World, player: &Player, day: &mut DayNightCycle, budget: Budget) {
    day.storm.flash_s = (day.storm.flash_s - STEP_SECONDS).max(0.0);
    let clock = world.farming.clock;
    if world.ca_view.is_some() || day.storm.last_clock == Some(clock) {
        return;
    }
    day.storm.last_clock = Some(clock);
    let cumulonimbus = day.cloud_controls().cumulonimbus;
    let centre = player.pos.floor().as_ivec3();
    let scale = world.voxel_scale.meters();
    cover(world, centre, precipitation(cumulonimbus), clock, budget);
    if let Some(column) = strike_column(world.seed, clock, centre, scale, cumulonimbus) {
        strike(world, day, column);
    }
}

/// Rolls a bounded set of columns around `centre` for snow, melt, freeze and thaw.
pub(crate) fn cover(world: &mut World, centre: IVec3, rain: f32, clock: u64, budget: Budget) {
    let scale = world.voxel_scale.meters();
    let r = cells(RADIUS_M, scale);
    let span = (2 * r + 1) as u32;
    // Storms run before the cellular pass in the same step budget; their own
    // slice keeps a cold scan from starving water and sand.
    let budget = budget.child(SLICE).min(budget);
    // Untouched columns hold generated terrain, read one generator call per
    // voxel. Only cold air can change them; when warm, only snow and ice that
    // weather placed can melt, and placing them made their chunk resident.
    let pristine = world.sandbox.cell(centre).temperature < SNOW_BELOW_C;
    let mut changed = Vec::new();
    // At most one chunk is made resident per step; it costs a generation.
    let mut admitted = false;
    for i in 0..COLUMNS_PER_STEP {
        if !budget.has_time() || world.adventure.dirty_voxels.len() + changed.len() >= DIRTY_LIMIT {
            break;
        }
        let h = roll(world.seed, clock, i);
        let column = centre + IVec3::new((h % span) as i32 - r, 0, ((h >> 16) % span) as i32 - r);
        let Some(top) = scan(world, column, scale, pristine, budget) else { continue };
        let Some((at, expected, next)) = change(world, top, rain, hash_u32(h ^ 0x5EED) % 1000) else {
            continue;
        };
        if !resident(world, at, &mut admitted) {
            continue;
        }
        if let Ok(commit) = effects::replace(world, at, expected, next) {
            commit.publish_voxel(world, &mut changed);
        }
    }
    world.adventure.dirty_voxels.extend(changed);
}

fn resident(world: &mut World, p: IVec3, admitted: &mut bool) -> bool {
    let c = chunk_of(p);
    if world.chunks.contains_key(&c) {
        return true;
    }
    if *admitted {
        return false;
    }
    *admitted = true;
    world.admit_edit_chunk(c)
}

/// A block read without disk I/O: resident, or generated and never edited (one
/// generator call when `pristine`). `None` for an edited chunk paged out.
fn known(world: &World, p: IVec3, pristine: bool) -> Option<B> {
    let c = chunk_of(p);
    if world.chunks.contains_key(&c) {
        return Some(world.get(p));
    }
    if world.regions.view.records.contains_key(&c) {
        return None;
    }
    // Warm passes only melt what weather placed, which made its chunk resident;
    // untouched cells around it count as open air instead of a generator call.
    Some(if pristine { world.get(p) } else { B::Air })
}

/// Top non-empty cell of the column through `column`, scanned down from a fixed
/// height above it. `None` when that start cell is not open air.
pub(crate) fn surface(world: &World, column: IVec3, scale: f32) -> Option<IVec3> {
    scan(world, column, scale, true, Budget::UNBOUNDED)
}

/// [`surface`] that skips untouched chunks unless `pristine`, and gives up when
/// `budget` runs out mid-column.
fn scan(world: &World, column: IVec3, scale: f32, pristine: bool, budget: Budget) -> Option<IVec3> {
    let cfg = world.sample_config();
    let start = (column.y + cells(SCAN_UP_M, scale)).min(cfg.max_world_y() - 1);
    let floor = (column.y - cells(SCAN_DOWN_M, scale)).max(cfg.min_world_y());
    let mut p = IVec3::new(column.x, start, column.z);
    if !known(world, p, pristine)?.is_empty() {
        return None;
    }
    while p.y > floor && budget.has_time() {
        p.y -= 1;
        if !known(world, p, pristine)?.is_empty() {
            return Some(p);
        }
    }
    None
}

fn torch_light(world: &World, p: IVec3) -> u8 {
    world.chunks.get(&chunk_of(p)).map_or(0, |c| c.light[ChunkData::local_index(p.x, p.y, p.z)] & 0x0F)
}

/// Full opaque cubes carry snow; models, fittings and thin blocks do not.
fn holds_snow(b: B) -> bool {
    b.is_solid() && b.is_opaque() && engine_content::fittings::state(b).is_none()
}

/// Full water with no open side or open cell below; a moving front is left alone.
fn still(world: &World, p: IVec3) -> bool {
    world.water_volume(p) == 8
        && [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y]
            .iter()
            .all(|&d| known(world, p + d, true).is_some_and(|b| b != B::Air))
}

/// The edit weather makes at `top`, the highest cell of an open column, for a
/// roll in 0..1000: the cell, the block expected there and its replacement.
pub(crate) fn change(world: &World, top: IVec3, rain: f32, roll: u32) -> Option<(IVec3, B, B)> {
    let above = top + IVec3::Y;
    let air_c = world.sandbox.cell(above).temperature;
    let ground_c = world.sandbox.cell_for(top, world.get(top)).temperature;
    match world.get(top) {
        B::SnowGrass if ground_c.max(air_c) > MELT_ABOVE_C => {
            (roll < MELT_PER_MILLE).then_some((top, B::SnowGrass, B::Grass))
        }
        B::SnowLayer => {
            let warm = world.sandbox.cell(top).temperature.max(air_c) > MELT_ABOVE_C
                || torch_light(world, top) >= TORCH_MELT;
            (warm && roll < MELT_PER_MILLE).then_some((top, B::SnowLayer, B::Air))
        }
        B::Water => (air_c < FREEZE_BELOW_C && roll < FREEZE_PER_MILLE && still(world, top))
            .then_some((top, B::Water, B::Ice)),
        // Only a skin over water thaws: generated ice sheets and ice masonry stay.
        B::Ice if air_c > THAW_ABOVE_C && world.get(top - IVec3::Y) == B::Water => {
            (roll < THAW_PER_MILLE).then_some((top, B::Ice, B::Water))
        }
        b if holds_snow(b) => (air_c < SNOW_BELOW_C
            && ground_c <= SNOW_BELOW_C
            && torch_light(world, above) < TORCH_MELT
            && (roll as f32) < SETTLE_PER_MILLE * rain)
            .then_some((above, B::Air, B::SnowLayer)),
        _ => None,
    }
}

/// Column a thunderstorm strikes on this step, if any, within range of `centre`.
pub(crate) fn strike_column(seed: u32, clock: u64, centre: IVec3, scale: f32, cumulonimbus: f32) -> Option<IVec3> {
    if cumulonimbus < THUNDER_CUMULONIMBUS {
        return None;
    }
    let h = roll(seed, clock, 0x7417_0000);
    if !h.is_multiple_of(STRIKE_STEPS) {
        return None;
    }
    let a = hash_u32(h ^ 0xA9);
    let angle = (a & 0xFFFF) as f32 / 65_536.0 * std::f32::consts::TAU;
    let distance = STRIKE_RANGE_M * (a >> 16) as f32 / 65_536.0;
    let d = Vec2::from_angle(angle) * distance / scale;
    Some(centre + IVec3::new(d.x.round() as i32, 0, d.y.round() as i32))
}

/// Flash, thunder and fire on a flammable top.
fn strike(world: &mut World, day: &mut DayNightCycle, column: IVec3) {
    let scale = world.voxel_scale.meters();
    let Some(top) = surface(world, column, scale) else { return };
    day.storm.flash_s = FLASH_SECONDS;
    let at_m = (top.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * scale;
    if world.adventure.sounds.len() < 64 {
        world.adventure.sounds.push(Event { cue: Cue::ForgeSlam, pos: Some(at_m), gain: 1.0, pitch: 0.45 });
    }
    ignite(world, top);
}

/// Lights a flammable `top` from the open cell above it, as fire spells do.
/// Thermal combustion then burns the block with its residue and spread rules;
/// replacing the block outright would delete a log without ash or ignition.
pub(crate) fn ignite(world: &mut World, top: IVec3) {
    let above = top + IVec3::Y;
    let mut admitted = false;
    if world.get(top).material().sim.thermal.fuel > 0.0
        && resident(world, above, &mut admitted)
        && let Ok(commit) = effects::replace(world, above, B::Air, B::Fire)
    {
        let mut changed = Vec::new();
        commit.publish_voxel(world, &mut changed);
        world.adventure.dirty_voxels.extend(changed);
        world.sandbox.set(above, ThermalCell { temperature: 820.0, fuel: 75.0, gas: 0 });
    }
}

/// Whether a strike landing at `at_m` hurts feet at `feet_m`. The vertical window
/// spares a player on a ledge above, or under a roof below, the struck surface.
pub(crate) fn hits(feet_m: Vec3, at_m: Vec3) -> bool {
    Vec2::new(feet_m.x - at_m.x, feet_m.z - at_m.z).length() < STRIKE_HIT_M
        && (feet_m.y - at_m.y).abs() < STRIKE_HIT_M
}

impl LightingState {
    /// Washes the scene toward cold white for a lightning flash of strength `f`.
    pub fn with_flash(mut self, f: f32) -> Self {
        if f <= 0.0 {
            return self;
        }
        let white = [0.78, 0.82, 0.96];
        let mix = |c: [f32; 3], t: f32| std::array::from_fn(|i| c[i] + (white[i] - c[i]) * t);
        self.sky_top = mix(self.sky_top, 0.55 * f);
        self.sky_horizon = mix(self.sky_horizon, 0.55 * f);
        self.fog_color = mix(self.fog_color, 0.45 * f);
        self.ambient_color = std::array::from_fn(|i| self.ambient_color[i] + white[i] * 0.9 * f);
        self
    }
}

#[cfg(test)]
#[path = "storms_tests.rs"]
mod tests;
