//! Tool, weapon and armour wear, kept beside the inventory and keyed by slot.
//!
//! `ItemStack` has no wear field and its encoding is saved, so wear lives here.
//! `reconcile` runs every fixed step: an entry whose slot no longer holds its
//! item moves to a slot where the same item newly arrived (cursor, shift-click,
//! swap). A stack that leaves the inventory (thrown, stored in a cache) loses
//! its wear and returns at full durability.
use super::table;
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Hotbar(u8),
    Pack(u8),
    Equipment(u8),
    Offhand,
    Cursor,
    CraftGrid(u8),
}

impl Slot {
    pub fn code(self) -> u8 {
        match self {
            Self::Hotbar(i) => i,
            Self::Pack(i) => 32 + i,
            Self::Equipment(i) => 96 + i,
            Self::Offhand => 120,
            Self::Cursor => 121,
            Self::CraftGrid(i) => 122 + i,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        use crate::world::inventory::{CRAFT_SLOTS, EQUIP_SLOTS, HOTBAR_SLOTS, PACK_SLOTS};
        Some(match code {
            c if (c as usize) < HOTBAR_SLOTS => Self::Hotbar(c),
            c if (32..32 + PACK_SLOTS as u8).contains(&c) => Self::Pack(c - 32),
            c if (96..96 + EQUIP_SLOTS as u8).contains(&c) => Self::Equipment(c - 96),
            120 => Self::Offhand,
            121 => Self::Cursor,
            c if (122..122 + CRAFT_SLOTS as u8).contains(&c) => Self::CraftGrid(c - 122),
            _ => return None,
        })
    }

    pub fn of_addr(addr: crate::world::inventory::SlotAddr) -> Option<Self> {
        use crate::world::inventory::SlotAddr as A;
        Some(match addr {
            A::Hotbar(i) => Self::Hotbar(u8::try_from(i).ok()?),
            A::Pack(i) => Self::Pack(u8::try_from(i).ok()?),
            A::Equipment(i) => Self::Equipment(u8::try_from(i).ok()?),
            A::Offhand => Self::Offhand,
            A::CraftGrid(i) => Self::CraftGrid(u8::try_from(i).ok()?),
            _ => return None,
        })
    }

    fn get(self, inv: &SurvivalInventory) -> Option<ItemStack> {
        match self {
            Self::Hotbar(i) => inv.hotbar.get(i as usize).copied().flatten(),
            Self::Pack(i) => inv.pack.get(i as usize).copied().flatten(),
            Self::Equipment(i) => inv.equipment.get(i as usize).copied().flatten(),
            Self::Offhand => inv.offhand,
            Self::Cursor => inv.cursor,
            Self::CraftGrid(i) => inv.craft_grid.get(i as usize).copied().flatten(),
        }
    }

    fn get_mut(self, inv: &mut SurvivalInventory) -> Option<&mut Option<ItemStack>> {
        match self {
            Self::Hotbar(i) => inv.hotbar.get_mut(i as usize),
            Self::Pack(i) => inv.pack.get_mut(i as usize),
            Self::Equipment(i) => inv.equipment.get_mut(i as usize),
            Self::Offhand => Some(&mut inv.offhand),
            Self::Cursor => Some(&mut inv.cursor),
            Self::CraftGrid(i) => inv.craft_grid.get_mut(i as usize),
        }
    }
}

fn slots() -> impl Iterator<Item = Slot> {
    use crate::world::inventory::{CRAFT_SLOTS, EQUIP_SLOTS, HOTBAR_SLOTS, PACK_SLOTS};
    (0..HOTBAR_SLOTS as u8)
        .map(Slot::Hotbar)
        .chain((0..PACK_SLOTS as u8).map(Slot::Pack))
        .chain((0..EQUIP_SLOTS as u8).map(Slot::Equipment))
        .chain([Slot::Offhand, Slot::Cursor])
        .chain((0..CRAFT_SLOTS as u8).map(Slot::CraftGrid))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub slot: Slot,
    pub item: Item,
    pub used: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wear {
    pub entries: Vec<Entry>,
    /// Durable items per slot at the last reconcile; never saved.
    seen: Vec<(Slot, Item)>,
}

fn durable(inv: &SurvivalInventory) -> Vec<(Slot, Item)> {
    slots()
        .filter_map(|s| s.get(inv).map(|st| (s, st.item)))
        .filter(|(_, item)| table::max_durability(*item).is_some())
        .collect()
}

impl Wear {
    /// Moves entries after their stacks and drops entries whose stack left the inventory.
    pub fn reconcile(&mut self, inv: &SurvivalInventory) {
        let now = durable(inv);
        let holds = |slot: Slot, item: Item| now.iter().any(|&(s, i)| s == slot && i == item);
        let (mut kept, orphans): (Vec<Entry>, Vec<Entry>) =
            self.entries.drain(..).partition(|e| holds(e.slot, e.item));
        for orphan in orphans {
            let arrived = now.iter().find(|&&(s, i)| {
                i == orphan.item
                    && !self.seen.contains(&(s, i))
                    && !kept.iter().any(|e| e.slot == s)
            });
            if let Some(&(slot, _)) = arrived {
                kept.push(Entry { slot, ..orphan });
            }
        }
        self.entries = kept;
        self.seen = now;
    }

    #[cfg(test)]
    pub fn used(&self, slot: Slot) -> u32 {
        self.entries.iter().find(|e| e.slot == slot).map_or(0, |e| e.used)
    }

    /// Remaining and maximum durability of the stack in `slot`.
    pub fn durability(&self, inv: &SurvivalInventory, slot: Slot) -> Option<(u32, u32)> {
        let item = slot.get(inv)?.item;
        let max = table::max_durability(item)?;
        let used = self.entries.iter().find(|e| e.slot == slot && e.item == item).map_or(0, |e| e.used);
        Some((max.saturating_sub(used), max))
    }

    /// Spends `amount` on the stack in `slot`; removes it and returns its item once it breaks.
    pub fn spend(&mut self, inv: &mut SurvivalInventory, slot: Slot, amount: u32) -> Option<Item> {
        let item = slot.get(inv)?.item;
        let max = table::max_durability(item)?;
        if amount == 0 {
            return None;
        }
        let index = match self.entries.iter().position(|e| e.slot == slot && e.item == item) {
            Some(i) => i,
            None => {
                self.entries.retain(|e| e.slot != slot);
                self.entries.push(Entry { slot, item, used: 0 });
                self.entries.len() - 1
            }
        };
        let entry = &mut self.entries[index];
        entry.used = entry.used.saturating_add(amount);
        if entry.used < max {
            return None;
        }
        self.entries.remove(index);
        if let Some(cell) = slot.get_mut(inv) {
            *cell = None;
        }
        self.seen.retain(|&(s, _)| s != slot);
        Some(item)
    }
}
