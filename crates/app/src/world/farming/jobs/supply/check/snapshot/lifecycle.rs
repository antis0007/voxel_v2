//! Worker switches rebuild cargo accounting after real storage and loss.
use super::*;

pub(super) fn run(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let jobs=w.farming.jobs.clone();
    let primary=w.block_data.entries.get(&block_data::key(plan.primary)).cloned();
    let hub=w.block_data.entries.get(&block_data::key(plan.stockpile)).cloned();
    let revision=w.block_data.storage_revision;
    let drops=w.inventory.dropped.clone();let overflow=w.inventory.overflow.clone();
    let guards=w.travel.guard_posts.clone();
    let result=probe(w,original,plan);
    w.farming.jobs=jobs;w.inventory.dropped=drops;w.inventory.overflow=overflow;
    w.travel.guard_posts=guards;
    for (at,entry) in [(plan.primary,primary),(plan.stockpile,hub)] {
        if let Some(entry)=entry {w.block_data.entries.insert(block_data::key(at),entry);}
        else {w.block_data.entries.remove(&block_data::key(at));}
    }
    w.block_data.storage_revision=revision;
    result
}

fn switch(j: &mut State) -> anyhow::Result<()> {
    ensure!(j.helpers.len()==1,"Lifecycle needs exactly two workers");
    let mut helper=j.helpers.remove(0);helper.swap(j);j.helpers.push(helper);
    Ok(())
}

fn refresh(w: &World, j: &mut State, plan: HubPlan, item: Item,
    needed: u32, transit: u32, other: bool) -> anyhow::Result<()> {
    let mut direct=j.clone();clear_frame(&mut direct);
    begin_frame(w,j);
    let frame=j.supply_frame.as_ref().context("Lifecycle frame missing")?;
    ensure!(frame.required(item)==needed && frame.transit(item)==transit
        && frame.other_courier==other,"Worker switch retained old cargo accounting");
    ensure!(frame.worker_rows==crew::loads(j).count(),"Worker switch retained old roster");
    compare(w,&direct,j,plan)?;
    if other {ensure!(candidates(w,j).is_empty(),"Second courier admitted after worker switch");}
    Ok(())
}

fn guarded_switch(w: &mut World, j: &mut State, id: u32, feet: Vec3) -> anyhow::Result<()> {
    let prior=w.travel.guard_posts.insert(id,feet);
    let worker=j.worker;let cargo=crew::cargo(j);
    w.farming.jobs=std::mem::take(j);
    crew::service(w,feet,0);
    *j=std::mem::take(&mut w.farming.jobs);
    if let Some(prior)=prior {w.travel.guard_posts.insert(id,prior);}
    else {w.travel.guard_posts.remove(&id);}
    ensure!(j.worker==worker && crew::cargo(j)==cargo,"Helper service changed cargo ownership");
    ensure!(j.supply_frame.is_none() && j.hub_access.is_none(),"Helper service leaked a worker snapshot");
    Ok(())
}

fn probe(w: &mut World, original: &State, plan: HubPlan) -> anyhow::Result<serde_json::Value> {
    let workers:Vec<_>=crew::ids(original).filter_map(|id|vehicle::villager_feet(w,id)
        .map(|feet|(id,feet))).take(2).collect();
    ensure!(workers.len()==2,"Lifecycle needs two existing physical workers");
    let (first,feet)=workers[0];let (second,other_feet)=workers[1];
    let mut j=super::super::isolated(original,first,feet);
    let mut other=super::super::isolated(original,second,other_feet);
    let mut helper=crew::Worker::default();helper.swap(&mut other);j.helpers.push(helper);
    let item=j.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).context("Lifecycle project missing")?.cells.iter()
        .filter_map(|o|o.material()).find(|&item|transfer_needed(w,&j,item)>1
            && production::count(&w.block_data.chest(plan.primary),item)>1)
        .context("Lifecycle needs two outstanding funded units")?;
    let needed=required(w,&j,item);let deficit=transfer_needed(w,&j,item);
    let before=production::count(&w.block_data.chest(plan.primary),item)
        +production::count(&w.block_data.chest(plan.stockpile),item);
    ensure!(block_data::take_hauled(w,plan.primary,item,1)==1,"Lifecycle source debit failed");
    j.carry=Some(ItemStack::new(item,1));
    j.cargo=Some(Cargo::Supply {source:plan.primary.to_array(),target:plan.stockpile.to_array(),project:plan.project});
    refresh(w,&mut j,plan,item,needed,1,false)?;
    guarded_switch(w,&mut j,second,other_feet)?;
    switch(&mut j)?;
    ensure!(j.worker==Some(second),"Lifecycle did not select second worker");
    refresh(w,&mut j,plan,item,needed,1,true)?;
    ensure!(transfer_needed(w,&j,item)==deficit-1,"Switched courier cargo lost its credit");
    switch(&mut j)?;
    let mut unit=j.carry.take().context("Lifecycle courier lost cargo")?;
    block_data::store_hauled(w,plan.stockpile,&mut unit);
    ensure!(unit.count==0,"Lifecycle hub cannot accept borrowed unit");
    j.cargo=None;j.reset_task();
    switch(&mut j)?;
    refresh(w,&mut j,plan,item,needed,0,false)?;
    ensure!(transfer_needed(w,&j,item)==deficit-1 && !candidates(w,&j).is_empty(),
        "Delivered courier still excludes the next worker");
    guarded_switch(w,&mut j,first,feet)?;
    switch(&mut j)?;
    ensure!(block_data::take_hauled(w,plan.stockpile,item,1)==1,"Lifecycle builder debit failed");
    j.carry=Some(ItemStack::new(item,1));j.cargo=Some(Cargo::Build {source:plan.stockpile.to_array()});
    j.building_material=true;
    switch(&mut j)?;
    refresh(w,&mut j,plan,item,needed-1,0,false)?;
    ensure!(transfer_needed(w,&j,item)==deficit-1,"Switched builder cargo lost its credit");
    let drop_count=w.inventory.dropped.len();
    ensure!(drop_count<1024,"Lifecycle needs one drop slot");
    w.farming.jobs=j;
    super::super::super::super::worker_fell(w,first,feet);
    let mut j=std::mem::take(&mut w.farming.jobs);
    ensure!(j.worker==Some(second) && j.helpers.is_empty(),"Lost helper remains in the crew");
    refresh(w,&mut j,plan,item,needed,0,false)?;
    ensure!(transfer_needed(w,&j,item)==deficit && !candidates(w,&j).is_empty(),
        "Lost builder cargo remains credited or excludes the courier");
    ensure!(w.inventory.dropped.len()==drop_count+1
        && w.inventory.dropped[drop_count].stack==ItemStack::new(item,1),"Lifecycle lost borrowed unit");
    let after=production::count(&w.block_data.chest(plan.primary),item)
        +production::count(&w.block_data.chest(plan.stockpile),item)+crew::cargo(&j)
        +w.inventory.dropped[drop_count].stack.count;
    ensure!(after==before,"Lifecycle storage and drop do not conserve stock");
    Ok(serde_json::json!({"two_physical_workers":true,"helper_service_clears_frame":true,
        "courier_exclusion_after_switch":true,"delivery_releases_courier":true,
        "builder_credit_after_switch":true,"death_rebuilds_obligations":true,"actual_borrowed_units":1}))
}
