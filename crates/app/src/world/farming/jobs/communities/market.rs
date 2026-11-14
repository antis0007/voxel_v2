//! A community trade transfers the very same stock that its workers use.
use super::*;
use crate::world::{block_data::Slots, travel::trades::Trade};

pub(crate) fn market_ready(w: &World, id: u32) -> Option<bool> {
    let site = owner_site(w, id)?;
    let Some(pile) = stockpile(w, site) else { return Some(false); };
    let at = vehicle::villager_feet(w, id)
        .or_else(|| super::super::resident_state(w, id).and_then(|j| crew::view(j, id)).and_then(|p| p.pos).map(Vec3::from));
    Some(at.is_some_and(|at| at.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) <= 9.0)
        && super::super::resident(w, pile)
        && super::super::known(w, pile).is_some_and(|b| b.chest_tier().is_some()))
}

fn transferred(w: &World, site: u32, id: u32, offer: &Trade) -> Option<(IVec3, Slots)> {
    if market_ready(w, id) != Some(true) { return None; }
    let pile = stockpile(w, site)?;
    if offer.get.count == 0 || !super::super::resident(w, pile)
        || !super::super::known(w, pile).is_some_and(|b| b.chest_tier().is_some())
        || !w.block_data.can_store_chest(pile) { return None; }
    let mut slots = w.block_data.chest(pile);
    let mut needed = offer.get.count;
    for slot in &mut slots {
        if let Some(stack) = slot.as_mut().filter(|s| s.item == offer.get.item) {
            let n = needed.min(stack.count); needed -= n; stack.count -= n;
            if stack.count == 0 { *slot = None; }
        }
    }
    if needed != 0 { return None; }
    for payment in &offer.give {
        let mut remaining = payment.count;
        let limit = w.inventory.stack_limit(payment.item);
        for slot in &mut slots {
            if let Some(stack) = slot.as_mut().filter(|s| s.item == payment.item) {
                let n = remaining.min(limit.saturating_sub(stack.count));
                remaining -= n; stack.count += n;
            }
        }
        for slot in slots.iter_mut().filter(|s| s.is_none()) {
            let n = remaining.min(limit);
            if n > 0 { *slot = Some(ItemStack::new(payment.item, n)); remaining -= n; }
        }
        if remaining != 0 { return None; }
    }
    Some((pile, slots))
}

pub(crate) fn trade_stock(w: &World, id: u32, offer: &Trade) -> Option<u16> {
    let site = owner_site(w, id)?;
    let Some((pile, _)) = transferred(w, site, id, offer) else { return Some(0); };
    Some((production::count(&w.block_data.chest(pile), offer.get.item) / offer.get.count)
        .min(u16::MAX as u32) as u16)
}

/// Validate and commit the chest before the caller's synchronous player transfer.
/// No inventory changes when either side cannot complete the exchange.
pub(crate) fn exchange(w: &mut World, id: u32, offer: &Trade) -> Option<bool> {
    let site = owner_site(w, id)?;
    if !offer.affordable(&w.inventory) { return Some(false); }
    let Some((pile, stock)) = transferred(w, site, id, offer) else { return Some(false); };
    w.block_data.set_chest(pile, stock);
    Some(true)
}
