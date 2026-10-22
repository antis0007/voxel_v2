use super::*;
use crate::material::terrain_family as family;
use std::sync::atomic::AtomicBool;

fn fixture(coord: IVec3) -> GeneratedChunk {
    GeneratedChunk::new(coord, vec![BlockId::Air; PADDED_CHUNK_SIZE.pow(3) as usize])
}
fn build(chunk: &GeneratedChunk) -> ChunkMesh {
    memory::MeshPlan::for_chunk(chunk, &AtomicBool::new(false)).unwrap().build_lod(chunk, 0)
}
fn kind(v: &Vertex3) -> u32 { v.surface[2] as u32 % 64 }

#[test]
fn terrain_contract_material_key_reaches_production_vertices() {
    for (block, expected) in [(BlockId::Gravel, family::GRAVEL), (BlockId::Stone, 3),
        (BlockId::Basalt, 3), (BlockId::Dirt, 2), (BlockId::Grass, 1), (BlockId::Cobblestone, 13)] {
        let mut c = fixture(IVec3::ZERO);
        c.set_world(IVec3::new(16, 16, 16), block);
        let mesh = build(&c);
        let faces: Vec<_> = mesh.vertices.iter().filter(|v| kind(v) == expected).collect();
        assert!(!faces.is_empty(), "missing production material {block:?}");
        for v in faces {
            assert_eq!(v.surface[2] as u32 / 64, block as u32 + 1);
            assert_eq!(v.surface[2], (v.surface[2] as u32) as f32);
        }
    }
}

#[test]
fn terrain_contract_cap_halo_and_uncover_restore_fringe_and_detail() {
    for coord in [IVec3::ZERO, IVec3::new(-1, -1, -1)] {
        for scale in [1.0, 0.5, 0.25] {
            let mut c = fixture(coord);
            c.voxel_meters = scale;
            let origin = coord * CHUNK_SIZE;
            // Support is the highest row. The cover lives in the neighbor halo.
            for z in 0..32 { for x in 0..32 {
                c.set_world(origin + IVec3::new(x, 31, z), BlockId::Grass);
            }}
            let exposed = build(&c);
            assert!(exposed.vertices.iter().any(|v| kind(v) == family::TUFT));
            assert!(!exposed.vertices.iter().any(|v| kind(v) == family::CAPPED_TURF));
            for cap in [BlockId::Stone, BlockId::Water, BlockId::Leaves, BlockId::Ice] {
                for z in 0..32 { for x in 0..32 {
                    c.set_world(origin + IVec3::new(x, 32, z), cap);
                }}
                let capped = build(&c);
                assert!(capped.vertices.iter().any(|v| kind(v) == family::CAPPED_TURF));
                assert!(!capped.vertices.iter().any(|v| matches!(kind(v), family::TUFT | family::PEBBLE)));
            }
            for z in 0..32 { for x in 0..32 {
                c.set_world(origin + IVec3::new(x, 32, z), BlockId::Air);
            }}
            let restored = build(&c);
            assert_eq!(exposed.indices, restored.indices);
            assert_eq!(bytemuck::cast_slice::<Vertex3, u8>(&exposed.vertices),
                       bytemuck::cast_slice::<Vertex3, u8>(&restored.vertices));
        }
    }
}

#[test]
fn terrain_contract_cover_edit_invalidates_support_across_signed_chunk_edges() {
    for p in [IVec3::new(-1, 32, -1), IVec3::new(0, 0, 0), IVec3::new(32, -32, 32)] {
        let mut dirty = std::collections::HashSet::new();
        crate::world::mark_chunk_and_neighbors_dirty(&mut dirty, p);
        assert!(dirty.contains(&(p - IVec3::Y).div_euclid(IVec3::splat(32))));
        assert!(dirty.contains(&p.div_euclid(IVec3::splat(32))));
    }
}
