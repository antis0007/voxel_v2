use super::*;
use crate::BlockId;
use crate::{player::Player, world::{interaction, inventory::{Item, ItemStack}, ChunkData, WorldType}};
use engine_core::VoxelScalePreset;
use std::{collections::HashSet, sync::Arc};

fn scene() -> (World, Player, IVec3) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None, structural_bounds: None,
        blocks: Box::new([BlockId::Air; 32 * 32 * 32]),
        light: Box::new([0; 32 * 32 * 32]), torches: HashSet::new(),
    }));
    let mut p = Player::new();
    p.pos = Vec3::new(8.5, 8.0, 12.5);
    let site = IVec3::new(2, 2, 0);
    let state = (1 << 8) | (1 << 11);
    w.adventure.records.insert(site, state);
    w.adventure.reliquaries.push(reliquaries::Reliquary {
        site, pos: p.eye(w.voxel_scale) + p.forward_dir() * 2.0 - Vec3::Y * 0.85,
        kind: 0, state,
    });
    (w, p, site)
}

#[test]
fn interaction_regression_g_activates_shrine_with_either_hand_and_shield_raised() {
    let schema = engine_tools::PARTS.schemas.iter().position(|s| s.id == "shield").unwrap();
    let shield = Item::Assembled { id: 0, schema: schema as u16 };
    for offhand in [None, Some(Item::Block(BlockId::Torch)), Some(shield),
        Some(Item::Wand { id: 0, look: 0 }), Some(Item::Block(BlockId::Stone))] {
        let (mut w, p, site) = scene();
        let materials = vec![engine_tools::MATERIALS[0].id.clone(); engine_tools::PARTS.schemas[schema].parts.len()];
        w.adventure.tool_bank.push(engine_tools::ToolAssembly::new("shield", &materials).unwrap());
        w.inventory.offhand = offhand.map(|i| ItemStack::new(i, 1));
        w.inventory.hotbar[4] = Some(ItemStack::new(Item::Wand { id: 0, look: 0 }, 1));
        w.adventure.hand_slot = 4;
        w.adventure.shield_raised = true;
        let before = w.inventory.hotbar;
        let left = w.inventory.offhand;
        assert_eq!(interaction::target(&w, &p), Some(interaction::Target::Relic));
        assert_eq!(interaction::activate(&mut w, &p, 4, false, &mut HashSet::new(), &mut HashSet::new()), interaction::Effect::Taken);
        assert_ne!(w.adventure.records[&site] & (1 << 10), 0, "{offhand:?}");
        assert_eq!(w.inventory.hotbar, before);
        assert_eq!(w.inventory.offhand, left);
        assert_eq!(w.adventure.hand_slot, 4);
        assert!(w.adventure.shield_raised);
    }
}

#[test]
fn interaction_regression_one_press_only_claims_nearest_visible_shrine() {
    let (mut w, p, site) = scene();
    let second = IVec3::new(3, 2, 0);
    let mut relic = w.adventure.reliquaries[0].clone();
    relic.site = second;
    relic.pos += p.forward_dir() * 0.8;
    w.adventure.records.insert(second, relic.state);
    w.adventure.reliquaries.insert(0, relic);
    interaction::activate(&mut w, &p, 0, false, &mut HashSet::new(), &mut HashSet::new());
    assert_ne!(w.adventure.records[&site] & (1 << 10), 0);
    assert_eq!(w.adventure.records[&second] & (1 << 10), 0);
}

#[test]
fn interaction_regression_camera_block_occludes_shrine_and_chest_wins() {
    let (mut w, p, site) = scene();
    let cell = (p.eye(w.voxel_scale) + p.forward_dir()).floor().as_ivec3();
    w.set(cell, BlockId::Chest);
    assert_eq!(interaction::target(&w, &p), Some(interaction::Target::Block(cell)));
    assert_eq!(interaction::activate(&mut w, &p, 0, false, &mut HashSet::new(), &mut HashSet::new()), interaction::Effect::Block(cell));
    assert_eq!(w.adventure.records[&site] & (1 << 10), 0);
    w.set(cell, BlockId::Stone);
    assert_eq!(interaction::target(&w, &p), None);
}

#[test]
fn interaction_regression_focus_menus_and_text_suppress_gameplay() {
    for (focus, grabbed, menu, text) in [(false,true,false,false), (true,false,false,false),
        (true,true,true,false), (true,true,false,true)] {
        assert!(!interaction::gameplay_enabled(focus, grabbed, menu, text));
    }
    assert!(interaction::gameplay_enabled(true, true, false, false));
}
