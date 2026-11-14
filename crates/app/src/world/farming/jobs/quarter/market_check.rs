//! Production, barter and finite-stock checks for the opt-in market fixture.
use super::*;
use anyhow::{Result, ensure};
use crate::world::{adventure::Enemy, inventory::{Item, ItemStack}, block_data};
use super::super::{crew, market, State as Jobs, construction::Order};

pub(super) fn remaining(w: &World) -> u32 {
    let j = &w.farming.jobs;
    let payment = j.market.as_ref().map(|m| m.offer.payment);
    let count = |slots: Vec<Option<ItemStack>>| slots.iter().flatten()
        .filter(|s| Some(s.item) != payment).map(|s| s.count).sum::<u32>();
    let mut total = j.stockpile.map_or(0, |p| count(w.block_data.chest(p.into())));
    for (id, _) in crew::saved_workers(w) {
        total += crew::view(j, id).and_then(|a| a.carry).filter(|s| Some(s.item) != payment).map_or(0, |s| s.count);
    }
    if let Some(m) = &j.market {
        total += count(w.block_data.chest(m.seller.into()));
        total += m.source.iter().filter(|p| Some(Item::Block(w.get((**p).into()))) == Some(m.offer.goods)).count() as u32;
    }
    total
}

pub(super) fn money(w: &World) -> u32 {
    let j = &w.farming.jobs;
    let Some(m) = &j.market else { return 0 };
    [j.stockpile, Some(m.seller)].into_iter().flatten()
        .flat_map(|p| w.block_data.chest(p.into())).flatten()
        .filter(|s| s.item == m.offer.payment).map(|s| s.count).sum::<u32>()
        + crew::cargo_item(j, m.offer.payment)
}

pub(super) fn ledger(w: &World) -> Option<serde_json::Value> {
    let j = &w.farming.jobs;
    let m = j.market.as_ref()?;
    let count = |p, item| w.block_data.chest(p).iter().flatten()
        .filter(|s| s.item == item).map(|s| s.count).sum::<u32>();
    let lot = &j.quarter.as_ref()?.lots[0];
    let cover = if lot.market_cover { geometry::market_cover().iter()
        .filter(|(p,b)| w.get(geometry::at(lot,*p)) == *b).count() } else {0};
    Some(serde_json::json!({"seller_resident":m.resident,"sales":m.sold,
        "raw_timber_total":m.source.len(),"timber_produced":m.harvested.count_ones(),
        "merchant_timber":count(m.seller.into(),m.offer.goods),
        "buyer_iron":j.stockpile.map_or(0,|p|count(p.into(),m.offer.payment)),
        "merchant_iron":count(m.seller.into(),m.offer.payment),
        "carried_iron":crew::cargo_item(j,m.offer.payment),"paid_cover_blocks_present":cover}))
}

pub(super) fn complete(w: &World, checkpointed: bool) -> Result<()> {
    if let Some(m) = &w.farming.jobs.market {
        ensure!(m.harvested.count_ones() == super::market_fixture::TIMBER, "Timber was not produced");
        ensure!(m.sold == super::market_fixture::TIMBER, "Timber bypassed the market");
        ensure!(checkpointed, "No carried-payment checkpoint");
        ensure!(money(w) == super::market_fixture::TIMBER, "Barter payment was lost or created");
        let received = w.block_data.chest(m.seller.into()).iter().flatten()
            .filter(|s| s.item == m.offer.payment).map(|s| s.count).sum::<u32>();
        ensure!(received == super::market_fixture::TIMBER, "Merchant did not receive payment");
        println!("[city-check] PASS eight salvaged logs sold for eight physical iron inventory units");
    }
    Ok(())
}

pub(super) fn direct(root: &std::path::Path) -> Result<()> {
    let mut w = World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter, 9, &root.join("market-transactions"));
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 3;
    let buyer = IVec3::new(1,y,1);
    let seller = IVec3::new(4,y,1);
    for p in [buyer, seller] { w.set(p, B::Chest); }
    let offer = market::Offer { goods: Item::Block(B::Log), payment: Item::Resource(8) };
    let mut price = ItemStack::new(offer.payment, 2);
    block_data::store_hauled(&mut w, buyer, &mut price);
    let mut stock = ItemStack::new(offer.goods, 2);
    block_data::store_hauled(&mut w, seller, &mut stock);
    let mut actor = Enemy::new(IVec3::new(3,0,8), 0, 40, seller.as_vec3() + Vec3::Z, 0);
    actor.home = actor.pos;
    w.adventure.enemies.push(actor);
    let order = Order::new(IVec3::new(7,y,7), B::Log);
    let mut j = Jobs { worker: Some(0), stockpile: Some(buyer.to_array()), build: vec![order],
        market: Some(market::Market { seller: seller.to_array(), resident: 3, offer, source: vec![], harvested: 0, sold: 0 }),
        ..Jobs::default() };
    market::fund(&mut w, &mut j, buyer, order);
    let held = j.carry;
    market::fund(&mut w, &mut j, buyer, order);
    ensure!(j.carry == held && j.shopping.is_some() && j.valid(), "Repeated funding changed held payment");
    w.farming.jobs = j;
    let bytes = crate::world::farming::sidecar::encode(&w);
    let (loaded, _) = crate::world::farming::sidecar::decode(&bytes)?;
    w.farming = loaded;
    let mut j = std::mem::take(&mut w.farming.jobs);
    ensure!(j.shopping.is_some() && j.carry == held, "Payment obligation lost on reload");
    let full = vec![Some(ItemStack::new(Item::Block(B::Stone), w.inventory.stack_limit(Item::Block(B::Stone)))); 27];
    let mut full = full;
    full[0] = Some(ItemStack::new(offer.goods, 2));
    w.block_data.set_chest(seller, full.clone());
    market::buy(&mut w, &mut j, seller);
    ensure!(j.carry == held && w.block_data.chest(seller) == full && j.market.as_ref().is_some_and(|m| m.sold == 0),
        "Full merchant accepted a partial exchange");
    w.block_data.set_chest(seller, vec![None; 27]);
    market::buy(&mut w, &mut j, seller);
    ensure!(j.carry == held && j.shopping.is_some(), "Empty merchant lost held payment");
    let mut restocked = ItemStack::new(offer.goods, 1);
    block_data::store_hauled(&mut w, seller, &mut restocked);
    market::buy(&mut w, &mut j, seller);
    ensure!(j.carry == Some(ItemStack::new(offer.goods, 1)) && j.shopping.is_none()
        && j.market.as_ref().is_some_and(|m| m.sold == 1), "Barter did not transfer both goods");
    let delivered = j.carry;
    market::buy(&mut w, &mut j, seller);
    ensure!(j.carry == delivered && j.market.as_ref().is_some_and(|m| m.sold == 1), "Repeated exchange duplicated goods");
    // A second trip canceled before exchange keeps its physical payment refundable.
    j.carry = None; j.building_material = false;
    market::fund(&mut w, &mut j, buyer, order);
    j.build.clear();
    ensure!(j.shopping.is_some_and(|t| !market::wanted(&j, t)), "Canceled demand remained purchasable");
    j.shopping = None;
    let mut refund = j.carry.take().ok_or_else(|| anyhow::anyhow!("Missing refund"))?;
    block_data::store_hauled(&mut w, buyer, &mut refund);
    ensure!(refund.count == 0, "Refund did not return to buyer");
    println!("[city-check] PASS barter funding/reload, full and empty merchant, exact transfer, replay and cancellation refund");
    Ok(())
}
