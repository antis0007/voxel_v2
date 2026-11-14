//! Transactions against actual chest slots. A failed exchange has no effects.
use super::*;
use crate::world::block_data::Slots;

pub(super) fn ready(w: &World, at: IVec3) -> bool {
    super::super::resident(w, at)
        && super::super::known(w, at).is_some_and(|b| b.chest_tier().is_some())
        && w.block_data.can_store_chest(at)
}

pub(super) fn debit(slots: &mut Slots, request: ItemStack) -> bool {
    if production::count(slots, request.item) < request.count { return false; }
    let mut left = request.count;
    for slot in slots {
        if let Some(s) = slot.as_mut().filter(|s| s.item == request.item) {
            let n = s.count.min(left);
            s.count -= n; left -= n;
            if s.count == 0 { *slot = None; }
        }
    }
    true
}

pub(super) fn credit(w: &World, slots: &mut Slots, request: ItemStack) -> bool {
    let limit = w.inventory.stack_limit(request.item);
    let mut left = request.count;
    for slot in slots.iter_mut() {
        if let Some(s) = slot.as_mut().filter(|s| s.item == request.item) {
            let n = limit.saturating_sub(s.count).min(left);
            s.count += n; left -= n;
        }
    }
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        let n = limit.min(left);
        if n > 0 { *slot = Some(ItemStack::new(request.item, n)); left -= n; }
    }
    left == 0
}

pub(super) fn exchange(w: &mut World, at: IVec3, take: &[ItemStack], give: &[ItemStack]) -> bool {
    if !ready(w, at) { return false; }
    let mut next = w.block_data.chest(at);
    if !take.iter().all(|&s| debit(&mut next, s))
        || !give.iter().all(|&s| credit(w, &mut next, s)) { return false; }
    w.block_data.set_chest(at, next);
    true
}

pub(super) fn pack_take(pack: &mut Vec<ItemStack>, request: ItemStack) -> bool {
    let held: u32 = pack.iter().filter(|s| s.item == request.item).map(|s| s.count).sum();
    if held < request.count { return false; }
    let mut left = request.count;
    for stack in pack.iter_mut().filter(|s| s.item == request.item) {
        let n = stack.count.min(left);
        stack.count -= n; left -= n;
    }
    pack.retain(|s| s.count > 0);
    true
}

pub(super) fn pack_add(pack: &mut Vec<ItemStack>, stack: ItemStack) {
    if let Some(s) = pack.iter_mut().find(|s| s.item == stack.item) { s.count += stack.count; }
    else if stack.count > 0 { pack.push(stack); }
}
