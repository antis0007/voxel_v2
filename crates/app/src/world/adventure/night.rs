//! Hostiles that rise in the dark in a ring around the player and leave when far.
//!
//! Spawns are a pure function of the world seed, the player's chunk and a pass
//! counter, so two runs from the same state spawn the same bodies in the same
//! places. Night bodies carry `NIGHT_Z` as their site row and never write a
//! site record, so they leave nothing in the save.
use super::{Adventure, Enemy, MAX_ENEMIES, World, bestiary, difficulty::Difficulty, flying, free, habitat, is_boss, mobs, wildlife};
use crate::world::BlockId;
use engine_core::{Budget, hash::hash_u32};
use glam::{IVec3, Vec3};

mod roof;

pub(super) const NIGHT_Z: i32 = -29;
pub(super) const RING_MIN_M: f32 = 24.0;
pub(super) const RING_MAX_M: f32 = 44.0;
pub(super) const DESPAWN_M: f32 = 64.0;
/// No new body within `AREA_M` of this many night bodies.
pub(super) const AREA_M: f32 = 16.0;
pub(super) const AREA_CAP: usize = 2;
/// A light source keeps this radius clear of spawns.
pub(super) const LIGHT_M: f32 = 10.0;
/// Candidate columns per pass: the deterministic half of the search bound.
const ATTEMPTS: u32 = 6;
/// Ground search above and below the player's feet.
const SEARCH_M: f32 = 12.0;
/// Opaque cover overhead that counts as darkness by day.
const ROOF_M: f32 = 2.0;
const ROOF_SCAN_M: f32 = 24.0;

/// Spawn pass counter; not saved, as night bodies are not saved either.
#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    passes: u32,
    roofs: roof::Cache,
}

/// Night bodies alive at once around the player.
pub(super) fn cap(level: Difficulty) -> usize {
    match level {
        Difficulty::Peaceful => 0,
        Difficulty::Normal => 4,
        Difficulty::Hard => 7,
    }
}

/// Daytime bodies only rise under cover; half the night cap keeps caves from
/// being as crowded as the open ground after dark.
fn limit(level: Difficulty, night: bool) -> usize {
    if night { cap(level) } else { cap(level) / 2 }
}

pub(super) fn is_night_body(e: &Enemy) -> bool {
    e.site.z == NIGHT_Z
}

/// Anything Peaceful removes: not a farm resident, grazer or awakened boss.
pub(super) fn hostile(e: &Enemy) -> bool {
    e.habitat.is_none() && e.hostile() && !is_boss(e.kind)
}

/// Dead night bodies leave quietly: no record, loot or site completion.
pub(super) fn reap(enemies: &mut Vec<Enemy>) {
    enemies.retain(|e| !is_night_body(e) || e.health > 0.0);
}

/// Resolve transient night deaths before removing them; never create a site
/// clear record. An exploded seed pod has already scattered in the blast.
pub(super) fn fall(adv: &mut Adventure, world: &mut World) {
    use crate::world::inventory::{DroppedStack, ItemStack};
    for e in adv.enemies.iter().filter(|e| is_night_body(e) && e.health <= 0.0) {
        if e.attack == mobs::ATTACK_SPENT && e.kind == mobs::CREEPER { continue; }
        super::sound(&mut adv.sounds, super::Cue::Defeat, Some(e.center()), 0.65);
        if adv.remains.len() < 24 {
            adv.remains.push(super::Remains { pos: e.center(), kind: e.kind, life: 0.65, yaw: e.yaw });
        }
        let Some(species) = bestiary::species(e.kind) else { continue };
        for (i, drop) in species.drops.iter().enumerate() {
            let h = hash_u32(world.seed ^ e.slot ^ (i as u32).wrapping_mul(0x9e37_79b9));
            let count = drop.min + h % (drop.max.saturating_sub(drop.min) + 1);
            if count > 0 && world.inventory.dropped.len() < 1024 {
                world.inventory.dropped.push(DroppedStack { pos: e.center(), stack: ItemStack::new(drop.item, count) });
            }
        }
    }
}

impl Adventure {
    pub(crate) fn report_night_roof_stats(&self) {
        let stats = self.night.roofs.stats;
        println!("[night-roof] queries={} mask_hits={} mask_builds={} resident_reads={} scalar_reads={}",
            stats.queries, stats.mask_hits, stats.mask_builds, stats.resident_reads, stats.scalar_reads);
    }
}

/// One spawn pass at `feet` (metres). Runs once per adventure spawn second.
pub(super) fn tick(adv: &mut Adventure, world: &World, feet: Vec3, budget: Budget) {
    adv.night.passes = adv.night.passes.wrapping_add(1);
    if adv.difficulty == Difficulty::Peaceful {
        // Residents are saved herd members: calm them instead of removing them.
        for e in adv.enemies.iter_mut().filter(|e| wildlife::passive(e.kind)) { e.provoked = false; }
        adv.enemies.retain(|e| !hostile(e));
        return;
    }
    adv.enemies
        .retain(|e| !is_night_body(e) || flat(e.pos - feet).length() < DESPAWN_M);
    let night = crate::world::bed::is_night(world.sandbox.climate.time_of_day());
    if !night {
        daylight(adv, world);
    }
    if !world.rules.mob_spawning {
        return;
    }
    let alive = adv.enemies.iter().filter(|e| is_night_body(e)).count();
    let wild = adv.enemies.iter().filter(|e| e.habitat.is_none() && !wildlife::passive(e.kind)).count();
    if alive >= limit(adv.difficulty, night) || wild >= MAX_ENEMIES {
        return;
    }
    let s = world.voxel_scale.meters();
    let cfg = world.sample_config();
    // Fresh planets have no floors in orbit. Skip the surface biome and
    // voxel probes when the entire spawn search is above the build volume.
    if cfg.world_type == crate::world::WorldType::Planet
        && feet.y > cfg.world_y(cfg.max_world_y()) + SEARCH_M + s
        && world.orbital.habitats.habitats.is_empty()
    {
        return;
    }
    let chunk = (feet / (s * crate::world::CHUNK_SIZE as f32)).floor().as_ivec3();
    let pass = hash_u32(world.seed ^ adv.night.passes.wrapping_mul(0x9e37_79b9))
        ^ hash_u32((chunk.x as u32).wrapping_mul(0x85eb_ca6b) ^ (chunk.z as u32).wrapping_mul(0xc2b2_ae35));
    for attempt in 0..ATTEMPTS {
        if !budget.has_time() {
            break;
        }
        let h = hash_u32(pass ^ attempt.wrapping_mul(0x27d4_eb2f));
        let angle = (h & 0xffff) as f32 / 65536.0 * std::f32::consts::TAU;
        let radius = RING_MIN_M + ((h >> 16) & 0xff) as f32 / 255.0 * (RING_MAX_M - RING_MIN_M);
        let column = feet + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
        // The habitat is read at the player's height in this column; the body
        // then lands on the nearest floor there.
        let probe = (column / s).floor().as_ivec3();
        let mut trace = crate::gameplay_trace::Trace::new();
        let covered = roofed_cached(&mut adv.night.roofs, world, probe);
        trace.mark("night-roof");
        let mut ctx = habitat::Context::at(world, probe, night, covered);
        // Without a recorded spawn point the land counts as home ground.
        let from_spawn = adv.realm_spawn().point.map_or(0.0, |p| flat(column - p).length());
        ctx.danger = habitat::danger(from_spawn, ctx.biome);
        trace.mark("night-habitat");
        let Some(kind) = habitat::pick(&ctx, hash_u32(h ^ 0x5bd1_e995), hostile_kind) else { continue };
        let candidate = ground(world, column, feet.y, kind);
        trace.mark("night-ground");
        let Some((cell, pos)) = candidate else { continue };
        let crowded = adv
            .enemies
            .iter()
            .filter(|e| is_night_body(e) && (e.pos - pos).length() < AREA_M)
            .count()
            >= AREA_CAP;
        let blocked = crowded || engine_worldgen::fernwake::refuge(&world.sample_config(), pos)
            || lit(world, pos) || !(night || wild_by_day(ctx.danger) || roofed_cached(&mut adv.night.roofs, world, cell));
        trace.mark("night-safety");
        if blocked {
            continue;
        }
        let site = IVec3::new(cell.x, cell.z, NIGHT_Z);
        // Dangerous land fields tougher bodies: tiers 2 and 3 each add a step.
        let menace = u8::from(adv.difficulty == Difficulty::Hard) + ctx.danger.saturating_sub(1).min(2);
        adv.enemies.push(Enemy::new(site, adv.night.passes, kind, pos, menace));
        // Packs in dangerous land rise by day under the same rule as their lead.
        pack(adv, world, h, kind, pos, cell, night || wild_by_day(ctx.danger), menace);
        return;
    }
}

/// Bodies that rise beside the first one, within `PACK_M`, up to the species
/// pack size and the night cap. Pack members share the area, not the crowd rule.
const PACK_M: f32 = 3.5;
#[allow(clippy::too_many_arguments)]
fn pack(adv: &mut Adventure, world: &World, h: u32, kind: u32, pos: Vec3, cell: IVec3, night: bool, menace: u8) {
    let size = bestiary::species(kind).map_or(0, |s| s.pack);
    for extra in 1..u32::from(size.max(1)) {
        if adv.enemies.iter().filter(|e| is_night_body(e)).count() >= limit(adv.difficulty, night) {
            return;
        }
        let k = hash_u32(h ^ extra.wrapping_mul(0x1656_67b1));
        let angle = (k & 0xffff) as f32 / 65536.0 * std::f32::consts::TAU;
        let radius = 1.2 + ((k >> 16) & 0xff) as f32 / 255.0 * (PACK_M - 1.2);
        let column = pos + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
        let Some((c, p)) = ground(world, column, pos.y, kind) else { continue };
        if engine_worldgen::fernwake::refuge(&world.sample_config(), p)
            || lit(world, p) || !(night || roofed_cached(&mut adv.night.roofs, world, c)) {
            continue;
        }
        let site = IVec3::new(cell.x, cell.z, NIGHT_Z);
        adv.enemies.push(Enemy::new(site, adv.night.passes.wrapping_add(extra << 20), kind, p, menace));
    }
}

/// Daylight burns the bodies whose row says so unless cover shields them.
fn daylight(adv: &mut Adventure, world: &World) {
    let s = world.voxel_scale.meters();
    for e in &mut adv.enemies {
        if !is_night_body(e) || e.health <= 0.0 || !bestiary::species(e.kind).is_some_and(|sp| sp.burns) {
            continue;
        }
        let cell = ((e.pos + Vec3::Y * 0.5) / s).floor().as_ivec3();
        if !roofed_cached(&mut adv.night.roofs, world, cell) {
            e.hurt(mobs::DAY_BURN);
            e.status.burn = e.status.burn.max(1.2);
            e.flash = e.flash.max(0.1);
        }
    }
}

/// Open-ground daylight keeps hostiles away only below the danger day tier.
fn wild_by_day(danger: u8) -> bool {
    danger >= habitat::day_tier()
}

fn flat(v: Vec3) -> glam::Vec2 {
    glam::Vec2::new(v.x, v.z)
}

/// Kinds the spawn table may raise against the player: never a grazer, a
/// resident or a neutral.
fn hostile_kind(kind: u32) -> bool {
    !wildlife::passive(kind) && !bestiary::species(kind).is_some_and(|s| s.neutral)
}

/// Nearest open air on solid ground in `column`, searched outward from `feet_y`.
/// Returns the cell and the body's feet in metres; flyers hover above it.
fn ground(world: &World, column: Vec3, feet_y: f32, kind: u32) -> Option<(IVec3, Vec3)> {
    let s = world.voxel_scale.meters();
    let (x, z) = ((column.x / s).floor() as i32, (column.z / s).floor() as i32);
    let base = (feet_y / s).floor() as i32;
    let reach = (SEARCH_M / s).ceil() as i32;
    for i in 0..=2 * reach {
        let dy = if i % 2 == 0 { i / 2 } else { -(i + 1) / 2 };
        let cell = IVec3::new(x, base + dy, z);
        if world.get(cell) != BlockId::Air || !world.solid(cell - IVec3::Y) {
            continue;
        }
        let feet = (cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * s;
        let pos = if flying(kind) { feet + Vec3::Y * 2.5 } else { feet };
        if free(world, pos, kind) {
            return Some((cell, pos));
        }
    }
    None
}

/// Within `LIGHT_M` of a placed light that can reach this point through air.
/// A one-cell step to either side still counts, so light bends around a corner
/// and a solid wall still leaves the far room dark.
pub(super) fn lit(world: &World, pos: Vec3) -> bool {
    let s = world.voxel_scale.meters();
    let chunk_m = s * crate::world::CHUNK_SIZE as f32;
    let lo = ((pos - Vec3::splat(LIGHT_M)) / chunk_m).floor().as_ivec3();
    let hi = ((pos + Vec3::splat(LIGHT_M)) / chunk_m).floor().as_ivec3();
    for cz in lo.z..=hi.z {
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                let Some(chunk) = world.chunks.get(&IVec3::new(cx, cy, cz)) else { continue };
                let near = chunk.torches.iter().any(|&q| {
                    let torch = (q.as_vec3() + Vec3::splat(0.5)) * s;
                    (torch - pos).length() < LIGHT_M
                        && world.get(q).is_light_source()
                        && light_reaches(world, pos, torch)
                });
                if near {
                    return true;
                }
            }
        }
    }
    false
}

fn light_reaches(world: &World, from: Vec3, torch: Vec3) -> bool {
    if !crate::world::trace::opaque_between(world, from, torch) {
        return true;
    }
    let delta = torch - from;
    let mut side = delta.cross(Vec3::Y);
    if side.length_squared() < 1e-6 {
        side = delta.cross(Vec3::X);
    }
    if side.length_squared() < 1e-6 {
        return false;
    }
    let side = side.normalize() * world.voxel_scale.meters() * 0.95;
    let open = |p: Vec3| {
        let cell = (p / world.voxel_scale.meters()).floor().as_ivec3();
        !world.get(cell).is_opaque() && !crate::world::trace::opaque_between(world, p, torch)
    };
    open(from + side) || open(from - side)
}

/// Opaque cover of at least `ROOF_M` above `cell`: a cave or a roof.
fn roofed_cached(cache: &mut roof::Cache, world: &World, cell: IVec3) -> bool {
    let s = world.voxel_scale.meters();
    cache.covered(world, cell, (ROOF_SCAN_M / s).ceil() as u32, (ROOF_M / s).ceil() as u32)
}

#[cfg(test)]
fn roofed(world: &World, cell: IVec3) -> bool {
    roofed_cached(&mut roof::Cache::default(), world, cell)
}

#[cfg(test)]
#[path = "night_tests.rs"]
mod tests;
