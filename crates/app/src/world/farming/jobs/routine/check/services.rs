//! Native paid service-area acceptance uses existing actors and cargo.
use super::*;
use super::growth::evidence;
use crate::world::farming::jobs::{self,quarter::frontier};
use std::collections::BTreeSet;
mod ledger;
mod record;
mod walk;
mod recovery_probe;

fn site(w:&World)->Result<&frontier::State> {
    w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing service colony")
}
pub(crate) fn run(root:&Path)->Result<()> {
    std::fs::create_dir_all(root)?;
    let result=execute(root);
    if let Err(e)=&result {std::fs::write(root.join("service-failure.txt"),format!("{e:#}"))?;}
    result
}
fn execute(root:&Path)->Result<()> {
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Service check requires disposable saves");
    ensure!(std::env::var("VOXEL_CITY_GROWTH").is_ok_and(|v|v=="4"),"Service check requires growth4");
    let loaded=storage::load_review_checkpoint()?;
    let (mut w,mut player,mut day)=(loaded.world,loaded.player,loaded.day_night);
    if std::env::var("VOXEL_CHECKPOINT_RECOVERY_AUDIT").is_ok_and(|v|v=="1") {
        return recovery_probe::run(root,&mut w);
    }
    if std::env::var("VOXEL_CHECKPOINT_LIVENESS_CHECK").is_ok_and(|v|v=="1") {
        let report=storage::SaveService::check_maintenance_liveness(&mut w,&player,&day)?;
        std::fs::write(root.join("save-maintenance-liveness.json"),serde_json::to_vec_pretty(&report)?)?;
        ensure!(report["passed"]==true,"Save maintenance liveness failed");
        return Ok(());
    }
    let admitted=materialize::admit(&mut w)?;
    let base=site(&w)?.projects.len();
    ensure!(base==9&&site(&w)?.phase==base,"Expected nine completed paid homes");
    let old_households=ron::to_string(&site(&w)?.households)?;
    let old_projects=ron::to_string(&site(&w)?.projects)?;
    let old_ids:Vec<_>=site(&w)?.households.iter().flat_map(|h|h.members.iter().copied()).collect();
    let old_homes:BTreeMap<_,_>=old_ids.iter().filter_map(|id|w.travel.homes.get(id).copied().map(|p|(*id,p))).collect();
    let ids=w.farming.jobs.production.as_ref().context("Missing production")?.workers.clone();
    ensure!(ids.len()==64&&old_ids.len()==36,"Expected64physical residents and36housed");
    let before=(food_balance(&w),ledger::timber(&w));
    let original_targets=record::targets(&w);let mut production_targets_changed=false;
    frontier::enable_service_growth(&mut w).map_err(anyhow::Error::msg)?;
    let enabled=ron::to_string(&site(&w)?.growth)?;
    frontier::enable_service_growth(&mut w).map_err(anyhow::Error::msg)?;
    ensure!(ron::to_string(&site(&w)?.growth)?==enabled,"Repeated enable changed service proposal");
    ensure!(site(&w)?.growth.as_ref().is_some_and(|g|g.version==4),"Growth4 was not enabled");
    ensure!((food_balance(&w),ledger::timber(&w))==before&&ron::to_string(&site(&w)?.projects)?==old_projects,"Enable changed physical resources");
    let growth_probes=frontier::check_service_growth(&mut w)?;
    let roots:Vec<_>=w.farming.jobs.production.as_ref().context("Missing forestry")?.beds.iter()
        .filter(|b|b.crop==B::Sapling).map(|b|IVec3::from(b.at)).collect();
    let mut cycles=record::Cycles::new(&w,&ids);
    let mut activity=evidence::Activity::default();
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut input=Input::default();let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    let mut pending=Some(materialize::Pending::new(&w,0,"service-initial"));
    let mut stages=Vec::new();let mut prior=String::new();let mut reloads=Vec::new();let mut materializations=Vec::new();
    let mut reloaded=BTreeSet::new();let mut sidecars=BTreeSet::new();let mut grown=0u32;
    let mut game=Vec::new();let mut finish=Vec::new();let mut drop_ms=Vec::new();let mut samples=Vec::new();
    let mut planning=Vec::new();let mut last_poll=0u32;let mut max_queue=0usize;let mut max_couriers=0usize;
    let mut obstruction:Option<(IVec3,usize)>=None;let mut obstructed=false;let mut completed_at=None;
    let mut supply_probes=None;let mut plan_probes=None;
    let mut prior_hub_stock=0u32;let mut observed_deliveries=0u64;let mut home_admitted=false;
    let mut new_ids=Vec::new();let mut new_cycles=None;
    let mut physical_completed_at=None;let mut home_access=None;
    let limit=std::env::var("VOXEL_SERVICE_STEPS").ok().and_then(|v|v.parse::<usize>().ok()).unwrap_or(1000000).clamp(12000,1200000);
    let started=Instant::now();
    w.farming.jobs.quarter.as_mut().context("Missing city")?.paused=true;
    for step in 0..limit {
        let growing:Vec<_>=roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        ensure!(sim.prepare_step(),"Simulation unavailable");
        let at=Instant::now();evidence::settle(&mut w);drop_ms.push(at.elapsed().as_secs_f64()*1000.0);
        let at=Instant::now();let mut materialized=Ok(None);
        fixed::gameplay(&mut w,&mut player,&mut input,false,|hook,w,_,_| {
            if matches!(hook,fixed::Hook::BeforeAdventure)&&let Some(stage)=&mut pending {materialized=stage.observe(root,w,step);}
            fixed::Actions::default()
        });
        game.push(at.elapsed().as_secs_f64()*1000.0);
        if let Some(report)=materialized? {materializations.push(report);pending=None;activity.previous.clear();}
        for at in growing {if w.get(at)==B::Log&&!w.farming.plots.contains_key(&(at.x,at.y,at.z)){grown+=1;}}
        let at=Instant::now();fixed::finish(&mut sim,&mut w,&player,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        finish.push(at.elapsed().as_secs_f64()*1000.0);blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        if step<240 {ensure!(site(&w)?.projects.len()==base,"Paused colony admitted service project");}
        if step==240 {w.farming.jobs.quarter.as_mut().context("Missing city")?.paused=false;}
        ensure!(site(&w)?.projects.len()<=base+2,"More than one service hub and one house");
        let g=site(&w)?.growth.as_ref().context("Lost service state")?;
        if g.polls!=last_poll {production_targets_changed|=record::targets(&w)!=original_targets;planning.push(game.last().copied().unwrap_or(0.0));last_poll=g.polls;}
        let state=format!("{}:{:?}:{}",site(&w)?.projects.len(),g.project,g.waiting);
        if state!=prior {if stages.len()<512{stages.push(serde_json::json!({"step":step,"state":g}));}prior=state;}
        let active=g.active_project();let complete=g.complete;
        if g.proposal.is_some()&&sidecars.insert(site(&w)?.projects.len()) {if plan_probes.is_none(){plan_probes=Some(frontier::check_service_growth(&mut w)?);}
            let label=format!("proposal-{}",site(&w)?.projects.len());record::sidecar(&mut w,root,&label)?;}
        if active==Some(base+1)&&supply_probes.is_none() {
            let before_probe=ledger::timber(&w);
            supply_probes=Some(jobs::supply::check::run(&mut w).map_err(anyhow::Error::msg)?);
            ensure!(ledger::timber(&w)==before_probe,"Supply probes changed physical goods");
        }
        max_queue=max_queue.max(w.farming.jobs.build.len());ensure!(max_queue<=256,"Live construction queue exceeded256");
        let couriers=ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id))
            .filter(|a|matches!(a.cargo,Some(jobs::supply::Cargo::Supply{..}))).count();
        max_couriers=max_couriers.max(couriers);ensure!(couriers<=1,"Multiple service couriers");
        if let Some(h)=frontier::hub_plan(&w.farming.jobs) {
            let stock=ledger::stock(&w,h.stockpile);observed_deliveries+=u64::from(stock.saturating_sub(prior_hub_stock));prior_hub_stock=stock;
            if active==Some(h.project) {ensure!(site(&w)?.home_complete(&w,base),"Remote house funded before paid hub");home_admitted=true;}
        }
        if let Some((cell,placed))=obstruction&&step>=placed+12000 {
            ensure!(known(&w,cell)==Some(B::Stone),"Builder replaced obstruction");
            w.set(cell,B::Air);obstruction=None;
        }
        if pending.is_none() {
            let courier=ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id))
                .any(|a|a.carry.is_some()&&matches!(a.cargo,Some(jobs::supply::Cargo::Supply{..})));
            let builder=frontier::hub_plan(&w.farming.jobs).is_some_and(|h|ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id))
                .any(|a|a.carry.is_some()&&a.cargo==Some(jobs::supply::Cargo::Build{source:h.stockpile.to_array()})));
            let label=if courier&&!reloaded.contains("courier-cargo"){Some("courier-cargo")}
                else if builder&&!reloaded.contains("hub-builder-cargo"){Some("hub-builder-cargo")}
                else if site(&w)?.home_complete(&w,base)&&!home_admitted&&!reloaded.contains("paid-hub"){Some("paid-hub")}
                else {None};
            if label==Some("hub-builder-cargo")&&!obstructed {
                if let Some(order)=w.farming.jobs.claims.iter().map(|c|c.order).find(|o|
                    site(&w).ok().and_then(|s|s.projects.get(base+1)).is_some_and(|p|p.cells.contains(o))&&known(&w,o.cell())==Some(B::Air)) {
                    w.set(order.cell(),B::Stone);obstruction=Some((order.cell(),step));obstructed=true;
                }
            }
            if let Some(label)=label {
                record::reload(root,&mut w,&mut player,&mut day,label,step,&mut reloads)?;
                reloaded.insert(label);sim.reset_for_world(w.voxel_scale);
                pending=Some(materialize::Pending::new(&w,step+1,label));activity.previous.clear();
            }
        }
        if step%PULSE as usize==0 {
            ensure!(ron::to_string(&site(&w)?.projects[..base])?==old_projects,"Original paid home geometry changed");
            ensure!(ron::to_string(&site(&w)?.households.iter().filter(|h|h.project<base).collect::<Vec<_>>())?==old_households,"Original households changed");
            ensure!(old_homes.iter().all(|(id,p)|w.travel.homes.get(id)==Some(p)),"Original home assignment changed");
            if pending.is_none() {let at=Instant::now();activity.sample(&w,&ids,step)?;ensure!(w.farming.jobs.valid(),"Invalid jobs at service step{step}");samples.push(at.elapsed().as_secs_f64()*1000.0);}
            cycles.observe(&w);
            if let Some(c)=&mut new_cycles {record::Cycles::observe(c,&w);}
            ensure!(food_balance(&w)==before.0,"Food conservation failed at{step}");
            if step%1200==0 {
                ensure!(ledger::timber(&w)==before.1+grown*16,"Timber conservation failed at{step}:{} expected{}",ledger::timber(&w),before.1+grown*16);
            }
        }
        if site(&w)?.home_complete(&w,base+1)&&physical_completed_at.is_none() {
            physical_completed_at=Some(step);home_access=Some(record::home_access(&w,base+1)?);
        }
        if physical_completed_at.is_some()&&pending.is_none()&&!reloaded.contains("physical-home") {
            record::reload(root,&mut w,&mut player,&mut day,"physical-home",step,&mut reloads)?;
            reloaded.insert("physical-home");sim.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,"physical-home"));activity.previous.clear();
        }
        if complete&&completed_at.is_none() {
            ensure!(site(&w)?.projects.len()==base+2&&site(&w)?.home_complete(&w,base+1),"Service complete without physical home");
            new_ids=site(&w)?.households.iter().filter(|h|h.project>=base).flat_map(|h|h.members.iter().copied()).collect();
            if new_ids.len()==8 {new_cycles=Some(record::Cycles::new(&w,&new_ids));completed_at=Some(step);}
        }
        if step%12000==0 {evidence::write(root,&w,&activity)?;println!("[service-check] step={step} {prior} timber={} grown={grown}",ledger::timber(&w));}
        if completed_at.is_some()&&new_cycles.as_ref().is_some_and(|c|c.done(&new_ids))&&cycles.done(&old_ids)&&obstruction.is_none()&&pending.is_none() {
            if reloaded.contains("final"){break;}
            record::reload(root,&mut w,&mut player,&mut day,"final",step,&mut reloads)?;
            reloaded.insert("final");sim.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,"final"));activity.previous.clear();
        }
    }
    evidence::write(root,&w,&activity)?;
    let paid=site(&w)?.projects.iter().skip(base).flat_map(|p|&p.cells).filter(|o|known(&w,o.cell())==Some(o.block())).count();
    let planned=site(&w)?.projects.iter().skip(base).map(|p|p.cells.len()).sum::<usize>();
    let routes=if physical_completed_at.is_some(){walk::check(&w,&mut player,base+1)}else{Err(anyhow::anyhow!("Service construction unfinished"))};
    let moved=ids.iter().filter(|id|activity.moved.get(id).copied().unwrap_or(0.0)>=2.0).count();
    let cycles_done=new_cycles.as_ref().is_some_and(|c|c.done(&new_ids));
    let passed=completed_at.is_some()&&new_ids.len()==8&&cycles_done&&cycles.done(&old_ids)&&paid==planned&&home_admitted
        &&production_targets_changed&&record::targets(&w)==original_targets&&plan_probes.is_some()&&supply_probes.is_some()&&observed_deliveries>0&&max_couriers==1&&obstructed&&obstruction.is_none()&&reloaded.contains("courier-cargo")
        &&reloaded.contains("hub-builder-cargo")&&reloaded.contains("final")&&pending.is_none()&&sidecars.len()>=2
        &&food_balance(&w)==before.0&&ledger::timber(&w)==before.1+grown*16&&activity.minimum_live==64&&moved==64&&activity.jumps.is_empty()&&routes.is_ok();
    let mut report=serde_json::json!({"passed":passed,"steps":game.len(),"completed_at":completed_at,"paid_cells":paid,"planned_cells":planned,
        "new_occupants":new_ids,"new_ordered_cycles":new_cycles,"old_ordered_cycles":cycles,"housed_after":site(&w)?.households.iter().map(|h|h.members.len()).sum::<usize>(),
        "minimum_live":activity.minimum_live,"physically_moved":moved,"movement_discontinuities":activity.jumps,"maximum_queued_orders":max_queue,
        "maximum_couriers":max_couriers,"observed_positive_hub_stock_deltas":observed_deliveries,"obstruction_recovered":obstructed&&obstruction.is_none(),
        "food_conserved":food_balance(&w)==before.0,"timber_conserved":ledger::timber(&w)==before.1+grown*16,"grown_trees":grown,
        "full_reloads":reloads,"proposal_sidecars":sidecars,"materialization_stages":materializations,"initial_chunks_admitted":admitted});
    let details=serde_json::json!({"production_targets_changed_for_real_demand":production_targets_changed,"production_targets_restored":record::targets(&w)==original_targets,
        "initial_targets":original_targets,"final_targets":record::targets(&w),"growth_negative_checks":growth_probes,"canonical_plan_negative_checks":plan_probes,"supply_negative_checks":supply_probes,"wall_seconds":started.elapsed().as_secs_f64(),"gameplay_ms":stats(game),"finish_ms":stats(finish),
        "drop_settle_ms":stats(drop_ms),"sampling_ms":stats(samples),"gameplay_ticks_with_planning_ms":stats(planning),"stages":stages,
        "physical_completed_at":physical_completed_at,"home_access":home_access,"growth":site(&w)?.growth,"grounded_routes":routes.as_ref().ok(),"route_error":routes.err().map(|e|format!("{e:#}")),
        "limits":"One paid hub and one timber-style home; same64physical residents; no unrestricted city or FPS claim"});
    if let (Some(a),Some(b))=(report.as_object_mut(),details.as_object()){a.extend(b.clone());}
    std::fs::write(root.join("service-expansion.json"),serde_json::to_vec_pretty(&report)?)?;
    println!("[service-check] passed={passed} paid={paid}/{planned} moved={moved}");ensure!(passed,"Service-area acceptance incomplete");Ok(())
}
