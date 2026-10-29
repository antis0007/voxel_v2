use super::brewing::{self, Role};
use super::*;
use crate::world::block_data::{Stand, sidecar};
use crate::world::inventory::{FoodKind, SurvivalInventory};
use crate::world::progression::Kind;
use crate::world::{BlockId, ChunkData, WorldType};
use engine_core::VoxelScalePreset;
use glam::IVec3;
use std::collections::HashSet;
use std::sync::Arc;

const SWORD: Item = Item::Weapon(2);
const STEEL: Item = Item::Resource(15);

fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    w.inventory = SurvivalInventory::empty();
    w.rules.mode = crate::world::rules::GameMode::Survival;
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks: Box::new([BlockId::Air; 32 * 32 * 32]),
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w
}

/// A sword in hotbar slot `i` with `used` durability spent.
fn worn_sword(w: &mut World, i: usize, used: u32) {
    w.inventory.hotbar[i] = Some(ItemStack::new(SWORD, 1));
    w.progression.wear.reconcile(&w.inventory);
    let _ = w.progression.wear.spend(&mut w.inventory, Slot::Hotbar(i as u8), used);
}

fn max() -> u32 {
    crate::world::progression::table::max_durability(SWORD).expect("durable sword")
}

#[test]
fn anvil_repair_uses_whole_units_and_levels() {
    let max = max();
    let per = (max as f32 * STATIONS.repair_per_unit).ceil() as u32;
    // Damage of one and a half units needs two units.
    let damage = per + per / 2;
    let o = anvil(SWORD, max - damage, max, ItemStack::new(STEEL, 5), None).expect("repair");
    assert_eq!(o, Outcome { left: max, consumed: 2, levels: 2 * STATIONS.levels_per_unit });
    let o = anvil(SWORD, max - damage, max, ItemStack::new(STEEL, 1), None).expect("partial");
    assert_eq!((o.left, o.consumed), (max - damage + per, 1));
    assert_eq!(anvil(SWORD, max, max, ItemStack::new(STEEL, 1), None), None, "undamaged");
    assert_eq!(anvil(SWORD, 1, max, ItemStack::new(Item::Resource(6), 1), None), None, "wrong material");
}

#[test]
fn level_cost_keeps_progress() {
    // Level 5 needs 7 + 9 + 11 + 13 + 15 = 55 points; 3 more is progress into level 5.
    assert_eq!(xp::level_of(58), (5, 3));
    let left = spend_levels(58, 2).expect("enough levels");
    assert_eq!(xp::level_of(left), (3, 3));
    assert_eq!(spend_levels(58, 6), None);
}

#[test]
fn anvil_repair_in_inventory_charges_xp() {
    let mut w = world();
    let max = max();
    worn_sword(&mut w, 0, max / 2);
    w.inventory.hotbar[1] = Some(ItemStack::new(STEEL, 8));
    let (a, b) = (SlotAddr::Hotbar(0), SlotAddr::Hotbar(1));
    w.progression.xp = 0;
    assert!(!use_anvil(&mut w, a, b), "no levels to pay");
    w.progression.xp = 100;
    let before = w.progression.level();
    let o = anvil_preview(&w, a, b).expect("preview");
    assert!(use_anvil(&mut w, a, b));
    assert_eq!(durable(&w, a), Some((SWORD, max, max)));
    assert_eq!(w.inventory.hotbar[1].map(|s| s.count), Some(8 - o.consumed));
    assert_eq!(w.progression.level(), before - o.levels);
}

#[test]
fn anvil_and_grindstone_combine_two_worn_items() {
    let mut w = world();
    let max = max();
    worn_sword(&mut w, 0, max - 20);
    worn_sword(&mut w, 1, max - 30);
    let (a, b) = (SlotAddr::Hotbar(0), SlotAddr::Hotbar(1));
    let anvil = anvil_preview(&w, a, b).expect("anvil combine");
    let grind = grindstone_preview(&w, a, b).expect("grindstone combine");
    assert_eq!(grind.levels, 0);
    assert_eq!(grind.left, 50 + (max as f32 * STATIONS.grindstone_combine_bonus) as u32);
    assert!(anvil.left > grind.left && anvil.levels > 0);
    assert!(use_grindstone(&mut w, a, b));
    assert_eq!(durable(&w, a), Some((SWORD, grind.left, max)));
    assert_eq!(w.inventory.hotbar[1], None);
    assert!(w.progression.wear.entries.iter().all(|e| e.slot != Slot::Hotbar(1)));
}

#[test]
fn grindstone_salvage_refunds_by_durability() {
    let mut w = world();
    let max = max();
    let row = STATIONS.salvage.iter().find(|r| r.item == SWORD).expect("salvage row");
    assert_eq!(salvage(SWORD, max, max).map(|s| s.count), Some(row.count));
    assert_eq!(salvage(SWORD, max / (2 * row.count + 1), max), None, "too worn");
    worn_sword(&mut w, 0, 1);
    assert!(use_salvage(&mut w, SlotAddr::Hotbar(0)));
    assert_eq!(w.inventory.count_item(SWORD), 0);
    assert_eq!(w.inventory.count_item(row.material), (row.count * (max - 1) + max / 2) / max);
}

#[test]
fn stonecutter_lists_and_cuts_one_to_one() {
    let mut w = world();
    let cuts: Vec<_> = STATIONS.cuts(Item::Block(BlockId::Cobblestone)).collect();
    assert!(cuts.iter().any(|c| c.output == Item::Block(BlockId::CobbleSlab) && c.count == 2));
    assert!(cuts.iter().all(|c| c.count == 1 || c.output == Item::Block(BlockId::CobbleSlab)));
    w.inventory.hotbar[0] = Some(ItemStack::new(Item::Block(BlockId::Cobblestone), 3));
    let index = STATIONS
        .stonecutter
        .iter()
        .position(|c| c.input == Item::Block(BlockId::Cobblestone) && c.output == Item::Block(BlockId::CobbleStairs))
        .expect("stairs row");
    assert!(use_stonecutter(&mut w, SlotAddr::Hotbar(0), index));
    assert_eq!(w.inventory.count_item(Item::Block(BlockId::Cobblestone)), 2);
    assert_eq!(w.inventory.count_item(Item::Block(BlockId::CobbleStairs)), 1);
    let stone = STATIONS.stonecutter.iter().position(|c| c.input == Item::Block(BlockId::Stone)).expect("stone row");
    assert!(!use_stonecutter(&mut w, SlotAddr::Hotbar(0), stone), "wrong input");
}

fn regen_stand() -> Stand {
    let row = brewing::resolve(Item::Resource(18), Item::Resource(13)).expect("regeneration row");
    assert_eq!(row.output, FoodKind::RegenerationPotion);
    Stand {
        base: Some(ItemStack::new(row.base, 2)),
        modifier: Some(ItemStack::new(row.modifier, 1)),
        ..Stand::default()
    }
}

#[test]
fn brew_takes_its_steps_and_needs_water() {
    assert_eq!(brewing::role(Item::Resource(18)), Some(Role::Base));
    assert_eq!(brewing::role(Item::Resource(13)), Some(Role::Modifier));
    assert_eq!(brewing::role(STEEL), None);
    let mut stand = regen_stand();
    assert!(!brewing::advance(&mut stand, false), "dry stand holds");
    let steps = brewing::brew_steps();
    assert_eq!(steps as f32, (table::BREWING.seconds / crate::simulation::STEP_SECONDS).round());
    for _ in 1..steps {
        assert!(brewing::advance(&mut stand, true));
    }
    assert_eq!(stand.output, None);
    assert!(brewing::advance(&mut stand, true));
    let potion = Item::Food(FoodKind::RegenerationPotion);
    assert_eq!(stand.output, Some(ItemStack::new(potion, 3)));
    assert_eq!((stand.base.map(|s| s.count), stand.modifier, stand.steps), (Some(1), None, 0));
    // Without a modifier the brew stops.
    assert!(!brewing::advance(&mut stand, true));
}

#[test]
fn stand_ticks_beside_water_and_survives_the_sidecar() {
    let mut w = world();
    let p = IVec3::new(4, 10, 4);
    w.set(p, BlockId::BrewingStand);
    w.set(p + IVec3::X, BlockId::Water);
    let mut stand = regen_stand();
    stand.steps = 7;
    w.block_data.set_stand(p, stand);
    let bytes = sidecar::encode(&w.block_data);
    let back = sidecar::decode(&bytes).expect("decode");
    assert_eq!(back.stand(p), stand);
    assert!(back.stands.contains(&(p.x, p.y, p.z)));
    brewing::tick(&mut w);
    assert_eq!(w.block_data.stand(p).steps, 8);
    // Breaking the stand spills its slots.
    w.set(p, BlockId::Air);
    assert!(w.block_data.stands.is_empty());
    assert_eq!(w.inventory.dropped.len(), 2);
}

#[test]
fn drinking_a_potion_grants_its_effect_when_full() {
    let mut w = world();
    w.adventure.hunger = 100.0;
    w.adventure.health = w.adventure.max_health();
    let potion = FoodKind::SpeedPotion;
    w.inventory.hotbar[0] = Some(ItemStack::new(Item::Food(potion), 1));
    assert!(w.eat_food(potion, Some(0)));
    assert!(w.progression.effects.list.iter().any(|e| e.kind == Kind::Speed));
    assert_eq!(w.inventory.count_item(Item::Food(potion)), 0);
}
