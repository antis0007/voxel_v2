//! `ChunkData` and the brick store: the only place blocks meet cells.
//!
//! A block becomes a cell of its material, full (`VOLUME_FULL`) at ambient
//! temperature. Water carries its volume: `water(local)` is the partial volume
//! of a water cell in eighths (1..8, the existing water map's unit) and `None`
//! means full. Cells hold sixty-fourths, so eighths scale by exactly 8. Light and torches are not cell state and stay in the chunk.
use crate::ChunkData;
use engine_content::BlockId;
use engine_core::material::MaterialId;
use engine_sim::{BrickStore, Cell, PoolFull, VOLUME_FULL};
use glam::IVec3;

/// Levels in the legacy water map: 8 means a full cell.
const WATER_MAP_FULL: u8 = 8;
const LEVELS_PER_EIGHTH: u8 = VOLUME_FULL / WATER_MAP_FULL;

fn cell_of(block: BlockId, water_volume: Option<u8>) -> Cell {
    match block {
        BlockId::Air => Cell::EMPTY,
        BlockId::Water => match water_volume.unwrap_or(WATER_MAP_FULL).min(WATER_MAP_FULL) {
            0 => Cell::EMPTY,
            eighths => Cell { volume: eighths * LEVELS_PER_EIGHTH, ..Cell::filled(MaterialId::from(block).0) },
        },
        _ => Cell::filled(MaterialId::from(block).0),
    }
}

/// Store `chunk` as bricks at `coord`, replacing what was there. On `PoolFull` the store is unchanged.
pub fn load_chunk(
    store: &mut BrickStore,
    coord: IVec3,
    chunk: &ChunkData,
    water: impl Fn(IVec3) -> Option<u8>,
) -> Result<(), PoolFull> {
    store.fill_chunk(coord, |local| {
        let block = chunk.blocks[ChunkData::local_index(local.x, local.y, local.z)];
        cell_of(block, if block == BlockId::Water { water(local) } else { None })
    })
}

/// Write the stored blocks of `coord` into `chunk` (cells of absent bricks become air) and return
/// the partial water volumes as (local cell, 1..7 eighths, rounded to nearest). A cell with volume 0 is air,
/// as in the water map, and one that rounds to a full eighth count is full.
pub fn read_chunk(store: &BrickStore, coord: IVec3, chunk: &mut ChunkData) -> Vec<(IVec3, u8)> {
    chunk.blocks.fill(BlockId::Air);
    let mut partial = Vec::new();
    store.for_each_cell(coord, |local, cell| {
        if cell.volume == 0 {
            return;
        }
        let block = BlockId::from_u32(cell.material as u32);
        chunk.blocks[ChunkData::local_index(local.x, local.y, local.z)] = block;
        let eighths = (cell.volume as u32 * WATER_MAP_FULL as u32 + VOLUME_FULL as u32 / 2) / VOLUME_FULL as u32;
        if block == BlockId::Water && (1..WATER_MAP_FULL as u32).contains(&eighths) {
            partial.push((local, eighths as u8));
        }
    });
    partial.sort_by_key(|(p, _)| ChunkData::local_index(p.x, p.y, p.z));
    partial
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_content::ALL_BLOCK_IDS;
    use proptest::prelude::*;
    use std::collections::HashSet;

    fn empty_chunk() -> ChunkData {
        ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks: Box::new([BlockId::Air; 32768]),
            light: Box::new([0; 32768]),
            torches: HashSet::new(),
        }
    }

    /// splitmix64: a fixed stream from a seed, so a failing case is one number.
    fn next(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A chunk of mostly air with `density`/256 of its cells set to random blocks, and the water volumes to match.
    fn random_chunk(seed: u64, density: u8) -> (ChunkData, Vec<(IVec3, u8)>) {
        let mut state = seed;
        let mut chunk = empty_chunk();
        let mut partial = Vec::new();
        for i in 0..32768usize {
            if (next(&mut state) & 255) as u8 >= density {
                continue;
            }
            let block = ALL_BLOCK_IDS[(next(&mut state) % ALL_BLOCK_IDS.len() as u64) as usize];
            chunk.blocks[i] = block;
            let volume = (next(&mut state) % 8) as u8; // 0..7: 0 is "full" in this draw
            if block == BlockId::Water && volume != 0 {
                let local = IVec3::new((i % 32) as i32, (i / 1024) as i32, ((i / 32) % 32) as i32);
                partial.push((local, volume));
            }
        }
        (chunk, partial)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// Chunk to bricks to chunk loses nothing: every block and every partial water volume survives.
        #[test]
        fn chunks_round_trip_through_bricks(seed in any::<u64>(), density in any::<u8>()) {
            let (chunk, partial) = random_chunk(seed, density);
            let volumes: std::collections::HashMap<IVec3, u8> = partial.iter().copied().collect();
            let mut store = BrickStore::new(64);
            let coord = IVec3::new(-3, 1, 7);
            load_chunk(&mut store, coord, &chunk, |p| volumes.get(&p).copied()).unwrap();
            let mut back = empty_chunk();
            let got = read_chunk(&store, coord, &mut back);
            prop_assert!(back.blocks[..] == chunk.blocks[..], "blocks differ");
            let mut want = partial;
            want.sort_by_key(|(p, _)| ChunkData::local_index(p.x, p.y, p.z));
            prop_assert_eq!(got, want);
        }
    }

    #[test]
    fn air_chunks_store_nothing_and_a_single_block_stores_one_brick() {
        let mut store = BrickStore::new(64);
        let mut chunk = empty_chunk();
        load_chunk(&mut store, IVec3::ZERO, &chunk, |_| None).unwrap();
        assert_eq!(store.bricks_in_use(), 0);
        chunk.blocks[ChunkData::local_index(9, 17, 30)] = BlockId::Stone;
        load_chunk(&mut store, IVec3::ZERO, &chunk, |_| None).unwrap();
        assert_eq!(store.bricks_in_use(), 1);
        assert_eq!(store.active_bricks(), 1);
    }

    /// A water-map volume outside 1..=8 is normalised: 0 is air (the water map's own rule) and
    /// anything above full is full. Neither may leave a stored brick that reads back differently.
    #[test]
    fn out_of_range_water_volumes_are_normalised() {
        let mut chunk = empty_chunk();
        let (zero, over, ok) = (IVec3::new(1, 1, 1), IVec3::new(2, 1, 1), IVec3::new(3, 1, 1));
        for p in [zero, over, ok] {
            chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)] = BlockId::Water;
        }
        let volume = |p: IVec3| Some(if p == zero { 0 } else if p == over { 200 } else { 5 });
        let mut store = BrickStore::new(64);
        load_chunk(&mut store, IVec3::ZERO, &chunk, volume).unwrap();
        let mut back = empty_chunk();
        let partial = read_chunk(&store, IVec3::ZERO, &mut back);
        assert_eq!(back.blocks[ChunkData::local_index(1, 1, 1)], BlockId::Air, "volume 0 is air");
        assert_eq!(back.blocks[ChunkData::local_index(2, 1, 1)], BlockId::Water, "over-full is full water");
        assert_eq!(partial, [(ok, 5)], "only the genuinely partial cell is reported");
        // A chunk whose only water has volume 0 stores nothing at all.
        let mut only_zero = empty_chunk();
        only_zero.blocks[ChunkData::local_index(1, 1, 1)] = BlockId::Water;
        let mut store = BrickStore::new(64);
        load_chunk(&mut store, IVec3::ZERO, &only_zero, |_| Some(0)).unwrap();
        assert_eq!(store.bricks_in_use(), 0);
    }

    /// The kernel on the real material table: what each block does comes from its row in
    /// `assets/materials.ron`, so the data and the rules are checked together.
    #[test]
    fn the_real_table_makes_sand_fall_dirt_stay_and_smoke_rise() {
        let put = |s: &mut BrickStore, p: IVec3, b: BlockId| {
            let (chunk, local) = engine_sim::split_voxel(p);
            s.set(chunk, local, cell_of(b, None)).unwrap();
        };
        let at = |s: &BrickStore, p: IVec3| {
            let (chunk, local) = engine_sim::split_voxel(p);
            s.get(chunk, local)
        };
        let mut s = BrickStore::new(500);
        for x in -4..4 {
            for z in -4..4 {
                put(&mut s, IVec3::new(x, 0, z), BlockId::Stone);
            }
        }
        // Sand and dirt hang in the air side by side; smoke sits on the floor under a lid.
        let (sand, dirt, smoke) = (IVec3::new(-2, 8, 0), IVec3::new(2, 8, 0), IVec3::new(0, 1, 3));
        put(&mut s, sand, BlockId::Sand);
        put(&mut s, dirt, BlockId::Dirt);
        put(&mut s, smoke, BlockId::Smoke);
        for x in -4..4 {
            put(&mut s, IVec3::new(x, 6, 3), BlockId::Stone);
        }
        let table = &*engine_content::SIM_TABLE;
        for tick in 0..60 {
            engine_sim::step(&mut s, table, 1, tick);
        }
        assert_eq!(at(&s, sand).volume, 0, "sand left its cell");
        assert_eq!(at(&s, IVec3::new(-2, 1, 0)).material, BlockId::Sand as u8, "and landed on the floor");
        assert_eq!(at(&s, dirt), cell_of(BlockId::Dirt, None), "dirt is cohesive and stays in the air");
        assert!(
            (2..6).any(|y| at(&s, IVec3::new(0, y, 3)).material == BlockId::Smoke as u8),
            "smoke rose from the floor towards the lid"
        );
        assert!(BlockId::Lava.sim().flow < BlockId::Water.sim().flow, "lava is the more viscous");
    }

    #[test]
    fn a_pool_too_small_leaves_the_store_as_it_was() {
        let (chunk, _) = random_chunk(7, 255);
        let mut store = BrickStore::new(3);
        assert_eq!(load_chunk(&mut store, IVec3::ZERO, &chunk, |_| None), Err(PoolFull));
        assert_eq!((store.bricks_in_use(), store.chunk_count()), (0, 0));
    }
}
