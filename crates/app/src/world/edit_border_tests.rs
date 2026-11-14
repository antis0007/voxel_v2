//! Edits at negative chunk corners: remesh coverage and persistence (update pack T01).
use super::regions::{RegionStream, checkpoint_id};
use super::*;
use crate::fluid::VoxelScalePreset;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Chunks whose padded (one-voxel halo) mesh input contains `p`, by brute force.
fn halo_readers(p: IVec3) -> HashSet<IVec3> {
    let mut out = HashSet::new();
    let c = chunk_coord_from_world_block(p);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let k = c + IVec3::new(dx, dy, dz);
                let lo = k * CHUNK_SIZE - IVec3::ONE;
                let hi = k * CHUNK_SIZE + IVec3::splat(CHUNK_SIZE);
                if p.cmpge(lo).all() && p.cmple(hi).all() {
                    out.insert(k);
                }
            }
        }
    }
    out
}

#[test]
fn border_edits_dirty_exactly_the_chunks_that_read_them() {
    // Corners, edges and faces of chunks on both sides of zero, plus far coordinates.
    let origins = [IVec3::ZERO, IVec3::splat(-1), IVec3::new(-1, 0, 2), IVec3::new(70_000, -3, -70_001)];
    let offsets = [0, 1, CHUNK_SIZE / 2, CHUNK_SIZE - 2, CHUNK_SIZE - 1];
    for origin in origins {
        for &x in &offsets {
            for &y in &offsets {
                for &z in &offsets {
                    let p = origin * CHUNK_SIZE + IVec3::new(x, y, z);
                    let mut dirty = HashSet::new();
                    mark_chunk_and_neighbors_dirty(&mut dirty, p);
                    assert_eq!(dirty, halo_readers(p), "edit at {p}");
                }
            }
        }
    }
    let mut corner = HashSet::new();
    mark_chunk_and_neighbors_dirty(&mut corner, IVec3::splat(-CHUNK_SIZE));
    assert_eq!(corner.len(), 8, "a corner voxel feeds eight padded meshes");
}

/// Writes every dirty chunk in `coords` to region files and waits for the records.
fn write_back(world: &mut World, coords: &HashSet<IVec3>) {
    world.request_simulation_writeback();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !coords.iter().all(|c| world.regions.view.records.contains_key(c)) {
        assert!(Instant::now() < deadline, "region write timeout");
        world.stream_regions(IVec3::ZERO, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn corner_edits_across_eight_chunks_survive_reload() {
    let root = std::env::temp_dir().join(format!("voxel-edit-border-{}-{}", std::process::id(), checkpoint_id()));
    let mut writer = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 11);
    writer.regions = RegionStream::open(root.clone(), HashMap::new());
    // The corner shared by chunks (-1..0, k-1..k, -1..0) just under the surface.
    let top = writer.terrain_height_at(-CHUNK_SIZE, -CHUNK_SIZE);
    let corner = IVec3::new(-CHUNK_SIZE, (top - 1).div_euclid(CHUNK_SIZE) * CHUNK_SIZE, -CHUNK_SIZE);
    let mut edits = Vec::new();
    for dz in [-1, 0] {
        for dy in [-1, 0] {
            for dx in [-1, 0] {
                let p = corner + IVec3::new(dx, dy, dz);
                // Alternate dig and place so both directions cross every face.
                let id = if writer.get(p) == BlockId::Air { BlockId::Cobblestone } else { BlockId::Air };
                edits.push((p, id));
            }
        }
    }
    writer.set_many(&edits);
    for &(p, id) in &edits {
        assert_eq!(writer.get(p), id, "edit at {p} did not apply");
    }
    let touched: HashSet<IVec3> = edits.iter().map(|&(p, _)| chunk_coord_from_world_block(p)).collect();
    assert_eq!(touched.len(), 8);
    write_back(&mut writer, &touched);

    let records = touched.iter().map(|c| (*c, writer.regions.view.records.get(c).unwrap().clone())).collect();
    let mut reader = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 11);
    reader.regions = RegionStream::open(root.clone(), records);
    for &(p, id) in &edits {
        assert_eq!(reader.get(p), id, "edit at {p} lost on reload");
    }
    // Cells beside the edits keep generator output; nothing was smeared across a face.
    let cfg = reader.sample_config();
    for &(p, _) in &edits {
        for n in [p + IVec3::X * 2, p - IVec3::X * 2, p + IVec3::Z * 2, p - IVec3::Z * 2] {
            if edits.iter().all(|&(q, _)| q != n) {
                assert_eq!(reader.get(n), engine_worldgen::sample_block(&cfg, n), "neighbour {n} changed");
            }
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
