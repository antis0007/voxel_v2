//! The same request handler serves local play and ordered host commands.
use super::*;
use anyhow::{ensure, Result};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Withdraw(Item, u32, Option<IVec3>),
    QuickDeposit,
    SmartDeposit,
    DepositAll,
    Restock,
    Consolidate,
    Configure(IVec3, policy::Policy),
    Craft(usize),
    Recover,
    Minimum(Item, u32),
    CraftBatch(usize, u32),
    Chest(crate::world::block_data::ChestAction),
}

pub fn apply(world: &mut World, anchor: IVec3, eye_m: Vec3, action: Action) -> Result<u32> {
    let centre = (anchor.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
    ensure!(eye_m.is_finite() && eye_m.distance(centre) <= 6.0, "Scanner out of reach");
    ensure!(world.get(anchor).chest_tier().is_some(), "Scanner unavailable");
    if let Action::Chest(request) = action {
        crate::world::block_data::edit_chest(world, anchor, request);
        return Ok(0);
    }
    let found = discover(world, centre, RADIUS_M, Some(anchor));
    let moved = match action {
        Action::Recover => {
            let before: u64 = world.inventory.overflow.iter().map(|s| u64::from(s.count)).sum();
            world.inventory.migrate_balance(world.inventory.balance);
            let after: u64 = world.inventory.overflow.iter().map(|s| u64::from(s.count)).sum();
            before.saturating_sub(after).min(u64::from(u32::MAX)) as u32
        }
        Action::Minimum(item, n) => {
            // Only a new, nonzero minimum consumes another policy slot.
            // Validate before mutation so a rejected request changes nothing.
            let exists = world.inventory.minimums.iter().any(|(i, _)| *i == item);
            ensure!(n == 0 || exists || world.inventory.minimums.len() < 256, "Too many minimums");
            world.inventory.minimums.retain(|(i, _)| *i != item);
            if n > 0 { world.inventory.minimums.push((item, n)); }
            0
        }
        Action::Withdraw(item, n, source) => {
            let subset: Vec<_> = found.into_iter().filter(|c| source.is_none_or(|p| c.pos == p)).collect();
            withdraw(world, &subset, item, n)
        }
        Action::QuickDeposit => deposit(world, &found, Reach::Existing),
        Action::SmartDeposit => deposit(world, &found, Reach::Smart),
        Action::DepositAll => deposit(world, &found, Reach::Anywhere),
        Action::Restock => restock(world, &found),
        Action::Consolidate => consolidate(world, &found) as u32,
        Action::Configure(p, policy) => {
            let pos = (p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
            ensure!(pos.distance(centre) <= RADIUS_M
                && world.chunks.contains_key(&super::super::chunk_coord_from_world_block(p))
                && (world.get(p).chest_tier().is_some() || cache_id(world, p).is_some()), "Container unavailable");
            ensure!(policy.target == 0 || policy.target >= policy.reserve, "Target is below reserve");
            ensure!(policy::set(world, p, policy), "Storage policy limit");
            0
        }
        Action::Craft(id) => crafting::craft(world, &found, eye_m, id, 1)?,
        Action::CraftBatch(id, batches) => crafting::craft(world, &found, eye_m, id, batches)?,
        Action::Chest(_) => unreachable!(),
    };
    world.sync_from_inventory();
    Ok(moved)
}

