//! Production-order configuration and transactional feed/return actions.
use super::{
    Action,
    production::{MAX_JOBS, OPERATIONS, Order, SAND_CAP, STOCK_CAP},
};
use crate::world::factory::{CAPACITY, Kind, near};
use crate::world::{
    BlockId, World,
    inventory::{Item, ItemStack},
};
use glam::Vec3;
use std::sync::Arc;

pub(super) fn action(world: &mut World, player: Vec3, action: Action) {
    let site = match action {
        Action::SetOrder { site, .. }
        | Action::FeedOrder(site)
        | Action::ClearOrder(site)
        | Action::MoveOrder { site, .. } => site,
        _ => unreachable!(),
    };
    if !near(world, site, player)
        || world
            .factory
            .nodes
            .get(&site)
            .is_none_or(|n| n.kind != Kind::Fabricator)
    {
        world.expedition.say("Nearby Fabricator Required");
        return;
    }
    let k = (site.x, site.y, site.z);
    let node = world
        .factory
        .nodes
        .get_mut(&site)
        .expect("checked fabricator");
    let production = Arc::make_mut(&mut world.factory.chips.production);
    let mut moved_from = None;
    let result: Result<&str, &str> = (|| {
        match action {
            Action::MoveOrder { from, .. } => {
                if production.owns(k) {
                    return Err("Fabricator Busy");
                }
                let old = (from.x, from.y, from.z);
                let order = production
                    .orders
                    .iter_mut()
                    .find(|o| o.site == old)
                    .ok_or("No Production Order")?;
                order.site = k;
                node.lab_recipe = Some(order.recipe);
                if let Some(job) = production.jobs.iter_mut().find(|j| j.site == old) {
                    job.site = k;
                }
                moved_from = Some(from);
                Ok("Order Reassigned")
            }
            Action::SetOrder {
                recipe,
                target,
                enabled,
                ..
            } => {
                if usize::from(recipe) >= OPERATIONS.len() || !(1..=STOCK_CAP).contains(&target) {
                    return Err("Invalid Production Order");
                }
                if let Some(order) = production.orders.iter_mut().find(|o| o.site == k) {
                    // Clear first to change recipe, so stored sand is explicitly returned.
                    if order.recipe != recipe {
                        return Err("Production Order Active");
                    }
                    order.target = target;
                    order.enabled = enabled;
                } else {
                    if production.busy(k) {
                        return Err("Fabricator Busy");
                    }
                    if production.orders.len() >= MAX_JOBS {
                        return Err("Order Limit");
                    }
                    production.orders.push(Order {
                        site: k,
                        recipe,
                        target,
                        enabled,
                        sand: 0,
                    });
                }
                node.lab_recipe = Some(recipe);
                Ok("Order Saved")
            }
            Action::FeedOrder(_) => {
                let order = production
                    .orders
                    .iter_mut()
                    .find(|o| o.site == k)
                    .ok_or("No Production Order")?;
                let mut inventory = world.inventory.clone();
                let mut input = node.input;
                let mut sand = order.sand;
                let mut moved = 0;
                for &(item, _) in OPERATIONS[order.recipe as usize].raw {
                    let room = match item {
                        Item::Resource(r) => {
                            let incoming = world
                                .factory
                                .reserved
                                .get(&(site, r as usize))
                                .copied()
                                .unwrap_or(0);
                            CAPACITY.saturating_sub(input[r as usize].saturating_add(incoming)) / 64
                        }
                        Item::Block(BlockId::Sand) => SAND_CAP - sand,
                        _ => return Err("Unsupported Feed"),
                    };
                    let n = room.min(inventory.count_item(item));
                    if n == 0 {
                        continue;
                    }
                    if !inventory.remove_item(item, n) {
                        return Err("Missing Materials");
                    }
                    match item {
                        Item::Resource(r) => input[r as usize] += n * 64,
                        Item::Block(BlockId::Sand) => sand += n,
                        _ => unreachable!(),
                    }
                    moved += n;
                }
                if moved == 0 {
                    return Err("No Feed Transferred");
                }
                world.inventory = inventory;
                node.input = input;
                order.sand = sand;
                Ok("Feed Loaded")
            }
            Action::ClearOrder(_) => {
                let index = production
                    .orders
                    .iter()
                    .position(|o| o.site == k)
                    .ok_or("No Production Order")?;
                let sand = production.orders[index].sand;
                let mut inventory = world.inventory.clone();
                if sand > 0
                    && inventory
                        .try_insert(ItemStack::new(Item::Block(BlockId::Sand), sand))
                        .1
                        != 0
                {
                    return Err("Inventory Full");
                }
                production.orders.remove(index);
                world.inventory = inventory;
                node.lab_recipe = None;
                Ok("Order Cleared")
            }
            _ => unreachable!(),
        }
    })();
    if let Some(from) = moved_from
        && let Some(old) = world.factory.nodes.get_mut(&from)
    {
        old.lab_recipe = None;
    }
    world.sync_from_inventory();
    world.expedition.say(result.unwrap_or_else(|e| e));
}
