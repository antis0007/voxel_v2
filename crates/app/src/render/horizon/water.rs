//! One indexed water surface clipped to the canonical bed triangles.
//! Shared source knots/edges share water vertices, including ring interfaces.
use crate::{
    mesh::{ChunkMesh, Vertex3},
    world::{BlockId, WorldSampleConfig},
};
use glam::Vec3;
use std::collections::HashMap;

fn linear(c:f32)->f32 { if c<=0.04045 {c/12.92} else {((c+0.055)/1.055).powf(2.4)} }
fn srgb(c:f32)->f32 { if c<=0.0031308 {c*12.92} else {1.055*c.powf(1.0/2.4)-0.055} }
fn filtered_bed(a:[f32;3],b:[f32;3],t:f32)->[f32;3] {
    std::array::from_fn(|j|srgb(linear(a[j])+(linear(b[j])-linear(a[j]))*t))
}

fn vertex(
    mesh: &mut ChunkMesh,
    cache: &mut HashMap<(u32, u32), u32>,
    key: (u32, u32),
    p: Vec3,
    sea: f32,
    surface: f32,
    scale: f32,
    bed_color: [f32;3],
) -> u32 {
    if let Some(&i) = cache.get(&key) {
        return i;
    }
    let material = BlockId::Water.surface();
    let i = mesh.vertices.len() as u32;
    mesh.vertices.push(Vertex3 {
        pos: [p.x, surface, p.z],
        normal: [0.0, 1.0, 0.0],
        color: bed_color,
        surface: [
            material.roughness,
            material.specular,
            material.shader_key(),
            ((sea - p.y) * scale).clamp(0.0, 48.0) / 16.0,
        ],
        ao_strength: -1.0,
    });
    cache.insert(key, i);
    i
}

/// Far water is clipped one voxel below the drawn sheet. The coarse lattice
/// interpolates beach corners down toward the water, so clipping at the sheet
/// itself floods dry shore (measured in `horizon_agreement`: p95 error is 4 to
/// 19 m at the 16 to 64 m columns). Shallows under one voxel read as bed.
pub(super) const COAST_BIAS_VOXELS: f32 = 1.0;

pub(super) fn append(mesh: &mut ChunkMesh, cfg: &WorldSampleConfig) {
    let scale = cfg.voxel_meters();
    let sea = cfg.water_level() as f32 + 1.0;
    let surface = sea - crate::mesh::WATER_SURFACE_INSET;
    let waterline = surface - COAST_BIAS_VOXELS;
    let count = mesh.indices.len();
    let mut cache = HashMap::new();
    for offset in (0..count).step_by(3) {
        let ids = [
            mesh.indices[offset],
            mesh.indices[offset + 1],
            mesh.indices[offset + 2],
        ];
        let points = ids.map(|i| Vec3::from_array(mesh.vertices[i as usize].pos));
        if points.iter().all(|p| p.y >= waterline) {
            continue;
        }
        let mut polygon = [0u32; 4];
        let mut n = 0;
        for edge in 0..3 {
            let a = points[edge];
            let b = points[(edge + 1) % 3];
            let ia = ids[edge];
            let ib = ids[(edge + 1) % 3];
            let wet_a = a.y < waterline;
            let wet_b = b.y < waterline;
            if wet_a {
                let bed_color=mesh.vertices[ia as usize].color;
                polygon[n] = vertex(mesh, &mut cache, (ia, ia), a, sea, surface, scale,bed_color);
                n += 1;
            }
            if wet_a != wet_b {
                // Evaluate crossings in canonical endpoint order so adjacent
                // triangles obtain bit-identical shoreline positions.
                let (first, last) = if ia < ib { (a, b) } else { (b, a) };
                let p = first.lerp(last, (waterline - first.y) / (last.y - first.y));
                let (first_color,last_color)=if ia<ib {(mesh.vertices[ia as usize].color,mesh.vertices[ib as usize].color)} else {(mesh.vertices[ib as usize].color,mesh.vertices[ia as usize].color)};
                let t=(waterline-first.y)/(last.y-first.y);
                let bed_color=filtered_bed(first_color,last_color,t);
                polygon[n] = vertex(
                    mesh,
                    &mut cache,
                    (ia.min(ib), ia.max(ib)),
                    p,
                    sea,
                    surface,
                    scale,
                    bed_color,
                );
                n += 1;
            }
        }
        for i in 1..n.saturating_sub(1) {
            mesh.indices
                .extend_from_slice(&[polygon[0], polygon[i], polygon[i + 1]]);
        }
    }
    println!(
        "[horizon] water shared_vertices={} source_triangles={}",
        cache.len(),
        count / 3
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{World, WorldType},
    };
    #[test]
    fn shared_water_knots_do_not_duplicate_sheets_or_vertices() {
        let cfg =
            World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7).sample_config();
        let mut mesh = ChunkMesh::empty();
        for p in [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ] {
            mesh.vertices.push(Vertex3 {
                pos: p,
                color: [1.0; 3],
                normal: [0.0, 1.0, 0.0],
                surface: [0.9, 0.0, 0.0, 0.0],
                ao_strength: -1.0,
            });
        }
        mesh.indices = vec![0, 2, 3, 0, 3, 1];
        append(&mut mesh, &cfg);
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.indices.len(), 12);
        assert_eq!(mesh.indices[6], mesh.indices[9]);
        for v in &mesh.vertices[4..] {
            assert_eq!(v.color,[1.0;3],"far water carries the canonical bed, not a second blue albedo");
            assert_eq!(v.surface[2],BlockId::Water.surface().shader_key());
        }
    }

    #[test]
    fn shoreline_crossings_filter_bed_colour_without_averaging_material_ids() {
        let cfg=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7).sample_config();
        let sea=cfg.water_level() as f32+1.0;
        let mut mesh=ChunkMesh::empty();
        for (p,color) in [([0.0,0.0,0.0],[0.0,0.2,0.3]),([0.0,0.0,2.0],[0.0,0.2,0.3]),([2.0,sea+2.0,0.0],[1.0,0.2,0.3])] {
            mesh.vertices.push(Vertex3 {pos:p,color,normal:[0.0,1.0,0.0],surface:[0.9,0.0,3.0,0.0],ao_strength:-1.0});
        }
        mesh.indices=vec![0,1,2];append(&mut mesh,&cfg);
        assert_eq!(mesh.indices.len(),9);
        for v in &mesh.vertices[3..] {
            assert!((linear(v.color[0])-v.pos[0]*0.5).abs()<1e-6);
            assert_eq!(v.surface[2],BlockId::Water.surface().shader_key());
            assert!((v.pos[1]-(sea-crate::mesh::WATER_SURFACE_INSET)).abs()<1e-6);
        }
    }
}
