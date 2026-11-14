//! Disposable native regression for reusable crystal rewards.
use crate::world::{self, BlockId, World, WorldType};
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};
use crate::fluid::VoxelScalePreset;
use anyhow::{Context, Result, ensure};
use glam::{IVec3, Vec3};
use std::collections::HashSet;

pub fn run() -> Result<()> {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 1234);
    world.rules.mode = world::rules::GameMode::Survival;
    world.adventure.creative = false;
    world.inventory = SurvivalInventory::empty();
    world.inventory.hotbar[0] = Some(ItemStack::new(Item::Tool(8), 1));
    world.adventure.hand_slot = 0;
    let tool = world::mining::tool_for_world(&world, Some(Item::Tool(8)));
    let cell = IVec3::new(0, 100, 0);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());

    world.set(cell, BlockId::Crystal);
    // Generation bypasses the placement ledger; remove only this fixture marker.
    world.block_data.placed_sediment.remove(&world::block_data::key(cell));
    ensure!(world::mining::harvest(&mut world, cell, Some(tool), &mut blocks, &mut chunks, None, Vec3::ZERO), "Natural crystal harvest failed");
    let natural_xp = world::progression::table::mined_xp(BlockId::Crystal);
    ensure!(natural_xp > 0 && world.progression.xp == natural_xp, "Natural crystal lost XP");
    ensure!(world.inventory.count_item(Item::Resource(5)) == 1, "Natural crystal yield changed");
    let recipe = world::inventory::all_recipes().iter().find(|r| r.name == "Resonant crystal block")
        .context("Crystal recipe missing")?;
    for _ in 0..3 {
        ensure!(world.inventory.stage_recipe_into_grid(recipe, true), "Crystal recipe staging failed");
        ensure!(world.inventory.craft_from_grid(true, true).is_some(), "Crystal recipe commit failed");
        world.sync_from_inventory();
        ensure!(world.inventory.remove_item(Item::Block(BlockId::Crystal), 1), "Crafted crystal missing");
        world.sync_from_inventory();
        world.set(cell, BlockId::Crystal);
        let saved = world::block_data::sidecar::encode(&world.block_data);
        ensure!(saved[7] == 5, "Block-data writer version changed");
        world.block_data = world::block_data::sidecar::decode(&saved)?;
        ensure!(world::mining::harvest(&mut world, cell, Some(tool), &mut blocks, &mut chunks, None, Vec3::ZERO), "Placed crystal harvest failed");
        ensure!(world.progression.xp == natural_xp, "Placed crystal awarded repeat XP");
        ensure!(world.inventory.count_item(Item::Resource(5)) == 1, "Crystal recycle changed material quantity");
    }
    // A tracked cell can be replaced normally; records must not leak forever.
    world.set(cell, BlockId::Crystal);
    world.set(cell, BlockId::Air);
    ensure!(!world.block_data.placed_sediment.contains(&world::block_data::key(cell)), "Removed crystal retained a placement record");
    println!("[crystal-xp] PASS natural reward, three production recipe cycles, saved provenance, unchanged material yield, record cleanup");
    println!("[crystal-xp] PASS checkpoint=67 block-data=5 protocol=14; no player save opened");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placed_crystal_cannot_farm_xp_after_ledger_reload() {
        super::run().expect("native crystal regression");
    }
}
