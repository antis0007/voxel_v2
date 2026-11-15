//! Life in the Lightless that goes on without the player. The Hush live in
//! lairs, one small colony per region, and drift through the open caverns.
//! They keep clear of lit camps, close on anyone standing unlit, and put out
//! torches left burning alone. Away time is replayed in coarse steps from the
//! shared day clock when the realm next loads, so a lone torch left behind is
//! often dark on return.
//!
//! In every realm, living blocks the generator made (glowcaps, foxfire,
//! hanging roots) are logged when harvested and grow back on the day clock.
//!
//! Every choice is a positional hash of the realm seed and a step counter; no
//! wall clock. State lives in `lightless-life.ron` beside the checkpoint.
use super::{BlockId, World, chunk_coord_from_world_block as chunk_of, inventory::{DroppedStack, Item, ItemStack}};
use crate::player::Player;
use anyhow::{Context, Result, ensure};
use engine_core::hash::hash_u32;
use engine_worldgen::lightless;
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE: &str = "lightless-life.ron";
const VERSION: u32 = 1;

/// Colony regions, metres.
const REGION_M: f32 = 96.0;
/// Regions simulated around the focus, as a Chebyshev radius.
const REGION_REACH: i32 = 2;
/// Step while the realm is loaded, and while replaying away time.
const PRESENT_STEP_S: f32 = 0.25;
const AWAY_STEP_S: f32 = 4.0;
/// Away time beyond this is not replayed; the colony has settled by then.
const MAX_AWAY_S: f32 = 7200.0;

const WANDER_SPEED: f32 = 1.4;
const STALK_SPEED: f32 = 3.0;
const HUNT_SPEED: f32 = 5.2;
/// Unlit seconds before the Hush close in.
pub(super) const NOTICE_S: f32 = 2.5;
const NOTICE_M: f32 = 42.0;
/// They hold at this distance from a lit body.
pub const LIGHT_EDGE_M: f32 = 8.0;
const BITE_M: f32 = 1.7;
const BITE_DAMAGE: f32 = 14.0;
const BITE_COOLDOWN_S: f32 = 1.1;
/// A torch with fewer than `CAMP_TORCHES - 1` others within this reach is prey.
const CAMP_M: f32 = 6.0;
const CAMP_TORCHES: usize = 3;
const SNUFF_M: f32 = 2.2;
/// The player keeps a torch safe from this close.
const GUARD_M: f32 = 24.0;
const MAX_HUSH: usize = 128;
const MAX_TORCHES: usize = 512;
const MAX_REGIONS: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hush {
    pub region: [i32; 2],
    /// Metres.
    pub pos: [f32; 3],
    target: [f32; 3],
    /// Seconds of retreat after a bite.
    rest: f32,
    bite: f32,
    /// Orbit phase while stalking a lit body.
    orbit: f32,
    /// Closing on an unlit body; the change is what makes a sound.
    #[serde(default)]
    hunting: bool,
}

/// A harvested living block that grows back on the day clock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprout {
    cell: [i32; 3],
    block: u32,
    /// Absolute day it is due.
    due: f64,
}

const MAX_SPROUTS: usize = 4096;

/// Game days a harvested generated block takes to grow back, by realm.
/// Only living matter the realm made regrows; rock and placed blocks do not.
pub fn regrow_days(kind: engine_voxel::RealmKind, block: BlockId) -> Option<f64> {
    use engine_voxel::RealmKind as K;
    match (kind, block) {
        (K::Lightless, BlockId::FoxfireMoss) => Some(3.0),
        (K::Gloaming, BlockId::AmberCap) => Some(2.0),
        (K::Gloaming, BlockId::GloamingLeaves) => Some(4.0),
        (K::Highfold, BlockId::Vines) => Some(1.0),
        (K::Highfold, BlockId::Leaves) => Some(3.0),
        (K::Highfold, BlockId::Crystal) => Some(8.0),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Life {
    version: u32,
    /// Absolute days at the last update; None until the realm first ticks.
    clock: Option<f64>,
    step: u64,
    regions: Vec<[i32; 2]>,
    pub hush: Vec<Hush>,
    /// Placed torches the colony knows of, voxel cells.
    torches: Vec<[i32; 3]>,
    /// Torches put out while their chunk was not resident.
    pending: Vec<[i32; 3]>,
    /// Torches put out in total, for the codex and tests.
    pub snuffed: u32,
    /// Last player position, metres; the replay centres here.
    focus: [f32; 3],
    /// Harvested living blocks waiting to grow back, in every realm.
    #[serde(default)]
    sprouts: Vec<Sprout>,
    #[serde(skip)]
    accum: f32,
    #[serde(skip)]
    survey: f32,
}

fn unit(seed: u32) -> f32 {
    (hash_u32(seed) >> 8) as f32 / 16_777_216.0
}

fn region_of(m: Vec3) -> [i32; 2] {
    [(m.x / REGION_M).floor() as i32, (m.z / REGION_M).floor() as i32]
}

fn region_seed(seed: u32, r: [i32; 2]) -> u32 {
    hash_u32(seed ^ 0x4855_5348 ^ hash_u32(r[0] as u32) ^ (r[1] as u32).rotate_left(16))
}

/// Open rock the body can drift through: unedited caverns from the generator,
/// edited cells from the resident world so walls the player builds hold them.
fn passable(world: &World, seed: u32, m: Vec3) -> bool {
    if m.y < lightless::LAVA_M + 1.0 || m.y > lightless::ROOF_M {
        return false;
    }
    let cell = (m / world.voxel_scale.meters()).floor().as_ivec3();
    if world.chunks.get(&chunk_of(cell)).is_some() {
        let b = world.get(cell);
        return !b.is_solid() && !b.is_liquid();
    }
    lightless::open(seed, m)
}

/// The lair a region's colony returns to: the first open hashed point.
fn home(world: &World, seed: u32, r: [i32; 2]) -> Option<Vec3> {
    let rs = region_seed(seed, r);
    (0..24u32).find_map(|k| {
        let p = Vec3::new(
            (r[0] as f32 + 0.15 + 0.7 * unit(rs ^ k)) * REGION_M,
            lightless::DECKS[(k % 3) as usize] + 1.5 + unit(rs ^ k ^ 0x55) * 4.0,
            (r[1] as f32 + 0.15 + 0.7 * unit(rs ^ k ^ 0xaa)) * REGION_M,
        );
        passable(world, seed, p).then_some(p)
    })
}

fn population(seed: u32, r: [i32; 2]) -> usize {
    [0, 1, 1, 2, 2, 3][((unit(region_seed(seed, r) ^ 0x504f) * 6.0) as usize).min(5)]
}

/// Whether the player is in light the Hush will not enter.
pub fn player_lit(world: &World, player: &Player) -> bool {
    crate::player::hazards::lit(player, world, IVec3::ZERO)
}

impl Life {
    fn torch_m(&self, scale: f32) -> impl Iterator<Item = Vec3> + '_ {
        self.torches.iter().map(move |c| (IVec3::from_array(*c).as_vec3() + Vec3::splat(0.5)) * scale)
    }

    /// Torches with enough neighbours to make a lit camp (`true`) or prey (`false`).
    fn split_torches(&self, scale: f32) -> (Vec<Vec3>, Vec<Vec3>) {
        let lights: Vec<Vec3> = self.torch_m(scale).collect();
        lights.iter().partition(|&&t| lights.iter().filter(|&&o| o.distance(t) < CAMP_M).count() >= CAMP_TORCHES)
    }

    fn ensure_regions(&mut self, world: &World, focus: Vec3) {
        let seed = world.seed;
        let c = region_of(focus);
        for dz in -REGION_REACH..=REGION_REACH {
            for dx in -REGION_REACH..=REGION_REACH {
                let r = [c[0] + dx, c[1] + dz];
                if self.regions.contains(&r) || self.regions.len() >= MAX_REGIONS {
                    continue;
                }
                self.regions.push(r);
                let Some(h) = home(world, seed, r) else { continue };
                for i in 0..population(seed, r) {
                    if self.hush.len() >= MAX_HUSH {
                        break;
                    }
                    let jitter = Vec3::new(i as f32 * 1.7, 0.0, -(i as f32) * 1.3);
                    let pos = if passable(world, seed, h + jitter) { h + jitter } else { h };
                    self.hush.push(Hush { region: r, pos: pos.to_array(), target: pos.to_array(), rest: 0.0, bite: 0.0, orbit: i as f32 * 2.1, hunting: false });
                }
            }
        }
    }

    /// Finds placed torches in resident chunks near the player and forgets
    /// ones that are gone.
    fn survey_torches(&mut self, world: &World, eye: IVec3) {
        let centre = chunk_of(eye);
        self.torches.retain(|c| {
            let cell = IVec3::from_array(*c);
            world.chunks.get(&chunk_of(cell)).is_none() || world.get(cell).is_torch()
        });
        for dz in -3..=3 {
            for dy in -2..=2 {
                for dx in -3..=3 {
                    let Some(chunk) = world.chunks.get(&(centre + IVec3::new(dx, dy, dz))) else { continue };
                    for &p in &chunk.torches {
                        if self.torches.len() >= MAX_TORCHES {
                            return;
                        }
                        if world.get(p).is_torch() && !self.torches.contains(&p.to_array()) {
                            self.torches.push(p.to_array());
                        }
                    }
                }
            }
        }
    }

    /// Puts out `cell`: at once when resident, otherwise on its next load.
    fn snuff(&mut self, world: &mut World, cell: IVec3) {
        self.torches.retain(|c| *c != cell.to_array());
        self.snuffed = self.snuffed.saturating_add(1);
        if world.chunks.get(&chunk_of(cell)).is_some() {
            extinguish(world, cell);
        } else if self.pending.len() < MAX_TORCHES {
            self.pending.push(cell.to_array());
        }
    }

    fn apply_pending(&mut self, world: &mut World) {
        let ready: Vec<IVec3> = self
            .pending
            .iter()
            .map(|c| IVec3::from_array(*c))
            .filter(|&c| world.chunks.get(&chunk_of(c)).is_some())
            .collect();
        for cell in ready {
            self.pending.retain(|c| *c != cell.to_array());
            if world.get(cell).is_torch() {
                extinguish(world, cell);
            }
        }
    }

    /// One colony step. `player` is None while replaying away time.
    fn advance(&mut self, world: &mut World, player: Option<(&Player, bool)>, dt: f32) {
        let seed = world.seed;
        let scale = world.voxel_scale.meters();
        self.step = self.step.wrapping_add(1);
        let step = self.step as u32;
        let body = player.map(|(p, lit)| (p.pos * scale + Vec3::Y * 1.0, lit, p.dark_seconds));
        let mut bites = 0.0;
        let mut snuffs = Vec::new();
        let mut cues: Vec<(Vec3, f32, f32)> = Vec::new();
        let (camps, lone) = self.split_torches(scale);
        let homes: Vec<Option<Vec3>> = self.hush.iter().map(|h| home(world, seed, h.region)).collect();
        for (i, h) in self.hush.iter_mut().enumerate() {
            let hs = hash_u32(seed ^ step.wrapping_mul(0x9e37_79b9) ^ (i as u32).wrapping_mul(0x85eb_ca6b));
            let pos = Vec3::from_array(h.pos);
            h.rest = (h.rest - dt).max(0.0);
            h.bite = (h.bite - dt).max(0.0);
            let mut speed = WANDER_SPEED;
            let mut target = Vec3::from_array(h.target);
            let near_body = body.filter(|(b, _, _)| b.distance(pos) < NOTICE_M);
            if let Some((b, lit, dark)) = near_body {
                let away = (pos - b).try_normalize().unwrap_or(Vec3::X);
                if h.rest > 0.0 {
                    target = b + away * (LIGHT_EDGE_M + 4.0);
                    speed = HUNT_SPEED;
                } else if !lit && dark >= NOTICE_S {
                    target = b;
                    speed = HUNT_SPEED;
                } else if lit {
                    // Circle at the edge of the light: eyes in the dark.
                    h.orbit += dt * 0.25;
                    let ring = Vec3::new(h.orbit.cos(), 0.0, h.orbit.sin());
                    target = b + (away + ring * 0.6).normalize_or(away) * LIGHT_EDGE_M + Vec3::Y * 0.6;
                    speed = STALK_SPEED;
                }
            } else if pos.distance(target) < 1.5 {
                // Lone torches draw them; otherwise drift about the lair.
                let prey = lone.iter().copied().filter(|t| t.distance(pos) < 40.0).min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)));
                target = match (prey, homes[i]) {
                    (Some(t), _) if unit(hs ^ 1) < 0.35 => t,
                    (_, Some(home)) => home + Vec3::new(unit(hs ^ 2) * 56.0 - 28.0, unit(hs ^ 3) * 6.0 - 3.0, unit(hs ^ 4) * 56.0 - 28.0),
                    _ => pos + Vec3::new(unit(hs ^ 2) * 20.0 - 10.0, 0.0, unit(hs ^ 4) * 20.0 - 10.0),
                };
            }
            let to = target - pos;
            let mut next = pos;
            if to.length() > 0.05 {
                let stride = to.clamp_length_max(speed * dt);
                let candidate = pos + stride;
                // A lit body holds them off; so does a lit camp.
                let too_close = near_body.is_some_and(|(b, lit, _)| lit && candidate.distance(b) < LIGHT_EDGE_M * 0.8 && h.rest <= 0.0);
                let in_camp = camps.iter().any(|c| c.distance(candidate) < LIGHT_EDGE_M * 0.75);
                if !too_close && !in_camp && passable(world, seed, candidate) {
                    next = candidate;
                } else if passable(world, seed, pos + Vec3::Y * stride.length()) && !too_close {
                    next = pos + Vec3::Y * stride.length();
                } else {
                    target = next;
                }
            }
            h.pos = next.to_array();
            h.target = target.to_array();
            let hunting = h.rest <= 0.0 && near_body.is_some_and(|(_, lit, dark)| !lit && dark >= NOTICE_S);
            if hunting && !h.hunting {
                cues.push((next, 0.38, 0.36));
            }
            h.hunting = hunting;
            if let Some((b, lit, _)) = near_body
                && !lit && h.bite <= 0.0 && next.distance(b) < BITE_M
            {
                bites += BITE_DAMAGE;
                h.bite = BITE_COOLDOWN_S;
                h.rest = 1.6;
            }
            for &t in &lone {
                let guarded = body.is_some_and(|(b, _, _)| b.distance(t) < GUARD_M);
                if !guarded && t.distance(next) < SNUFF_M {
                    snuffs.push((t / scale).floor().as_ivec3());
                }
            }
        }
        for cell in snuffs {
            if self.torches.contains(&cell.to_array()) {
                self.snuff(world, cell);
                if body.is_some() {
                    cues.push(((cell.as_vec3() + Vec3::splat(0.5)) * scale, 0.3, 0.28));
                }
            }
        }
        for (pos, gain, pitch) in cues {
            if world.adventure.sounds.len() < 64 {
                world.adventure.sounds.push(crate::audio::Event { cue: crate::audio::Cue::Echo, pos: Some(pos), gain, pitch });
            }
        }
        if bites > 0.0 {
            world.adventure.environment_damage(bites, super::adventure::death::Cause::Hush);
        }
    }
}

/// Swaps a torch for darkness and leaves the torch where it stood.
fn extinguish(world: &mut World, cell: IVec3) {
    let block = world.get(cell);
    world.set(cell, BlockId::Air);
    if world.inventory.dropped.len() < 1024 {
        let pos = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
        let torch = if block.is_torch() { BlockId::Torch } else { block };
        world.inventory.dropped.push(DroppedStack { pos, stack: ItemStack::new(Item::Block(torch), 1) });
    }
}

/// Logs living blocks harvested this step and grows back the due ones whose
/// cells are resident and still empty. Away time needs no replay: due days
/// are absolute, so a forest picked bare regrows while the player is gone.
fn regrowth(life: &mut Life, world: &mut World, kind: engine_voxel::RealmKind, days: f64) {
    if world.changed_cells.is_empty() && life.sprouts.is_empty() {
        return;
    }
    let cfg = world.sample_config();
    for &p in &world.changed_cells {
        if life.sprouts.len() >= MAX_SPROUTS {
            break;
        }
        if !world.get(p).is_empty() {
            continue;
        }
        let made = engine_worldgen::sample_block(&cfg, p);
        if let Some(wait) = regrow_days(kind, made)
            && !life.sprouts.iter().any(|s| s.cell == p.to_array())
        {
            // Seeded spread so a harvested patch does not refill in one instant.
            let jitter = f64::from(unit(hash_u32(world.seed ^ hash_u32(p.x as u32) ^ (p.z as u32).rotate_left(13) ^ p.y as u32)));
            life.sprouts.push(Sprout { cell: p.to_array(), block: made.as_u32(), due: days + wait * (0.75 + 0.5 * jitter) });
        }
    }
    life.sprouts.retain(|sprout| {
        let cell = IVec3::from_array(sprout.cell);
        if sprout.due > days {
            return true;
        }
        let Some(current) = world.resident_block(cell) else {
            return true;
        };
        let block = BlockId::from_u32(sprout.block);
        if current.is_empty() && regrow_days(kind, block).is_some() {
            world.set(cell, block);
        }
        false
    });
}

/// Fixed-step entry. Replays Lightless colony activity after an absence.
pub fn tick(world: &mut World, player: &Player, day: &super::DayNightCycle, dt: f32) {
    let Some(kind) = world.world_type.realm() else { return };
    if world.ca_view.is_some() {
        return;
    }
    let mut life = std::mem::take(&mut world.lightless);
    life.version = VERSION;
    let days = super::synoptic::days(day);
    regrowth(&mut life, world, kind, days);
    if kind != engine_voxel::RealmKind::Lightless {
        life.clock = Some(days);
        world.lightless = life;
        return;
    }
    let scale = world.voxel_scale.meters();
    let body = player.pos * scale;
    if let Some(clock) = life.clock {
        let away = ((days - clock).max(0.0) * f64::from(day.day_length_seconds())) as f32;
        if away > 2.0 {
            let focus = Vec3::from_array(life.focus);
            life.ensure_regions(world, focus);
            let steps = (away.min(MAX_AWAY_S) / AWAY_STEP_S) as u32;
            for _ in 0..steps {
                life.advance(world, None, AWAY_STEP_S);
            }
        }
    }
    life.clock = Some(days);
    life.focus = body.to_array();
    life.apply_pending(world);
    life.survey -= dt;
    if life.survey <= 0.0 {
        life.survey = 2.0;
        life.ensure_regions(world, body);
        life.survey_torches(world, player.eye(world.voxel_scale).floor().as_ivec3());
    }
    life.accum += dt;
    let lit = player_lit(world, player);
    while life.accum >= PRESENT_STEP_S {
        life.accum -= PRESENT_STEP_S;
        life.advance(world, Some((player, lit)), PRESENT_STEP_S);
    }
    world.lightless = life;
}

pub fn encode(life: &Life) -> Vec<u8> {
    if life.clock.is_none() {
        return Vec::new();
    }
    ron::to_string(life).unwrap_or_default().into_bytes()
}

fn decode(bytes: &[u8]) -> Result<Life> {
    ensure!(bytes.len() <= 262_144, "Lightless life file exceeds size limit");
    let life: Life = ron::from_str(std::str::from_utf8(bytes)?).context("Unreadable Lightless life file")?;
    ensure!(life.version == VERSION, "Unsupported Lightless life version");
    ensure!(life.sprouts.len() <= MAX_SPROUTS && life.sprouts.iter().all(|s| s.due.is_finite()), "Realm regrowth exceeds limits");
    ensure!(life.hush.len() <= MAX_HUSH && life.torches.len() <= MAX_TORCHES
        && life.pending.len() <= MAX_TORCHES && life.regions.len() <= MAX_REGIONS, "Lightless life exceeds limits");
    let finite = |v: &[f32; 3]| v.iter().all(|x| x.is_finite() && x.abs() < 4.0e6);
    ensure!(life.hush.iter().all(|h| finite(&h.pos) && finite(&h.target)) && finite(&life.focus), "Lightless life position out of range");
    ensure!(life.clock.is_none_or(|c| c.is_finite() && c >= 0.0), "Lightless life clock out of range");
    Ok(life)
}

pub fn save(world_dir: &Path, bytes: &[u8]) -> Result<()> {
    let path = world_dir.join(FILE);
    if bytes.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let temporary = path.with_extension("ron.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, &path)?;
    Ok(())
}

/// A missing or damaged file starts a fresh colony.
pub fn load(world: &mut World, world_dir: &Path) {
    world.lightless = match std::fs::read(world_dir.join(FILE)) {
        Ok(bytes) => decode(&bytes).unwrap_or_else(|e| {
            eprintln!("[lightless] ignored {FILE}: {e:#}");
            Life::default()
        }),
        Err(_) => Life::default(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;
    use engine_voxel::RealmKind;

    fn realm() -> World {
        World::with_seed(WorldType::Realm(RealmKind::Lightless), VoxelScalePreset::OneMeter, 7)
    }

    /// The colony seeds from the realm seed alone and replays the same way.
    #[test]
    fn colony_is_deterministic_and_round_trips() {
        let run = || {
            let mut w = realm();
            let mut life = Life { version: VERSION, clock: Some(0.0), ..Life::default() };
            life.ensure_regions(&w, Vec3::new(2.5, 1.0, 0.5));
            for _ in 0..200 {
                life.advance(&mut w, None, AWAY_STEP_S);
            }
            life
        };
        let (a, b) = (run(), run());
        assert!(!a.hush.is_empty(), "some regions are inhabited");
        assert_eq!(a, b);
        let back = decode(&encode(&a)).expect("round trip");
        assert_eq!(back.hush, a.hush);
    }

    /// A torch left alone in the dark is put out while the player is away;
    /// a lit camp of three is left burning.
    #[test]
    fn away_hush_snuff_lone_torches_but_not_camps() {
        let mut w = realm();
        let seed = w.seed;
        let mut life = Life { version: VERSION, clock: Some(0.0), ..Life::default() };
        life.ensure_regions(&w, Vec3::new(2.5, 1.0, 0.5));
        let lair = life.hush.first().map(|h| Vec3::from_array(h.pos)).expect("an inhabited region");
        let lone = (lair / w.voxel_scale.meters()).floor().as_ivec3() + IVec3::new(3, 0, 0);
        assert!(lightless::open(seed, (lone.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters()));
        life.torches.push(lone.to_array());
        let camp = IVec3::new(2, 1, 2);
        for d in [IVec3::ZERO, IVec3::X, IVec3::Z] {
            life.torches.push((camp + d).to_array());
        }
        for _ in 0..900 {
            life.advance(&mut w, None, AWAY_STEP_S);
        }
        assert!(life.pending.contains(&lone.to_array()), "lone torch snuffed while away");
        for d in [IVec3::ZERO, IVec3::X, IVec3::Z] {
            assert!(life.torches.contains(&(camp + d).to_array()), "camp torch kept");
        }
    }

    /// A glowcap picked in the Gloaming is logged and grows back once due,
    /// whether or not anyone watched.
    #[test]
    fn harvested_glowcaps_grow_back_on_the_day_clock() {
        let mut w = World::with_seed(WorldType::Realm(RealmKind::Gloaming), VoxelScalePreset::OneMeter, 7);
        let cfg = w.sample_config();
        let cap = (20..120)
            .flat_map(|x| (20..120).map(move |z| (x, z)))
            .find_map(|(x, z)| {
                let top = engine_worldgen::gloaming::ground(cfg.seed, x as f32 + 0.5, z as f32 + 0.5).floor() as i32;
                (top..top + 2).map(|y| IVec3::new(x, y, z)).find(|&p| engine_worldgen::sample_block(&cfg, p) == BlockId::AmberCap)
            })
            .expect("a glowcap patch near the clearing");
        assert!(w.admit_edit_chunk(chunk_of(cap)));
        assert_eq!(w.get(cap), BlockId::AmberCap);
        let mut life = Life::default();
        w.set(cap, BlockId::Air);
        regrowth(&mut life, &mut w, RealmKind::Gloaming, 10.0);
        assert_eq!(life.sprouts.len(), 1, "the harvest is logged");
        regrowth(&mut life, &mut w, RealmKind::Gloaming, 10.5);
        assert_eq!(w.get(cap), BlockId::Air, "not yet due");
        regrowth(&mut life, &mut w, RealmKind::Gloaming, 13.0);
        assert_eq!(w.get(cap), BlockId::AmberCap, "grown back");
        assert!(life.sprouts.is_empty());
    }

    /// Drifting Hush stay in open rock.
    #[test]
    fn hush_stay_in_open_caverns() {
        let mut w = realm();
        let mut life = Life { version: VERSION, clock: Some(0.0), ..Life::default() };
        life.ensure_regions(&w, Vec3::ZERO);
        for _ in 0..400 {
            life.advance(&mut w, None, AWAY_STEP_S);
        }
        for h in &life.hush {
            assert!(lightless::open(w.seed, Vec3::from_array(h.pos)), "hush inside rock at {:?}", h.pos);
        }
    }
}
