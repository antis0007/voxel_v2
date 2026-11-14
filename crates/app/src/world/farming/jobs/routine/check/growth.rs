//! Runs a funded expansion on a copied completed colony checkpoint.
use super::*;
pub(super) mod evidence;
mod walk;

fn site(w:&World)->Result<&super::super::super::quarter::frontier::State> {
    w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing colony")
}
fn round_trip(w:&mut World,root:&Path,name:&str)->Result<()> {
    let before=ron::to_string(&w.farming.jobs)?;
    let encoded=crate::world::farming::sidecar::encode(w);
    std::fs::write(root.join(name),&encoded)?;
    let (restored,_)=crate::world::farming::sidecar::decode(&encoded)?;
    ensure!(ron::to_string(&restored.jobs)?==before,"Growth sidecar changed commitments");
    w.farming=restored;ensure!(crate::world::farming::sidecar::encode(w)==encoded,"Growth sidecar changed clock, plots or commitments");Ok(())
}

pub(crate) fn run(root:&Path)->Result<()> {
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Growth check requires a disposable copied save");
    std::fs::create_dir_all(root)?;
    let at=Instant::now();let loaded=storage::load_review_checkpoint()?;
    let (mut w,mut p,mut day)=(loaded.world,loaded.player,loaded.day_night);
    let admitted=materialize::admit(&mut w)?;
    let load_ms=at.elapsed().as_secs_f64()*1000.0;
    if std::env::var("VOXEL_GROWTH_BALANCE_ONLY").is_ok_and(|s|s=="1") {
        std::fs::write(root.join("timber-balance.json"),serde_json::to_vec_pretty(&evidence::timber(&w))?)?;return Ok(());
    }
    ensure!(site(&w)?.projects.len()==6&&site(&w)?.phase==6,"Expected six completed paid houses");
    let old_households=ron::to_string(&site(&w)?.households)?;
    let old_ids:Vec<_>=site(&w)?.households.iter().flat_map(|h|h.members.iter().copied()).collect();
    ensure!(old_ids.len()==24,"Expected 24 original occupants");
    let old_homes:BTreeMap<_,_>=old_ids.iter().filter_map(|id|w.travel.homes.get(id).map(|&p|(*id,p))).collect();
    let identities=w.farming.jobs.production.as_ref().context("Missing production")?.workers.clone();
    ensure!(identities.len()==64,"Growth fixture requires 64 real workers");
    let before_routines:BTreeMap<_,_>=identities.iter().filter_map(|&id|crew::view(&w.farming.jobs,id).map(|a|(id,a.routine.clone()))).collect();
    let before_served=w.farming.jobs.production.as_ref().context("Missing production")?.served.clone();
    round_trip(&mut w,root,"legacy-before-growth.vxf")?;
    let probe_balance=(food_balance(&w),timber_balance(&w));
    let at=Instant::now();super::super::super::check_housing_growth_sites(&mut w)?;
    ensure!((food_balance(&w),timber_balance(&w))==probe_balance,"Site probe changed resources");
    let site_negative_ms=at.elapsed().as_secs_f64()*1000.0;
    let before_food=food_balance(&w);let before_timber=timber_balance(&w);let before_components=evidence::timber(&w);
    let mut timber_events=Vec::new();let mut last_timber_delta=0i64;
    let roots:Vec<_>=w.farming.jobs.production.as_ref().context("Missing forestry")?.beds.iter()
        .filter(|b|b.crop==B::Sapling).map(|b|IVec3::from(b.at)).collect();
    super::super::super::enable_housing_growth(&mut w).map_err(anyhow::Error::msg)?;
    let enabled=ron::to_string(&site(&w)?.growth)?;
    super::super::super::enable_housing_growth(&mut w).map_err(anyhow::Error::msg)?;
    ensure!(ron::to_string(&site(&w)?.growth)?==enabled,"Enabling growth duplicated state");
    w.farming.jobs.quarter.as_mut().context("Missing quarter")?.paused=true;
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut input=Input::default();let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    let mut game=Vec::new();let mut finish=Vec::new();let mut drop_cost=Vec::new();let mut sample=Vec::new();
    let mut pending=Some(materialize::Pending::new(&w,0,"growth-initial"));
    let mut materializations=Vec::new();let mut observed=evidence::Activity::default();
    let mut stages=Vec::new();let mut last_stage=String::new();let mut grown=0u32;
    let mut unfunded_reload=false;let mut funded_reload=false;let mut cargo_reload=false;let mut full_reload=false;
    let mut final_reload=false;let mut funding_negative_ms=0.0;let mut funding_negative=false;let mut obstruction=None;let mut interrupted=false;
    let mut construction_start:Option<BTreeMap<u32,Resident>>=None;
    let mut resumed_during_build=HashSet::new();let mut finished_at=None;let mut reload_ms=Vec::new();
    let limit=std::env::var("VOXEL_GROWTH_STEPS").ok().and_then(|s|s.parse::<usize>().ok()).unwrap_or(180000).clamp(12000,600000);
    let started=Instant::now();
    for step in 0..limit {
        let growing:Vec<_>=roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        ensure!(sim.prepare_step(),"Growth simulation unavailable");
        let at=Instant::now();evidence::settle(&mut w);drop_cost.push(at.elapsed().as_secs_f64()*1000.0);
        let at=Instant::now();
        let mut materialized=Ok(None);
        fixed::gameplay(&mut w,&mut p,&mut input,false,|hook,w,_,_| {
            if matches!(hook,fixed::Hook::BeforeAdventure)&&let Some(stage)=&mut pending {
                materialized=stage.observe(root,w,step);
            }
            fixed::Actions::default()
        });
        if let Some(report)=materialized? {materializations.push(report);pending=None;observed.previous.clear();}
        for at in growing {if w.get(at)==B::Log&&!w.farming.plots.contains_key(&(at.x,at.y,at.z)){grown+=1;}}
        game.push(at.elapsed().as_secs_f64()*1000.0);
        let at=Instant::now();
        fixed::finish(&mut sim,&mut w,&p,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        finish.push(at.elapsed().as_secs_f64()*1000.0);
        blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        if step<240 {ensure!(site(&w)?.projects.len()==6,"Paused city admitted a proposal");}
        if step==240 {w.farming.jobs.quarter.as_mut().context("Missing quarter")?.paused=false;}
        let s=site(&w)?;let growth=s.growth.as_ref().context("Growth state disappeared")?;
        let funded=growth.project.is_some();let complete=growth.complete;
        let status=format!("{}:{}:{}",growth.proposal.is_some(),funded,growth.waiting);
        if status!=last_stage {
            if stages.len()<128 {stages.push(serde_json::json!({"step":step,"clock":w.farming.clock,"growth":growth}));}
            last_stage=status;
        }
        ensure!(s.projects.len()<=7,"Growth exceeded one funded house");
        ensure!(old_homes.iter().all(|(id,pos)|w.travel.homes.get(id)==Some(pos)),"Expansion displaced an original home");
        if funded&&construction_start.is_none() {
            construction_start=Some(old_ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id).map(|a|(id,a.routine.clone()))).collect());
        }
        if funded&&!complete&&let Some(baseline)=&construction_start {
            for &id in &old_ids {if crew::view(&w.farming.jobs,id).is_some_and(|a|a.routine.rests>baseline[&id].rests
                &&a.routine.socials>baseline[&id].socials&&a.routine.resumed>baseline[&id].resumed) {resumed_during_build.insert(id);}}
        }
        if !unfunded_reload&&growth.proposal.is_some()&&!funded {
            round_trip(&mut w,root,"unfunded-proposal.vxf")?;unfunded_reload=true;
        }
        if funded&&!funded_reload {
            round_trip(&mut w,root,"funded-proposal.vxf")?;funded_reload=true;
        }
        if funded&&!funding_negative&&pending.is_none() {
            let balance=(food_balance(&w),timber_balance(&w));
            let at=Instant::now();super::super::super::check_housing_growth_funding(&mut w)?;
            ensure!((food_balance(&w),timber_balance(&w))==balance,"Funding probe changed resources");
            funding_negative_ms=at.elapsed().as_secs_f64()*1000.0;funding_negative=true;
        }
        if let Some((at,until))=obstruction {
            ensure!(known(&w,at)==Some(B::Stone),"Growth erased an intervening player obstruction");
            if step>=until {w.set(at,B::Air);obstruction=None;}
        }
        if funded&&!cargo_reload&&pending.is_none()
            &&let Some(order)=w.farming.jobs.claims.iter().map(|c|c.order)
                .find(|o|w.farming.jobs.growth_funded_order(*o)&&known(&w,o.cell())==Some(B::Air)) {
            w.set(order.cell(),B::Stone);obstruction=Some((order.cell(),step+48000));interrupted=true;
            round_trip(&mut w,root,"growth-carried-construction.vxf")?;cargo_reload=true;
            let at=Instant::now();reload::full(root,&mut w,&mut p,&mut day)?;
            reload_ms.push(at.elapsed().as_secs_f64()*1000.0);full_reload=true;
            sim.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,"growth-cargo-reload"));observed.previous.clear();
        }
        if pending.is_none()&&step%PULSE as usize==0 {
            let at=Instant::now();observed.sample(&w,&identities,step)?;
            ensure!(w.farming.jobs.valid(),"Invalid growth commitments at step {step}");
            sample.push(at.elapsed().as_secs_f64()*1000.0);
        }
        if step%1200==0 {
            let expected=before_timber+grown*16;let actual=timber_balance(&w);let delta=i64::from(actual)-i64::from(expected);
            if delta!=last_timber_delta&&timber_events.len()<128 {
                timber_events.push(serde_json::json!({"step":step,"grown":grown,"expected":expected,"delta":delta,"components":evidence::timber(&w)}));
                last_timber_delta=delta;
            }
        }
        if step%12000==0 {
            evidence::write(root,&w,&observed)?;
            println!("[housing-growth] step={step} projects={} paid={} {}",site(&w)?.projects.len(),
                site(&w)?.projects.get(6).map_or(0,|p|p.cells.iter().filter(|o|known(&w,o.cell())==Some(o.block())).count()),last_stage);
        }
        if complete&&finished_at.is_none() {finished_at=Some(step);}
        if let Some(done)=finished_at&&step>done+60000&&pending.is_none() {
            if final_reload {break;}
            let at=Instant::now();reload::full(root,&mut w,&mut p,&mut day)?;
            reload_ms.push(at.elapsed().as_secs_f64()*1000.0);final_reload=true;
            sim.reset_for_world(w.voxel_scale);
            pending=Some(materialize::Pending::new(&w,step+1,"growth-final-reload"));observed.previous.clear();
        }
    }
    evidence::write(root,&w,&observed)?;
    let s=site(&w)?;let g=s.growth.as_ref().context("Missing final growth state")?;
    let complete=g.complete&&s.phase==7&&s.projects.len()==7;
    let housed=s.households.iter().map(|h|h.members.len()).sum::<usize>();
    let retained=ron::to_string(&s.households.iter().filter(|h|h.project<6).collect::<Vec<_>>())?==old_households;
    let new_home=s.projects.get(6).and_then(|p|p.home.as_ref()).cloned();
    let paid=s.projects.get(6).map_or(0,|p|p.cells.iter().filter(|o|known(&w,o.cell())==Some(o.block())).count());
    let planned=s.projects.get(6).map_or(0,|p|p.cells.len());
    let new_ids:Vec<_>=s.households.iter().filter(|h|h.project==6).flat_map(|h|h.members.iter().copied()).collect();
    let prod=w.farming.jobs.production.as_ref().context("Lost production")?;
    let new_cycles=new_ids.iter().filter(|&&id|crew::view(&w.farming.jobs,id).is_some_and(|a|
        a.routine.rests>before_routines[&id].rests&&a.routine.socials>before_routines[&id].socials
        &&a.routine.resumed>before_routines[&id].resumed
        &&prod.served.get(&id).copied().unwrap_or(0)>before_served.get(&id).copied().unwrap_or(0))).count();
    let material_counts=s.projects.get(6).map(|p|[p.cells.iter().filter(|o|o.block()==B::Planks).count(),p.cells.iter().filter(|o|o.block()==B::Chest).count()]);
    let physically_moved=identities.iter().filter(|id|observed.moved.get(id).copied().unwrap_or(0.0)>=2.0).count();
    let food=food_balance(&w)==before_food;let timber=timber_balance(&w)==before_timber+grown*16;
    let walking=if complete {let home=new_home.context("New house missing home")?;walk::check(&w,&mut p,home.entry,home.destination)?} else {0.0};
    let passed=complete&&housed==28&&new_ids.len()==4&&new_cycles==4&&retained&&paid==planned&&paid==91&&material_counts==Some([90,1])
        &&food&&timber&&unfunded_reload&&funded_reload&&cargo_reload&&full_reload&&final_reload&&funding_negative
        &&interrupted&&obstruction.is_none()&&pending.is_none()&&observed.minimum_live==64&&physically_moved==64&&observed.jumps.is_empty()
        &&resumed_during_build.len()==24;
    let steps=game.len();let final_growth=serde_json::to_value(&site(&w)?.growth)?;
    let mut result=serde_json::json!({"passed":passed,"steps":steps,"simulated_seconds":steps as f64*crate::simulation::STEP_SECONDS as f64,
        "residents":identities.len(),"housed_before":24,"housed_after":housed,"unhoused_after":64-housed,"new_occupants":new_ids,"new_occupants_completed_cycles":new_cycles,"paid_materials":material_counts,
        "completed":complete,"paid_cells":paid,"planned_cells":planned,"old_households_retained":retained,"old_residents_completed_cycles_during_construction":resumed_during_build,"construction_start_routines":construction_start,
        "minimum_live":observed.minimum_live,"physically_moved":physically_moved,"movement_discontinuities":observed.jumps,"distance_m":observed.moved});
    let details=serde_json::json!({"food_conserved":food,"timber_conserved":timber,"grown_trees":grown,
        "unfunded_sidecar_reload":unfunded_reload,"funded_sidecar_reload":funded_reload,"cargo_sidecar_reload":cargo_reload,"full_cargo_checkpoint_reload":full_reload,"final_checkpoint_reload_and_materialization":final_reload,
        "site_negative_checks":true,"site_negative_ms":site_negative_ms,"funding_negative_checks":funding_negative,"funding_negative_ms":funding_negative_ms,"obstruction_recovery":interrupted&&obstruction.is_none(),"paused_admission":true,"grounded_street_to_home_m":walking,
        "stages":stages,"growth":final_growth,"initial_load_ms":load_ms,"admitted_chunks":admitted,"materialization_stages":materializations,"reload_ms":reload_ms,
        "wall_seconds":started.elapsed().as_secs_f64(),"gameplay_ms":stats(game),"finish_ms":stats(finish),"sample_ms":stats(sample),"drop_settle_ms":stats(drop_cost),
        "limits":"One bounded four-bed proposal on an authored local site; existing 64 workers and physical supply only; no unrestricted planning or multiplayer claim"});
    if let (Some(out),Some(extra))=(result.as_object_mut(),details.as_object()) {out.extend(extra.clone());}
    result["timber_ledger"]=serde_json::json!({"before":before_components,"after":evidence::timber(&w),
        "grown":grown,"expected":before_timber+grown*16,"events":timber_events});
    std::fs::write(root.join("housing-growth-check.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[housing-growth] passed={passed} housed={housed} paid={paid}/{planned} food={food} timber={timber}");
    ensure!(passed,"Housing-growth acceptance incomplete");
    round_trip(&mut w,root,"growth-complete.vxf")?;Ok(())
}
