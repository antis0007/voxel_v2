//! Explicit finite salvage input and a staffed barter point for the local fixture.
use super::*;
use crate::world::{adventure::Enemy, inventory::{Item, ItemStack}};
use super::super::{crew, market::{Market, Offer}};

pub(super) const TIMBER: u32 = 8;

pub(super) fn enabled() -> bool { std::env::var("VOXEL_CITY_MARKET").is_ok_and(|s| s == "1") }

pub(super) fn install(w: &mut World, base: IVec3, buyer: IVec3) -> Result<(), String> {
    let seller=base+IVec3::new(-5,1,11);
    let source=(-9..=-6).flat_map(|x|[16,17].map(move |z|(base+IVec3::new(x,1,z)).to_array())).collect();
    let feet=base.as_vec3()+Vec3::new(-4.5,1.0,13.5);
    place(w,buyer,seller,source,feet)
}

pub(super) fn install_at(w:&mut World,buyer:IVec3,seller:IVec3,source:Vec<[i32;3]>,feet:Vec3)->Result<(),String> {
    if source.len()!=TIMBER as usize || w.get(seller)!=B::Air || source.iter().any(|p|w.get(IVec3::from(*p))!=B::Air) {
        return Err("Market service cells are obstructed".into());
    }
    place(w,buyer,seller,source,feet)
}

fn place(w:&mut World,buyer:IVec3,seller:IVec3,source:Vec<[i32;3]>,feet:Vec3)->Result<(),String> {
    let offer: Offer = ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/city_market.ron")))
        .map_err(|e| e.to_string())?;
    if offer.goods != Item::Block(B::Log) || offer.payment != Item::Resource(8) {
        return Err("Timber fixture requires the authored log and iron barter offer".into());
    }
    let mut edits = vec![(seller, B::Chest)];
    // Already fallen timber. No standing-tree felling or regrowth is implied.
    edits.extend(source.iter().map(|&p| (IVec3::from(p), B::Log)));
    w.set_many(&edits);
    let mut producer = Enemy::new(IVec3::new(3, 0, 8), 0, 40, feet, 0);
    producer.home = feet;
    w.adventure.enemies.push(producer);
    w.travel.homes.insert(3, feet);
    if !crew::supplier(w, 3, seller) { return Err("Timber supplier assignment failed".into()); }
    w.farming.jobs.mine.extend(source.iter().map(|&p| (p, B::Log as u16)));
    let mut payment = ItemStack::new(offer.payment, TIMBER);
    crate::world::block_data::store_hauled(w, buyer, &mut payment);
    if payment.count != 0 { return Err("Buyer barter storage full".into()); }
    w.farming.jobs.market = Some(Market { seller: seller.to_array(), resident: 3, offer,
        source, harvested: 0, sold: 0 });
    Ok(())
}
