//! Local barter: carried payment becomes goods only at a staffed physical chest.
use super::*;
use crate::world::inventory::Item;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Offer { pub goods: Item, pub payment: Item }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(super) struct Trip { pub seller: [i32; 3], pub offer: Offer }

impl Trip {
    pub(super) fn valid(self, cargo: Option<ItemStack>) -> bool {
        self.seller.iter().all(|n| n.unsigned_abs() < 1 << 28)
            && self.offer.goods != self.offer.payment
            && cargo.is_some_and(|s| s.item == self.offer.payment && s.count == 1)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Market {
    pub seller: [i32; 3],
    pub resident: u32,
    pub offer: Offer,
    pub source: Vec<[i32; 3]>,
    pub harvested: u16,
    pub sold: u32,
}

impl Market {
    pub(super) fn valid(&self) -> bool {
        self.source.len() <= 16 && self.offer.goods != self.offer.payment
            && self.seller.iter().all(|n| n.unsigned_abs() < 1 << 28)
            && self.source.iter().enumerate().all(|(i, p)| p.iter().all(|n| n.unsigned_abs() < 1 << 28)
                && !self.source[..i].contains(p))
            && u32::from(self.harvested) & !((1u32 << self.source.len()) - 1) == 0
    }
}

pub(super) fn candidate(w: &World, j: &State, pile: IVec3, order: Order, slots: &[Option<ItemStack>]) -> bool {
    j.market.as_ref().is_some_and(|m| order.material() == Some(m.offer.goods)
        && j.unreserved_from(w,pile,m.offer.payment,production::count(slots,m.offer.payment)) > 0)
}

pub(super) fn fund(w: &mut World, j: &mut State, pile: IVec3, order: Order) {
    let Some(m) = j.market.as_ref() else { return };
    if j.carry.is_some() || !j.build.contains(&order) || !j.available(order)
        || order.material() != Some(m.offer.goods)
        || !known(w, m.seller.into()).is_some_and(|b| b.chest_tier().is_some()) { return; }
    let trip = Trip { seller: m.seller, offer: m.offer };
    if j.unreserved_stock(w,pile,trip.offer.payment) == 0 { j.status = "Materials Reserved"; return; }
    if block_data::take_hauled(w, pile, trip.offer.payment, 1) == 1 {
        j.carry = Some(ItemStack::new(trip.offer.payment, 1));
        j.shopping = Some(trip);
        j.building_material = false;
        j.status = "Carrying market payment";
    }
}

pub(super) fn wanted(j: &State, trip: Trip) -> bool {
    j.market.as_ref().is_some_and(|m| m.seller == trip.seller && m.offer == trip.offer)
        && j.build.iter().any(|o| o.material() == Some(trip.offer.goods) && j.available(*o))
}

pub(super) fn buy(w: &mut World, j: &mut State, seller: IVec3) {
    let Some(trip) = j.shopping.filter(|t| t.seller == seller.to_array() && t.valid(j.carry)) else { return };
    if !wanted(j, trip) { j.shopping = None; return; }
    let Some(m) = j.market.as_ref() else { return };
    let point = (seller.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters();
    if !vehicle::villager_feet(w, m.resident).is_some_and(|p| p.distance(point) < 2.5) {
        j.status = "Merchant away";
        return;
    }
    if !known(w, seller).is_some_and(|b| b.chest_tier().is_some()) || !w.block_data.can_store_chest(seller) {
        j.status = "Market chest unavailable";
        return;
    }
    let mut slots = w.block_data.chest(seller);
    let Some(index) = slots.iter().position(|s| s.is_some_and(|s| s.item == trip.offer.goods && s.count > 0)) else {
        j.status = "Market awaiting timber";
        return;
    };
    let Some(mut goods) = slots[index] else { return };
    goods.count -= 1;
    slots[index] = (goods.count > 0).then_some(goods);
    let limit = w.inventory.stack_limit(trip.offer.payment);
    let payment_index = slots.iter().position(|s| s.is_some_and(|s| s.item == trip.offer.payment && s.count < limit))
        .or_else(|| slots.iter().position(|s| s.is_none()));
    let Some(payment_index) = payment_index else { j.status = "Market payment storage full"; return };
    if let Some(stack) = slots[payment_index].as_mut() { stack.count += 1; }
    else { slots[payment_index] = Some(ItemStack::new(trip.offer.payment, 1)); }
    commit(w, j, seller, slots, trip);
}

fn commit(w: &mut World, j: &mut State, seller: IVec3, slots: Vec<Option<ItemStack>>, trip: Trip) {
    // Exclusive World access and a precomputed chest image admit the exchange
    // without partial deposits. This is not a cross-file crash journal.
    if j.market.as_ref().is_none_or(|m| m.sold == u32::MAX) { return; }
    w.block_data.set_chest(seller, slots);
    j.carry = Some(ItemStack::new(trip.offer.goods, 1));
    j.shopping = None;
    j.building_material = true;
    if let Some(m) = j.market.as_mut() { m.sold += 1; }
    j.status = "Purchased construction timber";
}

pub(super) fn mined(j: &mut State, p: IVec3) {
    if let Some(m) = j.market.as_mut() && j.worker == Some(m.resident)
        && let Some(index) = m.source.iter().position(|at| *at == p.to_array()) {
        m.harvested |= 1u16 << index;
    }
}

pub(super) fn summary(w: &World) -> Option<String> {
    let m = w.farming.jobs.market.as_ref()?;
    let stock = w.block_data.chest(m.seller.into());
    let count = |item| stock.iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum::<u32>();
    Some(format!("Timber: {} · Sales: {} · Iron: {}", count(m.offer.goods), m.sold, count(m.offer.payment)))
}
