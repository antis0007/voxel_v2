use super::effects::{Effects, Kind};
use super::wear::{Slot, Wear};
use super::*;
use crate::world::inventory::{ArmorTier, EquipSlot, Item, ItemStack, SurvivalInventory};

const PICK: Item = Item::Weapon(2);

fn inv_with_pick() -> SurvivalInventory {
    let mut inv = SurvivalInventory::empty();
    inv.hotbar[0] = Some(ItemStack::new(PICK, 1));
    inv
}

#[test]
fn wear_follows_a_moved_stack() {
    let mut inv = inv_with_pick();
    let mut wear = Wear::default();
    wear.reconcile(&inv);
    assert!(wear.spend(&mut inv, Slot::Hotbar(0), 10).is_none());
    // Picked onto the cursor, then placed into the pack.
    inv.cursor = inv.hotbar[0].take();
    wear.reconcile(&inv);
    assert_eq!(wear.used(Slot::Cursor), 10);
    inv.pack[4] = inv.cursor.take();
    wear.reconcile(&inv);
    assert_eq!(wear.used(Slot::Pack(4)), 10);
    let max = table::max_durability(PICK).unwrap();
    assert_eq!(wear.durability(&inv, Slot::Pack(4)), Some((max - 10, max)));
    // A fresh pickaxe arriving in the old slot stays at full durability.
    inv.hotbar[0] = Some(ItemStack::new(PICK, 1));
    wear.reconcile(&inv);
    assert_eq!(wear.durability(&inv, Slot::Hotbar(0)), Some((max, max)));
    // Thrown out of the inventory: the wear is dropped.
    inv.pack[4] = None;
    wear.reconcile(&inv);
    assert_eq!(wear.entries.len(), 0);
}

#[test]
fn stack_breaks_at_zero() {
    let mut inv = inv_with_pick();
    let mut wear = Wear::default();
    let max = table::max_durability(PICK).unwrap();
    assert!(wear.spend(&mut inv, Slot::Hotbar(0), max - 1).is_none());
    assert_eq!(wear.durability(&inv, Slot::Hotbar(0)), Some((1, max)));
    assert_eq!(wear.spend(&mut inv, Slot::Hotbar(0), 1), Some(PICK));
    assert!(inv.hotbar[0].is_none());
    assert!(wear.entries.is_empty());
}

#[test]
fn armour_and_food_have_no_or_some_durability() {
    assert!(table::max_durability(Item::Armor(EquipSlot::Head, ArmorTier::Steel)).is_some());
    assert!(table::max_durability(Item::Food(FoodKind::Trailcake)).is_none());
    assert!(table::mined_xp(crate::world::BlockId::IronOre) > 0);
    assert_eq!(table::mined_xp(crate::world::BlockId::Stone), 0);
}

#[test]
fn xp_curve_matches_vanilla() {
    assert_eq!(xp::level_of(0), (0, 0));
    assert_eq!(xp::level_of(7), (1, 0));
    assert_eq!(xp::level_of(16), (2, 0));
    // Level 30 needs 1395 points in vanilla.
    assert_eq!(xp::level_of(1395).0, 30);
    assert_eq!(xp::level_of(1394).0, 29);
    assert_eq!(xp::to_next(30), 112);
    assert_eq!(xp::to_next(31), 121);
    assert!((xp::progress(10) - 3.0 / 9.0).abs() < 1e-6);
}

#[test]
fn saturation_drains_before_hunger() {
    let mut s = State { saturation: 3.0, ..State::default() };
    assert_eq!(s.absorb_drain(2.0), 0.0);
    assert!((s.saturation - 1.0).abs() < 1e-6);
    assert!((s.absorb_drain(2.0) - 1.0).abs() < 1e-6);
    assert_eq!(s.saturation, 0.0);
    s.eat(FoodKind::CookedMutton, 20.0);
    assert_eq!(s.saturation, 20.0, "capped by hunger");
}

#[test]
fn effects_expire_and_take_the_higher_level() {
    let mut e = Effects::default();
    e.add(Kind::Speed, 2.0, 1);
    e.add(Kind::Speed, 1.0, 3);
    assert_eq!(e.level(Kind::Speed), 3);
    e.add(Kind::Speed, 9.0, 2);
    assert_eq!((e.level(Kind::Speed), e.list[0].seconds), (3, 1.0));
    assert!(e.speed_factor() > 1.5);
    e.add(Kind::Regeneration, 1.0, 2);
    let healed = e.tick(0.5, 50.0);
    assert!((healed - 2.0).abs() < 1e-5, "{healed}");
    e.tick(0.6, 50.0);
    assert!(e.list.is_empty());
    e.add(Kind::Poison, 10.0, 1);
    assert!(e.tick(10.0, 6.0) >= -1.0 - 1e-6, "poison stops at the floor");
    assert_eq!(Kind::parse("fire_resistance"), Some(Kind::FireResistance));
}

#[test]
fn eating_a_trailcake_grants_regeneration() {
    let mut s = State::default();
    s.eat(FoodKind::Trailcake, 80.0);
    assert_eq!(s.effects.level(Kind::Regeneration), 1);
}

#[test]
fn sidecar_round_trips_and_missing_file_is_default() {
    let mut inv = inv_with_pick();
    let mut s = State { xp: 123, saturation: 7.5, ..State::default() };
    s.effects.add(Kind::Strength, 30.0, 2);
    s.wear.spend(&mut inv, Slot::Hotbar(0), 5);
    let bytes = sidecar::encode(&s, &inv);
    let back = sidecar::decode(&bytes).unwrap();
    assert_eq!((back.xp, back.saturation), (123, 7.5));
    assert_eq!(back.effects, s.effects);
    assert_eq!(back.wear.used(Slot::Hotbar(0)), 5);
    let mut bad = bytes.clone();
    bad[9] ^= 1;
    assert!(sidecar::decode(&bad).is_err());

    let dir = std::env::temp_dir().join(format!("vx_progression_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut world = crate::world::World::with_seed(crate::world::WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter, 7);
    sidecar::load(&mut world, &dir);
    assert_eq!(world.progression.xp, 0);
    std::fs::write(dir.join(sidecar::FILE), b"garbage").unwrap();
    sidecar::load(&mut world, &dir);
    assert_eq!(world.progression.xp, 0);
    sidecar::save(&dir, &bytes).unwrap();
    sidecar::load(&mut world, &dir);
    assert_eq!(world.progression.xp, 123);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn death_clears_xp_unless_keep_inventory() {
    let mut s = State { xp: 50, ..State::default() };
    s.effects.add(Kind::Speed, 30.0, 1);
    s.on_death(true);
    assert_eq!(s.xp, 50);
    assert!(s.effects.list.is_empty());
    s.on_death(false);
    assert_eq!(s.xp, 0);
}

#[test]
fn landed_strike_wears_the_held_weapon() {
    let mut world = crate::world::World::with_seed(crate::world::WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter, 7);
    world.rules.mode = crate::world::rules::GameMode::Survival;
    world.inventory.hotbar[0] = Some(ItemStack::new(Item::Weapon(1), 1));
    world.adventure.hand_slot = 0;
    step(&mut world, 1.0 / 120.0);
    world.adventure.hit = 0.18;
    step(&mut world, 1.0 / 120.0);
    assert_eq!(world.progression.wear.used(Slot::Hotbar(0)), 1);
    world.progression.pending_xp = 7;
    step(&mut world, 1.0 / 120.0);
    assert_eq!(world.progression.level(), 1);
}
