//! Terrain bounds, incremental residency selection, LOD bands, and frustum culling.
use super::columns::ColumnPlans;
use super::{ChunkCache, ChunkKey};
use crate::render::render_distance_settings;
use crate::world::{CHUNK_SIZE, WorldSampleConfig, WorldSnapshot, terrain_height};
use glam::{IVec3, Mat4, Vec3, Vec4};
use std::cmp::Reverse;
use std::collections::HashSet;
pub(super) type ColumnRange = (i32, i32, i32);
pub fn lod_for_chunk(center: IVec3, chunk: IVec3) -> u8 {
    let dx = (chunk.x - center.x).abs();
    let dz = (chunk.z - center.z).abs();
    // A terrain column shares one detail level, including its tree crowns.
    let d = dx.max(dz);
    let settings = render_distance_settings();

    if settings.lod_levels <= 1 || d <= settings.lod_start_chunks {
        0
    } else if settings.lod_levels == 2 {
        1
    } else if settings.lod_levels == 3 {
        let far_band = (settings.view_distance_chunks - settings.lod_start_chunks).max(2);
        let lod2_start = settings.lod_start_chunks + (far_band / 2).max(1);
        if d <= lod2_start { 1 } else { 2 }
    } else {
        // lod_levels >= 4: three LOD bands beyond lod_start_chunks
        let far_band = (settings.view_distance_chunks - settings.lod_start_chunks).max(3);
        let third = (far_band / 3).max(1);
        let lod2_start = settings.lod_start_chunks + third;
        let lod3_start = settings.lod_start_chunks + third * 2;
        if d <= lod2_start {
            1
        } else if d <= lod3_start {
            2
        } else {
            3
        }
    }
}

/// Compute the terrain-driven Y-chunk range that should be loaded for an XZ column.
/// Returns (min_cy, max_cy) inclusive chunk Y coords.
pub(super) fn column_y_chunk_range(cx: i32, cz: i32, cfg: &WorldSampleConfig) -> ColumnRange {
    // Sample a 5x5 grid of terrain heights within this column to find the Y extent.
    let ox = cx * CHUNK_SIZE;
    let oz = cz * CHUNK_SIZE;
    let mut min_h = i32::MAX;
    let mut max_h = i32::MIN;
    for dz in [0, 8, 16, 24, 31] {
        for dx in [0, 8, 16, 24, 31] {
            let h = terrain_height(cfg, ox + dx, oz + dz);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
    }
    if min_h == i32::MAX {
        min_h = cfg.min_world_y();
        max_h = cfg.max_world_y();
    }

    // Add margin for caves below and structures (trees) above.
    if cfg.generator_version >= 5 {
        // A crown can overhang from a higher neighbouring terrain column.
        let reach = cfg.blocks_for_span(10.0);
        for dz in [-reach, CHUNK_SIZE / 2, CHUNK_SIZE + reach] {
            for dx in [-reach, CHUNK_SIZE / 2, CHUNK_SIZE + reach] {
                max_h = max_h.max(terrain_height(cfg, ox + dx, oz + dz));
            }
        }
    }
    // This is terrain-driven only: do not force the player's Y chunk into every
    // XZ column, or flying high will generate a giant ring of empty air chunks.
    let cave_margin = CHUNK_SIZE * 2;
    // V5 crowns reach 24 m, including their leafy apex. Keep the complete
    // crown resident even when its trunk belongs to an adjacent column.
    let tree_margin = cfg.blocks_for_span(if cfg.generator_version >= 5 {
        26.0
    } else {
        13.0
    }) + 1;
    let water_level = cfg.water_level();
    let effective_min = (min_h - cave_margin).min(water_level - CHUNK_SIZE);
    // Ocean surface chunks are required even when the seabed is several chunks
    // lower (especially at quarter-metre voxel scales).
    let effective_max = (max_h + tree_margin).max(water_level);

    let min_cy =
        floor_div_i32(effective_min, CHUNK_SIZE).max(floor_div_i32(cfg.min_world_y(), CHUNK_SIZE));
    let max_cy = floor_div_i32(effective_max, CHUNK_SIZE)
        .min(floor_div_i32(cfg.max_world_y() - 1, CHUNK_SIZE));
    let surface_base = if min_h <= water_level {
        min_h.min(water_level)
    } else {
        min_h - cfg.blocks_for_span(8.0)
    };
    (
        min_cy,
        max_cy,
        floor_div_i32(surface_base, CHUNK_SIZE),
    )
}

/// Fast erosion-free estimate used while `ColumnPlans` is still evaluating
/// a newly uncovered column on its background worker threads.
fn preview_column_y_chunk_range(cx: i32, cz: i32, cfg: &WorldSampleConfig) -> ColumnRange {
    let ox = cx * CHUNK_SIZE;
    let oz = cz * CHUNK_SIZE;
    let mut min_h = i32::MAX;
    let mut max_h = i32::MIN;
    for (dx, dz) in [(0, 0), (31, 0), (0, 31), (31, 31), (16, 16)] {
        let (h, _) = crate::world::landscape::preview_surface(cfg, ox + dx, oz + dz);
        let hi = h as i32;
        min_h = min_h.min(hi);
        max_h = max_h.max(hi);
    }
    let water_level = cfg.water_level();
    let tree_margin = cfg.blocks_for_span(28.0) + 1;
    let effective_min = (min_h - CHUNK_SIZE * 2).min(water_level - CHUNK_SIZE);
    let effective_max = (max_h + tree_margin).max(water_level);
    let min_cy =
        floor_div_i32(effective_min, CHUNK_SIZE).max(floor_div_i32(cfg.min_world_y(), CHUNK_SIZE));
    let max_cy = floor_div_i32(effective_max, CHUNK_SIZE)
        .min(floor_div_i32(cfg.max_world_y() - 1, CHUNK_SIZE));
    let surface_base = if min_h <= water_level {
        min_h.min(water_level)
    } else {
        min_h - cfg.blocks_for_span(12.0)
    };
    (
        min_cy,
        max_cy,
        floor_div_i32(surface_base, CHUNK_SIZE),
    )
}

/// Surface/cave coverage plus a separate player-height band when underground.
/// Disjoint bands avoid generating an entire vertical mountain of unseen chunks.
fn column_y_bands(
    range: (i32, i32, i32),
    chebyshev: i32,
    center: IVec3,
    cfg: &WorldSampleConfig,
) -> [(i32, i32); 2] {
    let (min_y, max_y, surface_min) = range;
    // Horizontal visibility must not discard the ocean/ground when flying high.
    let floor = floor_div_i32(cfg.min_world_y(), CHUNK_SIZE);
    let ceiling = floor_div_i32(cfg.max_world_y() - 1, CHUNK_SIZE);
    let surface_floor = if chebyshev > 4 {
        surface_min.max(min_y)
    } else if chebyshev > 2 {
        (surface_min - 1).max(min_y)
    } else {
        min_y
    };
    let underground = if center.y <= surface_min {
        ((center.y - 1).max(floor), (center.y + 1).min(ceiling))
    } else {
        (1, 0)
    };
    [(surface_floor.max(floor), max_y.min(ceiling)), underground]
}
fn floor_div_i32(a: i32, b: i32) -> i32 {
    let d = a / b;
    if (a ^ b) < 0 && d * b != a { d - 1 } else { d }
}

fn chunk_radial_distance_sq(center: IVec3, chunk: IVec3) -> i32 {
    let dx = (chunk.x - center.x).abs();
    let dz = (chunk.z - center.z).abs();
    dx * dx + dz * dz
}

pub(super) fn chunk_sort_key(center: IVec3, key: ChunkKey) -> (u8, i32, i32, i32, i32) {
    let dx = (key.coord.x - center.x).abs();
    let dy = (key.coord.y - center.y).abs();
    let dz = (key.coord.z - center.z).abs();
    (
        key.lod,
        chunk_radial_distance_sq(center, key.coord),
        dy,
        dx.max(dy).max(dz),
        dx + dy + dz,
    )
}

pub(super) fn pending_priority(
    center: IVec3,
    key: ChunkKey,
) -> Reverse<(i32, i32, i32, i32, i32, i32, i32, u8)> {
    let dx = (key.coord.x - center.x).abs();
    let dy = (key.coord.y - center.y).abs();
    let dz = (key.coord.z - center.z).abs();
    Reverse((
        chunk_radial_distance_sq(center, key.coord),
        dy,
        key.lod as i32,
        dx.max(dy).max(dz),
        key.coord.x,
        key.coord.y,
        key.coord.z,
        key.lod,
    ))
}

pub fn desired_chunk_keys(center: IVec3) -> Vec<ChunkKey> {
    let mut out = Vec::new();

    // Snapshot-free fallback only.
    // Do NOT try to populate the whole distant horizon vertically from this path,
    // because without terrain sampling that recreates the high-altitude empty-air ring.
    //
    // This path should only provide a small local support volume until the snapshot-
    // aware path takes over.
    let local_air_radius = 2;
    let local_air_y_pad = 1;

    for dz in -local_air_radius..=local_air_radius {
        for dx in -local_air_radius..=local_air_radius {
            let cx = center.x + dx;
            let cz = center.z + dz;
            for cy in (center.y - local_air_y_pad)..=(center.y + local_air_y_pad) {
                let coord = IVec3::new(cx, cy, cz);
                let lod = lod_for_chunk(center, coord);
                out.push(ChunkKey { coord, lod });
            }
        }
    }

    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out.dedup();
    out
}
/// Build visible chunk keys using a cached column Y-range lookup.
/// `radius` limits how far from center we scan (for incremental expansion).
pub(super) fn desired_chunk_keys_with_snapshot_cached(
    center: IVec3,
    snapshot: &WorldSnapshot,
    y_cache: &mut ColumnPlans,
    radius: i32,
) -> Vec<ChunkKey> {
    y_cache.set_interest(
        (center.x, center.z),
        render_distance_settings().view_distance_chunks + 2,
    );
    y_cache.begin(snapshot.cfg);
    let mut out = Vec::with_capacity((radius as usize * 2 + 1).pow(2) * 4);
    let mut seen = HashSet::new();
    let radius_sq = radius * radius;

    let local_air_radius = 2;
    let local_air_y_pad = 1;

    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz > radius_sq {
                continue;
            }
            let cx = center.x + dx;
            let cz = center.z + dz;
            let range = y_cache
                .range((cx, cz))
                .unwrap_or_else(|| preview_column_y_chunk_range(cx, cz, &snapshot.cfg));
            for key in keys_for_column(center, (cx, cz), range, &snapshot.cfg) {
                if seen.insert(key.coord) {
                    out.push(key);
                }
            }

            if dx.abs() <= local_air_radius && dz.abs() <= local_air_radius {
                for cy in (center.y - local_air_y_pad)..=(center.y + local_air_y_pad) {
                    let coord = IVec3::new(cx, cy, cz);
                    if seen.insert(coord) {
                        let lod = lod_for_chunk(center, coord);
                        out.push(ChunkKey { coord, lod });
                    }
                }
            }
        }
    }

    for coord in snapshot.edited_coords_near(center, radius) {
        if chunk_radial_distance_sq(center, coord) <= radius_sq && seen.insert(coord) {
            out.push(ChunkKey {
                coord,
                lod: lod_for_chunk(center, coord),
            });
        }
    }
    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out
}

pub(super) fn keys_for_column(
    center: IVec3,
    c: (i32, i32),
    range: ColumnRange,
    cfg: &WorldSampleConfig,
) -> Vec<ChunkKey> {
    let (cx, cz) = c;
    let chebyshev = (cx - center.x).abs().max((cz - center.z).abs());
    let mut keys = Vec::new();
    for (min, max) in column_y_bands(range, chebyshev, center, cfg) {
        for y in min..=max {
            let coord = IVec3::new(cx, y, cz);
            keys.push(ChunkKey {
                coord,
                lod: lod_for_chunk(center, coord),
            });
        }
    }
    keys
}

/// Build chunk keys only for the annular ring between `min_radius` and
/// `max_radius` (inclusive, circular XZ distance from center). Used by
/// `expand_visible` to append only the newly uncovered ring each frame
/// instead of rebuilding the entire visible list from scratch.
pub(super) fn desired_chunk_keys_ring(
    center: IVec3,
    snapshot: &WorldSnapshot,
    y_cache: &mut ColumnPlans,
    min_radius: i32,
    max_radius: i32,
) -> Vec<ChunkKey> {
    let mut out = Vec::new();
    let min_radius_sq = min_radius * min_radius;
    let max_radius_sq = max_radius * max_radius;

    for dz in -max_radius..=max_radius {
        for dx in -max_radius..=max_radius {
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > max_radius_sq || dist_sq <= min_radius_sq {
                continue;
            }

            let cx = center.x + dx;
            let cz = center.z + dz;
            let chebyshev = dx.abs().max(dz.abs());
            let range = y_cache
                .range((cx, cz))
                .unwrap_or_else(|| preview_column_y_chunk_range(cx, cz, &snapshot.cfg));
            for (min_y, max_y) in column_y_bands(range, chebyshev, center, &snapshot.cfg) {
                for cy in min_y..=max_y {
                    let coord = IVec3::new(cx, cy, cz);
                    let lod = lod_for_chunk(center, coord);
                    out.push(ChunkKey { coord, lod });
                }
            }
        }
    }

    for coord in snapshot.edited_coords_near(center, max_radius) {
        let distance = chunk_radial_distance_sq(center, coord);
        if distance > min_radius_sq && distance <= max_radius_sq {
            out.push(ChunkKey {
                coord,
                lod: lod_for_chunk(center, coord),
            });
        }
    }
    let mut seen = HashSet::new();
    out.retain(|key| seen.insert(key.coord));
    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out
}

pub fn chunk_bounds_sphere(chunk: IVec3) -> (Vec3, f32) {
    let half = CHUNK_SIZE as f32 * 0.5;
    let center = Vec3::new(
        chunk.x as f32 * CHUNK_SIZE as f32 + half,
        chunk.y as f32 * CHUNK_SIZE as f32 + half,
        chunk.z as f32 * CHUNK_SIZE as f32 + half,
    );
    // Circumradius of a cube: half * sqrt(3)
    // Fixed-metre torches can extend beyond a quarter-metre owning voxel.
    let radius = half * 3.0_f32.sqrt() + 4.5;
    (center, radius)
}

pub fn extract_planes(view_proj: Mat4) -> [Vec4; 6] {
    let m = view_proj.to_cols_array_2d();
    let row0 = Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]);
    let row1 = Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]);
    let row2 = Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]);
    let row3 = Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]);

    let mut planes = [
        row3 + row0,
        row3 - row0,
        row3 + row1,
        row3 - row1,
        row2, // WebGPU clip depth is 0..w, not OpenGL's -w..w.
        row3 - row2,
    ];

    for plane in &mut planes {
        let normal = plane.truncate();
        let len = normal.length();
        if len > 0.0 {
            *plane /= len;
        }
    }

    planes
}

pub fn is_chunk_visible_frustum(chunk: IVec3, view_proj: Mat4) -> bool {
    let planes = extract_planes(view_proj);
    let (center, radius) = chunk_bounds_sphere(chunk);
    planes
        .iter()
        .all(|plane| plane.truncate().dot(center) + plane.w >= -radius)
}

pub fn build_draw_list(cache: &ChunkCache, view_proj: Mat4) -> Vec<ChunkKey> {
    let planes = extract_planes(view_proj);
    let mut out = Vec::new();

    let distance = render_distance_settings().view_distance_chunks;
    for (&coord, mesh) in &cache.gpu_meshes {
        // Retained meshes remain drawable while the desired set expands again.
        if chunk_radial_distance_sq(cache.center_chunk, coord) > distance * distance {
            continue;
        }
        if mesh.index_count == 0 {
            continue;
        }

        let (center, radius) = chunk_bounds_sphere(coord);
        let outside = planes
            .iter()
            .any(|plane| plane.truncate().dot(center) + plane.w < -radius);
        if outside {
            continue;
        }

        out.push(ChunkKey {
            coord,
            lod: mesh.lod,
        });
    }

    // Opaque terrain benefits from early depth rejection and deterministic order.
    out.sort_unstable_by_key(|key| chunk_sort_key(cache.center_chunk, *key));
    out
}
