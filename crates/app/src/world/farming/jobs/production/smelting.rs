//! The existing field-smelter rules, paid from a worker's actual town chest.
//! Whole batches preserve the fractional ore/fuel units at every voxel scale.
use super::*;
use crate::world::expedition;

const RECEIPTS: [&str; 4] = ["Field copper", "Field tin", "Field iron", "Field gold"];

pub(super) fn receipt(index: u8) -> &'static str { RECEIPTS[index as usize] }
pub(super) fn is_receipt(name: &str) -> bool { RECEIPTS.contains(&name) }

fn feeds(slots: &Slots, resource: usize, volume: u32) -> Vec<(Item,u32)> {
    let mut out = vec![(Item::Resource(resource as u8),64)];
    for stack in slots.iter().flatten() {
        if let Item::Block(block) = stack.item
            && expedition::ore_resource(block) == Some(resource)
            && !out.iter().any(|(item,_)| *item == stack.item) {
            out.push((stack.item,volume));
        }
    }
    out
}

pub(super) fn plan(w: &World, j: &Jobs, pile: IVec3, station: IVec3, index: u8)
    -> Option<(Vec<ItemStack>,ItemStack)>
{
    let &(ore,amount,metal) = expedition::FIELD_SMELTING.get(index as usize)?;
    let state = j.production.as_ref()?;
    let worker = j.worker?;
    if !state.workers.contains(&worker) || j.carry.is_some() || j.batch.is_some()
        || w.resident_block(station) != Some(B::Smelter)
        || !super::super::crew::station_available(j,station)
        || !super::super::resident(w,pile)
        || (station-pile).as_vec3().length() * w.voxel_scale.meters() > 1.5
        || farming::base::overlaps(w,station,station) { return None; }
    let slots = w.block_data.chest(pile);
    if count(&slots,Item::Resource(metal as u8)) >= 16 { return None; }
    let volume = expedition::harvest_units(w.voxel_scale.meters());
    let fuel = feeds(&slots,3,volume);
    for (input,units) in feeds(&slots,ore,volume) {
        for &(coal,coal_units) in &fuel {
            let batches = (1..=64).find(|b| (amount * b) % units == 0
                && (expedition::SMELT_COAL * b) % coal_units == 0)?;
            let inputs = vec![ItemStack::new(input,amount * batches / units),
                ItemStack::new(coal,expedition::SMELT_COAL * batches / coal_units)];
            if inputs.iter().all(|s|j.unreserved_stock(w,pile,s.item) >= s.count) {
                return Some((inputs,ItemStack::new(Item::Resource(metal as u8),batches)));
            }
        }
    }
    None
}
