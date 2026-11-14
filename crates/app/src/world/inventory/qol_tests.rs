use super::*;
use crate::world::{World, WorldType, scanner};
use engine_core::VoxelScalePreset;

const COBBLE: Item = Item::Block(BlockId::Cobblestone);
const PICK: Item = Item::Tool(8);

fn bare() -> SurvivalInventory {
    SurvivalInventory::empty()
}

#[test]
fn emptied_hotbar_stack_refills_from_the_smallest_pack_stack() {
    let mut inv = bare();
    inv.hotbar[2] = Some(ItemStack::new(COBBLE, 1));
    inv.hotbar[3] = Some(ItemStack::new(COBBLE, 5));
    inv.pack[0] = Some(ItemStack::new(COBBLE, 40));
    inv.pack[7] = Some(ItemStack::new(COBBLE, 9));
    assert!(inv.consume_from_hotbar(2, COBBLE));
    assert_eq!(inv.hotbar[2], Some(ItemStack::new(COBBLE, 9)), "the smallest pack stack moves up whole");
    assert_eq!(inv.pack[7], None);
    assert_eq!(inv.hotbar[3], Some(ItemStack::new(COBBLE, 5)), "other hotbar slots keep their layout");
    assert_eq!(inv.count_item(COBBLE), 54, "no unit is lost or made");
}

#[test]
fn refill_needs_an_empty_slot_and_a_matching_pack_stack() {
    let mut inv = bare();
    inv.hotbar[0] = Some(ItemStack::new(COBBLE, 1));
    assert!(inv.consume_from_hotbar(0, COBBLE));
    assert_eq!(inv.hotbar[0], None, "nothing to refill from");
    inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Planks), 3));
    inv.pack[0] = Some(ItemStack::new(COBBLE, 3));
    assert!(!inv.refill_hotbar(0, COBBLE), "an occupied slot is never overwritten");
    assert!(!inv.refill_hotbar(HOTBAR_SLOTS, COBBLE));
}

#[test]
fn pick_block_prefers_hotbar_then_empty_slots_then_the_selected_slot() {
    let mut inv = bare();
    inv.hotbar[5] = Some(ItemStack::new(COBBLE, 2));
    assert_eq!(inv.pick_into_hotbar(COBBLE, 0), Some(5), "already in the hotbar");
    let planks = Item::Block(BlockId::Planks);
    inv.pack[3] = Some(ItemStack::new(planks, 4));
    inv.pack[9] = Some(ItemStack::new(planks, 30));
    inv.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Torch), 8));
    assert_eq!(inv.pick_into_hotbar(planks, 0), Some(1), "first empty slot, the torch stays in hand");
    assert_eq!(inv.hotbar[1], Some(ItemStack::new(planks, 30)), "the largest pack stack");
    assert_eq!(inv.hotbar[0].map(|s| s.item), Some(Item::Block(BlockId::Torch)));
    for i in 0..HOTBAR_SLOTS {
        inv.hotbar[i].get_or_insert(ItemStack::new(Item::Block(BlockId::Dirt), 1));
    }
    let gravel = Item::Block(BlockId::Gravel);
    inv.pack[20] = Some(ItemStack::new(gravel, 6));
    assert_eq!(inv.pick_into_hotbar(gravel, 4), Some(4), "a full hotbar swaps with the selection");
    assert_eq!(inv.pack[20].map(|s| s.item), Some(Item::Block(BlockId::Dirt)), "the swapped stack is kept");
    assert_eq!(inv.pick_into_hotbar(Item::Block(BlockId::Sand), 4), None);
}

/// One resident air chunk at the origin, so block edits land.
fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    w.chunks.insert(glam::IVec3::ZERO, std::sync::Arc::new(crate::world::ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks: Box::new([BlockId::Air; 32 * 32 * 32]),
        light: Box::new([0; 32 * 32 * 32]),
        torches: std::collections::HashSet::new(),
    }));
    w.inventory = bare();
    w
}

#[test]
fn broken_hotbar_tool_is_replaced_from_the_pack_with_its_own_wear() {
    let mut w = world();
    w.rules.mode = crate::world::rules::GameMode::Survival;
    let max = crate::world::progression::table::max_durability(PICK).expect("pickaxe wears");
    w.inventory.hotbar[0] = Some(ItemStack::new(PICK, 1));
    w.inventory.pack[4] = Some(ItemStack::new(PICK, 1));
    let pack = crate::world::progression::Slot::Pack(4);
    let hand = crate::world::progression::Slot::Hotbar(0);
    w.progression.wear.reconcile(&w.inventory);
    crate::world::progression::spend(&mut w, pack, 3);
    crate::world::progression::spend(&mut w, hand, max);
    assert_eq!(w.inventory.hotbar[0].map(|s| s.item), Some(PICK), "the spare moves into the hand");
    assert_eq!(w.inventory.pack[4], None);
    assert_eq!(w.progression.wear.durability(&w.inventory, hand), Some((max - 3, max)), "the spare keeps its wear");
}

#[test]
fn chest_take_all_and_sort_conserve_units() {
    let mut w = world();
    let p = glam::IVec3::new(3, 9, 3);
    w.set(p, BlockId::Chest);
    assert_eq!(w.get(p), BlockId::Chest);
    let mut slots = [None; crate::world::block_data::CHEST_SLOTS];
    slots[4] = Some(ItemStack::new(COBBLE, 30));
    slots[9] = Some(ItemStack::new(Item::Resource(6), 7));
    slots[20] = Some(ItemStack::new(COBBLE, 12));
    w.block_data.set_chest(p, slots);
    let one = scanner::chest(p);
    scanner::sort(&mut w, &one);
    let sorted = w.block_data.chest(p);
    assert!(sorted[..2].iter().all(Option::is_some) && sorted[2..].iter().all(Option::is_none), "{sorted:?}");
    assert_eq!(sorted.iter().flatten().filter(|s| s.item == COBBLE).map(|s| s.count).sum::<u32>(), 42);
    assert_eq!(scanner::take_all(&mut w, &one), 49);
    assert!(w.block_data.chest(p).iter().all(Option::is_none));
    assert_eq!(w.inventory.count_item(COBBLE), 42);
}
