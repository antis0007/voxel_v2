//! Lossless slot sorting: merge equal items, order by category, id and count.
use super::{Item, ItemStack, SurvivalInventory};

/// Category order for sorted slots; names match `Item::category`.
const CATEGORY_ORDER: [&str; 16] = [
    "Melee weapon",
    "Excavation tool",
    "Arcane focus",
    "Wand",
    "Equipment · Head",
    "Equipment · Chest",
    "Equipment · Legs",
    "Equipment · Feet",
    "Food",
    "Raw material",
    "Refined component",
    "Packed power assembly",
    "Machine",
    "Crafting station",
    "Lighting block",
    "Building block",
];

/// Total order used by every sort: category rank, then encoded id.
pub fn sort_key(item: Item) -> (usize, u32, u32) {
    let category = item.category();
    // A category missing from the table sorts last instead of failing.
    let rank = CATEGORY_ORDER
        .iter()
        .position(|name| *name == category)
        .unwrap_or(CATEGORY_ORDER.len());
    let (kind, payload) = item.encode_u32();
    (rank, kind, payload)
}

/// Merges, sorts and compacts `slots`; never drops or duplicates a unit.
///
/// A stack already above `limit` (the voxel scale shrank since it was filled)
/// raises that item's cap to its largest stack, so the merged item needs at
/// most as many slots as it had before and always fits back.
pub fn sort_slots(slots: &mut [Option<ItemStack>], limit: impl Fn(Item) -> u32) {
    let mut totals: Vec<(Item, u64, u32)> = Vec::new();
    for stack in slots.iter_mut().filter_map(Option::take) {
        if stack.count == 0 {
            continue;
        }
        match totals.iter_mut().find(|(item, _, _)| *item == stack.item) {
            Some((_, total, largest)) => {
                *total += u64::from(stack.count);
                *largest = (*largest).max(stack.count);
            }
            None => totals.push((stack.item, u64::from(stack.count), stack.count)),
        }
    }
    totals.sort_by(|a, b| sort_key(a.0).cmp(&sort_key(b.0)).then(b.1.cmp(&a.1)));
    let mut next = 0;
    for (item, mut total, largest) in totals {
        let cap = u64::from(limit(item).max(largest).max(1));
        while total > 0 {
            let count = total.min(cap);
            let Some(slot) = slots.get_mut(next) else {
                debug_assert!(false, "sorted stacks exceeded the slot count");
                return;
            };
            *slot = Some(ItemStack { item, count: count as u32 });
            total -= count;
            next += 1;
        }
    }
}

impl SurvivalInventory {
    /// Removes up to `max` of `item` from pack slots only; returns the count.
    pub fn take_from_pack(&mut self, item: Item, max: u32) -> u32 {
        let mut taken = 0;
        for slot in &mut self.pack {
            let Some(stack) = slot else { continue };
            if stack.item != item || taken == max {
                continue;
            }
            let take = stack.count.min(max - taken);
            stack.count -= take;
            taken += take;
            if stack.count == 0 {
                *slot = None;
            }
        }
        taken
    }

    /// Count of `item` in pack slots, excluding hotbar and transient slots.
    #[cfg(test)]
    pub fn pack_count(&self, item: Item) -> u32 {
        self.pack
            .iter()
            .flatten()
            .filter(|stack| stack.item == item)
            .fold(0u32, |sum, stack| sum.saturating_add(stack.count))
    }
}

#[cfg(test)]
#[path = "sorting_tests.rs"]
mod tests;
