//! Anvil, grindstone and stonecutter actions on inventory slots, and the brewing stand.
//!
//! Station input cells reuse the persisted transient grid and participate in wear tracking.
pub mod brewing;
pub mod table;
#[cfg(test)]
mod tests;

use super::World;
use super::inventory::{Item, ItemStack, SlotAddr};
use super::progression::{Slot, xp};
use table::STATIONS;

/// What an anvil or grindstone does to the first input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Remaining durability of the first input afterwards.
    pub left: u32,
    /// Units taken from the second input.
    pub consumed: u32,
    pub levels: u32,
}

/// Anvil result for an item with `left` of `max` durability and a second input.
/// `other_left` is the second input's remaining durability when it is durable.
pub fn anvil(item: Item, left: u32, max: u32, other: ItemStack, other_left: Option<u32>) -> Option<Outcome> {
    let t = &*STATIONS;
    if left >= max {
        return None;
    }
    if other.item == item {
        let bonus = (max as f32 * t.anvil_combine_bonus) as u32;
        let left = left.saturating_add(other_left?).saturating_add(bonus).min(max);
        return Some(Outcome { left, consumed: 1, levels: t.anvil_combine_levels });
    }
    if t.material(item)? != other.item {
        return None;
    }
    let per = ((max as f32 * t.repair_per_unit).ceil() as u32).max(1);
    let units = (max - left).div_ceil(per).min(other.count);
    Some(Outcome { left: (left + units * per).min(max), consumed: units, levels: units * t.levels_per_unit })
}

/// Grindstone result for two damaged items of the same kind; never costs levels.
pub fn grindstone(item: Item, left: u32, max: u32, other: Item, other_left: Option<u32>) -> Option<Outcome> {
    if other != item || left >= max {
        return None;
    }
    let bonus = (max as f32 * STATIONS.grindstone_combine_bonus) as u32;
    Some(Outcome { left: left.saturating_add(other_left?).saturating_add(bonus).min(max), consumed: 1, levels: 0 })
}

/// Material refunded for salvaging an item with `left` of `max` durability.
pub fn salvage(item: Item, left: u32, max: u32) -> Option<ItemStack> {
    let row = STATIONS.salvage.iter().find(|r| r.item == item)?;
    let max = u64::from(max.max(1));
    let count = ((u64::from(row.count) * u64::from(left) + max / 2) / max) as u32;
    (count > 0).then(|| ItemStack::new(row.material, count))
}

/// Experience left after paying `levels` whole levels; progress into the level is kept.
pub fn spend_levels(total: u32, levels: u32) -> Option<u32> {
    let (level, into) = xp::level_of(total);
    let target = level.checked_sub(levels)?;
    let base: u32 = (0..target).map(xp::to_next).sum();
    Some(base + into.min(xp::to_next(target) - 1))
}

/// Item and durability of the stack at `addr`.
pub fn durable(world: &World, addr: SlotAddr) -> Option<(Item, u32, u32)> {
    let item = world.inventory.slot(addr)?.item;
    let (left, max) = world.progression.wear.durability(&world.inventory, Slot::of_addr(addr)?)?;
    Some((item, left, max))
}

/// Anvil preview for the stacks at `a` and `b`.
pub fn anvil_preview(world: &World, a: SlotAddr, b: SlotAddr) -> Option<Outcome> {
    if a == b {
        return None;
    }
    let (item, left, max) = durable(world, a)?;
    let other = world.inventory.slot(b)?;
    anvil(item, left, max, other, durable(world, b).map(|(_, l, _)| l))
}

pub fn grindstone_preview(world: &World, a: SlotAddr, b: SlotAddr) -> Option<Outcome> {
    if a == b {
        return None;
    }
    let (item, left, max) = durable(world, a)?;
    let (other, other_left, _) = durable(world, b)?;
    grindstone(item, left, max, other, Some(other_left))
}

/// Repairs or combines on the anvil, paying its levels. False when it cannot.
pub fn use_anvil(world: &mut World, a: SlotAddr, b: SlotAddr) -> bool {
    let Some(outcome) = anvil_preview(world, a, b) else { return false };
    if !world.rules.mode.creative_tools() {
        let Some(xp) = spend_levels(world.progression.xp, outcome.levels) else { return false };
        world.progression.xp = xp;
    }
    apply(world, a, b, outcome);
    true
}

pub fn use_grindstone(world: &mut World, a: SlotAddr, b: SlotAddr) -> bool {
    let Some(outcome) = grindstone_preview(world, a, b) else { return false };
    apply(world, a, b, outcome);
    true
}

/// Breaks the item at `a` down into its salvage refund.
pub fn use_salvage(world: &mut World, a: SlotAddr) -> bool {
    let Some((item, left, max)) = durable(world, a) else { return false };
    let Some(refund) = salvage(item, left, max) else { return false };
    take(world, a, 1);
    // The emptied slot always has room for the refund.
    let _ = world.inventory.try_insert(refund);
    world.sync_from_inventory();
    true
}

pub(crate) fn salvage_in_slot(world: &mut World, addr: SlotAddr) -> bool {
    let Some((item, left, max)) = durable(world, addr) else { return false };
    let Some(refund) = salvage(item, left, max) else { return false };
    if world.inventory.slot(addr).is_none_or(|s| s.count != 1) { return false; }
    take(world, addr, 1);
    *world.inventory.slot_mut(addr).unwrap() = Some(refund);
    world.sync_from_inventory();
    true
}

pub(crate) fn cut_from_slot(world: &mut World, addr: SlotAddr, row: usize, transfer: bool) {
    let Some(cut) = STATIONS.stonecutter.get(row) else { return };
    let Some(input) = world.inventory.slot(addr).filter(|s| s.item == cut.input) else { return };
    let output = ItemStack::new(cut.output, cut.count);
    if transfer {
        let room = world.inventory.remaining_capacity_for(output.item);
        let batches = input.count.min(room / output.count);
        if batches == 0 { return; }
        take(world, addr, batches);
        world.inventory.try_insert(ItemStack::new(output.item, batches * output.count));
    } else {
        let inv = &mut world.inventory;
        let count = inv.cursor.map_or(0, |s| s.count);
        if inv.cursor.is_some_and(|s| s.item != output.item)
            || inv.stack_limit(output.item).saturating_sub(count) < output.count { return; }
        take(world, addr, 1);
        world.inventory.cursor = Some(ItemStack::new(output.item, count + output.count));
    }
    world.sync_from_inventory();
}

/// Cuts one block at `addr` with stonecutter row `index`. False when the row does not apply.
pub fn use_stonecutter(world: &mut World, addr: SlotAddr, index: usize) -> bool {
    let Some(cut) = STATIONS.stonecutter.get(index) else { return false };
    if world.inventory.slot(addr).map(|s| s.item) != Some(cut.input) {
        return false;
    }
    take(world, addr, 1);
    let (_, left) = world.inventory.try_insert(ItemStack::new(cut.output, cut.count));
    if left > 0 {
        let _ = world.inventory.remove_item(cut.output, cut.count - left);
        let _ = world.inventory.try_insert(ItemStack::new(cut.input, 1));
        world.sync_from_inventory();
        return false;
    }
    world.sync_from_inventory();
    true
}

fn apply(world: &mut World, a: SlotAddr, b: SlotAddr, outcome: Outcome) {
    let (Some((item, _, max)), Some(slot)) = (durable(world, a), Slot::of_addr(a)) else { return };
    let used = max - outcome.left.min(max);
    let entries = &mut world.progression.wear.entries;
    if let Some(entry) = entries.iter_mut().find(|e| e.slot == slot && e.item == item) {
        entry.used = used;
    }
    entries.retain(|e| e.used > 0);
    take(world, b, outcome.consumed);
    world.sync_from_inventory();
}

/// Removes `count` from the stack at `addr`, and its wear once the slot empties.
fn take(world: &mut World, addr: SlotAddr, count: u32) {
    let Some(cell) = world.inventory.slot_mut(addr) else { return };
    let Some(stack) = cell.as_mut() else { return };
    stack.count = stack.count.saturating_sub(count);
    if stack.count == 0 {
        *cell = None;
        if let Some(slot) = Slot::of_addr(addr) {
            world.progression.wear.entries.retain(|e| e.slot != slot);
        }
    }
}
