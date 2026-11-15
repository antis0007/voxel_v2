//! Hurled ground: chunks of real terrain that creatures tear loose and throw.
//!
//! A thrower (the Stone Colossus) asks for a `heave`: the cells inside a small
//! sphere of ground are removed under the spell-carve guards and held, with
//! their blocks and colours, under the thrower's key. On `hurl` the held chunk
//! becomes a `Stone` on a ballistic arc aimed at where the player stood on
//! release; nothing steers it afterwards, so a sidestep dodges it. A stone that
//! meets the player shatters and hurts; one that meets the ground sets its
//! blocks back down as a rough pile, so terrain moves rather than vanishes.
//! Everything here is transient runtime state and never saved.
use super::{Adventure, Cue, World, sound_pitched};
use crate::mesh::models::Model;
use crate::world::{BlockId, chunk_coord_from_world_block};
use engine_core::hash::hash_u32;
use glam::{IVec3, Quat, Vec3};

/// A thrower's identity: its site and slot.
pub(super) type Key = (IVec3, u32);

/// Downward pull on a stone, m/s².
const GRAVITY: f32 = 16.0;
/// Most cells one chunk may hold.
const MAX_CELLS: usize = 27;
/// Stones older than this shatter where they are.
const MAX_AGE_S: f32 = 6.0;
/// Player capsule radius for a direct hit, metres.
const PLAYER_RADIUS_M: f32 = 0.4;
/// At most this many stones fly at once.
const MAX_STONES: usize = 8;
/// Material kind used for drawn chunks (the colossus' stone finish).
const STONE_KIND: f32 = 71.0;

#[derive(Clone, Debug)]
struct Cell {
    /// Cell offset from the chunk's centre cell.
    offset: IVec3,
    block: BlockId,
    color: [f32; 3],
}

#[derive(Clone, Debug)]
struct Held {
    key: Key,
    cells: Vec<Cell>,
}

#[derive(Clone, Debug)]
struct Stone {
    pos: Vec3,
    vel: Vec3,
    cells: Vec<Cell>,
    axis: Vec3,
    angle: f32,
    spin: f32,
    damage: f32,
    kind: u32,
    age: f32,
    /// Bounding radius in metres.
    reach: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    held: Vec<Held>,
    stones: Vec<Stone>,
}

fn unit(h: u32) -> f32 {
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

fn chunk_reach(cells: &[Cell], s: f32) -> f32 {
    let far = cells.iter().map(|c| c.offset.as_vec3().length()).fold(0.0f32, f32::max);
    (far + 0.87) * s
}

/// Fallback when no ground could be torn up: a plain boulder that shatters.
fn boulder_color() -> [f32; 3] {
    [0.5, 0.47, 0.43]
}

/// Tears up the ground inside `radius` metres of `at` and holds it for `key`.
pub(super) fn heave(adv: &mut Adventure, world: &mut World, key: Key, at: Vec3, radius: f32) {
    let s = world.voxel_scale.meters();
    let c = (at / s).floor().as_ivec3();
    let r = (radius / s).ceil().clamp(1.0, 3.0) as i32;
    let r_sq = ((radius / s).max(0.9)).powi(2);
    let mut offsets: Vec<IVec3> = Vec::new();
    for dy in -r..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                let o = IVec3::new(dx, dy, dz);
                if (o.length_squared() as f32) <= r_sq {
                    offsets.push(o);
                }
            }
        }
    }
    offsets.sort_unstable_by_key(|o| o.length_squared());
    let mut cells = Vec::new();
    for offset in offsets {
        if cells.len() >= MAX_CELLS {
            break;
        }
        let cell = c + offset;
        if !world.chunks.contains_key(&chunk_coord_from_world_block(cell)) {
            continue;
        }
        let old = world.get(cell);
        if !old.is_solid() || old.is_liquid() {
            continue;
        }
        let color = old.color(cell);
        if let Some(block) = adv.carve_cell_with(world, cell, false, false) {
            cells.push(Cell { offset, block, color });
        }
    }
    let color = cells.first().map_or(boulder_color(), |c| c.color);
    adv.spawn_voxel_burst(at + Vec3::Y * 0.3, color, 18, 3.4, 0.07);
    adv.hurled.held.retain(|h| h.key != key);
    adv.hurled.held.push(Held { key, cells });
}

/// Lets `key`'s held chunk fly from `from` so it lands on `to` after
/// `flight_s` seconds.
pub(super) fn hurl(adv: &mut Adventure, key: Key, from: Vec3, to: Vec3, flight_s: f32, damage: f32, kind: u32) {
    let cells = match adv.hurled.held.iter().position(|h| h.key == key) {
        Some(i) => adv.hurled.held.swap_remove(i).cells,
        None => Vec::new(),
    };
    if adv.hurled.stones.len() >= MAX_STONES {
        return;
    }
    let t = flight_s.max(0.2);
    let vel = (to - from) / t + Vec3::Y * (0.5 * GRAVITY * t);
    let k = hash_u32(key.1 ^ (key.0.x as u32).rotate_left(7) ^ (key.0.z as u32).rotate_left(19) ^ adv.time.to_bits());
    let axis = Vec3::new(unit(k) - 0.5, unit(k >> 6) - 0.5, unit(k >> 12) - 0.5).normalize_or(Vec3::X);
    // Reach in metres is rescaled at the first tick, once the scale is known.
    adv.hurled.stones.push(Stone { pos: from, vel, cells, axis, angle: 0.0, spin: 4.0 + unit(k >> 18) * 4.0, damage, kind, age: 0.0, reach: 0.0 });
}

/// Distance from `p` to the player's capsule (feet to eye).
fn capsule_distance(p: Vec3, feet: Vec3, eye: Vec3) -> f32 {
    let axis = eye - feet;
    let t = ((p - feet).dot(axis) / axis.length_squared().max(1e-4)).clamp(0.0, 1.0);
    p.distance(feet + axis * t)
}

/// Advances stones. Returns damage to the player, its direction and the
/// thrower's kind, the same shape the enemy loop accumulates.
pub(super) fn tick(adv: &mut Adventure, world: &mut World, feet: Vec3, eye: Vec3, dt: f32) -> (f32, Vec3, Option<u32>) {
    // A thrower that died or was despawned lets go: its chunk crumbles away
    // with the body's own death burst.
    let alive: Vec<Key> = adv.enemies.iter().filter(|e| e.health > 0.0).map(|e| (e.site, e.slot)).collect();
    adv.hurled.held.retain(|h| alive.contains(&h.key));
    let s = world.voxel_scale.meters();
    let mut damage = 0.0;
    let mut direction = Vec3::ZERO;
    let mut source = None;
    let mut stones = std::mem::take(&mut adv.hurled.stones);
    let mut kept = Vec::with_capacity(stones.len());
    for mut stone in stones.drain(..) {
        if stone.reach <= 0.0 {
            stone.reach = if stone.cells.is_empty() { 0.35 } else { chunk_reach(&stone.cells, s).min(1.2) };
        }
        stone.age += dt;
        stone.angle += stone.spin * dt;
        let steps = ((stone.vel.length() * dt) / (s * 0.5).max(0.05)).ceil().clamp(1.0, 24.0) as usize;
        let h = dt / steps as f32;
        let mut outcome = None;
        for _ in 0..steps {
            let previous = stone.pos;
            stone.vel.y -= GRAVITY * h;
            stone.pos += stone.vel * h;
            if world.rules.mode.visible_to_mobs()
                && capsule_distance(stone.pos, feet, eye) < stone.reach + PLAYER_RADIUS_M
            {
                outcome = Some((false, previous));
                break;
            }
            let cell = (stone.pos / s).floor().as_ivec3();
            if !world.chunks.contains_key(&chunk_coord_from_world_block(cell)) {
                // Flew off the resident world: let it go quietly.
                outcome = Some((true, previous));
                stone.cells.clear();
                break;
            }
            let b = world.get(cell);
            if b.is_solid() || b.is_liquid() {
                outcome = Some((true, previous));
                break;
            }
        }
        if outcome.is_none() && stone.age > MAX_AGE_S {
            outcome = Some((true, stone.pos));
        }
        match outcome {
            None => kept.push(stone),
            Some((false, at)) => {
                // Direct hit: the chunk bursts against the player.
                damage += stone.damage;
                direction += (at - eye).normalize_or_zero() * stone.damage;
                source = Some(stone.kind);
                shatter(adv, at, &stone);
                sound_pitched(&mut adv.sounds, Cue::Impact, Some(at), 1.0, 0.5);
            }
            Some((true, at)) => land(adv, world, at, &stone, feet, eye),
        }
    }
    adv.hurled.stones = kept;
    (damage, direction, source)
}

fn shatter(adv: &mut Adventure, at: Vec3, stone: &Stone) {
    for (i, cell) in stone.cells.iter().enumerate().take(10) {
        adv.spawn_voxel_burst(at, cell.color, 3, 4.0, 0.07 + (i % 3) as f32 * 0.02);
    }
    if stone.cells.is_empty() {
        adv.spawn_voxel_burst(at, boulder_color(), 12, 4.0, 0.08);
    }
}

/// Sets the chunk's blocks down around the landing cell, lowest first, in air
/// only and never inside the player; whatever cannot settle shatters.
fn land(adv: &mut Adventure, world: &mut World, at: Vec3, stone: &Stone, feet: Vec3, eye: Vec3) {
    let s = world.voxel_scale.meters();
    let base = (at / s).floor().as_ivec3();
    let mut cells = stone.cells.clone();
    cells.sort_by_key(|c| (c.offset.y, c.offset.length_squared()));
    let lowest = cells.first().map_or(0, |c| c.offset.y);
    let mut placed = 0usize;
    let mut leftover = Vec::new();
    for cell in &cells {
        let target = base + IVec3::new(cell.offset.x, cell.offset.y - lowest, cell.offset.z);
        let centre = (target.as_vec3() + Vec3::splat(0.5)) * s;
        let clear_of_player = capsule_distance(centre, feet, eye) > PLAYER_RADIUS_M + s * 0.87;
        if clear_of_player
            && world.chunks.contains_key(&chunk_coord_from_world_block(target))
            && adv.place_terrain_cell(world, target, cell.block)
        {
            placed += 1;
        } else {
            leftover.push(cell.clone());
        }
    }
    let color = cells.first().map_or(boulder_color(), |c| c.color);
    adv.spawn_voxel_burst(at + Vec3::Y * 0.2, color, 14, 3.6, 0.07);
    for cell in leftover.iter().take(8) {
        adv.spawn_voxel_burst(at, cell.color, 2, 3.0, 0.07);
    }
    if stone.cells.is_empty() {
        adv.spawn_voxel_burst(at, boulder_color(), 10, 3.6, 0.08);
    }
    let gain = if placed > 0 { 0.95 } else { 0.75 };
    sound_pitched(&mut adv.sounds, Cue::ForgeSlam, Some(at), gain, 0.5);
}

impl Adventure {
    /// Draws held and flying chunks as their own voxels.
    pub(crate) fn draw_hurled(&self, m: &mut Model, voxel_m: f32) {
        let s = voxel_m;
        for held in &self.hurled.held {
            let Some(e) = self.enemies.iter().find(|e| (e.site, e.slot) == held.key) else { continue };
            let Some(at) = super::field_foes::heave_hold(e) else { continue };
            draw_chunk(m, at, Quat::from_rotation_y(e.yaw), &held.cells, s);
        }
        for stone in &self.hurled.stones {
            draw_chunk(m, stone.pos, Quat::from_axis_angle(stone.axis, stone.angle), &stone.cells, s);
        }
    }
}

fn draw_chunk(m: &mut Model, at: Vec3, q: Quat, cells: &[Cell], s: f32) {
    if cells.is_empty() {
        m.material_box(at, Vec3::splat(0.7), q, boulder_color(), STONE_KIND, 0.0);
        return;
    }
    for cell in cells {
        let local = cell.offset.as_vec3() * s;
        m.material_box(at + q * local, Vec3::splat(s * 0.98), q, cell.color, STONE_KIND, 0.0);
    }
}
