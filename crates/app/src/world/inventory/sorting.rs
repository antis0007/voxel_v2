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
    pub fn recover_overflow(&mut self) {
        #[cfg(feature = "multiplayer")]
        let _record = crate::multiplayer::inventory::record(crate::multiplayer::inventory::Command::Recover);
        self.migrate_balance(self.balance);
    }
    /// Split excess into spare slots and retain everything else in the saved
    /// recovery ledger. Hotbar positions and protected stacks keep their items.
    pub fn migrate_balance(&mut self, balance: super::stacks::Balance) {
        self.balance = balance;
        let scale = self.voxel_m;
        let mut excess = std::mem::take(&mut self.overflow);
        for slot in self.hotbar.iter_mut().chain(self.pack.iter_mut()).chain(self.craft_grid.iter_mut())
            .chain(self.equipment.iter_mut()).chain(std::iter::once(&mut self.cursor)).chain(std::iter::once(&mut self.offhand)) {
            if let Some(s) = slot {
                let cap = balance.limit(s.item, scale);
                if s.count > cap { excess.push(ItemStack { item: s.item, count: s.count - cap }); s.count = cap; }
            }
        }
        for stack in excess {
            let (_, left) = self.try_insert(stack);
            if left > 0 { self.overflow.push(ItemStack { item: stack.item, count: left }); }
        }
    }
    pub fn is_protected(&self, addr: super::SlotAddr) -> bool {
        use super::SlotAddr;
        match addr {
            SlotAddr::Hotbar(i) if i < super::HOTBAR_SLOTS => self.protected & (1 << i) != 0,
            SlotAddr::Pack(i) if i < super::PACK_SLOTS => self.protected & (1 << (i + super::HOTBAR_SLOTS)) != 0,
            _ => false,
        }
    }

    pub fn toggle_protected(&mut self, addr: super::SlotAddr) {
        #[cfg(feature = "multiplayer")]
        let _record = crate::multiplayer::inventory::record(crate::multiplayer::inventory::Command::Lock(addr));
        let bit = match addr {
            super::SlotAddr::Hotbar(i) if i < super::HOTBAR_SLOTS => i,
            super::SlotAddr::Pack(i) if i < super::PACK_SLOTS => i + super::HOTBAR_SLOTS,
            _ => return,
        };
        self.protected ^= 1 << bit;
    }

    /// Merge backwards into earlier pack slots without moving any other item.
    pub fn auto_stack_item(&mut self, item: Item) {
        let cap = self.stack_limit(item);
        for a in 0..super::PACK_SLOTS {
            if self.is_protected(super::SlotAddr::Pack(a)) { continue; }
            let Some(mut dst) = self.pack[a].filter(|s| s.item == item && s.count < cap) else { continue };
            for b in (a + 1..super::PACK_SLOTS).rev() {
                if self.is_protected(super::SlotAddr::Pack(b)) { continue; }
                let Some(mut src) = self.pack[b].filter(|s| s.item == item) else { continue };
                let n = src.count.min(cap - dst.count);
                src.count -= n; dst.count += n;
                self.pack[b] = (src.count > 0).then_some(src);
                if dst.count == cap { break; }
            }
            self.pack[a] = Some(dst);
        }
    }

    pub fn auto_stack(&mut self) {
        let mut seen = Vec::new();
        for i in 0..super::PACK_SLOTS {
            if let Some(s) = self.pack[i] && !seen.contains(&s.item) {
                seen.push(s.item);
                self.auto_stack_item(s.item);
            }
        }
    }

    pub fn minimum(&self, item: Item) -> u32 {
        self.minimums.iter().find(|(i, _)| *i == item).map_or(0, |(_, n)| *n)
    }

    /// Removes up to `max` of `item` from pack slots only; returns the count.
    pub fn take_from_pack(&mut self, item: Item, max: u32) -> u32 {
        let mut taken = 0;
        for (i, slot) in self.pack.iter_mut().enumerate() {
            if self.protected & (1 << (i + super::HOTBAR_SLOTS)) != 0 { continue; }
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
