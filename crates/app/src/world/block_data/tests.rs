use super::*;
use crate::world::{ChunkData, WorldType};
use engine_core::{Budget, VoxelScalePreset};
use std::collections::HashSet;
use std::sync::Arc;

const SIGN: IVec3 = IVec3::new(10, 9, 10);
const CHEST: IVec3 = IVec3::new(12, 9, 10);

/// One resident chunk of stone below y = 8 and air above.
fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    let mut blocks = Box::new([BlockId::Air; 32 * 32 * 32]);
    for y in 0..9 {
        for z in 0..32 {
            for x in 0..32 {
                blocks[ChunkData::local_index(x, y, z)] = BlockId::Stone;
            }
        }
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks,
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|s| s.to_string()).collect()
}

#[test]
fn sign_text_keeps_four_lines_of_sixteen() {
    let mut w = world();
    w.set(SIGN, BlockId::WallSignSouth);
    assert_eq!(w.get(SIGN), BlockId::WallSignSouth);
    let long = lines(&["0123456789abcdefXYZ", "two\u{7}", "", "four", "five"]);
    assert!(set_text(&mut w, SIGN, &long));
    assert_eq!(w.block_data.text(SIGN), lines(&["0123456789abcdef", "two", "", "four"]).as_slice());
    // Blank text leaves no record.
    assert!(set_text(&mut w, SIGN, &lines(&["", ""])));
    assert!(w.block_data.entries.is_empty());
    assert!(!set_text(&mut w, IVec3::new(1, 3, 1), &long), "stone holds no text");
}

#[test]
fn chest_moves_stacks_and_spills_when_broken() {
    let mut w = world();
    w.set(CHEST, BlockId::Chest);
    w.inventory.hotbar = [None; crate::world::inventory::HOTBAR_SLOTS];
    w.inventory.pack = [None; crate::world::inventory::PACK_SLOTS];
    w.inventory.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Planks), 20));
    store(&mut w, CHEST, SlotAddr::Hotbar(0));
    assert_eq!(w.inventory.hotbar[0], None);
    assert_eq!(w.block_data.chest(CHEST)[0], Some(ItemStack::new(Item::Block(BlockId::Planks), 20)));
    take(&mut w, CHEST, 0);
    assert_eq!(w.inventory.hotbar[0].map(|s| s.count), Some(20));
    store(&mut w, CHEST, SlotAddr::Hotbar(0));
    let before = w.inventory.dropped.len();
    w.set(CHEST, BlockId::Air);
    assert!(w.block_data.entries.is_empty(), "breaking removes the record");
    assert_eq!(w.inventory.dropped.len(), before + 1);
    assert_eq!(w.inventory.dropped[before].stack.count, 20);
}

#[test]
fn sidecar_round_trips_and_rejects_damage() {
    let mut w = world();
    w.set(SIGN, BlockId::SignPost);
    w.set(CHEST, BlockId::Chest);
    assert!(set_text(&mut w, SIGN, &lines(&["north", "ünïcode"])));
    let mut slots = [None; CHEST_SLOTS];
    slots[3] = Some(ItemStack::new(Item::Resource(4), 7));
    w.block_data.put(CHEST, Entry::Chest(Box::new(slots)));
    tnt::arm(&mut w.block_data, IVec3::new(5, 9, 5), 77);
    let bytes = sidecar::encode(&w.block_data);
    let back = sidecar::decode(&bytes).expect("decodes");
    assert_eq!(back.entries, w.block_data.entries);
    assert_eq!(back.fuses, w.block_data.fuses);
    let mut bad = bytes.clone();
    bad[12] ^= 1;
    assert!(sidecar::decode(&bad).is_err());
    assert!(sidecar::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut future = bytes;
    future[7] = 9;
    assert!(sidecar::decode(&future).is_err(), "unknown version");
}

#[test]
fn tnt_blast_is_deterministic_and_bounded() {
    let cells = tnt::blast_cells(IVec3::ZERO, 1.0);
    assert_eq!(cells, tnt::blast_cells(IVec3::ZERO, 1.0));
    assert_eq!(cells[0], IVec3::ZERO);
    assert!(cells.iter().all(|c| c.as_vec3().length() <= tnt::RADIUS_M + 1e-4));
    let fine = tnt::blast_cells(IVec3::ZERO, 0.05);
    assert!(fine.iter().all(|c| c.as_vec3().length() <= 6.0 + 1e-4), "voxel radius is capped");

    let run = || {
        let mut w = world();
        let centre = IVec3::new(16, 8, 16);
        w.set(centre, BlockId::Tnt);
        w.set(centre + IVec3::X * 2, BlockId::Tnt);
        assert!(tnt::ignite(&mut w, centre));
        let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
        let eye = (centre.as_vec3() + Vec3::new(0.5, 1.5, 3.5)) * w.voxel_scale.meters();
        for _ in 0..tnt::FUSE_STEPS {
            tnt::tick(&mut w, eye, &mut blocks, &mut chunks, Budget::UNBOUNDED);
        }
        let removed: Vec<_> = (0..32).flat_map(|y| (0..32).flat_map(move |z| (0..32).map(move |x| IVec3::new(x, y, z))))
            .filter(|&p| p.y <= 8 && w.get(p) == BlockId::Air)
            .collect();
        let fuses = w.block_data.fuses.clone();
        (removed, fuses, w.inventory.dropped.len())
    };
    let (removed, fuses, drops) = run();
    assert!(removed.contains(&IVec3::new(16, 8, 16)) && removed.len() > 20);
    assert_eq!(fuses.len(), 1, "the neighbouring charge is lit by the blast");
    assert!(drops > 0 && drops < removed.len(), "drops are reduced");
    assert_eq!(run(), (removed, fuses, drops));
}

#[test]
fn fire_lights_tnt() {
    let mut w = world();
    let p = IVec3::new(4, 9, 4);
    w.set(p, BlockId::Tnt);
    w.set(p, BlockId::Fire);
    assert_eq!(w.block_data.fuses.get(&key(p)), Some(&tnt::FUSE_STEPS));
}
