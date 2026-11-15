//! Terrain bounds, incremental residency selection, LOD bands, and frustum culling.
use super::columns::ColumnPlans;
use super::{ChunkCache, ChunkKey};
use crate::render::{RenderDistanceSettings, render_distance_settings};
use crate::world::{CHUNK_SIZE, WorldSampleConfig, WorldSnapshot, terrain_height};
use engine_core::coords::chunk_axis_from_world_block;
use glam::{IVec3, Mat4, Vec3, Vec4};
use std::cmp::Reverse;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
pub(super) type ColumnRange = (i32, i32, i32);
pub fn lod_for_chunk(center: IVec3, chunk: IVec3) -> u8 {
    lod_for_chunk_with_settings(center, chunk, render_distance_settings())
}

pub(super) fn lod_for_chunk_with_settings(center: IVec3, chunk: IVec3, settings: RenderDistanceSettings) -> u8 {
    let dx = (chunk.x - center.x).abs();
    let dz = (chunk.z - center.z).abs();
    // A terrain column shares one detail level, including its tree crowns.
    lod_at_distance(dx.max(dz), settings)
}

fn lod_at_distance(d: i32, settings: RenderDistanceSettings) -> u8 {
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
    if cfg.world_type == engine_voxel::WorldType::Manyfold {
        let bands = manyfold_chunk_bands(cfg, cx, cz);
        let mut min_cy = i32::MAX;
        let mut max_cy = i32::MIN;
        for (lo, hi) in bands {
            if lo <= hi {
                min_cy = min_cy.min(lo);
                max_cy = max_cy.max(hi);
            }
        }
        if min_cy > max_cy {
            return (1, 0, 1);
        }
        return (min_cy, max_cy, min_cy);
    }
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
    // Retain the legacy margin as a floor; actual overlapping crowns below
    // provide the upper bound for taller trees and neighbouring trunks.
    let tree_margin = cfg.blocks_for_span(if cfg.generator_version >= 5 {
        26.0
    } else {
        13.0
    }) + 1;
    let water_level = cfg.water_level();
    let mut effective_min = if min_h <= water_level {
        (min_h - cave_margin).min(water_level - CHUNK_SIZE)
    } else {
        min_h - cave_margin
    };
    // Ocean surface chunks are required even when the seabed is several chunks
    // lower (especially at quarter-metre voxel scales).
    // Landmark crowns rise above the ordinary tree margin.
    let lo = (ox, oz);
    let hi = (ox + CHUNK_SIZE - 1, oz + CHUNK_SIZE - 1);
    let structure_top = if cfg.world_type.realm() == Some(engine_voxel::RealmKind::Gloaming) {
        engine_worldgen::gloaming::tree_top_near(cfg, lo, hi)
    } else {
        engine_worldgen::giant_tree_top_near(cfg, lo, hi)
            .max(engine_worldgen::ruins::top_near(cfg, lo, hi))
            .max(crown_top_over_column(cfg, ox, oz))
    };
    let mut effective_max = (max_h + tree_margin)
        .max(water_level)
        .max(structure_top.unwrap_or(i32::MIN));
    if cfg.world_type.realm() == Some(engine_voxel::RealmKind::Highfold)
        && let Some((bottom, top)) = engine_worldgen::highfold::island_bounds(cfg, lo, hi)
    {
        effective_min = effective_min.min(bottom);
        effective_max = effective_max.max(top);
    }

    let min_cy =
        chunk_axis_from_world_block(effective_min).max(chunk_axis_from_world_block(cfg.min_world_y()));
    let max_cy = chunk_axis_from_world_block(effective_max)
        .min(chunk_axis_from_world_block(cfg.max_world_y() - 1));
    let surface_base = if min_h <= water_level {
        min_h.min(water_level)
    } else {
        min_h - cfg.blocks_for_span(8.0)
    };
    (min_cy, max_cy, chunk_axis_from_world_block(surface_base))
}

struct CrownBound {
    x: f32,
    z: f32,
    reach: f32,
    top: i32,
}

thread_local! {
    static TREE_BOUNDS: RefCell<(Option<WorldSampleConfig>, HashMap<(i32, i32), Option<CrownBound>>)> =
        RefCell::default();
}

/// Highest voxel of any tree crown overlapping the column at `(ox, oz)`.
/// The fixed tree margin sat below the tallest conifers (27 m trunks) and below
/// trunks rooted on bumps the 5x5 height sample missed, so their tops were cut.
fn crown_top_over_column(cfg: &WorldSampleConfig, ox: i32, oz: i32) -> Option<i32> {
    let m = cfg.voxel_meters();
    let cell = cfg.blocks_for_span(12.0);
    // Match structure_block's two-cell search around every voxel in the column.
    let cells = |o: i32| o.div_euclid(cell) - 2..=(o + CHUNK_SIZE - 1).div_euclid(cell) + 2;
    // Adjacent columns search many of the same placement cells. Cache misses
    // too, with a per-worker cap and a full reset on generator/config changes.
    TREE_BOUNDS.with_borrow_mut(|(cached_cfg, bounds)| {
        if *cached_cfg != Some(*cfg) || bounds.len() >= 16_384 {
            bounds.clear();
            *cached_cfg = Some(*cfg);
        }
        let mut top = None;
        for tz in cells(oz) {
            for tx in cells(ox) {
                let bound = bounds.entry((tx, tz)).or_insert_with(|| {
                    let tree = crate::world::tree_visual(cfg, tx, tz)?;
                    Some(CrownBound {
                        x: tree.base.x / m,
                        z: tree.base.z / m,
                        reach: tree.radius / m,
                        top: ((tree.base.y + tree.height + tree.crown_height) / m).ceil() as i32,
                    })
                });
                let Some(bound) = bound else { continue };
                if bound.x + bound.reach >= ox as f32
                    && bound.x - bound.reach <= (ox + CHUNK_SIZE) as f32
                    && bound.z + bound.reach >= oz as f32
                    && bound.z - bound.reach <= (oz + CHUNK_SIZE) as f32
                {
                    top = top.max(Some(bound.top));
                }
            }
        }
        top
    })
}

/// Surface/cave coverage plus a separate player-height band when underground.
/// Disjoint bands avoid generating an entire vertical mountain of unseen chunks.
fn manyfold_chunk_bands(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> [(i32, i32); 2] {
    let metres = cfg.voxel_meters();
    let floor = chunk_axis_from_world_block(cfg.min_world_y());
    let ceiling = chunk_axis_from_world_block(cfg.max_world_y() - 1);
    let mut bands = [(1, 0), (1, 0)];
    for (i, (lo_m, hi_m)) in engine_worldgen::manyfold::column_shell_bands(cfg, cx, cz)
        .into_iter()
        .enumerate()
    {
        if lo_m > hi_m {
            continue;
        }
        let lo = chunk_axis_from_world_block((lo_m / metres).floor() as i32).max(floor);
        let hi = chunk_axis_from_world_block((hi_m / metres).floor() as i32).min(ceiling);
        if lo <= hi {
            bands[i] = (lo, hi);
        }
    }
    if bands[0].0 <= bands[0].1 && bands[1].0 <= bands[1].1 && bands[0].1 + 1 >= bands[1].0 && bands[1].1 + 1 >= bands[0].0
    {
        bands = [(bands[0].0.min(bands[1].0), bands[0].1.max(bands[1].1)), (1, 0)];
    }
    bands
}

fn column_y_bands(
    range: (i32, i32, i32),
    chebyshev: i32,
    center: IVec3,
    cx: i32,
    cz: i32,
    cfg: &WorldSampleConfig,
    eye_underground: bool,
) -> [(i32, i32); 2] {
    if cfg.world_type == engine_voxel::WorldType::Manyfold {
        return manyfold_chunk_bands(cfg, cx, cz);
    }
    let (min_y, max_y, surface_min) = range;
    // Horizontal visibility must not discard the ocean/ground when flying high.
    let floor = chunk_axis_from_world_block(cfg.min_world_y());
    let ceiling = chunk_axis_from_world_block(cfg.max_world_y() - 1);
    let surface_floor = if chebyshev > 4 {
        surface_min.max(min_y)
    } else if chebyshev > 2 {
        (surface_min - 1).max(min_y)
    } else {
        min_y
    };
    let interior_realm = matches!(cfg.world_type, engine_voxel::WorldType::Nether) || cfg.world_type.is_interior();
    let underground = if (chebyshev <= 2 || eye_underground || interior_realm) && center.y <= surface_min + 1 {
        // Only a player underground needs distant cave floors and ceilings.
        // Keep a small local band at shallow entrances and near the player.
        let depth = render_distance_settings().view_distance_chunks.clamp(2, 8);
        ((center.y - depth).max(floor), (center.y + depth).min(ceiling))
    } else {
        (1, 0)
    };
    let surface = (surface_floor.max(floor), max_y.min(ceiling));
    if underground.0 <= underground.1 && underground.1 + 1 >= surface.0 && underground.0 <= surface.1 + 1 {
        [(surface.0.min(underground.0), surface.1.max(underground.1)), (1, 0)]
    } else {
        [surface, underground]
    }
}

pub(super) fn chunk_radial_distance_sq(center: IVec3, chunk: IVec3) -> i32 {
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
        dx * dx + dz * dz,
        dy,
        dx.max(dy).max(dz),
        dx + dy + dz,
    )
}

pub(super) fn pending_priority(
    center: IVec3,
    key: ChunkKey,
    urgent: bool,
) -> Reverse<(i32, i32, i32, i32, i32, i32, i32, u8)> {
    let dx = (key.coord.x - center.x).abs();
    let dy = (key.coord.y - center.y).abs();
    let dz = (key.coord.z - center.z).abs();
    let radial = dx * dx + dz * dz;
    Reverse((
        if urgent { i32::MIN.saturating_add(radial) } else { radial },
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
    let eye_underground = y_cache.eye_underground(center.y);
    let mut out = Vec::with_capacity((radius as usize * 2 + 1).pow(2) * 4);
    let mut seen = HashSet::new();
    let radius_sq = radius * radius;

    for dz in -radius..=radius {
        let extent = (radius_sq - dz * dz).isqrt();
        for dx in -extent..=extent {
            let cx = center.x + dx;
            let cz = center.z + dz;
            let Some(range) = y_cache.range((cx, cz)) else { continue };
            for key in keys_in_view(center, (cx, cz), range, &snapshot.cfg, eye_underground) {
                if seen.insert(key.coord) {
                    out.push(key);
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

pub(super) fn keys_in_view(
    center: IVec3,
    column: (i32, i32),
    range: ColumnRange,
    cfg: &WorldSampleConfig,
    eye_underground: bool,
) -> Vec<ChunkKey> {
    let above_top = center.y - range.1;
    let distance = (center.x - column.0).abs().max((center.z - column.1).abs());
    let settings = render_distance_settings();
    // The altitude policy sets the resident radius. A second cutoff in
    // vertical chunks drops the ground while it is still visible below.
    let (cx, cz) = column;
    let lod = lod_at_distance(distance, settings);
    let bands = column_y_bands(range, distance, center, cx, cz, cfg, eye_underground);
    let mut keys = Vec::new();
    for (min, max) in bands {
        for y in min..=max {
            keys.push(ChunkKey { coord: IVec3::new(cx, y, cz), lod });
        }
    }
    if distance <= 2 && above_top <= 2 {
        for y in center.y - 1..=center.y + 1 {
            if !bands.iter().any(|&(min, max)| min <= y && y <= max) {
                keys.push(ChunkKey { coord: IVec3::new(cx, y, cz), lod });
            }
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
    let eye_underground = y_cache.eye_underground(center.y);

    for dz in -max_radius..=max_radius {
        let outer = (max_radius_sq - dz * dz).isqrt();
        let inner = min_radius_sq - dz * dz;
        let offsets = if inner >= 0 {
            let first = inner.isqrt() + 1;
            [(-outer)..=(-first), first..=outer]
        } else {
            [(-outer)..=outer, 1..=0]
        };
        for dx in offsets.into_iter().flatten() {
            let cx = center.x + dx;
            let cz = center.z + dz;
            let Some(range) = y_cache.range((cx, cz)) else { continue };
            out.extend(keys_in_view(center, (cx, cz), range, &snapshot.cfg, eye_underground));
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

pub fn build_draw_list(cache: &ChunkCache, view_proj: Mat4) -> Vec<ChunkKey> {
    let distance = render_distance_settings().view_distance_chunks;
    let coverage = cache.resident_coverage(Vec3::ZERO, distance, 1.0);
    let mut out = Vec::new();
    build_draw_list_with_coverage(cache, view_proj, super::occlusion::eye_from(view_proj), &coverage, &mut DrawScratch::default(), &mut out);
    out
}

#[derive(Default)]
pub struct DrawScratch {
    occlusion: super::occlusion::Scratch,
    depth_keys: Vec<(f32, ChunkKey)>,
}

pub fn build_draw_list_with_coverage(
    cache: &ChunkCache,
    view_proj: Mat4,
    eye: Option<Vec3>,
    coverage: &crate::render::coverage::ResidentCoverage,
    scratch: &mut DrawScratch,
    out: &mut Vec<ChunkKey>,
) {
    let now = std::time::Instant::now();
    let planes = extract_planes(view_proj);
    let mut occlusion = super::occlusion::Occlusion::new(cache, eye, &mut scratch.occlusion);
    let keys = &mut scratch.depth_keys;
    keys.clear();
    let forward = Vec3::new(view_proj.x_axis.w, view_proj.y_axis.w, view_proj.z_axis.w);

    // Coverage admits complete surface bundles or full underground/proxy
    // bundles. The horizon and draw list use the same ownership decision.
    for (&(x, z), members) in &cache.published_columns {
        if !coverage.contains_column(x, z) {
            continue;
        }
        // Members are published in height order. Reject a tall column only
        // when every member sphere is outside the same frustum plane.
        if let [first, _, _, .., last] = members.as_slice() {
            let (bottom, radius) = chunk_bounds_sphere(*first);
            let (top, _) = chunk_bounds_sphere(*last);
            if planes[..5].iter().any(|plane| {
                let endpoint = if plane.y >= 0.0 { top } else { bottom };
                plane.truncate().dot(endpoint) + plane.w < -radius
            }) {
                continue;
            }
        }
        for &coord in members {
            let Some(mesh) = cache.gpu_meshes.get(&coord) else { continue; };
            let transitioning = (mesh.index_count == 0 || mesh.lod == 0)
                && mesh.outgoing_lod.is_some()
                && cache.lod_transition(coord, now).is_some();
            if mesh.index_count == 0 && !transitioning {
                continue;
            }

            let (center, radius) = chunk_bounds_sphere(coord);
            // Residency bounds horizontal reach. A far plane can otherwise
            // cull terrain that the resident column already masks in the horizon.
            let outside = planes[..5]
                .iter()
                .any(|plane| plane.truncate().dot(center) + plane.w < -radius);
            if outside {
                continue;
            }
            // The outgoing coarse mesh is still part of the visible handoff.
            if !transitioning && occlusion.hides(coord, mesh.lod) {
                continue;
            }

            keys.push((forward.dot(coord.as_vec3()), ChunkKey {
                coord,
                lod: mesh.lod,
            }));
        }
    }
    occlusion.publish();

    // Publication waits for a complete column so the horizon can safely
    // yield ownership. If that leaves the whole view empty, draw the nearby
    // uploaded pieces as temporary cover without claiming horizon ownership.
    if keys.is_empty() && cache.spawn_ready && cache.radius_cap != Some(0) {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let column = cache.center_chunk + IVec3::new(dx, 0, dz);
                if coverage.contains_column(column.x, column.z) {
                    continue;
                }
                for dy in -1..=1 {
                    let coord = column + IVec3::Y * dy;
                    let Some(mesh) = cache.gpu_meshes.get(&coord) else { continue };
                    if mesh.index_count == 0 {
                        continue;
                    }
                    let (center, radius) = chunk_bounds_sphere(coord);
                    if planes[..5].iter().any(|plane| {
                        plane.truncate().dot(center) + plane.w < -radius
                    }) {
                        continue;
                    }
                    keys.push((forward.dot(coord.as_vec3()), ChunkKey { coord, lod: mesh.lod }));
                }
            }
        }
    }

    // For a perspective camera clip W is forward depth. Submit nearer opaque
    // chunks first, rather than LOD/radial order used by the work scheduler.
    // Coordinate ties keep insertion/rehash order from changing overdraw.
    keys.sort_unstable_by(|(da, a), (db, b)| da.total_cmp(db)
        .then_with(|| (a.coord.x, a.coord.y, a.coord.z).cmp(&(b.coord.x, b.coord.y, b.coord.z))));
    out.clear();
    out.extend(keys.iter().map(|&(_, key)| key));
}

#[cfg(test)]
mod crown_coverage {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType};

    /// Every tree crown that reaches into a column must sit inside that column's
    /// resident Y range, or the chunk holding its top is never generated and the
    /// crown is sliced flat at a chunk boundary.
    fn uncovered_crowns(world_type: WorldType, scale: VoxelScalePreset) -> (usize, usize, i32) {
        let cfg = World::with_seed(world_type, scale, 7).sample_config();
        let m = cfg.voxel_meters();
        let cell = cfg.blocks_for_span(12.0);
        let (mut missed, mut checked, mut worst) = (0, 0, 0);
        for cz in -24..24 {
            for cx in -24..24 {
                let (_, max_cy, _) = column_y_chunk_range(cx, cz, &cfg);
                let top_allowed = (max_cy + 1) * CHUNK_SIZE;
                let (x0, z0) = (cx * CHUNK_SIZE, cz * CHUNK_SIZE);
                for tz in
                    z0.div_euclid(cell) - 2..=(z0 + CHUNK_SIZE - 1).div_euclid(cell) + 2
                {
                    for tx in
                        x0.div_euclid(cell) - 2..=(x0 + CHUNK_SIZE - 1).div_euclid(cell) + 2
                    {
                        let Some(tree) = crate::world::tree_visual(&cfg, tx, tz) else {
                            continue;
                        };
                        let reach = tree.radius;
                        let (bx, bz) = (tree.base.x / m, tree.base.z / m);
                        if bx + reach / m < x0 as f32
                            || bx - reach / m > (x0 + CHUNK_SIZE) as f32
                            || bz + reach / m < z0 as f32
                            || bz - reach / m > (z0 + CHUNK_SIZE) as f32
                        {
                            continue;
                        }
                        checked += 1;
                        let top =
                            ((tree.base.y + tree.height + tree.crown_height) / m).ceil() as i32;
                        if top >= top_allowed {
                            missed += 1;
                            worst = worst.max(top - top_allowed);
                        }
                    }
                }
            }
        }
        (missed, checked, worst)
    }

    #[test]
    fn resident_columns_cover_whole_tree_crowns() {
        for world_type in [WorldType::Hills, WorldType::Ridged] {
            for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
                let (missed, checked, worst) = uncovered_crowns(world_type, scale);
                println!(
                    "{world_type:?} {scale:?}: {missed}/{checked} crowns exceed range (worst +{worst} voxels)"
                );
                assert_eq!(missed, 0);
            }
        }
    }
}
