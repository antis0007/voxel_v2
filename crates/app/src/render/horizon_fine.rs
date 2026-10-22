//! Fine smooth distant terrain. Inside a square around the backdrop centre the
//! coarse lattice is cut away and replaced by a regular grid sampled every 8 m
//! from the same `distant_surface` source the chunks use. Height, colour and
//! normal are per vertex, so ridges keep their real shape and shade smoothly.
//! A skirt along the outer edge hides the T-junctions where the grid meets the
//! coarser lattice. The grid sits on the world lattice, so recentring never
//! moves an existing vertex; the grid is anchored to the snapped centre.
use super::edits::{append, clip};
use crate::mesh::{ChunkMesh, Vertex3};
use crate::world::{WorldSampleConfig, distant_surface};
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};

/// Half width of the replaced square, in metres.
pub(super) const HALF_M: f32 = 1536.0;
/// Grid spacing in metres. Matches the coarsest chunk LOD (8 m cells), which
/// is the terrain the grid meets at the chunk edge.
const STEP_M: f32 = 8.0;
/// Skirt depth below the outer edge, in metres.
const SKIRT_M: f32 = 8.0;

/// Knot coordinate along one axis, in voxel units. Sampling and vertex placement
/// both use it, so the height that is sampled is the one that is drawn.
fn knot(origin: f32, index: usize, scale: f32) -> f32 {
    (origin + index as f32 * STEP_M) / scale
}

/// `VOXEL_HORIZON_SMOOTH=1` keeps the old lattice backdrop, for A/B captures.
fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("VOXEL_HORIZON_SMOOTH").is_none())
}

/// Cut every lattice triangle that lies inside the outer square. Pieces outside
/// the square are kept. Uses the same four-slab decomposition as the reference
/// patch, so no triangle is emitted twice.
fn cut_square(mesh: &mut ChunkMesh, lo: [f32; 2], hi: [f32; 2], cancel: &AtomicBool) -> Option<()> {
    let indices = std::mem::take(&mut mesh.indices);
    for (n, tri) in indices.chunks_exact(3).enumerate() {
        if n % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize]);
        if v.iter().all(|v| v.pos[0] < lo[0])
            || v.iter().all(|v| v.pos[0] > hi[0])
            || v.iter().all(|v| v.pos[2] < lo[1])
            || v.iter().all(|v| v.pos[2] > hi[1])
        {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut inside = v.to_vec();
        for (axis, bound, less) in [
            (0, lo[0], true),
            (0, hi[0], false),
            (2, lo[1], true),
            (2, hi[1], false),
        ] {
            append(mesh, &clip(&inside, axis, bound, less));
            inside = clip(&inside, axis, bound, !less);
        }
    }
    Some(())
}

/// Height gradient at grid knot (i, k), using neighbours on the grid. Edge
/// knots fall back to one-sided differences. Returns a unit normal in voxel
/// units, where `step` is the knot spacing in voxels.
fn normal_at(heights: &[f32], side: usize, i: usize, k: usize, step: f32) -> Vec3 {
    let at = |i: usize, k: usize| heights[k * side + i];
    let (il, ir) = (i.saturating_sub(1), (i + 1).min(side - 1));
    let (kd, ku) = (k.saturating_sub(1), (k + 1).min(side - 1));
    let dx = (at(ir, k) - at(il, k)) / ((ir - il) as f32 * step);
    let dz = (at(i, ku) - at(i, kd)) / ((ku - kd) as f32 * step);
    Vec3::new(-dx, 1.0, -dz).try_normalize().unwrap_or(Vec3::Y)
}

/// Emit one quad from four vertices in perimeter order. The winding flips when
/// it faces away from `normal`, so the surface matches its lighting normal.
fn push_quad(mesh: &mut ChunkMesh, verts: [Vertex3; 4], normal: Vec3) {
    let p = verts.map(|v| Vec3::from_array(v.pos));
    let order = if (p[1] - p[0]).cross(p[2] - p[0]).dot(normal) >= 0.0 {
        [0, 1, 2, 3]
    } else {
        [0, 3, 2, 1]
    };
    let base = mesh.vertices.len() as u32;
    for index in order {
        let mut v = verts[index];
        v.normal = normal.to_array();
        mesh.vertices.push(v);
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// One skirt segment: the edge `top` and the same edge dropped by `drop`.
fn push_skirt(mesh: &mut ChunkMesh, top: [Vertex3; 2], normal: Vec3, drop: f32) {
    let down = |v: Vertex3| Vertex3 {
        pos: [v.pos[0], v.pos[1] - drop, v.pos[2]],
        ao_strength: -1.0,
        ..v
    };
    push_quad(mesh, [top[0], top[1], down(top[1]), down(top[0])], normal);
}

/// Replace the lattice backdrop inside the outer square with a fine smooth grid.
/// Runs on the base lattice before water and vegetation are appended.
pub(super) fn apply(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
) -> Option<()> {
    if !enabled() {
        return Some(());
    }
    let scale = cfg.voxel_meters();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let (cx, cz) = (center.0 as f32, center.1 as f32);
    let origin = (cx - HALF_M, cz - HALF_M);
    let side = (2.0 * HALF_M / STEP_M).round() as usize + 1;
    let step = STEP_M / scale;

    cut_square(
        mesh,
        [origin.0 / scale, origin.1 / scale],
        [(cx + HALF_M) / scale, (cz + HALF_M) / scale],
        cancel,
    )?;

    // Sample every knot once. Heights come from the same source as the chunks,
    // not from the lattice, so the grid carries the full ridge detail. Each knot
    // is a pure function of its position, so rows run on shared workers and come
    // back in row order: the grid is identical to a serial pass.
    let rows: Vec<usize> = (0..side).collect();
    let samples: Vec<(f32, crate::world::BlockId)> =
        super::parallel::map_blocks(&rows, 4, super::parallel::workers(8), cancel, |&k| {
            let z = knot(origin.1, k, scale);
            (0..side)
                .map(|i| {
                    let x = knot(origin.0, i, scale);
                    let (h, block) = distant_surface(cfg, x.floor() as i32, z.floor() as i32);
                    h.is_finite().then_some((h, block))
                })
                .collect::<Option<Vec<_>>>()
        })?
        .into_iter()
        .flatten()
        .collect();
    let (heights, blocks): (Vec<f32>, Vec<crate::world::BlockId>) = samples.into_iter().unzip();

    let base = mesh.vertices.len();
    for k in 0..side {
        for i in 0..side {
            let index = k * side + i;
            let x = knot(origin.0, i, scale);
            let z = knot(origin.1, k, scale);
            let block = blocks[index];
            let material = block.surface();
            let h = heights[index];
            mesh.vertices.push(Vertex3 {
                pos: [x, h, z],
                color: block.color(IVec3::new(x.floor() as i32, h as i32, z.floor() as i32)),
                normal: normal_at(&heights, side, i, k, step).to_array(),
                surface: [
                    material.roughness,
                    material.specular,
                    0.0,
                    material.shader_key(),
                ],
                ao_strength: -1.0,
            });
        }
    }

    // Two triangles per cell, wound so the normal points up.
    for k in 0..side - 1 {
        for i in 0..side - 1 {
            let a = (base + k * side + i) as u32;
            let b = a + 1;
            let c = a + side as u32;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }

    // Skirts hang below the outer edge. They cover the cracks where the fine
    // grid meets the coarser lattice beyond the square.
    let skirt_drop = SKIRT_M / scale;
    let edge = |mesh: &ChunkMesh, i: usize, k: usize| mesh.vertices[base + k * side + i];
    for m in 0..side - 1 {
        let south = [edge(mesh, m, 0), edge(mesh, m + 1, 0)];
        let north = [edge(mesh, m + 1, side - 1), edge(mesh, m, side - 1)];
        let west = [edge(mesh, 0, m + 1), edge(mesh, 0, m)];
        let east = [edge(mesh, side - 1, m), edge(mesh, side - 1, m + 1)];
        push_skirt(mesh, south, -Vec3::Z, skirt_drop);
        push_skirt(mesh, north, Vec3::Z, skirt_drop);
        push_skirt(mesh, west, -Vec3::X, skirt_drop);
        push_skirt(mesh, east, Vec3::X, skirt_drop);
    }

    println!(
        "[horizon-fine] grid_m={HALF_M}x2 step_m={STEP_M} knots={} triangles={}",
        side * side,
        mesh.indices.len() / 3,
    );
    Some(())
}
