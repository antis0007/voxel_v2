//! Bounded fallback for the current opaque voxel renderer. Never generates or
//! loads a chunk. Portal views and illusion facades require their own adapter;
//! they must not be supplied through this ordinary-terrain path.
use super::knowledge::Knowledge;
use crate::{
    mesh::GeneratedChunk,
    player::Player,
    render::{ChunkCache, TorchLight},
    world::{self, BlockId, World},
};
use glam::{IVec3, Vec3};
use std::time::{Duration, Instant};

struct Work {
    started: Instant,
    cells: usize,
}
impl Work {
    fn available(&self) -> bool {
        self.cells > 0 && self.started.elapsed() < Duration::from_micros(650)
    }
}

fn aperture(view: &View<'_>, point: Vec3, scale: f32) -> bool {
    if view.world.world_type != world::WorldType::Manyfold
        || view.world.generator_version < engine_worldgen::manyfold::weave::VERSION
    {
        return false;
    }
    use engine_worldgen::manyfold::weave as layout;
    let (_, arc, height, z) = layout::coordinates(point);
    arc.abs() < layout::APERTURE_WIDTH + scale
        && (height - layout::FRAME_HEIGHT).abs() < layout::APERTURE_HEIGHT + scale
        && ((z - layout::TRAP_Z).abs() <= scale || (z - layout::GATE_Z).abs() <= scale)
}

fn chunk(cache: &ChunkCache, cell: IVec3) -> Option<&GeneratedChunk> {
    let coord = world::chunk_coord_from_world_block(cell);
    cache.observation_source(coord)
}

/// Exact cell traversal; a half-metre step could skip a thin quarter-metre wall.
fn ray(
    work: &mut Work,
    cache: &ChunkCache,
    origin: Vec3,
    direction: Vec3,
    range: f32,
    mut visit: impl FnMut(IVec3, BlockId, f32, IVec3) -> bool,
) {
    let mut cell = origin.floor().as_ivec3();
    let step = direction.signum().as_ivec3();
    let delta = Vec3::ONE / direction.abs();
    let boundary = cell.as_vec3()
        + Vec3::new(
            if step.x > 0 { 1.0 } else { 0.0 },
            if step.y > 0 { 1.0 } else { 0.0 },
            if step.z > 0 { 1.0 } else { 0.0 },
        );
    let mut next = Vec3::new(
        if direction.x == 0.0 {
            f32::INFINITY
        } else {
            (boundary.x - origin.x) / direction.x
        },
        if direction.y == 0.0 {
            f32::INFINITY
        } else {
            (boundary.y - origin.y) / direction.y
        },
        if direction.z == 0.0 {
            f32::INFINITY
        } else {
            (boundary.z - origin.z) / direction.z
        },
    );
    let mut distance = 0.0;
    let mut normal = IVec3::ZERO;
    for i in 0..512 {
        if work.cells == 0 || (i % 8 == 0 && !work.available()) {
            break;
        }
        work.cells -= 1;
        if distance > range {
            break;
        }
        let Some(source) = chunk(cache, cell) else {
            break;
        };
        if !visit(cell, source.get_world(cell), distance, normal) {
            break;
        }
        let axis = if next.x <= next.y && next.x <= next.z {
            0
        } else if next.y <= next.z {
            1
        } else {
            2
        };
        distance = next[axis];
        cell[axis] += step[axis];
        normal = IVec3::ZERO;
        normal[axis] = -step[axis];
        next[axis] += delta[axis];
    }
}

fn clear(work: &mut Work, view: &View<'_>, start: Vec3, end: Vec3, scale: f32) -> bool {
    let delta = end - start;
    let length = delta.length();
    if length < scale * 0.2 {
        return true;
    }
    let mut reached = false;
    let target = (end / scale).floor().as_ivec3();
    ray(
        work,
        view.cache,
        start / scale,
        delta / length,
        length / scale,
        |cell, b, t, _| {
            if aperture(view, start + delta / length * t * scale, scale) {
                return false;
            }
            if cell == target {
                reached = b.is_empty() || b.is_torch() || b.is_light_source();
                return false;
            }
            b.is_empty() || b.is_torch()
        },
    );
    reached
}

pub struct View<'a> {
    pub world: &'a World,
    pub player: &'a Player,
    pub cache: &'a ChunkCache,
    pub lights: &'a [TorchLight],
    pub held: Option<TorchLight>,
    pub daylight: bool,
    pub fov: f32,
    pub aspect: f32,
}

fn lit(work: &mut Work, view: &View<'_>, cell: IVec3, point: Vec3, scale: f32) -> bool {
    if !work.available() {
        return false;
    }
    let Some(source) = chunk(view.cache, cell) else {
        return false;
    };
    if source.get_world(cell).is_light_source() {
        return true;
    }
    if view.daylight
        && view.world.world_type != world::WorldType::Manyfold
        && !source.sky_ceiling.is_empty()
        && source.sky_visibility(point / scale, Vec3::Y) > 0.8
    {
        return true;
    }
    view.held
        .iter()
        .chain(view.lights.iter())
        .take(24)
        .any(|l| {
            let d = point.distance(l.position);
            l.strength > 0.1 && d < l.radius * 0.65 && clear(work, view, point, l.position, scale)
        })
}

pub fn capture(knowledge: &mut Knowledge, view: View<'_>, cursor: &mut usize) {
    let scale = view.world.voxel_scale.meters();
    let p = view.player;
    knowledge.enabled = view.world.rules.mode.interacts();
    knowledge.crossings(&p.map_crossings, &view.world.regions.view.root);
    knowledge.locate(
        p.pos * scale,
        p.up_dir(),
        p.forward_dir(),
        scale,
        p.on_ground,
    );
    if !knowledge.enabled || knowledge.active.is_none() {
        return;
    }
    let mut work = Work {
        started: Instant::now(),
        cells: 8192,
    };
    let eye = p.eye(view.world.voxel_scale);
    // A conservative cone inside the ordinary camera view; no omnidirectional reveal.
    // Sampling is interleaved across frames, not a synchronous GPU readback.
    let right = p.right_dir();
    let forward = p.forward_dir();
    let up = right.cross(forward).normalize_or_zero();
    for _ in 0..64 {
        if !work.available() {
            break;
        }
        let sample = *cursor % (24 * 16);
        *cursor = cursor.wrapping_add(137);
        let half_y = (view.fov * 0.5).tan().clamp(0.01, 2.0) * 0.95;
        let half_x = half_y * view.aspect.clamp(0.2, 5.0);
        let x = (((sample % 24) as f32 + 0.5) / 24.0 * 2.0 - 1.0) * half_x;
        let y = (((sample / 24) as f32 + 0.5) / 16.0 * 2.0 - 1.0) * half_y;
        let direction = (forward + right * x + up * y).normalize();
        let mut empty = Vec::new();
        let mut hit = None;
        ray(
            &mut work,
            view.cache,
            eye,
            direction,
            28.0 / scale,
            |cell, block, t, normal| {
                // These apertures can be misleading views. Do not ask their hidden
                // link/anchor state which is real, or read ordinary cells beyond one.
                if aperture(&view, (eye + direction * t) * scale, scale) {
                    return false;
                }
                if block.is_empty() || block.is_torch() {
                    if knowledge.remembers(cell) {
                        empty.push(cell);
                    }
                    true
                } else {
                    hit = Some((cell, block, t, normal));
                    false
                }
            },
        );
        if let Some((cell, block, t, normal)) = hit {
            let air = (eye + direction * (t - 0.02 / scale).max(0.0)) * scale;
            if lit(&mut work, &view, cell, air, scale) {
                let colour = block
                    .color(cell)
                    .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
                knowledge.observe(cell, colour, normal);
            }
        }
        for cell in empty {
            if !work.available() {
                break;
            }
            if lit(
                &mut work,
                &view,
                cell,
                (cell.as_vec3() + Vec3::splat(0.5)) * scale,
                scale,
            ) {
                knowledge.observed_empty(cell);
            }
        }
    }
}
