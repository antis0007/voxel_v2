use super::*;
use glam::IVec3;

const P: i32 = PADDED_CHUNK_SIZE;

fn chunk_with(coord: IVec3, fill: BlockId, open: impl Fn(i32, i32, i32) -> bool) -> GeneratedChunk {
    let mut blocks = vec![fill; (P * P * P) as usize];
    for y in 0..P {
        for z in 0..P {
            for x in 0..P {
                // Padded coordinates; own cells are 1..=32.
                if open(x - CHUNK_PAD, y - CHUNK_PAD, z - CHUNK_PAD) {
                    blocks[GeneratedChunk::local_index(x, y, z)] = BlockId::Air;
                }
            }
        }
    }
    GeneratedChunk::new(coord, blocks)
}

fn links(chunk: &GeneratedChunk) -> ChunkConnectivity {
    ChunkConnectivity::compute(chunk, &AtomicBool::new(false)).expect("uncancelled")
}

fn pairs(c: ChunkConnectivity) -> Vec<(usize, usize)> {
    (0..6).flat_map(|a| (0..6).map(move |b| (a, b))).filter(|&(a, b)| a != b && c.connects(a, b)).collect()
}

#[test]
fn empty_chunk_links_every_pair() {
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Air, |_, _, _| false));
    assert_eq!(c, ChunkConnectivity::ALL_OPEN);
    assert_eq!(pairs(c).len(), 30);
}

#[test]
fn solid_chunk_links_nothing() {
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Stone, |_, _, _| false));
    assert_eq!(c, ChunkConnectivity::SEALED);
    assert_eq!(c.exits_from(0x3f), 0);
}

#[test]
fn straight_tunnel_links_only_its_two_ends() {
    let tunnel = |x: i32, y: i32, z: i32| (0..CHUNK_SIZE).contains(&x) && y == 16 && z == 16;
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Stone, tunnel));
    assert_eq!(pairs(c), vec![(0, 1), (1, 0)]);
    let vertical = |x: i32, y: i32, z: i32| x == 3 && (0..CHUNK_SIZE).contains(&y) && z == 30;
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Stone, vertical));
    assert_eq!(pairs(c), vec![(2, 3), (3, 2)]);
}

#[test]
fn wall_splits_the_faces_on_each_side() {
    // Stone plane at local x = 16; air elsewhere.
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Stone, |x, _, _| x != 16));
    assert!(!c.connects(0, 1), "-X and +X are on opposite sides of the wall");
    for side in 2..6 {
        assert!(c.connects(0, side) && c.connects(1, side) && c.connects(side, 0));
    }
    // Entering from -X can reach the side faces but never +X.
    assert_eq!(c.exits_from(1), 0b11_1101);
}

#[test]
fn halo_cells_do_not_link_faces() {
    // Only the one-voxel halo is open: the chunk's own cells are all stone.
    let halo = |x: i32, y: i32, z: i32| !(0..CHUNK_SIZE).contains(&x) || !(0..CHUNK_SIZE).contains(&y) || !(0..CHUNK_SIZE).contains(&z);
    assert_eq!(links(&chunk_with(IVec3::new(-3, -7, 2), BlockId::Stone, halo)), ChunkConnectivity::SEALED);
}

#[test]
fn see_through_blocks_count_as_open() {
    for block in [BlockId::Leaves, BlockId::Water, BlockId::Bush, BlockId::Torch] {
        let c = links(&chunk_with(IVec3::new(-1, -1, -1), block, |_, _, _| false));
        assert_eq!(pairs(c).len(), 30, "{block:?} must not occlude");
        assert_eq!(ChunkConnectivity::uniform(block).exits_from(1), 0x3f);
    }
    assert!(links(&chunk_with(IVec3::ZERO, BlockId::Torch, |_, _, _| false)).spills());
    assert!(occludes(BlockId::Stone) && !occludes(BlockId::Air));
}

#[test]
fn pocket_touching_one_face_links_nothing() {
    let pocket = |x: i32, y: i32, z: i32| x < 4 && (10..12).contains(&y) && (10..12).contains(&z);
    let c = links(&chunk_with(IVec3::ZERO, BlockId::Stone, pocket));
    assert!(pairs(c).is_empty());
    assert!(c.connects(0, 0), "the face itself has open cells");
}
