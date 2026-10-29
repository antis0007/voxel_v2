//! Per-block records: sign and lectern text, chest slots and lit fuses.
//! They persist in `sidecar::FILE`, not the save body, so the save version is unchanged.
use super::inventory::{DroppedStack, Item, ItemStack, SlotAddr};
use super::{BlockId, World};
use engine_content::shaped;
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, VecDeque};

pub(crate) mod sidecar;
pub(crate) mod tnt;
#[cfg(test)]
mod tests;

pub type Key = (i32, i32, i32);
pub const CHEST_SLOTS: usize = 27;
pub const MAX_ENTRIES: usize = 65_536;
/// Loose stacks a broken chest may add; matches the cache spill limit.
const DROP_LIMIT: usize = 1024;

pub type Slots = [Option<ItemStack>; CHEST_SLOTS];

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Text(Vec<String>),
    Chest(Box<Slots>),
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub entries: BTreeMap<Key, Entry>,
    /// Remaining fixed steps per lit charge.
    pub fuses: BTreeMap<Key, u16>,
    /// Cells a detonation still has to clear, with the blast centre; drained under a budget.
    pub(crate) blasts: VecDeque<(IVec3, IVec3)>,
}

pub fn key(p: IVec3) -> Key {
    (p.x, p.y, p.z)
}

/// Keeps at most `lines` printable lines of at most `chars` characters each.
pub fn clamp_text(text: &[String], (lines, chars): (usize, usize)) -> Vec<String> {
    let mut out: Vec<String> = text
        .iter()
        .take(lines)
        .map(|l| l.chars().filter(|c| !c.is_control()).take(chars).collect())
        .collect();
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

impl State {
    pub fn text(&self, p: IVec3) -> &[String] {
        match self.entries.get(&key(p)) {
            Some(Entry::Text(lines)) => lines,
            _ => &[],
        }
    }

    pub fn chest(&self, p: IVec3) -> Slots {
        match self.entries.get(&key(p)) {
            Some(Entry::Chest(slots)) => **slots,
            _ => [None; CHEST_SLOTS],
        }
    }

    fn put(&mut self, p: IVec3, entry: Entry) {
        let empty = match &entry {
            Entry::Text(lines) => lines.is_empty(),
            Entry::Chest(slots) => slots.iter().all(Option::is_none),
        };
        if empty {
            self.entries.remove(&key(p));
        } else if self.entries.len() < MAX_ENTRIES || self.entries.contains_key(&key(p)) {
            self.entries.insert(key(p), entry);
        }
    }

    /// Drops the record of a replaced block; a chest's slots become loose stacks.
    pub(crate) fn on_replaced(&mut self, p: IVec3, old: BlockId, new: BlockId, dropped: &mut Vec<DroppedStack>, metres: f32) {
        if old == BlockId::Tnt && new == BlockId::Fire {
            tnt::arm(self, p, tnt::FUSE_STEPS);
        }
        let Some(entry) = self.entries.remove(&key(p)) else { return };
        if let Entry::Chest(slots) = entry {
            let centre = (p.as_vec3() + Vec3::splat(0.5)) * metres;
            for stack in slots.into_iter().flatten() {
                if dropped.len() < DROP_LIMIT {
                    dropped.push(DroppedStack { pos: centre, stack });
                }
            }
        }
    }
}

/// Blocks whose use opens a page instead of acting.
pub fn opens_page(block: BlockId) -> bool {
    shaped::text_limits(block).is_some() || block == BlockId::Chest
}

/// Stores edited text for the sign or lectern at `p`, within that block's limits.
pub fn set_text(world: &mut World, p: IVec3, text: &[String]) -> bool {
    let Some(limits) = shaped::text_limits(world.get(p)) else { return false };
    world.block_data.put(p, Entry::Text(clamp_text(text, limits)));
    true
}

/// Moves chest slot `index` at `p` into the player's hotbar and pack.
pub fn take(world: &mut World, p: IVec3, index: usize) {
    if world.get(p) != BlockId::Chest {
        return;
    }
    let mut slots = world.block_data.chest(p);
    let Some(stack) = slots.get(index).copied().flatten() else { return };
    let (_, left) = world.inventory.try_insert(stack);
    slots[index] = (left > 0).then_some(ItemStack::new(stack.item, left));
    world.block_data.put(p, Entry::Chest(Box::new(slots)));
}

/// Moves the player's stack at `addr` into the chest at `p`, merging first.
pub fn store(world: &mut World, p: IVec3, addr: SlotAddr) {
    if world.get(p) != BlockId::Chest {
        return;
    }
    let Some(mut stack) = world.inventory.slot(addr) else { return };
    let max = world.inventory.stack_limit(stack.item);
    let mut slots = world.block_data.chest(p);
    merge(&mut slots, &mut stack, max);
    if let Some(slot) = world.inventory.slot_mut(addr) {
        *slot = (stack.count > 0).then_some(stack);
    }
    world.block_data.put(p, Entry::Chest(Box::new(slots)));
}

fn merge(slots: &mut Slots, stack: &mut ItemStack, max: u32) {
    for slot in slots.iter_mut().flatten() {
        if slot.item == stack.item && slot.count < max {
            let add = stack.count.min(max - slot.count);
            slot.count += add;
            stack.count -= add;
        }
    }
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        if stack.count == 0 {
            break;
        }
        let add = stack.count.min(max);
        *slot = Some(ItemStack::new(stack.item, add));
        stack.count -= add;
    }
}

/// The item a block drops when a blast removes it.
pub(crate) fn blast_drop(block: BlockId) -> Option<Item> {
    Item::canonical_block(block).map(Item::Block)
}
