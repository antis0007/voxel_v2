//! Crop harvester: harvests and replants ripe plots around it, like a Create
//! mechanical harvester or an Immersive Engineering cloche, without a contraption.
use super::flowers::{SIDES, store};
use super::table::{self, Planting};
use super::{Key, Plot, World, resident};
use crate::world::inventory::{DroppedStack, Item, ItemStack};
use crate::world::{BlockId as B, effects};
use glam::{IVec3, Vec3};

/// Plots one harvester works per second, so a large field drains over several seconds.
const MAX_PER_PULSE: usize = 8;

/// What one ripe plot gives up and the block its cell keeps.
fn reap(world: &World, p: IVec3, plot: &Plot) -> Option<(ItemStack, Vec<IVec3>, B)> {
    let crop = table::get(plot.crop)?;
    if crop.aura.is_some() || crop.tree.is_some() {
        return None;
    }
    // Gourd and stalk ripeness is the fruit or the height; growth resets their stage.
    let block = world.get(p);
    if crop.gourd {
        if block != crop.sprout {
            return None;
        }
        let fruit: Vec<IVec3> = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .into_iter().map(|d| p + d).filter(|&q| world.get(q) == crop.crop).collect();
        return (!fruit.is_empty()).then(|| (ItemStack::new(Item::Block(crop.crop), fruit.len() as u32), fruit, B::Air));
    }
    if let Some(stalk) = crop.stalk {
        if block != crop.crop {
            return None;
        }
        // Leave the base cell so the column regrows from it.
        let ceiling = world.sample_config().blocks_for_height(stalk.max_metres).max(1);
        let cut: Vec<IVec3> = (1..ceiling).map(|y| p + IVec3::Y * y).take_while(|&q| world.get(q) == crop.crop).collect();
        let empty = super::forage::vacated(crop.crop);
        return (!cut.is_empty()).then(|| (ItemStack::new(Item::Block(crop.crop), cut.len() as u32), cut, empty));
    }
    if plot.stage < crop.mature() || block != crop.crop || crop.sprout == crop.crop {
        return None;
    }
    let count = super::harvest_count(world.seed, p, plot, crop);
    // Replanting spends one seed. A produce crop's seed is paid on top of its
    // produce by hand, and a regrowing plant keeps its stem, so neither loses one.
    let keeps_seed = crop.regrow || crop.planting == Planting::Grafted || crop.produce.is_some();
    let paid = if keeps_seed { count } else { count.saturating_sub(1) };
    let item = crop.produce.map_or(Item::Block(crop.crop), Item::Resource);
    Some((ItemStack::new(item, paid), vec![p], crop.sprout))
}

/// One harvester pulse at `at`.
pub(super) fn service(world: &mut World, at: IVec3, radius_metres: f32) {
    let scale = world.voxel_scale.meters();
    let r = (radius_metres / scale).ceil() as i32;
    let lo: Key = (at.x - r, i32::MIN, i32::MIN);
    let hi: Key = (at.x + r, i32::MAX, i32::MAX);
    let r2 = (radius_metres / scale).powi(2);
    let candidates: Vec<(IVec3, Plot)> = world.farming.plots.range(lo..=hi)
        .map(|(&(x, y, z), &plot)| (IVec3::new(x, y, z), plot))
        .filter(|(p, _)| (*p - at).as_vec3().length_squared() <= r2 && resident(world, *p))
        .collect();
    let chest = SIDES.iter().map(|&d| at + d).find(|&q| world.get(q).chest_tier().is_some());
    let mut changed = Vec::new();
    let mut worked = 0;
    for (p, plot) in candidates {
        if worked >= MAX_PER_PULSE {
            break;
        }
        let Some((mut stack, cells, after)) = reap(world, p, &plot) else { continue };
        // Reserve an output route before consuming plants. A full loose-item
        // budget must leave the crop available for the next pulse.
        if stack.count > 0 && world.inventory.dropped.len() >= 1024 {
            let fits = chest.is_some_and(|c| {
                let mut slots = world.block_data.chest(c);
                slots.resize(world.get(c).chest_slots(), None);
                store(&mut slots, stack, world.inventory.stack_limit(stack.item)) == 0
                    && world.block_data.ensure_chest(c)
            });
            if !fits {
                continue;
            }
        }
        let mut removed = 0;
        for q in cells.iter().rev() {
            let before = world.get(*q);
            if let Ok(commit) = effects::replace(world, *q, before, after) {
                commit.publish_voxel(world, &mut changed);
                removed += 1;
            }
        }
        if removed == 0 {
            continue;
        }
        worked += 1;
        let crop = table::get(plot.crop);
        // Stalks and gourds pay one item per committed cell, even when admission
        // rejects another cell in the same plant.
        if crop.is_some_and(|c| c.stalk.is_some() || c.gourd) {
            stack.count = removed;
        }
        if crop.is_some_and(|c| c.stalk.is_none() && !c.gourd) {
            world.farming.plots.insert((p.x, p.y, p.z), Plot::new(plot.crop, world.farming.clock));
        }
        if stack.count > 0 {
            deposit(world, at, chest, stack);
        }
    }
    world.adventure.dirty_voxels.extend(changed);
}

/// Into the touching chest when it has room, otherwise dropped beside the harvester.
fn deposit(world: &mut World, at: IVec3, chest: Option<IVec3>, mut stack: ItemStack) {
    if let Some(c) = chest {
        let mut slots = world.block_data.chest(c);
        slots.resize(world.get(c).chest_slots(), None);
        let limit = world.inventory.stack_limit(stack.item);
        let left = store(&mut slots, stack, limit);
        if left < stack.count && world.block_data.ensure_chest(c) {
            world.block_data.set_chest(c, slots);
            stack.count = left;
        }
    }
    if stack.count > 0 && world.inventory.dropped.len() < 1024 {
        let pos = (at.as_vec3() + Vec3::new(0.5, 1.2, 0.5)) * world.voxel_scale.meters();
        world.inventory.dropped.push(DroppedStack { pos, stack });
    }
}

#[cfg(test)]
mod tests {
    use crate::world::BlockId as B;
    use crate::world::farming::table;

    #[test]
    fn harvester_row_is_a_working_block() {
        let (_, row) = table::by_seed(B::CropHarvester).expect("harvester row");
        assert!(matches!(row.aura, Some(table::Aura::Harvest { .. })));
        assert!(engine_content::flora::parts(B::CropHarvester, glam::IVec3::ZERO).is_some());
    }
}
