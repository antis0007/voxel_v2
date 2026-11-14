use super::*;
use crate::headless::Session;
use crate::world::inventory::{Item, ItemStack};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;

fn crusher() -> Node {
    Node::new(Kind::Crusher)
}

#[test]
fn crusher_turns_stone_into_gravel_without_creating_mass() {
    let mut factory = Factory::default();
    let mut node = crusher();
    node.blocks.input[blocks::Material::Stone as usize] = 2;
    node.energy = 4096;
    factory.nodes.insert(IVec3::ZERO, node);
    for _ in 0..blocks::TICKS {
        factory.tick(0.25, 1);
    }
    let node = &factory.nodes[&IVec3::ZERO];
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 1);
    assert_eq!(node.blocks.output[blocks::Material::Gravel as usize], 1);
    assert_eq!(node.blocks.output[blocks::Material::Sand as usize], 0);
    assert_eq!(node.energy, 4096 - blocks::POWER * blocks::TICKS);
}

#[test]
fn crusher_turns_gravel_into_sand_after_stone_feed_is_exhausted() {
    let mut node = crusher();
    node.blocks.input[blocks::Material::Gravel as usize] = 1;
    node.energy = 4096;
    for _ in 0..blocks::TICKS {
        assert_eq!(blocks::step(&mut node), blocks::POWER);
    }
    assert_eq!(node.blocks.input[blocks::Material::Gravel as usize], 0);
    assert_eq!(node.blocks.output[blocks::Material::Sand as usize], 1);
    assert_eq!(node.progress, 0);
}

#[test]
fn full_crusher_output_preserves_feed_progress_and_energy() {
    let mut node = crusher();
    node.blocks.input[blocks::Material::Stone as usize] = 1;
    node.blocks.output[blocks::Material::Gravel as usize] = CAPACITY;
    node.progress = 3;
    node.energy = 100;
    assert_eq!(blocks::step(&mut node), 0);
    assert_eq!(node.progress, 3);
    assert_eq!(node.energy, 100);
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 1);
    assert_eq!(node.status, "Output blocked");
}

#[test]
fn block_loading_and_collection_are_lossless() {
    let mut inventory = crate::world::inventory::SurvivalInventory::default();
    assert_eq!(
        inventory
            .try_insert(ItemStack {
                item: Item::Block(B::Stone),
                count: 3
            })
            .1,
        0
    );
    let mut node = crusher();
    blocks::load(&mut inventory, &mut node, 1, &[0; RESOURCE_COUNT]);
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 3);
    assert_eq!(inventory.count_item(Item::Block(B::Stone)), 0);
    node.blocks.output[blocks::Material::Gravel as usize] = 2;
    blocks::collect(&mut inventory, &mut node);
    assert_eq!(inventory.count_item(Item::Block(B::Stone)), 3);
    assert_eq!(inventory.count_item(Item::Block(B::Gravel)), 2);
    assert!(
        node.blocks
            .input
            .iter()
            .chain(node.blocks.output.iter())
            .all(|count| *count == 0)
    );
}

#[test]
fn active_ore_batch_finishes_before_block_feed() {
    let mut factory = Factory::default();
    let mut node = crusher();
    node.input[0] = 64;
    node.blocks.input[blocks::Material::Stone as usize] = 1;
    node.energy = 4096;
    factory.nodes.insert(IVec3::ZERO, node);
    for _ in 0..16 {
        factory.tick(0.25, 1);
    }
    let node = &factory.nodes[&IVec3::ZERO];
    assert_eq!(node.input[0], 0);
    assert_eq!(node.output[9], 128);
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 1);
    assert_eq!(node.blocks.output[blocks::Material::Gravel as usize], 0);
    for _ in 0..blocks::TICKS {
        factory.tick(0.25, 1);
    }
    let node = &factory.nodes[&IVec3::ZERO];
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 0);
    assert_eq!(node.blocks.output[blocks::Material::Gravel as usize], 1);
}

#[test]
fn full_pack_leaves_every_block_in_the_machine() {
    let mut inventory = crate::world::inventory::SurvivalInventory::default();
    for slot in inventory.hotbar.iter_mut().chain(inventory.pack.iter_mut()) {
        *slot = Some(ItemStack {
            item: Item::Block(B::Dirt),
            count: 64,
        });
    }
    let mut node = crusher();
    node.blocks.input[blocks::Material::Stone as usize] = 3;
    node.blocks.output[blocks::Material::Sand as usize] = 2;
    blocks::collect(&mut inventory, &mut node);
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 3);
    assert_eq!(node.blocks.output[blocks::Material::Sand as usize], 2);
    assert_eq!(inventory.count_item(Item::Block(B::Stone)), 0);
    assert_eq!(inventory.count_item(Item::Block(B::Sand)), 0);
}

fn drill_fixture(scale: VoxelScalePreset, blocked: bool) -> (Session, IVec3, IVec3) {
    let mut session = Session::new(WorldType::Superflat, scale, 71, None);
    let size = super::step(&session.world);
    let p = IVec3::new(16 * size, -4 * size, 0);
    let target = p + quarry::offset(0, size, 0);
    session.world.set(target, B::Stone);
    let node = fixture_place(&mut session.world, p, Kind::MiningDrill).expect("drill fixture");
    node.energy = 100_000;
    if blocked {
        node.blocks.output[blocks::Material::Stone as usize] = CAPACITY;
    }
    (session, p, target)
}

#[test]
fn drill_recovers_one_whole_block_per_cubic_metre_at_every_scale() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        let (mut session, p, target) = drill_fixture(scale, false);
        let end = session.ticks + 240;
        while session.ticks < end
            && session.world.factory.nodes[&p].blocks.output[blocks::Material::Stone as usize] == 0
        {
            session.tick();
        }
        assert!(session.ticks < end, "drill did not produce at {scale:?}");
        assert_eq!(session.world.get(target), B::Air);
        let node = &session.world.factory.nodes[&p];
        assert_eq!(
            node.blocks.output[blocks::Material::Stone as usize],
            1,
            "{scale:?}"
        );
        assert!(node.excavation >= 1, "{scale:?}");
    }
}

#[test]
fn blocked_drill_leaves_terrain_and_energy_untouched() {
    let (mut session, p, target) = drill_fixture(VoxelScalePreset::OneMeter, true);
    let energy = session.world.factory.nodes[&p].energy;
    let end = session.ticks + 120;
    while session.ticks < end {
        session.tick();
    }
    let node = &session.world.factory.nodes[&p];
    assert_eq!(session.world.get(target), B::Stone);
    assert_eq!(node.excavation, 0);
    assert_eq!(node.energy, energy);
}

#[test]
fn factory_format_11_round_trips_block_buffers_and_progress() {
    let mut factory = Factory::default();
    let mut node = crusher();
    node.blocks.input[blocks::Material::Stone as usize] = 7;
    node.blocks.output[blocks::Material::Sand as usize] = 4;
    node.progress = 5;
    factory.nodes.insert(IVec3::new(0, 32, 0), node);
    let mut bytes = Vec::new();
    factory.encode(&mut bytes);
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut cursor = words.into_iter();
    let decoded = Factory::decode(
        &mut || cursor.next().ok_or_else(|| anyhow::anyhow!("eof")),
        1,
        FACTORY_FORMAT,
    )
    .unwrap();
    let node = &decoded.nodes[&IVec3::new(0, 32, 0)];
    assert_eq!(node.blocks.input[blocks::Material::Stone as usize], 7);
    assert_eq!(node.blocks.output[blocks::Material::Sand as usize], 4);
    assert_eq!(node.progress, 5);
}

#[test]
fn factory_format_10_loads_with_empty_block_buffers() {
    let mut factory = Factory::default();
    factory.nodes.insert(IVec3::ZERO, crusher());
    let mut bytes = Vec::new();
    factory.encode_format(&mut bytes, 10);
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut cursor = words.into_iter();
    let decoded = Factory::decode(
        &mut || cursor.next().ok_or_else(|| anyhow::anyhow!("eof")),
        1,
        10,
    )
    .unwrap();
    let node = &decoded.nodes[&IVec3::ZERO];
    assert!(
        node.blocks
            .input
            .iter()
            .chain(node.blocks.output.iter())
            .all(|count| *count == 0)
    );
}
