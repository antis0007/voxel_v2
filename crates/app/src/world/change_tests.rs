//! Every committed cell reaches the mesh consumer through `publish_changes`,
//! whichever writer made it, with the one-voxel halo across chunk borders.
use super::*;
use crate::world::{ChunkData, WorldType};
use engine_core::VoxelScalePreset;
use std::collections::HashSet;
use std::sync::Arc;

/// Corner cell of chunk (-1, 2, -1): local (31, 31, 31), so its halo spans 8 chunks.
const CORNER: IVec3 = IVec3::new(-1, 95, -1);

/// Eight resident air chunks around `CORNER`; no terrain jobs or renderer.
fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    for cx in -1..=0 {
        for cy in 2..=3 {
            for cz in -1..=0 {
                w.chunks.insert(IVec3::new(cx, cy, cz), Arc::new(ChunkData {
                    modified_bounds: None,
                    structural_bounds: None,
                    blocks: Box::new([BlockId::Air; 32 * 32 * 32]),
                    light: Box::new([0; 32 * 32 * 32]),
                    torches: HashSet::new(),
                }));
            }
        }
    }
    w
}

fn published(w: &mut World) -> (HashSet<IVec3>, HashSet<IVec3>) {
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    w.publish_changes(&mut blocks, &mut chunks);
    (blocks, chunks)
}

#[test]
fn corner_edit_publishes_all_eight_chunks_at_negative_coordinates() {
    let mut w = world();
    w.set(CORNER, BlockId::Stone);
    assert_eq!(w.get(CORNER), BlockId::Stone);
    let (blocks, chunks) = published(&mut w);
    assert_eq!(blocks, HashSet::from([CORNER]));
    let expected: HashSet<IVec3> = (-1..=0)
        .flat_map(|x| (2..=3).flat_map(move |y| (-1..=0).map(move |z| IVec3::new(x, y, z))))
        .collect();
    assert_eq!(chunks, expected);
}

#[test]
fn unchanged_and_rejected_writes_publish_nothing() {
    let mut w = world();
    w.set(CORNER, BlockId::Air);
    let below = IVec3::new(0, w.sample_config().min_world_y() - 1, 0);
    w.set(below, BlockId::Stone);
    let (blocks, chunks) = published(&mut w);
    assert!(blocks.is_empty() && chunks.is_empty());
}

#[test]
fn repeated_writes_coalesce_and_publish_once() {
    let mut w = world();
    let p = CORNER - IVec3::splat(4);
    for id in [BlockId::Stone, BlockId::Dirt, BlockId::Stone] {
        w.set(p, id);
    }
    let (blocks, chunks) = published(&mut w);
    assert_eq!(blocks, HashSet::from([p]));
    assert_eq!(chunks, HashSet::from([chunk_coord_from_world_block(p)]));
    let (blocks, chunks) = published(&mut w);
    assert!(blocks.is_empty() && chunks.is_empty(), "journal drains on publish");
}

/// Prints publish cost for one full cellular pass (8192 cells) whose cells the
/// caller already marked, the transitional worst case. Run with `--ignored`.
#[test]
#[ignore]
fn publish_cost_for_a_full_cellular_pass() {
    let mut w = world();
    let cells: Vec<IVec3> = (0..8192)
        .map(|i| IVec3::new(-32 + i % 32, 64 + (i / 32) % 32, -32 + i / 1024))
        .collect();
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    for &p in &cells {
        blocks.insert(p);
        mark_chunk_and_neighbors_dirty(&mut chunks, p);
    }
    let runs = 50;
    let start = std::time::Instant::now();
    for _ in 0..runs {
        w.changed_cells.extend_from_slice(&cells);
        w.publish_changes(&mut blocks, &mut chunks);
    }
    let per_pass = start.elapsed().as_secs_f64() * 1e3 / runs as f64;
    println!("publish-cost cells=8192 per_pass_ms={per_pass:.3} ns_per_cell={:.1}", per_pass * 1e6 / 8192.0);
}

#[test]
fn water_volume_writes_publish_their_cells() {
    let mut w = world();
    let p = CORNER - IVec3::new(0, 4, 0);
    w.set_water_volume(p, 5);
    let q = p - IVec3::X;
    w.set_water_volume(q, 3);
    assert!(w.water_volume(p) > 0 || w.water_volume(q) > 0, "fixture accepts water");
    let (blocks, _) = published(&mut w);
    for cell in [p, q] {
        assert_eq!(blocks.contains(&cell), w.water_volume(cell) > 0, "cell {cell}");
    }
}
