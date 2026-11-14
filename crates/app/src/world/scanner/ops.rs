//! Slot moves across several containers. Pure, so unit totals are checkable without a world.
use crate::world::inventory::{Item, ItemStack};

pub const SLOTS: usize = 27;
pub type Slots = [Option<ItemStack>; SLOTS];
pub type Inventory = Vec<Option<ItemStack>>;

const _: () = assert!(SLOTS == crate::world::block_data::CHEST_SLOTS && SLOTS == crate::world::containers::SLOTS);

/// Removes up to `amount` of `item`; earlier containers first, each from its last slot.
pub fn take<S: AsRef<[Option<ItemStack>]> + AsMut<[Option<ItemStack>]>>(containers: &mut [S], item: Item, amount: u32) -> u32 {
    let mut taken = 0;
    for slots in containers.iter_mut() {
        for slot in slots.as_mut().iter_mut().rev() {
            if taken == amount {
                return taken;
            }
            let Some(stack) = slot.as_mut().filter(|s| s.item == item) else { continue };
            let n = stack.count.min(amount - taken);
            stack.count -= n;
            taken += n;
            if stack.count == 0 {
                *slot = None;
            }
        }
    }
    taken
}

/// How far a deposit may spread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Only containers that already hold the item.
    Existing,
    /// Then any empty slot, nearest container first.
    Anywhere,
    /// Assigned destinations, partial stacks, category policies, then overflow.
    Smart,
}

/// Moves as much of `stack` in as fits and leaves the rest in `stack`.
///
/// Order: partial stacks of the item, empty slots of containers holding it,
/// then (`Anywhere`) any empty slot. Earlier containers win every tie.
pub fn put<S: AsRef<[Option<ItemStack>]> + AsMut<[Option<ItemStack>]>>(containers: &mut [S], stack: &mut ItemStack, max: u32, reach: Reach) {
    for slot in containers.iter_mut().flat_map(|s| s.as_mut().iter_mut()).flatten() {
        if stack.count == 0 {
            return;
        }
        if slot.item == stack.item && slot.count < max {
            let n = stack.count.min(max - slot.count);
            slot.count += n;
            stack.count -= n;
        }
    }
    let fill = |slots: &mut S, stack: &mut ItemStack| {
        for slot in slots.as_mut().iter_mut().filter(|s| s.is_none()) {
            if stack.count == 0 {
                return;
            }
            let n = stack.count.min(max);
            *slot = Some(ItemStack { item: stack.item, count: n });
            stack.count -= n;
        }
    };
    for slots in containers.iter_mut() {
        if slots.as_ref().iter().flatten().any(|s| s.item == stack.item) {
            fill(slots, stack);
        }
    }
    if reach == Reach::Anywhere {
        for slots in containers.iter_mut() {
            fill(slots, stack);
        }
    }
}

fn occupied<S: AsRef<[Option<ItemStack>]>>(containers: &[S]) -> usize {
    containers.iter().flat_map(|s| s.as_ref().iter()).flatten().count()
}

/// Merges each item's partial stacks into the containers holding most of it; returns slots freed.
///
/// Items with at most one partial stack stay where they are. A stack above
/// `limit` (the voxel scale shrank) raises that item's cap to its size, so the
/// merged item never needs more slots than it held.
pub fn consolidate<S: AsRef<[Option<ItemStack>]> + AsMut<[Option<ItemStack>]>>(containers: &mut [S], limit: impl Fn(Item) -> u32) -> usize {
    let before = occupied(containers);
    let mut items: Vec<Item> = Vec::new();
    for stack in containers.iter().flat_map(|s| s.as_ref().iter()).flatten() {
        if !items.contains(&stack.item) {
            items.push(stack.item);
        }
    }
    for item in items {
        let stacks = || containers.iter().flat_map(|s| s.as_ref().iter()).flatten().filter(|s| s.item == item);
        let largest = stacks().map(|s| s.count).max().unwrap_or(0);
        let cap = limit(item).max(largest).max(1);
        if stacks().filter(|s| s.count < cap).count() <= 1 {
            continue;
        }
        let mut held: Vec<(usize, u64)> = containers.iter().map(|slots| {
            slots.as_ref().iter().flatten().filter(|s| s.item == item).map(|s| u64::from(s.count)).sum()
        }).enumerate().collect();
        held.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut total: u64 = held.iter().map(|h| h.1).sum();
        for slot in containers.iter_mut().flat_map(|s| s.as_mut().iter_mut()) {
            if slot.is_some_and(|s| s.item == item) {
                *slot = None;
            }
        }
        for (index, _) in held {
            for slot in containers[index].as_mut().iter_mut().filter(|s| s.is_none()) {
                if total == 0 {
                    break;
                }
                let n = total.min(u64::from(cap));
                *slot = Some(ItemStack { item, count: n as u32 });
                total -= n;
            }
        }
        debug_assert_eq!(total, 0, "consolidation lost {item:?}");
    }
    before - occupied(containers)
}
