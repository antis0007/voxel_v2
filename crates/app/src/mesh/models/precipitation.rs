//! Near-field weather geometry. The cloud pass supplies the distant curtains.
use super::Model;
use crate::mesh::PADDED_CHUNK_SIZE;
use crate::player::Player;
use crate::render::ChunkCache;
use crate::simulation::storms::{self, SNOW_BELOW_C};
use crate::world::{
    BlockId, CHUNK_SIZE, DayNightCycle, World, WorldType, chunk_coord_from_world_block,
};
use engine_core::hash::hash_u32;
use glam::{DVec2, IVec3, Vec3, Vec3Swizzles};

const TILE_M: f32 = 4.0;
const RADIUS_M: f32 = 16.0;
const COLUMN_M: f32 = 16.0;
const ROOF_SCAN_M: f32 = 24.0;

pub(super) fn draw(
    mesh: &mut Model,
    world: &World,
    player: &Player,
    day: &DayNightCycle,
    chunks: &ChunkCache,
) {
    if world.ca_view.is_some()
        || matches!(
            world.world_type,
            WorldType::Nether | WorldType::Manyfold | WorldType::Hall | WorldType::Realm(_)
        )
    {
        return;
    }

    let scale = world.voxel_scale.meters();
    let eye_voxels = player.precise_eye(world.voxel_scale);
    let eye = (eye_voxels * f64::from(scale)).as_vec3();
    let controls = day.cloud_controls();
    // Fade out at the upper edge of the distant precipitation volume.
    let ceiling =
        600.0 + (80.0 + 200.0 * controls.base_height.clamp(0.0, 1.0)) * 0.85 - 2.0 + 220.0;
    let altitude = ((ceiling - eye.y) / 40.0).clamp(0.0, 1.0);
    if altitude <= 0.0 {
        return;
    }
    let amount = storms::precipitation(storms::local_cumulonimbus(day, Some(world.sample_config()), eye.xz()));
    if amount <= 0.0 {
        return;
    }

    let cold = world
        .sandbox
        .cell(eye_voxels.floor().as_ivec3())
        .temperature
        < SNOW_BELOW_C;
    // Wind and drift come from the fixed-step air at the player (`Glide::wind`,
    // `Glide::drift_m`), so streaks slant with the real wind and the field
    // never jumps when the weather turns.
    let ground = &engine_content::climate::storm_dials().ground;
    let share = if cold { ground.snow_drift } else { ground.rain_drift } as f32;
    let air = player.glide.wind;
    let wind = Vec3::new(air.x, 0.0, air.z) * share;
    let time = world.adventure.time;
    let drift = glam::DVec3::new(player.glide.drift_m.x, 0.0, player.glide.drift_m.y) * f64::from(share);
    let grid_eye = eye_voxels * f64::from(scale) - drift;
    let tile = (DVec2::new(grid_eye.x, grid_eye.z) / f64::from(TILE_M))
        .floor()
        .as_ivec2();
    let scan = (ROOF_SCAN_M / scale).ceil() as i32;
    let inv_scale = scale.recip();

    // Hash fixed world tiles so particles keep their paths as the camera moves.
    let previous_alpha = mesh.alpha;
    for z in tile.y - 5..=tile.y + 5 {
        for x in tile.x - 5..=tile.x + 5 {
            let seed = hash_u32(world.seed ^ hash_u32(x as u32) ^ (z as u32).rotate_left(16));
            for i in 0..4 {
                let seed = hash_u32(seed ^ i);
                let density = ((amount - unit(seed)) * 6.0).clamp(0.0, 1.0);
                if density <= 0.0 {
                    continue;
                }
                let speed = if cold {
                    0.65 + unit(seed ^ 1) * 0.75
                } else {
                    9.0 + unit(seed ^ 1) * 5.0
                };
                let fall =
                    f64::from(unit(seed ^ 2) * COLUMN_M) - f64::from(time) * f64::from(speed);
                let y = eye.y
                    + (fall - f64::from(eye.y) + f64::from(COLUMN_M * 0.5))
                        .rem_euclid(f64::from(COLUMN_M)) as f32
                    - COLUMN_M * 0.5;
                let vertical_fade = ((COLUMN_M * 0.5 - (y - eye.y).abs()) / 2.0).clamp(0.0, 1.0);
                if vertical_fade <= 0.01 {
                    continue;
                }
                let sway = if cold {
                    let phase = time * 2.0 + unit(seed ^ 3) * std::f32::consts::TAU;
                    Vec3::new(phase.sin(), 0.0, (phase * 0.75).cos()) * 0.32
                } else {
                    Vec3::ZERO
                };
                let pos = Vec3::new(
                    ((f64::from(x) + f64::from(unit(seed ^ 4))) * f64::from(TILE_M) + drift.x)
                        as f32,
                    y,
                    ((f64::from(z) + f64::from(unit(seed ^ 5))) * f64::from(TILE_M) + drift.z)
                        as f32,
                ) + sway;
                let to_eye = eye - pos;
                let distance_squared = to_eye.length_squared();
                if !(0.25..RADIUS_M * RADIUS_M).contains(&distance_squared) {
                    continue;
                }
                let distance = distance_squared.sqrt();
                let fade = ((RADIUS_M - distance) / 4.0).clamp(0.0, 1.0)
                    * vertical_fade
                    * ((distance - 0.5) / 1.2).clamp(0.0, 1.0)
                    * density
                    * altitude;
                if fade <= 0.01 || !exposed(chunks, (pos * inv_scale).floor().as_ivec3(), scan) {
                    continue;
                }

                let facing = to_eye / distance;
                if cold {
                    let radius = 0.023 + unit(seed ^ 6) * 0.027;
                    let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
                    let up = facing.cross(right) * radius;
                    let side = right * radius;
                    let color = [0.82, 0.89, 0.96];
                    mesh.alpha = 0.88 * fade;
                    mesh.triangle([pos + up, pos - side, pos + side], facing, color, 0.16);
                    mesh.triangle([pos - up, pos + side, pos - side], facing, color, 0.16);
                } else {
                    let axis = (Vec3::Y * speed - wind) * 0.0115;
                    let side = axis.cross(facing).normalize_or_zero() * 0.0065;
                    let top = pos + axis;
                    let bottom = pos - axis;
                    let color = [0.48, 0.62, 0.72];
                    mesh.alpha = 0.60 * fade;
                    mesh.triangle(
                        [top - side, bottom - side, bottom + side],
                        facing,
                        color,
                        0.13,
                    );
                    mesh.triangle([top - side, bottom + side, top + side], facing, color, 0.13);
                }
            }
        }
    }
    mesh.alpha = previous_alpha;
}

fn unit(seed: u32) -> f32 {
    (hash_u32(seed) >> 8) as f32 / 16_777_216.0
}

fn exposed(chunks: &ChunkCache, mut p: IVec3, scan: i32) -> bool {
    let coord = chunk_coord_from_world_block(p);
    let Some(source) = chunks.cpu_chunks.get(&coord) else {
        return false;
    };
    let local = p - source.origin();
    let column = (local.z * PADDED_CHUNK_SIZE + local.x) as usize;
    // Generated terrain can cover a cave even when the chunks above are absent.
    if source
        .sky_ceiling
        .get(column)
        .is_some_and(|&roof| roof >= p.y)
    {
        return false;
    }
    // Check streamed blocks too, including transparent roofs and water.
    let top = p.y + scan;
    while p.y <= top {
        let coord = chunk_coord_from_world_block(p);
        let end = ((coord.y + 1) * CHUNK_SIZE).min(top + 1);
        if let Some(source) = chunks.cpu_chunks.get(&coord) {
            let roof = source.weather_roof(p);
            if (p.y..end).contains(&roof) {
                return false;
            }
            // A roof above the scan limit can hide another roof inside it.
            while roof >= end && p.y < end {
                let block = source.get_world(p);
                if block.is_solid() || matches!(block, BlockId::Water | BlockId::Lava) {
                    return false;
                }
                p.y += 1;
            }
        }
        p.y = end;
    }
    true
}
