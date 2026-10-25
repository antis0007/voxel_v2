//! Fixed-volume resource transfers. Capacity is checked after inputs are removed.
use super::{SurvivalInventory as Inventory, Item, ItemStack};

pub fn exchange(inv: &mut Inventory, costs: &[(usize, u32)], gains: &[(usize, u32)]) -> bool {
    let mut next = inv.clone();
    for &(i, amount) in costs {
        let item = Item::Resource(i as u8);
        let whole = next.count_item(item);
        let total = whole * 64 + next.resource_remainders[i];
        if total < amount { return false; }
        let left = total - amount;
        if !next.remove_item(item, whole - left / 64) { return false; }
        next.resource_remainders[i] = left % 64;
    }
    for &(i, amount) in gains {
        let total = next.resource_remainders[i] + amount;
        let item = Item::Resource(i as u8);
        if total / 64 > 0 {
            let (_, remainder) = next.try_insert(ItemStack::new(item, total / 64));
            if remainder != 0 { return false; }
            next.notify_pickup(item, total / 64);
        }
        next.resource_remainders[i] = total % 64;
    }
    *inv = next;
    true
}

/// Accept only what fits; the caller retains the unaccepted machine buffer.
pub fn collect(inv: &mut Inventory, i: usize, amount: u32) -> u32 {
    let item = Item::Resource(i as u8);
    let before = inv.resource_remainders[i];
    let total = before + amount;
    let (inserted, rejected) = if total / 64 == 0 { (0, 0) }
        else { inv.try_insert(ItemStack::new(item, total / 64)) };
    let remainder = if rejected == 0 { total % 64 } else { 63 };
    let accepted = (inserted * 64 + remainder).saturating_sub(before).min(amount);
    inv.resource_remainders[i] = before + accepted - inserted * 64;
    if inserted > 0 { inv.notify_pickup(item, inserted); }
    accepted
}
