//! Per-block records: sign and lectern text, chest slots, brewing stands and lit fuses.
//! They persist in `sidecar::FILE`, not the save body, so the save version is unchanged.
use super::inventory::{DroppedStack, Item, ItemStack, SlotAddr};
use super::{BlockId, World};
use engine_content::shaped;
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) mod sidecar;
pub(crate) mod tnt;
#[cfg(test)]
mod tests;

pub type Key = (i32, i32, i32);
pub const CHEST_SLOTS: usize = 27;
pub const MAX_ENTRIES: usize = 65_536;
pub(crate) const MAX_PLACED_SEDIMENT: usize = 1_048_576;
/// Loose stacks a broken chest may add; matches the cache spill limit.
const DROP_LIMIT: usize = 1024;

pub type Slots = Vec<Option<ItemStack>>;

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Text(Vec<String>),
    Chest(Box<Slots>),
    Stand(Stand),
}

/// Brewing stand slots and progress of the current brew in fixed steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stand {
    pub base: Option<ItemStack>,
    pub modifier: Option<ItemStack>,
    pub output: Option<ItemStack>,
    pub steps: u16,
}

impl Stand {
    pub fn is_empty(&self) -> bool {
        self.base.is_none() && self.modifier.is_none() && self.output.is_none() && self.steps == 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Non-natural sediment and crystal; persisted before harvest removes a cell.
    pub(crate) placed_sediment: BTreeSet<Key>,
    pub storage_policies: BTreeMap<Key, super::scanner::policy::Policy>,
    pub storage_revision: u64,
    pub entries: BTreeMap<Key, Entry>,
    /// Remaining fixed steps per lit charge.
    pub fuses: BTreeMap<Key, u16>,
    /// Cells a detonation still has to clear, with the blast centre; drained under a budget.
    pub(crate) blasts: VecDeque<(IVec3, IVec3)>,
    /// Keys of `Entry::Stand` records, so the brew tick skips every other record.
    pub(crate) stands: BTreeSet<Key>,
    /// Dungeon-bound keys per complex cell id (sidecar version 6).
    pub dungeon_keys: BTreeMap<(i32, i32), KeyRing>,
}

/// Keys held for one dungeon complex, and which key rooms are already looted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyRing {
    pub small: u8,
    pub great: bool,
    /// Bit per room index whose key has been taken.
    pub looted: u64,
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
    pub(crate) fn can_write_sediment(&self, p: IVec3, block: BlockId) -> bool {
        !(super::travel::panning::ordinary_sediment(block) || block == BlockId::Crystal)
            || self.placed_sediment.len() < MAX_PLACED_SEDIMENT
            || self.placed_sediment.contains(&key(p))
    }

    pub(crate) fn note_sediment_write(&mut self, p: IVec3, old: BlockId, new: BlockId) {
        if super::travel::panning::ordinary_sediment(new) || new == BlockId::Crystal {
            self.placed_sediment.insert(key(p));
        } else if super::travel::panning::ordinary_sediment(old) || old == BlockId::Crystal {
            self.placed_sediment.remove(&key(p));
        }
    }

    pub fn text(&self, p: IVec3) -> &[String] {
        match self.entries.get(&key(p)) {
            Some(Entry::Text(lines)) => lines,
            _ => &[],
        }
    }

    pub fn chest(&self, p: IVec3) -> Slots {
        match self.entries.get(&key(p)) {
            Some(Entry::Chest(slots)) => slots.as_ref().clone(),
            _ => vec![None; CHEST_SLOTS],
        }
    }

    pub fn stand(&self, p: IVec3) -> Stand {
        match self.entries.get(&key(p)) {
            Some(Entry::Stand(stand)) => *stand,
            _ => Stand::default(),
        }
    }

    /// Existing chests remain writable at capacity; other records are not storage.
    pub(crate) fn can_store_chest(&self, p: IVec3) -> bool {
        match self.entries.get(&key(p)) {
            Some(Entry::Chest(_)) => true,
            Some(_) => false,
            None => self.entries.len() < MAX_ENTRIES,
        }
    }

    /// Reserve the record before removing items from any source inventory.
    pub(crate) fn ensure_chest(&mut self, p: IVec3) -> bool {
        if !self.can_store_chest(p) { return false; }
        if !self.entries.contains_key(&key(p)) {
            self.set_chest(p, vec![None; CHEST_SLOTS]);
        }
        true
    }

    pub(crate) fn set_chest(&mut self, p: IVec3, slots: impl Into<Slots>) {
        let slots = slots.into();
        debug_assert!(matches!(slots.len(), 27 | 36 | 45 | 54));
        self.storage_revision = self.storage_revision.wrapping_add(1);
        self.put(p, Entry::Chest(Box::new(slots)));
    }

    pub(crate) fn set_stand(&mut self, p: IVec3, stand: Stand) {
        self.put(p, Entry::Stand(stand));
    }

    fn put(&mut self, p: IVec3, entry: Entry) {
        let empty = match &entry {
            Entry::Text(lines) => lines.is_empty(),
            // Empty chests remain discoverable without opening them first.
            Entry::Chest(_) => false,
            Entry::Stand(stand) => stand.is_empty(),
        };
        let stand = matches!(entry, Entry::Stand(_));
        if empty {
            self.entries.remove(&key(p));
            self.stands.remove(&key(p));
        } else if self.entries.len() < MAX_ENTRIES || self.entries.contains_key(&key(p)) {
            self.entries.insert(key(p), entry);
            if stand {
                self.stands.insert(key(p));
            }
        }
    }

    /// Drops the record of a replaced block; a chest's slots become loose stacks.
    pub(crate) fn on_replaced(&mut self, p: IVec3, old: BlockId, new: BlockId, dropped: &mut Vec<DroppedStack>, recovery: &mut Vec<ItemStack>, metres: f32) {
        self.note_sediment_write(p, old, new);
        if old.chest_tier().is_some() || new.chest_tier().is_some() {
            self.storage_revision = self.storage_revision.wrapping_add(1);
            self.storage_policies.remove(&key(p));
        }
        if old == BlockId::Tnt && new == BlockId::Fire {
            tnt::arm(self, p, tnt::FUSE_STEPS);
        }
        let entry = self.entries.remove(&key(p));
        if new.chest_tier().is_some() { self.set_chest(p, vec![None; new.chest_slots()]); }
        let Some(entry) = entry else { return };
        self.stands.remove(&key(p));
        let spilled: Vec<ItemStack> = match entry {
            Entry::Chest(slots) => slots.iter().flatten().copied().collect(),
            Entry::Stand(s) => [s.base, s.modifier, s.output].into_iter().flatten().collect(),
            Entry::Text(_) => Vec::new(),
        };
        if !spilled.is_empty() {
            let centre = (p.as_vec3() + Vec3::splat(0.5)) * metres;
            for stack in spilled {
                if dropped.len() < DROP_LIMIT {
                    dropped.push(DroppedStack { pos: centre, stack });
                } else {
                    recovery.push(stack);
                }
            }
        }
    }
}

/// Blocks whose use opens a page instead of acting.
pub fn opens_page(block: BlockId) -> bool {
    shaped::text_limits(block).is_some() || block.chest_tier().is_some() || shaped::STATIONS.contains(&block)
}

/// Stores edited text for the sign or lectern at `p`, within that block's limits.
pub fn set_text(world: &mut World, p: IVec3, text: &[String]) -> bool {
    let Some(limits) = shaped::text_limits(world.get(p)) else { return false };
    world.block_data.put(p, Entry::Text(clamp_text(text, limits)));
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SlotGesture { Stack, Split, Transfer, Gather, Hotbar(u8) }

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum ChestAction {
    Chest(usize, SlotGesture),
    Player(SlotAddr, SlotGesture),
    TakeAll,
    StackMatching,
    Sort,
}

/// Applies the same validated slot action in local play and on the host.
pub fn edit_chest(world: &mut World, p: IVec3, action: ChestAction) {
    if world.get(p).chest_tier().is_none() { return; }
    match action {
        ChestAction::Chest(index, gesture) => {
            if index >= world.get(p).chest_slots() { return; }
            if gesture == SlotGesture::Transfer { take(world, p, index); }
            else if world.block_data.ensure_chest(p) {
                let mut slots = world.block_data.chest(p);
                if let Some(slot) = slots.get_mut(index) {
                    let before = *slot;
                    match gesture {
                        SlotGesture::Gather => {
                            if world.inventory.cursor.is_none() { world.inventory.cursor = slot.take(); }
                            gather_chest(world, &mut slots);
                            world.inventory.gather_matching_to_cursor();
                            world.block_data.set_chest(p, slots);
                        }
                        SlotGesture::Hotbar(i) => {
                            if let Some(hotbar) = world.inventory.hotbar.get_mut(usize::from(i)) {
                                std::mem::swap(slot, hotbar);
                                if *slot != before { world.block_data.set_chest(p, slots); }
                            }
                        }
                        _ => {
                            world.inventory.edit_cursor_stack(slot, gesture == SlotGesture::Split);
                            if *slot != before { world.block_data.set_chest(p, slots); }
                        }
                    }
                }
            }
        }
        ChestAction::Player(addr, gesture) => {
            if !matches!(addr, SlotAddr::Pack(i) if i < super::inventory::PACK_SLOTS)
                && !matches!(addr, SlotAddr::Hotbar(i) if i < super::inventory::HOTBAR_SLOTS) { return; }
            match gesture {
                SlotGesture::Stack => world.inventory.click_slot(addr),
                SlotGesture::Split => world.inventory.right_click_slot(addr),
                SlotGesture::Transfer => store(world, p, addr),
                SlotGesture::Hotbar(i) if usize::from(i) < super::inventory::HOTBAR_SLOTS =>
                    world.inventory.swap_slot_with_hotbar(addr, usize::from(i)),
                SlotGesture::Hotbar(_) => {}
                SlotGesture::Gather => {
                    world.inventory.gather_to_cursor(addr);
                    if world.block_data.can_store_chest(p) {
                        let mut slots = world.block_data.chest(p);
                        if gather_chest(world, &mut slots) { world.block_data.set_chest(p, slots); }
                    }
                }
            }
        }
        ChestAction::TakeAll => { super::scanner::take_all(world, &super::scanner::chest(p)); }
        ChestAction::StackMatching => { super::scanner::deposit(world, &super::scanner::chest(p), super::scanner::Reach::Existing); }
        ChestAction::Sort => { super::scanner::sort(world, &super::scanner::chest(p)); }
    }
    world.sync_from_inventory();
}

fn gather_chest(world: &mut World, slots: &mut Slots) -> bool {
    let Some(mut held) = world.inventory.cursor else { return false };
    let before = held.count;
    let cap = world.inventory.stack_limit(held.item);
    for slot in slots {
        if held.count >= cap { break; }
        let Some(mut stack) = slot.filter(|s| s.item == held.item) else { continue };
        let n = stack.count.min(cap - held.count);
        held.count += n;
        stack.count -= n;
        *slot = (stack.count > 0).then_some(stack);
    }
    world.inventory.cursor = Some(held);
    held.count != before
}

/// Moves chest slot `index` at `p` into the player's hotbar and pack.
pub fn take(world: &mut World, p: IVec3, index: usize) {
    if world.get(p).chest_tier().is_none() {
        return;
    }
    let mut slots = world.block_data.chest(p);
    let Some(stack) = slots.get(index).copied().flatten() else { return };
    let (_, left) = world.inventory.try_insert(stack);
    slots[index] = (left > 0).then_some(ItemStack::new(stack.item, left));
    world.block_data.set_chest(p, slots);
}

/// Moves the player's stack at `addr` into the chest at `p`, merging first.
pub fn store(world: &mut World, p: IVec3, addr: SlotAddr) {
    if world.get(p).chest_tier().is_none() {
        return;
    }
    let Some(mut stack) = world.inventory.slot(addr) else { return };
    if !world.block_data.ensure_chest(p) { return; }
    let max = world.inventory.stack_limit(stack.item);
    let mut slots = world.block_data.chest(p);
    merge(&mut slots, &mut stack, max);
    if let Some(slot) = world.inventory.slot_mut(addr) {
        *slot = (stack.count > 0).then_some(stack);
    }
    world.block_data.set_chest(p, slots);
}

pub(crate) fn store_hauled(world: &mut World, p: IVec3, stack: &mut ItemStack) {
    if world.get(p).chest_tier().is_none() || !world.block_data.ensure_chest(p) {
        return;
    }
    let mut slots = world.block_data.chest(p);
    merge(&mut slots, stack, world.inventory.stack_limit(stack.item));
    world.block_data.set_chest(p, slots);
}

pub(crate) fn take_hauled(world: &mut World, p: IVec3, item: Item, mut count: u32) -> u32 {
    if world.get(p).chest_tier().is_none() || !world.block_data.can_store_chest(p) {
        return 0;
    }
    let requested = count;
    let mut slots = world.block_data.chest(p);
    for slot in &mut slots {
        let Some(stack) = slot.as_mut().filter(|s| s.item == item) else { continue };
        let take = count.min(stack.count);
        stack.count -= take;
        count -= take;
        if stack.count == 0 {
            *slot = None;
        }
        if count == 0 {
            break;
        }
    }
    if count < requested {
        world.block_data.set_chest(p, slots);
    }
    requested - count
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
