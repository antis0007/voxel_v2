//! Bounded grazing herds with saved identities, health and metre-space bodies.
use super::{Adventure, Enemy, World, bestiary, free};
use crate::world::BlockId;
use glam::{IVec3, Quat, Vec3};
use std::collections::BTreeMap;
mod husbandry;
#[cfg(test)]
mod husbandry_tests;
mod model;
mod persistence;
#[cfg(test)]
mod tests;
pub(crate) mod vehicle;
pub(crate) use husbandry::{Herd, feed, service as husbandry};
pub(super) use husbandry::fall;
pub(super) const SHEEP: u32 = 16;
pub(super) const GOAT: u32 = 17;
const ACTIVE_LIMIT: usize = 12;
const RECORD_LIMIT: usize = 4096;
const PATCH_METERS: f32 = 48.0;
type Id = (i32, i32, u32);

#[derive(Clone, Debug)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
struct Resident {
    kind: u32,
    home: Vec3,
    pos: Vec3,
    health: f32,
    yaw: f32,
}
#[derive(Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct State {
    residents: BTreeMap<Id, Resident>,
    /// Persisted in the farming sidecar, not the save body.
    pub(crate) herd: Herd,
}
pub(super) fn passive(kind: u32) -> bool { matches!(kind, SHEEP | GOAT | vehicle::VILLAGER | vehicle::BOAT) }
fn id(e: &Enemy) -> Id { (e.site.x, e.site.y, e.slot) }
impl Resident {
    fn actor(&self, id: Id) -> Enemy {
        let mut e = Enemy::new(IVec3::new(id.0, id.1, 6), id.2, self.kind, self.pos, 0);
        e.home = self.home;
        e.health = self.health;
        e.yaw = self.yaw;
        e
    }
}
impl State {
    pub(super) fn remember(&mut self, enemies: &[Enemy]) {
        for e in enemies.iter().filter(|e| passive(e.kind)) {
            if let Some(r) = self.residents.get_mut(&id(e)) {
                r.pos = e.pos;
                r.health = if e.pos.y <= -256.0 { 0.0 } else { e.health.max(0.0) };
                r.yaw = e.yaw;
            }
        }
    }
}

fn candidate(seed: u32, x: i32, z: i32, slot: u32) -> (u32, Vec3) {
    let h = engine_core::hash::hash_u32(seed ^ (x as u32).wrapping_mul(0x9e3779b9)
        ^ (z as u32).wrapping_mul(0x85ebca6b));
    let ox = 10.0 + (h & 255) as f32 / 255.0 * 24.0;
    let oz = 10.0 + ((h >> 8) & 255) as f32 / 255.0 * 24.0;
    (if h & 0x10000 == 0 { SHEEP } else { GOAT },
        Vec3::new(x as f32 * PATCH_METERS + ox + slot as f32 * 2.0,
            0.0, z as f32 * PATCH_METERS + oz))
}

pub(super) fn spawn(a: &mut Adventure, world: &World, feet: Vec3) {
    a.wildlife.remember(&a.enemies);
    let mut count = a.enemies.iter().filter(|e| passive(e.kind)).count();
    for (&key, r) in &a.wildlife.residents {
        if count >= ACTIVE_LIMIT { break; }
        if r.health <= 0.0 || r.pos.distance_squared(feet) > 75.0 * 75.0
            || a.enemies.iter().any(|e| passive(e.kind) && id(e) == key) { continue; }
        if free(world, r.pos, r.kind) {
            a.enemies.push(r.actor(key));
            count += 1;
        }
    }
    let cx = (feet.x / PATCH_METERS).floor() as i32;
    let cz = (feet.z / PATCH_METERS).floor() as i32;
    let scale = world.voxel_scale.meters();
    for z in cz-1..=cz+1 { for x in cx-1..=cx+1 { for slot in 0..2 {
        if count >= ACTIVE_LIMIT || a.wildlife.residents.len() >= RECORD_LIMIT { return; }
        let key = (x, z, slot);
        if a.wildlife.residents.contains_key(&key) { continue; }
        let (kind, mut p) = candidate(world.seed, x, z, slot);
        let horizontal = Vec3::new(feet.x, 0.0, feet.z).distance_squared(p);
        if !(12.0*12.0..=68.0*68.0).contains(&horizontal) { continue; }
        let cell = (p / scale).floor().as_ivec3();
        let ground = world.terrain_height_at(cell.x, cell.z);
        if !matches!(world.get(IVec3::new(cell.x, ground, cell.z)), BlockId::Grass) { continue; }
        p.y = (ground + 1) as f32 * scale + 0.02;
        if (p.y - feet.y).abs() > 24.0 || !free(world, p, kind) || !supported(world, p, 0.2) { continue; }
        let r = Resident { kind, home: p, pos: p, health: super::enemy_health(kind), yaw: 0.0 };
        a.enemies.push(r.actor(key));
        a.wildlife.residents.insert(key, r);
        count += 1;
    }}}
}

fn supported(world: &World, p: Vec3, depth: f32) -> bool {
    let s = world.voxel_scale.meters();
    for (x,z) in [(0.0,0.0),(-0.35,-0.35),(0.35,0.35),(-0.35,0.35),(0.35,-0.35)] {
        let mut found = false;
        let steps = (depth / s).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let probe = (0.1 + i as f32 * s).min(depth);
            let cell = ((p + Vec3::new(x, -probe, z)) / s).floor().as_ivec3();
            if matches!(world.get(cell), BlockId::Water | BlockId::Lava) { return false; }
            if world.solid(cell) {
                let point=(p+Vec3::new(x,-probe,z))/s;
                if engine_content::fittings::state(world.get(cell)).is_some()
                    && !engine_content::fittings::boxes(world.get(cell),cell,s).iter()
                        .any(|&(a,b)|point.cmpge(a).all() && point.cmple(b).all()) { continue; }
                found=true; break;
            }
        }
        if !found { return false; }
    }
    true
}

pub(super) fn tick(e: &mut Enemy, world: &World, feet: Vec3, time: f32, dt: f32) {
    if vehicle::tick(e, world, dt) { return; }
    e.visible = false;
    e.tell = 0.0;
    e.slow = (e.slow - dt).max(0.0);
    let danger = (feet.distance_squared(e.pos) < 4.0 * 4.0 && e.kind != vehicle::VILLAGER) || e.flash > 0.0;
    e.alert = if danger { 4.0 } else { (e.alert - dt).max(0.0) };
    let phase = time + (engine_core::hash::hash_u32(e.site.x as u32 ^ e.site.y as u32) % 100) as f32;
    let grazing = e.alert <= 0.0 && phase.rem_euclid(14.0) < 7.0;
    let goal = if e.alert > 0.0 {
        e.pos + (e.pos - feet).normalize_or_zero() * 6.0
    } else {
        let angle = (phase / 14.0).floor() * 2.4 + e.slot as f32 * 0.5;
        e.home + Vec3::new(angle.cos(), 0.0, angle.sin()) * 5.0
    };
    let heading = ((goal - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    let speed = bestiary::species(e.kind).map_or(1.0, |s| s.speed)
        * if e.alert > 0.0 { 2.6 } else { 1.0 }
        * if e.slow > 0.0 { 0.4 } else { 1.0 };
    let before = e.pos;
    if !grazing && e.stagger <= 0.0 {
        let distance = speed * dt.min(0.1);
        let steps = (distance / 0.12).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let mut advanced = false;
            for turn in [0.0, 0.7, -0.7, 1.3, -1.3] {
                let q = e.pos + Quat::from_rotation_y(turn) * heading * distance / steps as f32;
                if free(world, q, e.kind) && supported(world, q, 1.1) {
                    e.pos = q;
                    advanced = true;
                    break;
                }
                // One-metre terrain steps remain walkable at every voxel scale.
                for lift in [0.25, 0.5, 0.75, 1.0] {
                    let raised = q + Vec3::Y * lift;
                    if supported(world, raised, 0.2) && free(world, raised, e.kind)
                        && (1..=4).all(|i| free(world, e.pos + Vec3::Y * lift * i as f32 / 4.0, e.kind))
                    {
                        e.pos = raised;
                        advanced = true;
                        break;
                    }
                }
                if advanced { break; }
            }
            if !advanced { break; }
        }
    }
    let movement = e.pos - before;
    let travel = movement.length();
    e.gait += ((travel / dt.max(0.001) / speed.max(0.01)).clamp(0.0, 1.0) - e.gait) * (dt * 8.0).min(1.0);
    e.phase += travel * 4.0;
    if travel > 0.0001 {
        let yaw = (-movement.x).atan2(-movement.z);
        let delta = (yaw - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        e.yaw += delta.clamp(-dt * 5.0, dt * 5.0);
    }
    e.stagger = (e.stagger - dt).max(0.0);
}

pub(super) fn draw(e: &Enemy, m: &mut crate::mesh::models::Model, time: f32) {
    if !vehicle::draw(e, m, time) { model::draw(e, m, time); }
}

pub(super) fn observe_review(world: &World) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static LAST: AtomicU32 = AtomicU32::new(u32::MAX);
    let second = world.adventure.time as u32;
    if LAST.swap(second, Ordering::Relaxed) == second { return; }
    for e in world.adventure.enemies.iter().filter(|e| passive(e.kind)) {
        println!("[wildlife-review] t={second} kind={} pos_m={:?} health={} clear={} gait={} player_health={}",
            e.kind, e.pos, e.health, free(world, e.pos, e.kind), e.gait, world.adventure.health);
    }
}

