use super::*;
use crate::world::{World, BlockId, scanner::policy};
use crate::world::inventory::ItemStack;
use engine_core::{Budget, VoxelScalePreset};
use engine_voxel::WorldType;
use glam::IVec3;

const POS: IVec3 = IVec3::new(-16, 64, -16);

fn world() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7)
}

fn node(name: &str) -> Node {
    let mut n = Node::new(Kind::Assembler);
    n.target = all_recipes().iter().position(|r| r.name == name).unwrap() as u32 + 1;
    assert!(recipe(&n).is_some());
    n.energy = 64;
    n
}

fn chest(w: &mut World, pos: IVec3, items: &[(Item, u32)]) {
    w.set(pos, BlockId::Chest);
    let mut slots = vec![None; BlockId::Chest.chest_slots()];
    for (slot, &(item, count)) in slots.iter_mut().zip(items) {
        *slot = Some(ItemStack::new(item, count));
    }
    w.block_data.set_chest(pos, slots);
}

fn count(w: &World, pos: IVec3, item: Item) -> u32 {
    w.block_data.chest(pos).iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum()
}

fn transfer(w: &mut World, n: &mut Node) {
    n.work_clock = chests::INTERVAL;
    chests(w, POS, n, 1, Budget::UNBOUNDED);
}

#[test]
fn assembler_chests_feed_ingots_and_return_finished_coils() {
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 4), (Item::Resource(4), 2)]);
    let revision = w.block_data.storage_revision;
    transfer(&mut w, &mut n);
    assert_eq!(n.input[6], 128);
    assert_eq!(n.input[4], 64);
    assert_eq!(count(&w, p, Item::Resource(6)), 2);
    assert!(w.block_data.storage_revision > revision);
    w.factory.nodes.insert(POS, n);
    for _ in 0..CRAFT_TICKS { w.factory.run_assemblers(); }
    let mut n = w.factory.nodes.remove(&POS).unwrap();
    assert_eq!(n.output[16], 64);
    transfer(&mut w, &mut n);
    assert_eq!(count(&w, p, Item::Resource(16)), 1);
    assert_eq!(n.output[16], 0);
    assert_eq!(n.input[6], 128);
    assert_eq!(n.input[4], 64);
}

#[test]
fn assembler_chests_keep_partial_units_and_respect_input_reservations() {
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 8), (Item::Resource(4), 8)]);
    n.input[6] = 65;
    n.input[4] = 63;
    w.factory.reserved.insert((POS, 6), 63);
    transfer(&mut w, &mut n);
    assert_eq!(n.input[6], 65, "incoming pipe already funds the copper");
    assert_eq!(n.input[4], 127, "whole items never discard partial pipe units");
    n.enabled = false;
    n.output[16] = 127;
    transfer(&mut w, &mut n);
    assert_eq!(count(&w, p, Item::Resource(16)), 1);
    assert_eq!(n.output[16], 63);
    assert_eq!(count(&w, p, Item::Resource(6)), 8);
}

#[test]
fn assembler_chests_respect_reserves_filters_and_output_backpressure() {
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 4), (Item::Resource(4), 3)]);
    policy::set(&mut w, p, policy::Policy { reserve: 3, assigned: Some(Item::Resource(6)), ..Default::default() });
    n.output[16] = 128;
    transfer(&mut w, &mut n);
    assert_eq!(n.input[6], 64);
    assert_eq!(n.input[4], 0);
    assert_eq!(n.output[16], 128, "assigned chest refuses the coil");
    assert_eq!(count(&w, p, Item::Resource(6)), 3);
    policy::set(&mut w, p, policy::Policy { target: 1, ..Default::default() });
    transfer(&mut w, &mut n);
    assert_eq!(n.output[16], 64);
    assert_eq!(count(&w, p, Item::Resource(16)), 1);
    n.enabled = false;
    let revision = w.block_data.storage_revision;
    transfer(&mut w, &mut n);
    assert_eq!(n.output[16], 64);
    assert_eq!(w.block_data.storage_revision, revision, "unchanged chest is not republished");
}

#[test]
fn assembler_chests_roles_prevent_wrong_direction_and_off_transfers() {
    use crate::world::factory::sides::{self, Role};
    for role in [Role::In, Role::Out, Role::Off] {
        let mut w = world();
        let mut n = node("Hand-wound induction coil");
        let p = POS + IVec3::X;
        chest(&mut w, p, &[(Item::Resource(6), 2), (Item::Resource(4), 1)]);
        n.output[16] = 64;
        assert!(sides::apply_role(&mut n, 0, role as u8));
        transfer(&mut w, &mut n);
        assert_eq!(n.input[6], if role == Role::In { 128 } else { 0 });
        assert_eq!(n.output[16], if role == Role::Out { 0 } else { 64 });
    }
}

#[test]
fn assembler_chests_share_output_limit_and_wait_for_simulated_time() {
    for name in ["Hand-wound induction coil", "Bucket"] {
        let mut w = world();
        let mut n = node(name);
        n.enabled = false;
        let item = recipe(&n).unwrap().output.item;
        if let Item::Resource(i) = item { n.output[i as usize] = 48 * 64; } else { n.stock = 48; }
        for dir in crate::world::factory::DIRS { chest(&mut w, POS + dir, &[]); }
        n.work_clock = chests::INTERVAL - 0.001;
        chests(&mut w, POS, &mut n, 1, Budget::UNBOUNDED);
        assert_eq!(count(&w, POS + IVec3::X, item), 0);
        transfer(&mut w, &mut n);
        let total = || crate::world::factory::DIRS.iter().map(|&d| count(&w, POS + d, item)).sum::<u32>();
        assert_eq!(total(), 16, "one allowance for all faces");
        chests(&mut w, POS, &mut n, 1, Budget::UNBOUNDED);
        assert_eq!(crate::world::factory::DIRS.iter().map(|&d| count(&w, POS + d, item)).sum::<u32>(), 16);
    }
}

#[test]
fn assembler_chests_disabled_and_full_containers_preserve_stock() {
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 4), (Item::Resource(4), 4)]);
    policy::set(&mut w, p, policy::Policy { enabled: false, ..Default::default() });
    n.output[16] = 128;
    transfer(&mut w, &mut n);
    assert_eq!(n.input[6], 0);
    assert_eq!(n.output[16], 128);
    policy::set(&mut w, p, policy::Policy::default());
    w.block_data.set_chest(p, vec![Some(ItemStack::new(Item::Block(BlockId::Dirt), 64)); BlockId::Chest.chest_slots()]);
    transfer(&mut w, &mut n);
    assert_eq!(n.output[16], 128);
}

#[test]
fn assembler_chests_preserve_bulk_block_recipes_at_fine_scale() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 7);
    let mut n = node("Seared Brick");
    let p = POS + IVec3::new(2, 1, 1);
    let inputs = recipe(&n).unwrap().ingredients;
    chest(&mut w, p, inputs);
    n.work_clock = chests::INTERVAL;
    chests(&mut w, POS, &mut n, 2, Budget::UNBOUNDED);
    for (material, count) in block_needs(recipe(&n).unwrap()) { assert_eq!(n.blocks.input[material.index()], count); }
    w.factory.nodes.insert(POS, n);
    for _ in 0..CRAFT_TICKS { w.factory.run_assemblers(); }
    let mut n = w.factory.nodes.remove(&POS).unwrap();
    n.work_clock = chests::INTERVAL;
    chests(&mut w, POS, &mut n, 2, Budget::UNBOUNDED);
    assert_eq!(count(&w, p, Item::Block(BlockId::SearedBrick)), 4);
    assert_eq!(n.stock, 0);
}

#[test]
fn assembler_chests_paused_output_drains_through_the_world_scheduler() {
    use crate::simulation::residency::Residency;
    use std::collections::HashSet;
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    n.enabled = false;
    n.output[16] = 129;
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 2)]);
    w.factory.nodes.insert(POS, n);
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let player = crate::player::Player::new();
    for _ in 0..29 {
        crate::world::factory::tick_world(&mut w, &player, crate::simulation::STEP_SECONDS,
            &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
    }
    assert_eq!(count(&w, p, Item::Resource(16)), 0);
    for _ in 0..2 {
        crate::world::factory::tick_world(&mut w, &player, crate::simulation::STEP_SECONDS,
            &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
    }
    assert_eq!(count(&w, p, Item::Resource(16)), 2);
    assert_eq!(w.factory.nodes[&POS].output[16], 1);
    assert_eq!(count(&w, p, Item::Resource(6)), 2, "paused intake stays paused");
}

#[test]
fn assembler_chests_resume_mid_batch_after_factory_and_chest_round_trip() {
    let mut w = world();
    let mut n = node("Hand-wound induction coil");
    let p = POS + IVec3::X;
    chest(&mut w, p, &[(Item::Resource(6), 2), (Item::Resource(4), 1)]);
    transfer(&mut w, &mut n);
    w.factory.nodes.insert(POS, n);
    for _ in 0..4 { w.factory.run_assemblers(); }
    let mut bytes = Vec::new();
    w.factory.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    w.factory = Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, crate::world::factory::FACTORY_FORMAT).unwrap();
    assert_eq!(w.factory.nodes[&POS].progress, 4);
    let bytes = crate::world::block_data::sidecar::encode(&w.block_data);
    w.block_data = crate::world::block_data::sidecar::decode(&bytes).unwrap();
    for _ in 0..4 { w.factory.run_assemblers(); }
    let mut n = w.factory.nodes.remove(&POS).unwrap();
    transfer(&mut w, &mut n);
    assert_eq!(count(&w, p, Item::Resource(16)), 1);
    assert_eq!(count(&w, p, Item::Resource(6)), 0);
    assert_eq!(n.input[6] + n.input[4] + n.output[16], 0);
    assert_eq!(n.energy, 64 - POWER * CRAFT_TICKS);
}
