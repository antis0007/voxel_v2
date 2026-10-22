//! Bounded diagnostic reference geometry, rendered by the actual world pipeline.
//! Never enabled in play. Parent triangles are cut out, not overdrawn/blended.
use super::{
    edits::{append, clip},
    *,
};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
struct Patch {
    x: f32,
    z: f32,
    span: f32,
    step: f32,
}
fn patch() -> Option<Patch> {
    static PATCH: OnceLock<Option<Patch>> = OnceLock::new();
    *PATCH.get_or_init(|| {
        let spec = std::env::var("VOXEL_HORIZON_EXACT_PATCH").ok()?;
        let v: Vec<f32> = spec
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if v.len() != 4 || v.iter().any(|n| !n.is_finite()) || v[2] <= 0.0 {
            return None;
        }
        Some(Patch {
            x: v[0],
            z: v[1],
            span: v[2].clamp(16.0, 512.0),
            step: v[3].clamp(2.0, 32.0),
        })
    })
}

pub(super) fn contains(x: f32, z: f32) -> bool {
    patch().is_some_and(|p| (x - p.x).abs().max((z - p.z).abs()) <= p.span * 0.5)
}

pub(super) fn apply(mesh: &mut ChunkMesh, cfg: &WorldSampleConfig) {
    if voxel_reference() {
        return;
    }
    let Some(p) = patch() else {
        return;
    };
    let scale = cfg.voxel_meters();
    let n = (p.span / p.step).ceil() as usize;
    let step = p.span / n as f32;
    let lo = [(p.x - p.span * 0.5) / scale, (p.z - p.span * 0.5) / scale];
    let hi = [(p.x + p.span * 0.5) / scale, (p.z + p.span * 0.5) / scale];
    let indices = std::mem::take(&mut mesh.indices);
    for tri in indices.chunks_exact(3) {
        let v = tri
            .iter()
            .map(|&i| mesh.vertices[i as usize])
            .collect::<Vec<_>>();
        if v.iter().all(|v| v.pos[0] < lo[0])
            || v.iter().all(|v| v.pos[0] > hi[0])
            || v.iter().all(|v| v.pos[2] < lo[1])
            || v.iter().all(|v| v.pos[2] > hi[1])
        {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut inside = v;
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
    let base = mesh.vertices.len() as u32;
    for z in 0..=n {
        for x in 0..=n {
            let vx = lo[0] + x as f32 * step / scale;
            let vz = lo[1] + z as f32 * step / scale;
            let bx = vx.floor() as i32;
            let bz = vz.floor() as i32;
            let (height, block) = distant_surface(cfg, bx, bz);
            let delta = (step / scale).ceil() as i32;
            let dx =
                distant_surface(cfg, bx + delta, bz).0 - distant_surface(cfg, bx - delta, bz).0;
            let dz =
                distant_surface(cfg, bx, bz + delta).0 - distant_surface(cfg, bx, bz - delta).0;
            let material = block.surface();
            mesh.vertices.push(Vertex3 {
                pos: [vx, height - HORIZON_SINK_VOXELS, vz],
                normal: Vec3::new(-dx, 2.0 * delta as f32, -dz)
                    .normalize()
                    .to_array(),
                color: block.color(IVec3::new(bx, height as i32, bz)),
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
    for z in 0..n {
        for x in 0..n {
            let a = base + (z * (n + 1) + x) as u32;
            let b = a + 1;
            let c = a + (n + 1) as u32;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    println!(
        "[horizon-reference] exact_patch_m={},{},{},{} vertices={} triangles={} same_camera=true",
        p.x,
        p.z,
        p.span,
        step,
        (n + 1) * (n + 1),
        n * n * 2
    );
}

fn voxel_reference() -> bool {
    std::env::var("VOXEL_HORIZON_REFERENCE_VOXELS").as_deref() == Ok("1")
}

/// Full resident-resolution reference including canonical trees, caves and water.
/// Restrict this diagnostic to at most 256 m at 1 m scale. No whole far-view
/// voxel generation is used in normal rendering.
pub(super) fn apply_voxels(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    cancel: &AtomicBool,
) -> Option<()> {
    if !voxel_reference() {
        return Some(());
    }
    let Some(p) = patch() else {
        return Some(());
    };
    if cfg.voxel_meters() != 1.0 || p.span > 256.0 {
        eprintln!("[horizon-reference] rejected full voxel patch: requires scale=1m span<=256m");
        return None;
    }
    let min_x = ((p.x - p.span * 0.5) / 32.0).floor() as i32;
    let min_z = ((p.z - p.span * 0.5) / 32.0).floor() as i32;
    let max_x = ((p.x + p.span * 0.5) / 32.0).ceil() as i32;
    let max_z = ((p.z + p.span * 0.5) / 32.0).ceil() as i32;
    let lo = [(min_x * 32) as f32, (min_z * 32) as f32];
    let hi = [(max_x * 32) as f32, (max_z * 32) as f32];
    let indices = std::mem::take(&mut mesh.indices);
    for tri in indices.chunks_exact(3) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let v = tri
            .iter()
            .map(|&i| mesh.vertices[i as usize])
            .collect::<Vec<_>>();
        if v.iter().all(|v| v.pos[0] < lo[0])
            || v.iter().all(|v| v.pos[0] > hi[0])
            || v.iter().all(|v| v.pos[2] < lo[1])
            || v.iter().all(|v| v.pos[2] > hi[1])
        {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut inside = v;
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
    let mut chunks = 0;
    for z in min_z..max_z {
        for x in min_x..max_x {
            let mut low = i32::MAX;
            let mut high = cfg.water_level();
            for dz in [0, 8, 16, 24, 31] {
                for dx in [0, 8, 16, 24, 31] {
                    let h = crate::world::terrain_height(cfg, x * 32 + dx, z * 32 + dz);
                    low = low.min(h);
                    high = high.max(h);
                }
            }
            for y in (low - 32).div_euclid(32)..=(high + 48).div_euclid(32) {
                let source = crate::world::generate_padded(cfg, IVec3::new(x, y, z), cancel)?;
                let plan = crate::mesh::MeshPlan::for_chunk(&source, cancel)?;
                let output_bytes =
                    plan.vertices * std::mem::size_of::<Vertex3>() + plan.indices * 4;
                if super::mesh_bytes(mesh) + output_bytes > super::MAX_MESH_BYTES {
                    eprintln!(
                        "[horizon-reference] full voxel patch exceeds mesh budget; reduce span"
                    );
                    return None;
                }
                let mut patch = plan.build_lod(&source, 0);
                let base = mesh.vertices.len() as u32;
                for v in &mut patch.vertices {
                    v.ao_strength = -2.0 - v.ao_strength;
                }
                mesh.vertices.extend(patch.vertices);
                mesh.indices
                    .extend(patch.indices.into_iter().map(|i| base + i));
                chunks += 1;
            }
        }
    }
    println!(
        "[horizon-reference] resident_voxel_patch bounds_m={lo:?}..{hi:?} chunks={chunks} source=canonical geometry=1m same_camera=true"
    );
    Some(())
}
