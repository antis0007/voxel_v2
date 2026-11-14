//! Snapshot probes compare live queries and reversible real-stock transfers.
use super::*;
mod lifecycle;
mod deferred;

pub(super) fn run(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let primary=w.block_data.entries.get(&block_data::key(plan.primary)).cloned();
    let hub=w.block_data.entries.get(&block_data::key(plan.stockpile)).cloned();
    let revision=w.block_data.storage_revision;
    let result=probe(w,original,plan);
    for (at,entry) in [(plan.primary,primary),(plan.stockpile,hub)] {
        if let Some(entry)=entry {w.block_data.entries.insert(block_data::key(at),entry);}
        else {w.block_data.entries.remove(&block_data::key(at));}
    }
    w.block_data.storage_revision=revision;
    result
}

fn compare(w: &World, direct: &State, cached: &State, plan: HubPlan) -> anyhow::Result<()> {
    let project=direct.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).context("Snapshot remote project missing")?;
    let alien=Order::new(plan.primary+IVec3::X,B::Planks);
    for order in project.cells.iter().copied().chain(std::iter::once(alien)) {
        ensure!(remote(direct,order)==remote(cached,order),"Snapshot changed remote membership");
        ensure!(direct.growth_funded_order(order)==cached.growth_funded_order(order),"Snapshot changed funded membership");
        ensure!(source(w,direct,order)==source(w,cached,order),"Snapshot changed source resolution");
        for at in [plan.primary,plan.stockpile] {
            ensure!(direct.build_stock(w,at,order)==cached.build_stock(w,at,order),"Snapshot changed available build stock");
        }
    }
    for item in accounting::unique(project.cells.iter().filter_map(|o|o.material())) {
        ensure!(required(w,direct,item)==required(w,cached,item),"Snapshot changed material requirements");
        ensure!(transfer_needed(w,direct,item)==transfer_needed(w,cached,item),"Snapshot changed supply deficit");
        for at in [plan.primary,plan.stockpile] {
            ensure!(direct.reserved_at(w,at,item)==cached.reserved_at(w,at,item),"Snapshot changed reservations");
            ensure!(direct.unreserved_stock(w,at,item)==cached.unreserved_stock(w,at,item),"Snapshot changed free stock");
        }
    }
    ensure!(candidates(w,direct)==candidates(w,cached),"Snapshot changed courier candidates");
    Ok(())
}

fn probe(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let mut direct=original.clone();clear_frame(&mut direct);
    let mut cached=direct.clone();begin_frame(w,&mut cached);
    let frame=cached.supply_frame.as_ref().context("Snapshot was not prepared")?;
    let project_rows=frame.project_rows;let worker_rows=frame.worker_rows;
    let project=direct.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).context("Snapshot project missing")?;
    ensure!(project_rows==project.cells.len() && worker_rows==crew::loads(&direct).count(),
        "Snapshot repeated project or worker scans");
    compare(w,&direct,&cached,plan)?;
    let order=project.cells.iter().copied().find(|o|o.material().is_some_and(|item|
        transfer_needed(w,&direct,item)>0 && production::count(&w.block_data.chest(plan.primary),item)>0))
        .context("Snapshot stock probe needs one outstanding funded unit")?;
    let item=order.material().context("Snapshot order is not material")?;
    let initial_hub=cached.build_stock(w,plan.stockpile,order);
    let initial_need=transfer_needed(w,&cached,item);
    ensure!(block_data::take_hauled(w,plan.primary,item,1)==1,"Snapshot source debit failed");
    compare(w,&direct,&cached,plan)?;
    let mut unit=ItemStack::new(item,1);
    block_data::store_hauled(w,plan.stockpile,&mut unit);
    ensure!(unit.count==0,"Snapshot stock probe hub has no room");
    ensure!(cached.build_stock(w,plan.stockpile,order)==initial_hub+1
        && transfer_needed(w,&cached,item)==initial_need-1,"Snapshot cached stale chest stock after deposit");
    compare(w,&direct,&cached,plan)?;
    ensure!(block_data::take_hauled(w,plan.stockpile,item,1)==1,"Snapshot hub debit failed");
    ensure!(cached.build_stock(w,plan.stockpile,order)==initial_hub
        && transfer_needed(w,&cached,item)==initial_need,"Snapshot cached stale chest stock after debit");
    compare(w,&direct,&cached,plan)?;
    let mut returned=ItemStack::new(item,1);block_data::store_hauled(w,plan.primary,&mut returned);
    ensure!(returned.count==0,"Snapshot probe could not return borrowed material");
    let saved=ron::to_string(&cached)?;
    ensure!(!saved.contains("supply_frame:") && !saved.contains("hub_access:"),"Worker snapshot leaked into save");
    let restored:State=ron::from_str(&saved)?;
    ensure!(restored.supply_frame.is_none() && restored.hub_access.is_none(),"Reload retained a worker snapshot");
    clear_frame(&mut cached);
    ensure!(cached.supply_frame.is_none() && cached.hub_access.is_none(),"Worker snapshot did not clear");
    let lifecycle=lifecycle::run(w,original,plan)?;
    let deferred=deferred::run(w,original,plan)?;
    Ok(serde_json::json!({"exact_queries_match":true,"fresh_stock_after_debit":true,
        "fresh_stock_after_deposit":true,"project_rows_scanned":project_rows,"worker_rows_scanned":worker_rows,
        "save_omits_snapshot":true,"cleared":true,"actual_borrowed_units":1,
        "two_worker_lifecycle":lifecycle,"deferred_work":deferred}))
}
