//! Check-only fixtures compare eager indexing with deferred worker pulses.
use super::*;

fn rows(j: &State) -> (usize,usize) {
    j.supply_frame.as_ref().map_or((0,0),|f|(f.project_rows,f.worker_rows))
}

fn pulse(w: &mut World, fixture: &State, feet: Vec3) -> anyhow::Result<serde_json::Value> {
    let plan=frontier::hub_plan(fixture).context("Deferred probe needs a service hub")?;
    let stock=[w.block_data.chest(plan.primary),w.block_data.chest(plan.stockpile)];
    let drops=w.inventory.dropped.clone();let overflow=w.inventory.overflow.clone();
    let revision=w.block_data.storage_revision;
    let mut eager=fixture.clone();begin_frame(w,&mut eager);
    let eager_rows=rows(&eager);
    let mut deferred=fixture.clone();clear_frame(&mut deferred);
    super::super::super::super::work(w,feet,feet,&mut eager);
    ensure!(stock==[w.block_data.chest(plan.primary),w.block_data.chest(plan.stockpile)]
        && drops==w.inventory.dropped && overflow==w.inventory.overflow
        && revision==w.block_data.storage_revision,"Eager noncommitting pulse changed inventory");
    super::super::super::super::work(w,feet,feet,&mut deferred);
    let deferred_rows=rows(&deferred);
    ensure!(deferred_rows==(0,0),"Noncommitting pulse prepared unused service rows");
    ensure!(stock==[w.block_data.chest(plan.primary),w.block_data.chest(plan.stockpile)]
        && drops==w.inventory.dropped && overflow==w.inventory.overflow
        && revision==w.block_data.storage_revision,"Deferred noncommitting pulse changed inventory");
    clear_frame(&mut eager);clear_frame(&mut deferred);
    ensure!(ron::to_string(&eager)?==ron::to_string(&deferred)?,"Deferred pulse changed saved worker state");
    ensure!(format!("{eager:?}")==format!("{deferred:?}"),"Deferred pulse changed transient worker state");
    Ok(serde_json::json!({"eager_project_rows":eager_rows.0,"eager_worker_rows":eager_rows.1,
        "deferred_project_rows":deferred_rows.0,"deferred_worker_rows":deferred_rows.1,
        "saved_and_transient_state_equal":true,"inventory_unchanged":true}))
}

pub(super) fn run(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let mut active=original.clone();clear_frame(&mut active);
    ensure!(prepare_frame(w,&mut active),"Active construction did not prepare its snapshot");
    ensure!(!prepare_frame(w,&mut active),"Active construction repeated its snapshot");
    compare(w,original,&active,plan)?;
    let active_rows=rows(&active);
    let task=Task::Store(plan.primary);
    let v=w.voxel_scale.meters();
    let (index,id,feet,route)=crew::ids(original).enumerate().filter_map(|(index,id)| {
        let feet=vehicle::villager_feet(w,id)?;
        if crew::view(original,id)?.carry.is_some() || reachable(w,feet,task,v) { return None; }
        let route=path_in_range(w,feet,task,4096,RANGE*2.0+4.0);
        (!route.is_empty()).then_some((index,id,feet,route))
    }).next().context("Deferred pulse needs an empty physical worker walking to the primary chest")?;
    ensure!(original.worker.is_some(),"Deferred pulse needs the lead worker");
    let mut walking=original.clone();clear_frame(&mut walking);
    if index>0 {
        let mut helper=walking.helpers.remove(index-1);helper.swap(&mut walking);walking.helpers.push(helper);
    }
    ensure!(walking.worker==Some(id),"Deferred pulse selected the wrong physical worker");
    walking.reset_task();walking.build.clear();walking.claims.clear();walking.building_material=false;
    walking.shopping=None;walking.sowing=None;
    walking.routine=routine::Resident::default();walking.routine.due=w.farming.clock.saturating_add(PULSE);
    let item=original.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).context("Deferred project missing")?.cells.iter()
        .filter_map(|o|o.material()).find(|&item|production::count(&w.block_data.chest(plan.primary),item)>0)
        .context("Deferred pulse needs one real stored unit")?;
    ensure!(block_data::take_hauled(w,plan.primary,item,1)==1,"Deferred pulse could not borrow cargo");
    walking.carry=Some(ItemStack::new(item,1));walking.cargo=Some(Cargo::Build {source:plan.primary.to_array()});
    walking.task=Some(task);walking.path=route;
    ensure!(super::super::super::super::task_still_valid(w,feet,feet,&walking,plan.primary,task),
        "Deferred walking fixture is not a valid delivery");
    let walking_result=pulse(w,&walking,feet)?;
    let mut returned=ItemStack::new(item,1);block_data::store_hauled(w,plan.primary,&mut returned);
    ensure!(returned.count==0,"Deferred pulse could not return borrowed cargo");
    walking.carry=None;walking.cargo=None;walking.reset_task();
    walking.retry_at=w.farming.clock.saturating_add(PULSE);
    let retry_result=pulse(w,&walking,feet)?;
    let mut completed=original.clone();clear_frame(&mut completed);
    completed.build.clear();completed.claims.clear();
    completed.quarter.as_mut().and_then(|q|q.frontier.as_mut()).and_then(|s|s.growth.as_mut())
        .context("Deferred completion metadata missing")?.complete=true;
    let mut completed_eager=completed.clone();begin_frame(w,&mut completed_eager);
    ensure!(!prepare_frame(w,&mut completed) && rows(&completed)==(0,0),"Completed idle stage prepared a snapshot");
    compare(w,&completed,&completed_eager,plan)?;
    let completed_rows=rows(&completed_eager);
    let order=completed.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).and_then(|p|p.cells.first()).copied()
        .context("Deferred repair order missing")?;
    completed.build.push(order);
    let repair_direct=completed.clone();
    ensure!(prepare_frame(w,&mut completed),"Completed queued repair lost its membership index");
    compare(w,&repair_direct,&completed,plan)?;
    Ok(serde_json::json!({"synthetic_work_state_with_real_actor_and_cargo":true,"borrowed_units":1,
        "active_prepares_once":true,"active_project_rows":active_rows.0,"active_worker_rows":active_rows.1,
        "walking_primary_return":walking_result,"retry_wait":retry_result,
        "completion_metadata_fixture":{"eager_project_rows":completed_rows.0,"eager_worker_rows":completed_rows.1,
            "deferred_project_rows":0,"deferred_worker_rows":0,"exact_queries_match":true},
        "completed_queued_repair_prepares":true}))
}
