//! Coarse architecture in the same mesh and edit coverage as distant trees.
use super::vegetation::TreeSource;
use crate::{
    mesh::{ChunkMesh, Vertex3},
    world::{BlockId, WorldSampleConfig},
};
use glam::{Vec2, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};

const REACH: f32 = 2048.0;
const MAX_BYTES: usize = 512 * 1024;
const WING_BYTES: usize = 56 * std::mem::size_of::<Vertex3>() + 72 * std::mem::size_of::<u32>();

pub(super) fn append(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    chart_clip: Option<super::clip::ChartClip>,
    cancel: &AtomicBool,
    sources: &mut Vec<TreeSource>,
) {
    let start = super::mesh_bytes(mesh);
    let limit = (start + MAX_BYTES).min(super::MAX_MESH_BYTES.saturating_sub(8 * 1024 * 1024));
    if start + WING_BYTES > limit || cancel.load(Ordering::Relaxed) {
        return;
    }
    let scale = cfg.voxel_meters();
    let center = Vec2::new(center.0 as f32, center.1 as f32);
    let mut emit = |roof: [Vec3; 9], base: f32, wall: BlockId, cap: BlockId| {
        if super::mesh_bytes(mesh) + WING_BYTES > limit
            || chart_clip.is_some_and(|(x0, z0, x1, z1)| {
                roof.iter().any(|p| {
                    !(x0..=x1).contains(&f64::from(p.x)) || !(z0..=z1).contains(&f64::from(p.z))
                })
            })
        {
            return;
        }
        let start = mesh.indices.len();
        for side in 0..8 {
            let a = roof[side];
            let b = roof[(side + 1) % 8];
            face(mesh, [a.with_y(base), a, b, b.with_y(base)], wall, scale);
            face(mesh, [b, a, roof[8]], cap, scale);
        }
        let lo = roof.iter().copied().reduce(Vec3::min).unwrap().with_y(base);
        let hi = roof.iter().copied().reduce(Vec3::max).unwrap();
        sources.push(TreeSource {
            indices: start..mesh.indices.len(),
            low: (lo / scale).floor().as_ivec3() - glam::IVec3::ONE,
            high: (hi / scale).ceil().as_ivec3() + glam::IVec3::ONE,
        });
    };
    engine_worldgen::planet_freight::visit_skylines(cfg, center, REACH, &mut emit);
    engine_worldgen::civilization::visit_skylines(cfg, center, REACH, cancel, &mut emit);
    engine_worldgen::ruins::visit_skylines(cfg, center, REACH, cancel, &mut emit);
    engine_worldgen::sites::visit_skylines(cfg, center, REACH, cancel, emit);
}

fn face<const N: usize>(mesh: &mut ChunkMesh, points: [Vec3; N], block: BlockId, scale: f32) {
    let normal = (points[1] - points[0])
        .cross(points[2] - points[0])
        .normalize_or_zero();
    let base = mesh.vertices.len() as u32;
    let color = block.color((points[0] / scale).floor().as_ivec3());
    for point in points {
        mesh.vertices.push(Vertex3 {
            pos: (point / scale).to_array(),
            normal: normal.to_array(),
            color,
            surface: [0.95, 0.02, block.surface().shader_key(), 0.0],
            ao_strength: -1.0,
        });
    }
    for i in 1..N as u32 - 1 {
        mesh.indices
            .extend_from_slice(&[base, base + i, base + i + 1]);
    }
}
