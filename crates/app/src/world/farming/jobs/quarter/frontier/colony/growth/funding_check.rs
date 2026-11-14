//! Native debit checks restore the actual chest after each temporary probe.
use super::*;
use anyhow::{Context, ensure};
use crate::world::{block_data, inventory::{ItemStack, all_recipes}};

pub(crate) fn check(w: &mut World) -> anyhow::Result<()> {
    let mut j = w.farming.jobs.clone();
    ensure!(j.growth_active(), "Funding check requires an active funded project");
    let pile = IVec3::from(j.stockpile.context("Missing stockpile")?);
    let original = w.block_data.chest(pile);
    let result = probe(w,&mut j,pile);
    w.block_data.set_chest(pile,original);
    result
}

fn probe(w: &mut World, j: &mut jobs::State, pile: IVec3) -> anyhow::Result<()> {
    let plank = Item::Block(B::Planks);
    let order = j.build.iter().copied().find(|o|j.growth_funded_order(*o)
        && o.material() == Some(plank) && o.ready(w)).context("No funded ground order")?;
    let alien = (-4..=4).flat_map(|x|(-4..=4).map(move|z|pile+IVec3::new(x,0,z)))
        .map(|at|jobs::construction::Order::new(at,B::Planks))
        .find(|o|o.ready(w) && !j.build.iter().any(|old|old.at == o.at) && !j.growth_funded_order(*o))
        .context("No unrelated build probe cell")?;
    j.release_claims();j.carry=None;j.building_material=false;j.shopping=None;
    j.claims.retain(|c|c.order != order);
    j.build.push(alien);
    let reserved = j.growth_reserved(w,plank);
    ensure!(reserved > 0,"No protected planks");
    let free = j.unreserved_stock(w,pile,plank);
    ensure!(block_data::take_hauled(w,pile,plank,free) == free,"Surplus removal failed");
    let protected_stock = production::count(&w.block_data.chest(pile),plank);
    jobs::construction::fetch(w,j,pile,alien);
    ensure!(j.carry.is_none() && production::count(&w.block_data.chest(pile),plank) == protected_stock,
        "Unfunded fetch spent reserved stock");
    j.market=Some(jobs::market::Market{seller:pile.to_array(),resident:j.worker.context("Missing worker")?,
        offer:jobs::market::Offer{goods:Item::Block(B::Stone),payment:plank},source:Vec::new(),harvested:0,sold:0});
    let market_order = jobs::construction::Order::new(alien.cell(),B::Stone);
    j.build.push(market_order);
    ensure!(!jobs::market::candidate(w,j,pile,market_order,&w.block_data.chest(pile)),"Reserved stock offered as market payment");
    jobs::market::fund(w,j,pile,market_order);
    ensure!(j.carry.is_none() && production::count(&w.block_data.chest(pile),plank) == protected_stock,
        "Pending market task spent reserved stock");
    let chest = all_recipes().iter().find(|r|r.name == "Chest").context("Missing chest recipe")?;
    let oak = all_recipes().iter().find(|r|r.name == "Oak planks").context("Missing plank recipe")?;
    let empty = Vec::new();
    ensure!(!j.growth_ingredients_available(w,&empty,chest),"Competing chest craft ignored reserved planks");
    ensure!(j.growth_ingredients_available(w,&empty,oak),"Plank shortage blocked its own replenishment recipe");
    let before = production::count(&w.block_data.chest(pile),plank);
    jobs::construction::fetch(w,j,pile,order);
    let cargo = j.carry.context("Funded fetch could not use reserved stock")?;
    ensure!(cargo.item == plank && cargo.count > 0 && production::count(&w.block_data.chest(pile),plank)+cargo.count == before,
        "Funded fetch did not conserve stock");
    ensure!(j.claims.iter().filter(|c|Some(c.worker)==j.worker).all(|c|j.growth_funded_order(c.order)),
        "Funded batch mixed unrelated orders");
    let carried_reserve = j.growth_reserved(w,plank);
    j.release_claims();
    ensure!(j.growth_reserved(w,plank) == carried_reserve+cargo.count,"Released claims minted a refund");
    j.reconcile_claims();
    ensure!(j.claims.iter().filter(|c|Some(c.worker)==j.worker).all(|c|j.growth_funded_order(c.order)),
        "Orphan cargo diverted to unrelated work");
    let funded:Vec<_>=j.build.iter().copied().filter(|o|j.growth_funded_order(*o)).collect();
    j.build.retain(|o|!funded.contains(o));
    j.release_claims();j.reconcile_claims();
    ensure!(!j.claims.iter().any(|c|Some(c.worker)==j.worker) && j.carry == Some(cargo),
        "Canceled funded cargo silently reassigned");
    let mut returned = cargo;
    block_data::store_hauled(w,pile,&mut returned);
    ensure!(returned.count == 0 && production::count(&w.block_data.chest(pile),plank) == before,
        "Released physical cargo did not return to stock");
    println!("[city-growth] funding-check passed reserved={reserved} surplus={free} carried={}",cargo.count);
    Ok(())
}
