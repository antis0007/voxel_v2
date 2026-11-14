//! Bounded grazing herds with saved identities, health and metre-space bodies.
use super::{Adventure, Enemy, World, bestiary, free, habitat, navigation};
use crate::world::{BlockId, chunk_coord_from_world_block};
use glam::{IVec3, Quat, Vec3};
use std::collections::BTreeMap;
mod arrival;
mod husbandry;
#[cfg(test)]
mod husbandry_tests;
mod model;
mod cave;
mod persistence;
#[cfg(test)]
mod tests;
pub(crate) mod vehicle;
pub(crate) use husbandry::{Herd, feed, service as husbandry, shear};
pub(super) use husbandry::fall;
pub(super) const SHEEP: u32 = 16;
pub(super) const GOAT: u32 = 17;
const SALAMANDER: u32 = 42;
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
pub(super) fn passive(kind: u32) -> bool {
    matches!(kind, SHEEP | GOAT | SALAMANDER | vehicle::BOAT) || super::fauna::is_kind(kind) || vehicle::is_villager(kind)
}
/// A neutral brawler resident that turns on whoever hurts it.
pub(super) fn fights_back(kind: u32) -> bool {
    passive(kind) && bestiary::species(kind).is_some_and(|s| s.neutral && s.brawler)
}
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
    let p = Vec3::new(x as f32 * PATCH_METERS + ox + slot as f32 * 2.0, 0.0, z as f32 * PATCH_METERS + oz);
    (h, p)
}

pub(super) fn spawn(a: &mut Adventure, world: &World, feet: Vec3) {
    a.wildlife.remember(&a.enemies);
    let mut count = a.enemies.iter().filter(|e| passive(e.kind)).count();
    for (&key, r) in &a.wildlife.residents {
        if count >= ACTIVE_LIMIT { break; }
        if r.health <= 0.0 || r.pos.distance_squared(feet) > 75.0 * 75.0
            || a.enemies.iter().any(|e| passive(e.kind) && id(e) == key) { continue; }
        if cave::loaded(world,r.pos) && free(world, r.pos, r.kind) {
            a.enemies.push(r.actor(key));
            count += 1;
        }
    }
    cave::spawn(a, world, feet, &mut count);
    let cx = (feet.x / PATCH_METERS).floor() as i32;
    let cz = (feet.z / PATCH_METERS).floor() as i32;
    let scale = world.voxel_scale.meters();
    let cfg = world.sample_config();
    let hell = cfg.hell_ecology();
    for z in cz-1..=cz+1 { for x in cx-1..=cx+1 { for slot in 0..2 {
        if count >= ACTIVE_LIMIT || a.wildlife.residents.len() >= RECORD_LIMIT { return; }
        let key = (x, z, slot);
        if a.wildlife.residents.contains_key(&key) { continue; }
        let (roll, mut p) = candidate(world.seed, x, z, slot);
        let horizontal = Vec3::new(feet.x, 0.0, feet.z).distance_squared(p);
        if !(12.0*12.0..=68.0*68.0).contains(&horizontal) { continue; }
        let cell = (p / scale).floor().as_ivec3();
        let lower = ((feet.y - 24.0) / scale).floor() as i32 - 2;
        let upper = ((feet.y + 24.0) / scale).floor() as i32;
        let mut chunk = chunk_coord_from_world_block(cell.with_y(lower));
        let top_chunk = chunk_coord_from_world_block(cell.with_y(upper)).y;
        // A candidate needs resident footing within the existing vertical
        // spawn range. Check that before asking worldgen for the column.
        if !(chunk.y..=top_chunk).any(|y| {
            chunk.y = y;
            world.chunks.contains_key(&chunk)
        }) { continue; }
        let ground = world.terrain_height_at(cell.x, cell.z);
        p.y = (ground + 1) as f32 * scale + 0.02;
        let ground_cell = cell.with_y(ground);
        if (p.y - feet.y).abs() > 24.0 || !cave::loaded(world,p)
            || !world.chunks.contains_key(&chunk_coord_from_world_block(ground_cell))
        { continue; }
        let substrate = world.get(ground_cell);
        let suitable = match engine_worldgen::nether::biome(&cfg, p.x, p.z) {
            Some(engine_worldgen::nether::Biome::FungalHollows) => matches!(substrate, BlockId::FungalSoil | BlockId::Hellstone),
            Some(_) => false,
            None if hell => matches!(substrate, BlockId::Basalt | BlockId::Hellstone),
            None => substrate == BlockId::Grass,
        };
        if !suitable { continue; }
        let ctx = habitat::Context::at(world, cell.with_y(ground + 1), false, false);
        let grazer = |k: u32| passive(k) && bestiary::species(k).is_some_and(|s| s.role == bestiary::Role::Grazer);
        let Some(kind) = habitat::pick(&ctx, roll >> 17, grazer)
            .or_else(|| habitat::pick_any_biome(&ctx, roll >> 17, grazer)) else { continue; };
        let kind = if hell { SALAMANDER } else { kind };
        if !free(world, p, kind) || !supported(world, p, 0.2) { continue; }
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

fn resident_footing(world: &World, p: Vec3, kind: u32, depth: f32) -> bool {
    let s = world.voxel_scale.meters();
    let (lo,hi)=super::body_bounds(p,kind,s);
    let centre=p/s;
    // Collision and support must share a footprint. Inset probes leave a
    // gap where the body hits a step before its leading foot can reach it.
    [(centre.x,centre.z),(lo.x,lo.z),(hi.x,hi.z),(lo.x,hi.z),(hi.x,lo.z)]
        .into_iter().any(|(x,z)| {
            (0..=(depth/s).ceil() as usize).any(|i| {
                let probe=(0.1+i as f32*s).min(depth);
                let point=Vec3::new(x,(p.y-probe)/s,z);
                let cell=point.floor().as_ivec3();
                world.solid(cell) && (engine_content::fittings::state(world.get(cell)).is_none()
                    || engine_content::fittings::boxes(world.get(cell),cell,s).iter()
                        .any(|&(a,b)|point.cmpge(a).all() && point.cmple(b).all()))
            })
        })
}

pub(super) fn tick(e: &mut Enemy, world: &World, feet: Vec3, time: f32, dt: f32, guard_goal: Option<Vec3>, navigation_budget: &mut usize) {
    if vehicle::tick(e, world, dt) { return; }
    if !cave::loaded(world,e.pos) { e.gait=0.0; return; }
    e.visible = false;
    e.tell = 0.0;
    e.slow = (e.slow - dt).max(0.0);
    let guarding = vehicle::villager_id(e).is_some_and(|id| world.travel.guard_posts.contains_key(&id));
    let danger = !guarding && ((feet.distance_squared(e.pos) < 4.0 * 4.0
        && !vehicle::is_villager(e.kind)
        && !bestiary::species(e.kind).is_some_and(|s| s.neutral)) || e.flash > 0.0);
    e.alert = if danger { 4.0 } else { (e.alert - dt).max(0.0) };
    let phase = time + (engine_core::hash::hash_u32(e.site.x as u32 ^ e.site.y as u32) % 100) as f32;
    let resident = vehicle::villager_id(e);
    let retreat = resident.is_some() && e.alert > 0.0;
    let daily_goal = if retreat { Some(e.home) } else { guard_goal.or_else(|| vehicle::daily_goal(world, e, feet)) };
    let route_speed = resident.filter(|_| !retreat)
        .and_then(|id| crate::world::farming::jobs::route_speed_multiplier(world, id));
    let job_route = route_speed.is_some();
    let stop = if job_route {
        0.08
    } else { 0.35 };
    let grazing = (e.alert <= 0.0 || retreat) && daily_goal.map_or(phase.rem_euclid(14.0) < 7.0,
        |goal| ((goal-e.pos)*Vec3::new(1.0,0.0,1.0)).length_squared() < stop * stop);
    let goal = if e.alert > 0.0 && !retreat {
        e.pos + (e.pos - feet).normalize_or_zero() * 6.0
    } else if let Some(goal) = daily_goal {
        goal
    } else {
        let angle = (phase / 14.0).floor() * 2.4 + e.slot as f32 * 0.5;
        e.home + Vec3::new(angle.cos(), 0.0, angle.sin()) * if e.kind == SALAMANDER { 2.0 } else { 5.0 }
    };
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    let heading = if let Some(id) = resident
        && !grazing && !job_route && up.dot(Vec3::Y) > 0.999
        && e.pos.distance_squared(goal) <= 18.0 * 18.0
    {
        e.navigation.heading(world, e.pos, goal, e.kind, dt, id, navigation_budget)
    } else if up.dot(Vec3::Y) > 0.999 {
        ((goal - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero()
    } else {
        let raw = goal - e.pos;
        (raw - up * raw.dot(up)).normalize_or_zero()
    };
    let speed = bestiary::species(e.kind).map_or(1.0, |s| s.speed)
        * route_speed.unwrap_or(1.0)
        * if e.alert > 0.0 { 2.6 } else { 1.0 }
        * if e.slow > 0.0 { 0.4 } else { 1.0 };
    // Unassigned grazers can reach a wander target before their walking window
    // ends. Stop at it instead of overshooting and reversing every fixed step.
    // Resident routes, guard goals and escape movement retain their own policy.
    let step = speed * dt.min(0.1);
    let distance = if resident.is_none() && daily_goal.is_none() && e.alert <= 0.0 {
        let delta = goal - e.pos;
        let remaining = (delta - up * delta.dot(up)).length();
        arrival::step(step, remaining, stop)
    } else { step };
    let before = e.pos;
    if !grazing && e.stagger <= 0.0 && distance > 0.0 {
        let steps = (distance / 0.12).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let mut advanced = false;
            for turn in [0.0, 0.7, -0.7, 1.3, -1.3] {
                let turn_q = if up.dot(Vec3::Y) > 0.999 { Quat::from_rotation_y(turn) } else { Quat::from_axis_angle(up, turn) };
                let q = e.pos + turn_q * heading * distance / steps as f32;
                let fits = if up.dot(Vec3::Y) > 0.999 {
                    free(world, q, e.kind) && (if vehicle::is_villager(e.kind) {
                        resident_footing(world,q,e.kind,1.1)
                    } else {supported(world, q, 1.1)})
                } else {
                    navigation::clear(world, q, e.kind, up) && navigation::on_floor(world, q, up)
                };
                if fits {
                    e.pos = q;
                    advanced = true;
                    break;
                }
                // One-metre terrain steps remain walkable at every voxel scale.
                for lift in [0.25, 0.5, 0.75, 1.0] {
                    let raised = q + up * lift;
                    let stepped = if up.dot(Vec3::Y) > 0.999 {
                        (if vehicle::is_villager(e.kind) { resident_footing(world, raised, e.kind, 0.2) }
                            else { supported(world, raised, 0.2) }) && free(world, raised, e.kind)
                            && (1..=4).all(|i| free(world, e.pos + Vec3::Y * lift * i as f32 / 4.0, e.kind))
                    } else {
                        navigation::on_floor(world, raised, up) && navigation::clear(world, raised, e.kind, up)
                            && (1..=4).all(|i| navigation::clear(world, e.pos + up * lift * i as f32 / 4.0, e.kind, up))
                    };
                    if stepped {
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
    let travel = (movement - up * movement.dot(up)).length();
    super::enemy_presentation::advance_walk(e, movement, speed, dt);
    let facing = if travel > 0.0001 { Some(movement) } else if !retreat {
        resident.and_then(|id| crate::world::farming::jobs::presentation(world, id).0)
            .map(|pose| pose.target() - e.pos).filter(|delta| delta.x.hypot(delta.z) > 0.02)
    } else { None };
    if let Some(direction) = facing {
        let yaw = super::local_yaw(up, direction);
        let delta = (yaw - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        e.yaw += delta.clamp(-dt * 5.0, dt * 5.0);
    }
    e.stagger = (e.stagger - dt).max(0.0);
}

pub(super) fn draw(e: &Enemy, m: &mut crate::mesh::models::Model, time: f32, world: &World) {
    if !vehicle::draw(e, m, time, world) { model::draw(e, m, time); }
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

