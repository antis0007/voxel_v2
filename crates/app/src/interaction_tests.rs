use super::*;
use world::inventory::{Item, ItemStack};

#[test]
fn interaction_regression_secondary_use_places_offhand_block_and_keeps_main_hand() {
    let mut w = World::with_seed(WorldType::Superflat, fluid::VoxelScalePreset::OneMeter, 31);
    let mut p = Player::new();
    p.pos = Vec3::new(8.5, 40.0, 12.5);
    let cell = glam::IVec3::new(8, 41, 9);
    let place = cell + glam::IVec3::Z;
    w.set(cell, BlockId::Stone);
    w.set(place, BlockId::Air);
    w.inventory.hotbar[4] = Some(ItemStack::new(Item::Wand { id: 0, look: 0 }, 1));
    w.inventory.offhand = Some(ItemStack::new(Item::Block(BlockId::Stone), 2));
    let main = w.inventory.hotbar[4];
    assert!(!world::interaction::secondary_targets_world(&w, &p));
    assert!(use_offhand(&mut w, &p, 4, Some((cell, place)), &mut HashSet::new(), &mut HashSet::new()));
    assert_eq!(w.get(place), BlockId::Stone);
    assert_eq!(w.inventory.offhand.unwrap().count, 1);
    assert_eq!(w.inventory.hotbar[4], main);
}

#[test]
fn interaction_regression_chest_secondary_use_precedes_both_equipped_items() {
    let mut w = World::with_seed(WorldType::Superflat, fluid::VoxelScalePreset::OneMeter, 31);
    let mut p = Player::new();
    p.pos = Vec3::new(8.5, 40.0, 12.5);
    let cell = (p.eye(w.voxel_scale) + p.forward_dir() * 2.0).floor().as_ivec3();
    w.set(cell, BlockId::Chest);
    w.inventory.hotbar[4] = Some(ItemStack::new(Item::Bucket(0), 1));
    w.inventory.offhand = Some(ItemStack::new(Item::Block(BlockId::Torch), 8));
    let main = w.inventory.hotbar;
    let left = w.inventory.offhand;
    assert!(world::interaction::secondary_targets_world(&w, &p));
    assert_eq!(world::interaction::target(&w, &p), Some(world::interaction::Target::Block(cell)));
    assert_eq!(world::interaction::activate(&mut w, &p, 4, false, &mut HashSet::new(), &mut HashSet::new()), world::interaction::Effect::Block(cell));
    assert!(world::block_data::opens_page(w.get(cell)));
    assert_eq!(w.inventory.hotbar, main);
    assert_eq!(w.inventory.offhand, left);
}
