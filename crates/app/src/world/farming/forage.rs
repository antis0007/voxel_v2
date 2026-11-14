//! Picking ripe regrowing plants, eating raw produce, gourd fruit and plant-cell targeting.
use super::table::{self, Crop, Planting};
use super::{Plot, World, key, resident};
use crate::world::inventory::{Item, ItemStack};
use crate::world::{BlockId as B, effects};
use glam::IVec3;
use std::collections::HashSet;

const SIDES: [IVec3; 4] = [IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z];

/// The block a plant leaves when removed: water for a submerged plant.
pub fn vacated(block: B) -> B {
    if engine_content::flora::is_submerged(block) { B::Water } else { B::Air }
}

/// True when the cell at `at` may hold `crop` now, from its medium and supports.
pub fn fits(get: impl Fn(IVec3) -> B, crop: &Crop, at: IVec3) -> bool {
    get(at) == crop.planting.medium()
        && crop.planting.supports().iter().any(|&d| crop.soil.contains(&get(at + d)))
        && match crop.planting {
            Planting::Submerged => get(at + IVec3::Y) == B::Water,
            Planting::Paddy => get(at + IVec3::Y) == B::Air,
            _ => true,
        }
}

/// The cell a use on `cell`'s face toward `place` plants `crop` in, if it fits.
pub fn target(world: &World, crop: &Crop, cell: IVec3, place: IVec3) -> Option<IVec3> {
    let normal = place - cell;
    let at = match crop.planting {
        Planting::Soil | Planting::Submerged | Planting::Paddy => (normal == IVec3::Y).then_some(place)?,
        Planting::Side => (normal.y == 0).then_some(place)?,
        Planting::Hanging => (normal == IVec3::NEG_Y).then_some(place)?,
        // The ray passes through water, so climb from the bed to the surface.
        Planting::Floating => {
            let mut q = place;
            let mut steps = 0;
            while world.get(q) == B::Water && steps < 16 {
                q += IVec3::Y;
                steps += 1;
            }
            q
        }
        Planting::Grafted => return None,
    };
    fits(|q| world.get(q), crop, at).then_some(at)
}

/// Picks the ripe regrowing plant at `p` back to its bare cell. True when handled.
pub fn pick(world: &mut World, p: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    let block = world.get(p);
    let planted = world.farming.plots.get(&key(p)).copied();
    let row = match planted {
        Some(plot) => table::get(plot.crop).filter(|c| c.regrow && c.crop == block).map(|c| (plot.crop, c)),
        None => table::regrowing(block),
    };
    let Some((index, crop)) = row else { return false };
    if planted.is_none() && world.farming.plots.len() >= super::MAX_PLOTS {
        world.expedition.say(format!("Plots Full: {0}/{0}", super::MAX_PLOTS));
        return true;
    }
    let now = world.farming.clock;
    let ripe = Plot { stage: crop.mature(), ..planted.unwrap_or(Plot::new(index, now)) };
    let count = super::harvest_count(world.seed, p, &ripe, crop);
    let item = crop.produce.map_or(Item::Block(crop.crop), Item::Resource);
    if !world.inventory.can_insert(ItemStack::new(item, count)) {
        world.inventory.notify_overflow(item);
        world.expedition.say("Inventory Full");
        return true;
    }
    // Picking a wild sub-metre stamp leaves one sprout where it was picked.
    let stamp = engine_content::stamp::cells(block, p, world.voxel_scale.meters(), |q| world.get(q));
    let Ok(commit) = effects::replace(world, p, block, crop.sprout) else { return true };
    commit.publish(world, blocks, chunks);
    for q in stamp.into_iter().skip(1) {
        if let Ok(commit) = effects::replace(world, q, block, vacated(block)) {
            commit.publish(world, blocks, chunks);
        }
    }
    let centre = (p.as_vec3() + glam::Vec3::splat(0.5)) * world.voxel_scale.meters();
    let tint = engine_content::flora::parts(block, p)
        .and_then(|parts| parts.iter().rev().find_map(|part| part.color))
        .unwrap_or_else(|| block.color(p));
    world.adventure.spawn_voxel_burst(centre, tint, 4, 1.0, 0.04);
    let (inserted, _) = world.inventory.try_insert(ItemStack::new(item, count));
    if inserted > 0 {
        world.inventory.notify_pickup(item, inserted);
    }
    world.sync_from_inventory();
    // Picking restarts the plot clock so regrowth never replays the ripe wait.
    world.farming.plots.insert(key(p), Plot::new(index, now));
    crate::world::progression::on_harvest(world, block);
    true
}

/// Eats one held `block` item that has a food row. True when the item is edible.
pub fn eat(world: &mut World, slot: usize, block: B) -> bool {
    let Some(bite) = table::bite(block) else { return false };
    let max_hp = world.adventure.max_health();
    if world.adventure.hunger >= 99.5 && world.adventure.health >= max_hp - 0.1 {
        world.expedition.say_for("Hunger Full", 2.0);
        return true;
    }
    if !world.inventory.consume_from_hotbar(slot, Item::Block(block)) {
        return false;
    }
    let a = &mut world.adventure;
    a.hunger = (a.hunger + bite.hunger).min(100.0);
    a.health = (a.health + bite.health).min(max_hp);
    let hunger = a.hunger;
    let p = &mut world.progression;
    p.saturation = (p.saturation + bite.saturation).min(hunger).max(0.0);
    world.goals.note(crate::world::goals::Event::Ate);
    world.inventory.eat_timer = 1.35;
    world.sync_from_inventory();
    world.expedition.say_for(
        format!("Ate {} · Hunger +{:.0} · Health +{:.0}", block.display_name(), bite.hunger, bite.health),
        2.4,
    );
    true
}

/// Sets one fruit on free ground beside a mature stem. False while one is attached or no side is free.
pub fn set_gourd(world: &mut World, p: IVec3, crop: &Crop, changed: &mut Vec<IVec3>) -> bool {
    if SIDES.iter().any(|&d| world.get(p + d) == crop.crop) {
        return false;
    }
    let start = super::stage::roll(world.seed, p, world.farming.clock) as usize;
    for i in 0..SIDES.len() {
        let q = p + SIDES[(start + i) % SIDES.len()];
        if !resident(world, q) || !resident(world, q - IVec3::Y) {
            continue;
        }
        let ground = world.get(q - IVec3::Y);
        if world.get(q) != B::Air || !(crop.soil.contains(&ground) || ground == super::BED) {
            continue;
        }
        if let Ok(commit) = effects::replace(world, q, B::Air, crop.crop) {
            commit.publish_voxel(world, changed);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(block: B) -> &'static Crop {
        table::by_seed(block).map(|(_, c)| c).expect("row")
    }

    #[test]
    fn plantings_check_medium_and_support() {
        let bed = |surface: B, water: i32| move |q: IVec3| match q.y {
            y if y < 0 => surface,
            y if y < water => B::Water,
            _ => B::Air,
        };
        let at = IVec3::new(-3, 0, 1 << 20);
        assert!(fits(bed(B::Sand, 3), row(B::Kelp), at), "kelp under water");
        assert!(!fits(bed(B::Sand, 1), row(B::Kelp), at), "kelp at the surface");
        assert!(fits(bed(B::Mud, 1), row(B::RiceCrop), at), "rice in a paddy");
        assert!(!fits(bed(B::Mud, 2), row(B::RiceCrop), at), "rice drowned");
        assert!(!fits(bed(B::Mud, 0), row(B::RiceCrop), at), "rice on dry mud");
        assert!(fits(bed(B::Grass, 0), row(B::Potato), at), "potato on grass");
        assert!(!fits(bed(B::Grass, 1), row(B::Potato), at), "potato under water");
        let pond = |q: IVec3| if q.y < 0 { B::Water } else { B::Air };
        assert!(fits(pond, row(B::LilyPad), at));
        let log = |q: IVec3| if q.x > 0 { B::Log } else { B::Air };
        assert!(fits(log, row(B::CocoaPod), IVec3::ZERO));
        let roof = |q: IVec3| if q.y > 0 { B::Stone } else { B::Air };
        assert!(fits(roof, row(B::GlowBerries), IVec3::ZERO));
        assert_eq!(vacated(B::Kelp), B::Water);
        assert_eq!(vacated(B::SweetBerries), B::Air);
    }

    #[test]
    fn fruit_rows_resolve_to_grafted_rows_and_foods() {
        for fruit in [B::Apple, B::Cherry, B::Fig] {
            let tree = row(fruit);
            assert_eq!(tree.tree.and_then(|t| t.fruit), Some(fruit), "{fruit:?} plants its tree");
            let (_, grafted) = table::regrowing(fruit).expect("regrowing");
            assert_eq!(grafted.planting, Planting::Grafted);
            assert!(table::bite(fruit).is_some(), "{fruit:?} is edible");
        }
        assert!(table::regrowing(B::SweetBerries).is_some_and(|(_, c)| c.sprout == B::BerryBush));
        assert!(table::by_seed(B::FruitBud).is_none());
    }
}
