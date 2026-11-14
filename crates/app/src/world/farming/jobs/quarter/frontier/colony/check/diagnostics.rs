//! Record physical supply and task state when construction cannot advance.
use super::*;

pub(super) fn write(root:&Path,w:&World)->Result<()> {
    let jobs=&w.farming.jobs;
    let pile=IVec3::from(jobs.stockpile.context("Missing diagnostic stockpile")?);
    let depot=jobs::Task::Store(pile).point(w.voxel_scale.meters());
    let drops:Vec<_>=w.inventory.dropped.iter().enumerate().filter(|(_,d)|d.stack.item==Item::Block(B::Log))
        .map(|(index,d)|serde_json::json!({"index":index,"position":d.pos.to_array(),"count":d.stack.count,
            "depot_distance_m":d.pos.distance(depot)})).collect();
    let bills:Vec<_>=jobs.production.as_ref().into_iter().flat_map(|p|&p.bills)
        .map(|b|serde_json::json!({"recipe":b.recipe,"worker":b.worker,
            "ready":production::craft_ready(w,jobs,pile,b).is_some(),
            "worker_status":jobs::crew::view(jobs,b.worker).map(|v|v.status)})).collect();
    let mut prefix=BTreeMap::<String,u32>::new();
    for d in w.inventory.dropped.iter().take(256) {*prefix.entry(format!("{:?}",d.stack.item)).or_default()+=d.stack.count;}
    let report=serde_json::json!({"tick":w.farming.clock,"stockpile":w.block_data.chest(pile),
        "total_drops":w.inventory.dropped.len(),"first_256_items":prefix,"dropped_logs":drops,"crafting_bills":bills,"claims":jobs.claims.len(),
        "remaining_orders":jobs.build,"felled_bodies":w.sandbox.bodies.iter().filter(|b|b.felled_tree).count(),
        "pending_body_logs":w.sandbox.bodies.iter().filter(|b|b.felled_tree)
            .map(|b|b.voxels.iter().filter(|(_,block)|*block==B::Log).count()).sum::<usize>()});
    std::fs::write(root.join("supply-diagnostics.json"),serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
