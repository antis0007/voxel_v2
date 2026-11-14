//! Reversible probes borrow real stock without moving actors or placing blocks.
use super::*;
use anyhow::{Context, ensure};
mod storage;
mod snapshot;

pub(crate) fn run(w: &mut World) -> anyhow::Result<serde_json::Value> {
    let mut original = w.farming.jobs.clone();
    clear_frame(&mut original);
    let plan = active(&original).context("Supply check needs an active remote project")?;
    ensure!(frontier::hub_available(w,&original),"Supply check needs the paid accessible hub");
    let source = w.block_data.entries.get(&block_data::key(plan.primary)).cloned();
    let hub = w.block_data.entries.get(&block_data::key(plan.stockpile)).cloned();
    let revision = w.block_data.storage_revision;
    let drops = w.inventory.dropped.clone();
    let overflow = w.inventory.overflow.clone();
    let follow = w.travel.following;
    let guards = w.travel.guard_posts.clone();
    let result = probe(w,&original,plan);
    w.farming.jobs = original;
    for (at,entry) in [(plan.primary,source),(plan.stockpile,hub)] {
        if let Some(entry)=entry { w.block_data.entries.insert(block_data::key(at),entry); }
        else { w.block_data.entries.remove(&block_data::key(at)); }
    }
    w.block_data.storage_revision = revision;
    w.inventory.dropped = drops;
    w.inventory.overflow = overflow;
    w.travel.following = follow;
    w.travel.guard_posts = guards;
    result
}

fn isolated(original: &State, id: u32, feet: Vec3) -> State {
    let mut j=original.clone();
    j.worker=Some(id);j.worker_pos=Some(feet.to_array());j.helpers.clear();j.claims.clear();
    j.carry=None;j.cargo=None;j.building_material=false;j.shopping=None;j.sowing=None;j.reset_task();
    clear_frame(&mut j);
    j
}

fn probe(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let snapshot=snapshot::run(w,original,plan)?;
    let order=original.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).context("Remote project missing")?.cells.iter().copied()
        .find(|o|o.material().is_some_and(|item|required(w,original,item)>0
            && production::count(&w.block_data.chest(plan.primary),item)>0)).context("No funded supply probe material")?;
    let item=order.material().context("Nonmaterial order")?;
    let mut detached=original.clone();
    clear_frame(&mut detached);
    let attached=std::mem::take(&mut w.farming.jobs);
    let detached_available=frontier::hub_available(w,&detached);
    let detached_source=source(w,&detached,order);
    w.farming.jobs=attached;
    ensure!(detached_available,"Paid hub depends on the world-owned jobs slot");
    ensure!(detached_source==Some(plan.stockpile),"Remote source failed with detached worker authority");
    let task=Task::Supply(plan.primary,item);
    let (id,feet)=crew::ids(original).filter_map(|id|vehicle::villager_feet(w,id).map(|feet|(id,feet)))
        .find(|(_,feet)|feet.distance_squared(Task::Store(plan.stockpile).point(w.voxel_scale.meters()))<RANGE*RANGE
            && !reachable(w,*feet,task,w.voxel_scale.meters())).context("No physical recovery worker outside source reach")?;
    let mut j=isolated(original,id,feet);
    ensure!(source(w,&j,order)==Some(plan.stockpile),"Remote source is not the paid hub");
    ensure!(j.build_stock(w,plan.primary,order)==0,"Remote order can withdraw from primary");
    for at in [plan.primary,plan.stockpile] {
        let stock=production::count(&w.block_data.chest(at),item);
        let held=j.reserved_at(w,at,item);
        ensure!(j.unreserved_stock(w,at,item)==stock.saturating_sub(held),"Source reservation mismatch");
    }
    let before=production::count(&w.block_data.chest(plan.primary),item);
    execute(w,&mut j,task);
    ensure!(j.carry.is_none() && production::count(&w.block_data.chest(plan.primary),item)==before,
        "Supply pickup debited before physical arrival");
    ensure!(block_data::take_hauled(w,plan.primary,item,1)==1,"Cannot borrow probe material");
    let borrowed=ItemStack::new(item,1);
    let purpose=Cargo::Supply {source:plan.primary.to_array(),target:plan.stockpile.to_array(),project:plan.project};
    j.carry=Some(borrowed);j.cargo=Some(purpose);j.building_material=true;
    if !j.build.contains(&order) {j.build.push(order);}
    j.reconcile_claims();ensure!(j.claims.is_empty(),"Transfer cargo acquired a construction claim");
    j.cargo=Some(Cargo::Build {source:plan.primary.to_array()});
    j.reconcile_claims();ensure!(j.claims.is_empty(),"Primary cargo acquired a remote claim");
    j.cargo=None;j.reconcile_claims();ensure!(j.claims.is_empty(),"Legacy cargo acquired a remote claim");
    j.cargo=Some(purpose);j.building_material=false;
    ensure!(destination(&j,plan.primary)==plan.stockpile,"Transfer return target changed");
    w.farming.jobs=j;w.travel.following=Some(id);
    super::super::service_worker(w,feet);
    ensure!(w.farming.jobs.carry==Some(borrowed) && w.farming.jobs.cargo==Some(purpose),"Follow lost supply cargo");
    w.travel.following=None;w.travel.guard_posts.insert(id,feet);
    super::super::service_worker(w,feet);
    ensure!(w.farming.jobs.carry==Some(borrowed) && w.farming.jobs.cargo==Some(purpose),"Guard lost supply cargo");
    w.travel.guard_posts.remove(&id);
    let encoded=ron::to_string(&w.farming.jobs)?;
    let restored:State=ron::from_str(&encoded)?;
    ensure!(restored.carry==Some(borrowed) && restored.cargo==Some(purpose) && restored.claims.is_empty(),
        "Reload changed supply ownership");
    w.farming.jobs=restored;
    let drop_count=w.inventory.dropped.len();
    ensure!(drop_count<1024,"Recovery probe needs one drop slot");
    super::super::worker_fell(w,id,feet);
    ensure!(w.inventory.dropped.len()==drop_count+1 && w.inventory.dropped[drop_count].stack==borrowed,
        "Death did not preserve borrowed cargo");
    let mut recovered=isolated(original,id,feet);
    let recover=Task::Recover(feet,item);
    ensure!(task_valid(w,&recovered,recover),"Hub-zone recovery was not eligible");
    let before_recovery=w.inventory.dropped.clone();
    let mut cached_recovered=recovered.clone();
    begin_frame(w,&mut cached_recovered);
    execute(w,&mut cached_recovered,recover);
    clear_frame(&mut cached_recovered);
    let cached_drops=w.inventory.dropped.clone();
    w.inventory.dropped=before_recovery;
    execute(w,&mut recovered,recover);
    ensure!(recovered.carry==cached_recovered.carry && recovered.cargo==cached_recovered.cargo
        && recovered.claims.is_empty() && cached_recovered.claims.is_empty() && w.inventory.dropped==cached_drops,
        "Snapshot changed physical recovery effects");
    ensure!(recovered.carry==Some(borrowed) && recovered.cargo==Some(purpose)
        && w.inventory.dropped.len()==drop_count,"Recovery lost or duplicated goods");
    ensure!(production::count(&w.block_data.chest(plan.primary),item)+borrowed.count==before,
        "Source and recovered cargo do not conserve stock");
    let synthetic_storage=storage::run(w,plan.stockpile)?;
    Ok(serde_json::json!({"source_binding":true,"detached_authority":true,"two_source_reservations":true,"arrival_only_debit":true,
        "claim_isolation":true,"follow_guard_reload":true,"death_recovery":true,"borrowed_units":1,
        "worker_snapshot":snapshot,"snapshot_recovery_effects_match":true,
        "synthetic_engine_storage_fixture":synthetic_storage}))
}
