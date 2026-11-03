//! Fine smooth distant terrain in nested rings around the backdrop centre.
//!
//! Inside the outer square the coarse lattice is cut away and replaced by rings
//! sampled from the same `distant_surface` source the chunks use, at the cell
//! sizes of F4 in `docs/superpowers/specs/2026-10-25-far-field-and-water-design.md`
//! (outer radius over 384): 4 m to 1,536 m, 8 m to 3,072 m, 16 m to 6,144 m.
//! Each ring is cut into tiles. Under a fixed triangle budget the tile with the
//! largest on-screen height error is refined first (`tile.rs`), so ridges and
//! peaks get every knot while flat ground keeps a few quads. Normals always come
//! from the full-resolution knots.
//! Skirts hang under tile edges whose neighbour is drawn at another resolution
//! and under ring edges. Tiles sit on the world lattice and the centre is
//! snapped to 512 m, so recentring never moves an existing vertex.
mod cut;
mod tile;

use crate::mesh::{ChunkMesh, Vertex3};
use super::clip::{ChartClip, within};
use crate::world::{BlockId, WorldSampleConfig, distant_surface};
use glam::{IVec3, Vec3};
use std::sync::atomic::AtomicBool;

/// (half width, cell) in metres, inner ring first. Every ring is `TILES` tiles
/// across, and each tile width divides the 512 m centre snap.
const RINGS: [(f32, f32); 3] = [(1536.0, 4.0), (3072.0, 8.0), (6144.0, 16.0)];
const TILES: usize = 24;
/// Half width of the replaced square, in metres. Must match
/// `FINE_RING_HALF_M` in `world_shader.wgsl`.
pub(super) const HALF_M: f32 = RINGS[RINGS.len() - 1].0;
/// Minimum skirt depth below a tile edge, in metres.
const SKIRT_M: f32 = 8.0;
/// Reference projection of the F4 cell table: 1440p, 60 degree vertical field.
const FOCAL_PX: f32 = 1247.0;
/// Tiles stop refining once their height error is under this, in reference pixels.
const ERROR_PX: f32 = 0.5;
/// Surface triangles the rings may use: about twice the 8 m grid they replaced,
/// so vegetation keeps its share of `MAX_MESH_BYTES`. Integer heights carry 1-2 m
/// relief everywhere, so a pure error bound kept nearly every tile at full
/// resolution and pushed 69,780 trees out of the budget.
const FINE_TRIANGLES: usize = 1_200_000;
/// The camera may sit up to one centre snap away from the build centre.
const CENTRE_SLACK_M: f32 = 512.0;
/// Knots along a sampled tile edge, including one border knot on each side
/// for central-difference normals.
const BORDERED: usize = tile::SIDE + 2;

/// `VOXEL_HORIZON_SMOOTH=1` keeps the old lattice backdrop, for A/B captures.
fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("VOXEL_HORIZON_SMOOTH").is_none())
}

/// Reference pixels per voxel of height error for a tile whose nearest point is
/// `distance_m` from the build centre, seen from the nearest the camera can be.
fn pixels_per_voxel(distance_m: f32, scale: f32) -> f32 {
    scale * FOCAL_PX / (distance_m - CENTRE_SLACK_M).max(256.0)
}

/// Whether tile (tx, tz) of `ring` belongs to it rather than to the ring inside.
fn in_ring(ring: usize, tx: usize, tz: usize) -> bool {
    if ring == 0 {
        return true;
    }
    let hole = (RINGS[ring].0 - RINGS[ring - 1].0) / (tile::CELLS as f32 * RINGS[ring].1);
    let lo = hole as usize;
    let inside = |t: usize| t >= lo && t < TILES - lo;
    !(inside(tx) && inside(tz))
}

/// One skirt segment under the tile edge `top`, facing `outward`. Reuses the
/// edge vertices and adds the two dropped ones, which keep the terrain normal
/// so a crack shows ground shading.
fn push_skirt(mesh: &mut ChunkMesh, top: [u32; 2], outward: Vec3, drop: f32) {
    let base = mesh.vertices.len() as u32;
    for index in [top[1], top[0]] {
        let v = mesh.vertices[index as usize];
        mesh.vertices.push(Vertex3 { pos: [v.pos[0], v.pos[1] - drop, v.pos[2]], ..v });
    }
    let quad = [top[0], top[1], base, base + 1];
    let p = quad.map(|i| Vec3::from_array(mesh.vertices[i as usize].pos));
    let order = if (p[1] - p[0]).cross(p[2] - p[0]).dot(outward) >= 0.0 {
        [0, 1, 2, 0, 2, 3]
    } else {
        [0, 3, 2, 0, 2, 1]
    };
    mesh.indices.extend(order.map(|n| quad[n]));
}

/// One tile's samples on the bordered knot grid, its height error at every
/// level in voxels, and its distance from the build centre.
struct Sampled {
    heights: Vec<f32>,
    blocks: Vec<BlockId>,
    errors: [f32; tile::LEVELS],
    distance_m: f32,
}

/// Where a tile sits: its ring and its index along each axis of that ring.
#[derive(Clone, Copy)]
struct Place {
    ring: usize,
    tx: usize,
    tz: usize,
}

impl Place {
    /// World metres of the tile's (0, 0) knot.
    fn origin(self, center: (f32, f32)) -> (f32, f32) {
        let (half, cell) = RINGS[self.ring];
        let width = tile::CELLS as f32 * cell;
        (center.0 - half + self.tx as f32 * width, center.1 - half + self.tz as f32 * width)
    }
}

/// Surface height in voxels and block at voxel column (x, z); NaN outside the globe face `clip`.
pub(super) fn clipped_surface(cfg: &WorldSampleConfig, clip: Option<ChartClip>, x: i32, z: i32) -> (f32, BlockId) {
    let scale = f64::from(cfg.voxel_meters());
    if clip.is_some_and(|c| !within(c, f64::from(x) * scale, f64::from(z) * scale)) {
        return (f32::NAN, BlockId::Air);
    }
    distant_surface(cfg, x, z)
}

/// Whether every knot of the place, border included, lies inside `clip`.
fn place_within(place: Place, center: (f32, f32), clip: Option<ChartClip>) -> bool {
    let Some(c) = clip else { return true };
    let cell = RINGS[place.ring].1;
    let (x0, z0) = place.origin(center);
    let far = BORDERED as f32 - 2.0;
    [(-1.0, -1.0), (far, far)].iter().all(|&(a, b)| within(c, f64::from(x0 + a * cell), f64::from(z0 + b * cell)))
}

fn sample(cfg: &WorldSampleConfig, clip: Option<ChartClip>, place: Place, center: (f32, f32), scale: f32) -> Option<Sampled> {
    let cell = RINGS[place.ring].1;
    let (x0, z0) = place.origin(center);
    let mut heights = Vec::with_capacity(BORDERED * BORDERED);
    let mut blocks = Vec::with_capacity(tile::SIDE * tile::SIDE);
    for k in 0..BORDERED {
        let z = ((z0 + (k as f32 - 1.0) * cell) / scale).floor() as i32;
        for i in 0..BORDERED {
            let x = ((x0 + (i as f32 - 1.0) * cell) / scale).floor() as i32;
            let (h, block) = clipped_surface(cfg, clip, x, z);
            if !h.is_finite() {
                return None;
            }
            heights.push(h);
            if (1..=tile::SIDE).contains(&i) && (1..=tile::SIDE).contains(&k) {
                blocks.push(block);
            }
        }
    }
    // Relief under the far water sheet is hidden, and every triangle below it
    // is also copied into the sheet (`water::append`), so measure error on the
    // surface as drawn: sea bed clamped up to the sea.
    let sea = cfg.water_level() as f32 + 1.0;
    let inner: Vec<f32> = (0..tile::SIDE * tile::SIDE)
        .map(|n| heights[(n / tile::SIDE + 1) * BORDERED + n % tile::SIDE + 1].max(sea))
        .collect();
    let width = tile::CELLS as f32 * cell;
    let (dx, dz) = (
        (center.0 - x0).clamp(0.0, width) + x0 - center.0,
        (center.1 - z0).clamp(0.0, width) + z0 - center.1,
    );
    let errors = tile::level_errors(&inner);
    Some(Sampled { heights, blocks, errors, distance_m: dx.hypot(dz) })
}

/// Unit normal at inner knot (i, k) from its four bordered neighbours, in voxel
/// units, where `step` is the knot spacing in voxels.
fn normal_at(heights: &[f32], i: usize, k: usize, step: f32) -> Vec3 {
    let at = |i: usize, k: usize| heights[k * BORDERED + i];
    let (bi, bk) = (i + 1, k + 1);
    let dx = (at(bi + 1, bk) - at(bi - 1, bk)) / (2.0 * step);
    let dz = (at(bi, bk + 1) - at(bi, bk - 1)) / (2.0 * step);
    Vec3::new(-dx, 1.0, -dz).try_normalize().unwrap_or(Vec3::Y)
}

/// How one tile is drawn: its level, which edges (south, north, west, east)
/// get a skirt, and the skirt depth in voxels.
struct Drawn {
    level: u32,
    skirted: [bool; 4],
    drop: f32,
}

fn push_tile(mesh: &mut ChunkMesh, place: Place, center: (f32, f32), scale: f32, s: &Sampled, drawn: Drawn) {
    let cell = RINGS[place.ring].1;
    let (x0, z0) = place.origin(center);
    let q = 1usize << drawn.level;
    let m = tile::CELLS / q;
    let base = mesh.vertices.len();
    for k in (0..tile::SIDE).step_by(q) {
        for i in (0..tile::SIDE).step_by(q) {
            let x = (x0 + i as f32 * cell) / scale;
            let z = (z0 + k as f32 * cell) / scale;
            let h = s.heights[(k + 1) * BORDERED + i + 1];
            let block = s.blocks[k * tile::SIDE + i];
            let material = block.surface();
            mesh.vertices.push(Vertex3 {
                pos: [x, h, z],
                color: block.color(IVec3::new(x.floor() as i32, h as i32, z.floor() as i32)),
                normal: normal_at(&s.heights, i, k, cell / scale).to_array(),
                surface: [material.roughness, material.specular, 0.0, material.shader_key()],
                ao_strength: -1.0,
            });
        }
    }
    let at = |i: usize, k: usize| (base + k * (m + 1) + i) as u32;
    // Two triangles per quad, split along the diagonal `tile::level_error` uses.
    for k in 0..m {
        for i in 0..m {
            let (a, b, c, d) = (at(i, k), at(i + 1, k), at(i, k + 1), at(i + 1, k + 1));
            mesh.indices.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    for n in 0..m {
        let edges = [
            ([at(n, 0), at(n + 1, 0)], -Vec3::Z),
            ([at(n + 1, m), at(n, m)], Vec3::Z),
            ([at(0, n + 1), at(0, n)], -Vec3::X),
            ([at(m, n), at(m, n + 1)], Vec3::X),
        ];
        for ((top, normal), skirt) in edges.into_iter().zip(drawn.skirted) {
            if skirt {
                push_skirt(mesh, top, normal, drawn.drop);
            }
        }
    }
}

/// Replace the lattice backdrop inside the outer square with the fine rings.
/// Runs on the base lattice before water and vegetation are appended.
pub(super) fn apply(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
    clip: Option<ChartClip>,
) -> Option<()> {
    if !enabled() {
        return Some(());
    }
    let scale = cfg.voxel_meters();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let center = (center.0 as f32, center.1 as f32);
    cut::cut_square(
        mesh,
        [(center.0 - HALF_M) / scale, (center.1 - HALF_M) / scale],
        [(center.0 + HALF_M) / scale, (center.1 + HALF_M) / scale],
        cancel,
    )?;

    // Row order keeps neighbouring tiles on one worker for the column cache.
    // Each tile is a pure function of its place, so the result matches a serial pass.
    let places: Vec<Place> = (0..RINGS.len())
        .flat_map(|ring| (0..TILES).flat_map(move |tz| (0..TILES).map(move |tx| Place { ring, tx, tz })))
        .filter(|p| in_ring(p.ring, p.tx, p.tz))
        // Past the face edge the globe draws the ground; a clipped knot would abort the pass.
        .filter(|p| place_within(*p, center, clip))
        .collect();
    let sampled = super::parallel::map_blocks(&places, 4, super::parallel::workers(8), cancel, |&p| {
        sample(cfg, clip, p, center, scale)
    })?;

    // Heights are whole voxels, so every tile shows a voxel of stair error that a
    // smooth surface is meant to drop. Only relief beyond it competes for budget.
    let errors_px: Vec<[f32; tile::LEVELS]> = sampled
        .iter()
        .map(|s| s.errors.map(|e| (e - 1.0).max(0.0) * pixels_per_voxel(s.distance_m, scale)))
        .collect();
    let chosen = tile::allocate(&errors_px, ERROR_PX, FINE_TRIANGLES);

    let mut levels = vec![None; RINGS.len() * TILES * TILES];
    // Worst remaining height error per ring, in voxels. A T-junction crack is at
    // most the two tiles' errors, so a skirt this deep on both sides covers it.
    let mut ring_error = [0.0_f32; RINGS.len()];
    for ((p, s), &level) in places.iter().zip(&sampled).zip(&chosen) {
        levels[(p.ring * TILES + p.tz) * TILES + p.tx] = Some(level);
        ring_error[p.ring] = ring_error[p.ring].max(s.errors[level as usize]);
    }
    let level_of = |ring: usize, tx: isize, tz: isize| {
        let inside = (0..TILES as isize).contains(&tx) && (0..TILES as isize).contains(&tz);
        inside.then(|| levels[(ring * TILES + tz as usize) * TILES + tx as usize]).flatten()
    };
    let start = mesh.indices.len();
    let mut histogram = [[0usize; tile::LEVELS]; RINGS.len()];
    let mut worst_px = 0.0_f32;
    for (((p, s), &level), e) in places.iter().zip(&sampled).zip(&chosen).zip(&errors_px) {
        histogram[p.ring][level as usize] += 1;
        worst_px = worst_px.max(e[level as usize]);
        let (tx, tz) = (p.tx as isize, p.tz as isize);
        let skirted = [(tx, tz - 1), (tx, tz + 1), (tx - 1, tz), (tx + 1, tz)]
            .map(|(x, z)| level_of(p.ring, x, z) != Some(level));
        let near = ring_error[p.ring.saturating_sub(1)..(p.ring + 2).min(RINGS.len())]
            .iter()
            .fold(0.0_f32, |a, &b| a.max(b));
        let drop = SKIRT_M.max(RINGS[p.ring].1) / scale + s.errors[level as usize] + near;
        push_tile(mesh, *p, center, scale, s, Drawn { level, skirted, drop });
    }

    println!(
        "[horizon-fine] rings_m={:?} tiles={} knots_sampled={} triangles={} budget={FINE_TRIANGLES} ring_levels={histogram:?} worst_error_px={worst_px:.2}",
        RINGS,
        places.len(),
        places.len() * BORDERED * BORDERED,
        (mesh.indices.len() - start) / 3,
    );
    Some(())
}

#[cfg(test)]
mod tests;
