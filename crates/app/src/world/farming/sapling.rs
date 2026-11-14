//! Saplings: leaf drops, biomass feeding and the tree a mature sapling becomes.
use super::table::{Crop, TreeShape};
use super::{Key, World, key, stage};
use crate::world::BlockId as B;
use crate::world::inventory::Item;
use glam::{IVec3, Vec3};

/// Fungal biomass: feeding one to a plot advances it a stage.
pub const BIOMASS: Item = Item::Resource(13);
/// One crown leaf cell in this many bears a fruit bud.
const FRUIT_EVERY: u32 = 5;
/// Leaf-break drop chances per cubic metre of leaves, in millionths.
const SAPLING_PER_MILLION: u64 = 50_000;
const VINES_PER_MILLION: u64 = 25_000;
/// Broadleaf crowns also drop an apple, which plants an apple tree.
const APPLE_PER_MILLION: u64 = 10_000;

/// The sapling or vines a broken leaf cell drops, if its positional roll wins.
pub fn leaf_drop(seed: u32, metres: f32, p: IVec3, block: B) -> Option<Item> {
    if !matches!(block, B::Leaves | B::PineLeaves) {
        return None;
    }
    let volume = f64::from(metres).powi(3);
    let threshold = |per_million: u64| (per_million as f64 * volume).round().max(1.0) as u64;
    let roll = u64::from(stage::roll(seed, p, 0x1eaf_d40b)) % 1_000_000;
    if roll < threshold(SAPLING_PER_MILLION) {
        Some(Item::Block(B::Sapling))
    } else if roll < threshold(SAPLING_PER_MILLION) + threshold(VINES_PER_MILLION) {
        Some(Item::Block(B::Vines))
    } else if block == B::Leaves
        && roll < threshold(SAPLING_PER_MILLION) + threshold(VINES_PER_MILLION) + threshold(APPLE_PER_MILLION)
    {
        Some(Item::Block(B::Apple))
    } else {
        None
    }
}

/// Spends one biomass from `slot` to advance the plot at `cell` a stage. True when handled.
pub fn fertilize(world: &mut World, cell: IVec3, slot: usize) -> bool {
    let k: Key = key(cell);
    let Some(mut plot) = world.farming.plots.get(&k).copied() else { return false };
    let Some(crop) = super::table::get(plot.crop) else { return false };
    if super::watershed::manages(world, cell, crop) {
        world.expedition.say("Nursery Uses Intake Water");
        return true;
    }
    if crop.irrigation_metres.is_some()
        && !super::conditions(world, cell, crop).is_some_and(|c| c.wet)
    {
        world.expedition.say("Needs Water");
        return true;
    }
    if plot.stage >= crop.mature() {
        world.expedition.say(format!("{} · Mature", crop.crop.display_name()));
        return true;
    }
    if !world.inventory.consume_from_hotbar(slot, BIOMASS) {
        return false;
    }
    world.sync_from_inventory();
    plot.stage += 1;
    world.farming.plots.insert(k, plot);
    world.expedition.say(format!("{} · Stage {}/{}", crop.crop.display_name(), plot.stage + 1, crop.stages));
    true
}

/// Cells a sapling at `p` becomes, or `None` while its trunk is obstructed.
/// Leaves fill only empty cells, so a grown tree never replaces terrain.
pub fn tree_edits(seed: u32, p: IVec3, shape: TreeShape, metres: f32, get: impl Fn(IVec3) -> B) -> Option<Vec<(IVec3, B)>> {
    let cells = |m: f32| (m / metres).round().max(1.0) as i32;
    let trunk = cells(shape.trunk_metres).max(2);
    let (r, rh) = (cells(shape.crown_radius_metres), cells(shape.crown_half_height_metres));
    let mut edits = vec![(p, B::Log)];
    for y in 1..trunk {
        let q = p + IVec3::Y * y;
        if !get(q).is_empty() {
            return None;
        }
        edits.push((q, B::Log));
    }
    let centre = p + IVec3::Y * (trunk - 1);
    let radii = Vec3::new(r as f32 + 0.5, rh as f32 + 0.5, r as f32 + 0.5);
    for y in -rh..=rh + 1 {
        for z in -r..=r {
            for x in -r..=r {
                let d = IVec3::new(x, y, z);
                let q = centre + d;
                if x == 0 && z == 0 && y <= 0 {
                    continue;
                }
                let reach = (d.as_vec3() / radii).length();
                // Ragged rim: the outer shell keeps two cells in three.
                if reach > 1.0 || (reach > 0.8 && stage::roll(seed, q, 0x7ee5).is_multiple_of(3)) {
                    continue;
                }
                if get(q).is_empty() {
                    // Fruit hangs from the lower outer crown, where it can be reached.
                    let bears = shape.fruit.is_some() && y <= 0 && reach > 0.55
                        && stage::roll(seed, q, 0xf27).is_multiple_of(FRUIT_EVERY);
                    edits.push((q, if bears { B::FruitBud } else { B::Leaves }));
                }
            }
        }
    }
    if let Some(root) = shape.roots {
        // Prop roots: two cells out and down from the lowest trunk metre on each side.
        let lift = cells(1.0).max(1);
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            for (out, up) in [(1, lift), (2, lift / 2), (2, 0)] {
                let q = p + d * out + IVec3::Y * up;
                if get(q).is_empty() || get(q) == B::Water {
                    edits.push((q, root));
                }
            }
        }
    }
    Some(edits)
}

/// Grows the mature sapling at `p` into a tree. False while blocked or the dirty list lacks room.
pub fn grow(world: &mut World, p: IVec3, crop: &Crop, room: usize, changed: &mut Vec<IVec3>) -> bool {
    let Some(shape) = crop.tree else { return false };
    let metres = world.voxel_scale.meters();
    let Some(edits) = tree_edits(world.seed, p, shape, metres, |q| world.get(q)) else { return false };
    if edits.len() > room {
        return false;
    }
    world.set_many_committed(&edits, |q, _, _| changed.push(q));
    if let Some((row, _)) = shape.fruit.and_then(super::table::grafted) {
        let now = world.farming.clock;
        for &(q, b) in &edits {
            if b == B::FruitBud && world.get(q) == B::FruitBud && world.farming.plots.len() < super::MAX_PLOTS {
                world.farming.plots.insert(key(q), super::Plot::new(row, now));
            }
        }
    }
    world.get(p) == B::Log
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::farming::{interact, table, tick};
    use crate::world::inventory::ItemStack;
    use crate::world::{ChunkData, WorldType};
    use engine_core::{Budget, VoxelScalePreset};
    use std::collections::HashSet;
    use std::sync::Arc;

    const CELL: IVec3 = IVec3::new(16, 9, 16);

    /// One resident chunk: dirt under grass at y = 8, air above.
    fn lawn() -> World {
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 157);
        let mut blocks = Box::new([B::Air; 32 * 32 * 32]);
        for y in 0..9 {
            for z in 0..32 {
                for x in 0..32 {
                    blocks[ChunkData::local_index(x, y, z)] = if y < 8 { B::Dirt } else { B::Grass };
                }
            }
        }
        w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks,
            light: Box::new([0; 32 * 32 * 32]),
            torches: HashSet::new(),
        }));
        w
    }

    fn use_item(w: &mut World, hit: (IVec3, IVec3), slot: usize) -> bool {
        let eye = (CELL.as_vec3() + Vec3::new(0.5, 1.5, 2.5)) * w.voxel_scale.meters();
        let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
        interact(w, Some(hit), eye, Vec3::NEG_Z, slot, &mut blocks, &mut chunks)
    }

    #[test]
    fn biomass_advances_a_sapling_until_it_grows_into_a_tree() {
        let mut w = lawn();
        w.inventory.hotbar[0] = Some(ItemStack::new(Item::Block(B::Sapling), 1));
        assert!(use_item(&mut w, (CELL - IVec3::Y, CELL), 0));
        assert_eq!(w.get(CELL), B::Sapling);
        w.inventory.hotbar[1] = Some(ItemStack::new(BIOMASS, 5));
        for stage in 1..=3 {
            assert!(use_item(&mut w, (CELL, CELL + IVec3::Y), 1));
            assert_eq!(w.farming.plots[&key(CELL)].stage, stage);
        }
        // A mature sapling refuses more biomass without spending it.
        assert!(use_item(&mut w, (CELL, CELL + IVec3::Y), 1));
        assert_eq!(w.inventory.count_item(BIOMASS), 2);
        for _ in 0..8 {
            tick(&mut w, Budget::UNBOUNDED);
        }
        assert!(w.farming.plots.is_empty(), "the grown plot is retired");
        for y in 0..4 {
            assert_eq!(w.get(CELL + IVec3::Y * y), B::Log, "trunk cell {y}");
        }
        assert_eq!(w.get(CELL + IVec3::new(1, 3, 0)), B::Leaves);
        assert_eq!(w.get(CELL + IVec3::Y * 4), B::Leaves);
    }

    #[test]
    fn tree_scales_with_voxels_and_waits_for_a_clear_trunk() {
        let (_, crop) = table::by_seed(B::Sapling).expect("sapling row");
        let shape = crop.tree.expect("sapling grows a tree");
        let open = |q: IVec3| if q == IVec3::ZERO { B::Sapling } else { B::Air };
        let logs = |edits: &[(IVec3, B)]| edits.iter().filter(|(_, b)| *b == B::Log).count();
        let metre = tree_edits(1, IVec3::ZERO, shape, 1.0, open).expect("clear");
        let quarter = tree_edits(1, IVec3::ZERO, shape, 0.25, open).expect("clear");
        assert_eq!((logs(&metre), logs(&quarter)), (4, 16));
        assert!(quarter.len() < 1024, "bounded edit: {}", quarter.len());
        assert_eq!(metre, tree_edits(1, IVec3::ZERO, shape, 1.0, open).expect("clear"));
        let blocked = |q: IVec3| if q == IVec3::Y * 2 { B::Stone } else { open(q) };
        assert!(tree_edits(1, IVec3::ZERO, shape, 1.0, blocked).is_none());
    }

    #[test]
    fn leaf_drops_are_positional_and_scale_with_cell_volume() {
        let cells = |metres: f32| {
            let (mut saplings, mut vines, mut apples) = (0, 0, 0);
            for i in 0..40_000 {
                let p = IVec3::new(i % 200, -40 + i / 4000, (i / 200) % 20);
                assert_eq!(leaf_drop(9, metres, p, B::Leaves), leaf_drop(9, metres, p, B::Leaves));
                match leaf_drop(9, metres, p, B::Leaves) {
                    Some(Item::Block(B::Sapling)) => saplings += 1,
                    Some(Item::Block(B::Vines)) => vines += 1,
                    Some(Item::Block(B::Apple)) => apples += 1,
                    None => {}
                    other => panic!("unexpected drop {other:?}"),
                }
            }
            (saplings, vines, apples)
        };
        let (saplings, vines, apples) = cells(1.0);
        assert!((1600..2400).contains(&saplings), "about 5%: {saplings}");
        assert!((700..1300).contains(&vines), "about 2.5%: {vines}");
        assert!((250..550).contains(&apples), "about 1%: {apples}");
        let (fine, _, _) = cells(0.25);
        assert!(fine < 80, "a quarter-metre leaf holds 1/64 of the chance: {fine}");
        assert_eq!(leaf_drop(9, 1.0, IVec3::ZERO, B::Stone), None);
    }
}
