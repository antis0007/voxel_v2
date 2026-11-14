//! Measures physical residents, paid construction and conserved production.
use super::*;
use anyhow::{Context,Result,ensure};
use crate::{player::{Input,Player},simulation::{Budget,WorldSimulation,fixed},world::DayNightCycle};
use std::{collections::{BTreeMap,HashSet},path::Path,time::Instant};
mod checkpoint;
mod walk;
mod diagnostics;
mod drops;
mod activity;
mod step;

pub(in super::super) fn run(root:&Path)->Result<()> {
    if std::env::var("VOXEL_CITY_NEIGHBORHOOD_CHECK").is_ok_and(|s|s=="1") {return jobs::routine::check::neighborhood::run(root);}
    if std::env::var("VOXEL_CITY_GROWTH_CHECK").is_ok_and(|s|s=="1") {return jobs::routine::check::growth::run(root);}
    if std::env::var("VOXEL_DAILY_LIFE_CHECK").is_ok_and(|s|s=="1") {return jobs::routine::check::run(root);}
    std::fs::create_dir_all(root)?;
    step::check(root)?;
    let mut visits=vec![0;jobs::crew::MAX_HELPERS];let mut peak=0;
    for phase in 0..jobs::PULSE {
        let mut n=usize::from(phase==0);
        for (i,count) in visits.iter_mut().enumerate() {if jobs::crew::service_phase(i)==phase {*count+=1;n+=1;}}
        peak=peak.max(n);
    }
    ensure!(visits.iter().all(|n|*n==1)&&peak<=3,"Worker scheduling omitted or duplicated a helper");
    let world_root=root.join("disposable-saves");std::fs::create_dir_all(&world_root)?;
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,424242,&world_root);
    let mut p=Player::new();scene::install(&mut w,&mut p).map_err(anyhow::Error::msg)?;
    let initial=site(&w)?.clone();let n=scene::residents();
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);let base=pile-IVec3::Y;
    ensure!(jobs::crew::saved_workers(&w).len()==n,"Physical roster mismatch");
    let depot=jobs::Task::Store(pile).point(1.0);
    ensure!(jobs::within_job_range(&w.farming.jobs,depot+Vec3::X*25.0,depot,depot),
        "A boundary worker cannot return cargo to the stockpile");
    ensure!(!jobs::within_job_range(&w.farming.jobs,depot,depot,depot+Vec3::X*25.0),
        "Colony admitted work outside its stockpile service radius");

    let distant=base.as_vec3()+Vec3::new(-23.5,1.0,-20.5);
    ensure!(jobs::path_with_limit(&w,distant,jobs::Task::Store(pile),4096).is_empty(),
        "Legacy path radius fixture did not exceed its start radius");
    ensure!(!jobs::path_in_range(&w,distant,jobs::Task::Store(pile),4096,jobs::RANGE*2.0+4.0).is_empty(),
        "A cross-site carrier cannot return to the stockpile within the node budget");
    ensure!(initial.projects.iter().all(|p|p.cells.iter().all(|o|known(&w,o.cell())==Some(B::Air))),"Prebuilt project");
    let roots:Vec<_>=w.farming.jobs.production.as_ref().context("Production missing")?.beds.iter()
        .filter(|b|b.crop==B::Sapling).map(|b|IVec3::from(b.at)).collect();
    sidecar(&mut w,root,"initial.vxf")?;
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut day=DayNightCycle::new(0.4,1200.0);let mut input=Input::default();
    let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    let mut ticks=Vec::new();let mut gameplay=Vec::new();let mut physics=Vec::new();let mut loose_drops=Vec::new();
    let mut active=activity::Record::default();
    let mut grown=0;let mut cargo_saved=false;let mut full_reload=false;
    let mut placements=BTreeMap::new();let mut last=BTreeMap::new();let mut max_sites=0;
    let bounded=std::env::var("VOXEL_CITY_PERF_STEPS").ok().and_then(|s|s.parse::<usize>().ok()).filter(|n|*n>0);
    let limit=bounded.unwrap_or(600_000).clamp(3_600,600_000);
    let mut obstruction=None;let mut interrupted=false;
    let started=Instant::now();
    for step in 0..limit {
        let growing:Vec<_>=roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        ensure!(sim.prepare_step(),"World simulation cannot advance");
        let tick=Instant::now();
        drops::settle(&mut w);
        loose_drops.push(tick.elapsed().as_secs_f64()*1000.0);
        let game=Instant::now();
        fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|fixed::Actions::default());
        gameplay.push(game.elapsed().as_secs_f64()*1000.0);
        for at in growing {if w.get(at)==B::Log&&!w.farming.plots.contains_key(&(at.x,at.y,at.z)) {grown+=1;}}
        let finish=Instant::now();
        fixed::finish(&mut sim,&mut w,&p,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        physics.push(finish.elapsed().as_secs_f64()*1000.0);
        ticks.push(tick.elapsed().as_secs_f64()*1000.0);
        blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        if !interrupted&&step>1500&&let Some(order)=w.farming.jobs.claims.first().map(|c|c.order)
            &&known(&w,order.cell())==Some(B::Air) {
            w.set(order.cell(),B::Stone);obstruction=Some((order.cell(),step+1200));interrupted=true;
        }
        if let Some((at,until))=obstruction {
            if known(&w,at)!=Some(B::Stone) {
                diagnostics::write(root,&w)?;
                std::fs::write(root.join("obstruction-failure.json"),serde_json::to_vec_pretty(&serde_json::json!({
                    "step":step,"tick":w.farming.clock,"cell":at.to_array(),"actual":format!("{:?}",known(&w,at)),
                    "remove_at_step":until,"checkpoint_reload":full_reload}))?)?;
                anyhow::bail!("Intervening player block changed at {at:?}, step {step}: {:?}",known(&w,at));
            }
            if step>=until {w.set(at,B::Air);obstruction=None;}
        }
        if step%30==0 {
            active.sample(&w)?;
            for (id,_) in jobs::crew::saved_workers(&w) {
                if let Some((at,_))=jobs::crew::view(&w.farming.jobs,id).and_then(|a|a.last_placement)
                    &&last.get(&id)!=Some(&at) {last.insert(id,at);*placements.entry(id).or_insert(0u32)+=1;}
            }
            max_sites=max_sites.max(initial.projects.iter().filter(|p|p.cells.iter().any(|o|w.farming.jobs.build.contains(o))).count());
        }
        if !cargo_saved&&step>2000&&w.farming.jobs.claims.len()>0 {
            sidecar(&mut w,root,"carried-construction.vxf")?;cargo_saved=true;
            if std::env::var("VOXEL_CITY_CHECKPOINT").is_ok_and(|s|s=="1") {
                checkpoint::reload(root,&mut w,&mut p,&mut day,true)?;
                sim.reset_for_world(w.voxel_scale);full_reload=true;
            }
        }
        if step%120==0 {ensure!(w.farming.jobs.valid(),"Invalid commitments at step {step}");}
        if step%12000==0 {
            diagnostics::write(root,&w)?;
            let paid:Vec<_>=(0..initial.projects.len()).map(|i|site(&w).map_or(0,|s|s.built(&w,i))).collect();
            println!("[city-colony-check] step={step} completed={} queue={} grown={grown} builders={} paid={paid:?} forestry={:?} {}",
                site(&w)?.phase,w.farming.jobs.build.len(),placements.len(),production::woodlot_counts(&w),production::summary(&w).unwrap_or_default());
        }
        let prod=w.farming.jobs.production.as_ref().context("Lost production")?;
        if site(&w)?.phase==initial.projects.len()&&prod.served.len()==n&&prod.served.values().all(|x|*x>=2)
            &&production::woodlot_counts(&w).is_some_and(|(_,felled)|felled>=4)
            &&w.sandbox.bodies.iter().all(|b|!b.felled_tree) {break;}
    }
    active.sample(&w)?;
    let activity=active.report(&w);
    let workers:Vec<_>=jobs::crew::saved_workers(&w).into_iter().map(|(id,feet)| {
        let a=jobs::crew::view(&w.farming.jobs,id).expect("Saved worker has a view");
        serde_json::json!({"id":id,"feet":feet.to_array(),"task":format!("{:?}",a.task),"status":a.status,
            "cargo":a.carry,"path":a.path.iter().map(|p|p.to_array()).collect::<Vec<_>>()})
    }).collect();
    std::fs::write(root.join("workers.json"),serde_json::to_vec_pretty(&workers)?)?;
    diagnostics::write(root,&w)?;
    let houses_complete=site(&w)?.phase==initial.projects.len();
    let housed=if houses_complete {super::housing::check(&mut w)?} else {
        site(&w)?.households.iter().map(|h|h.members.len()).sum()
    };
    let mut housing_checkpoint_reload=false;
    if std::env::var("VOXEL_CITY_CHECKPOINT").is_ok_and(|s|s=="1") {
        checkpoint::reload(root,&mut w,&mut p,&mut day,false)?;
        if houses_complete {
            ensure!(super::housing::check(&mut w)?==housed,"Housing changed across full checkpoint");
            housing_checkpoint_reload=true;
        }
    }
    std::fs::write(root.join("final-jobs.ron"),ron::to_string(&w.farming.jobs)?)?;
    let prod=w.farming.jobs.production.as_ref().context("Lost production")?;
    let slots=w.block_data.chest(pile);
    let held=|item|production::count(&slots,item)+jobs::crew::cargo_item(&w.farming.jobs,item)
        +w.inventory.dropped.iter().filter(|d|d.stack.item==item).map(|d|d.stack.count).sum::<u32>();
    let built=|block|initial.projects.iter().flat_map(|p|&p.cells).filter(|o|o.block()==block&&known(&w,o.cell())==Some(block)).count() as u32;
    let logs=world_logs(&w,base);
    let balanced=(100+grown*4)*4==(logs+held(Item::Block(B::Log)))*4+built(B::Planks)+held(Item::Block(B::Planks))
        +(built(B::Chest)+held(Item::Block(B::Chest)))*8;
    let food=96+prod.counts.harvested==prod.counts.planted+2*prod.counts.meals
        +held(Item::Block(B::CarrotCrop))+held(Item::Block(B::BeetCrop))
        &&held(prod.meal)+prod.counts.eaten==prod.counts.meals;
    let complete=site(&w)?.phase==initial.projects.len();
    let passed=balanced&&food&&cargo_saved&&interrupted&&obstruction.is_none()
        &&activity["final_active_count"].as_u64()==Some(n as u64)
        &&activity["moving_resident_count"].as_u64()==Some(n as u64)
        &&if bounded.is_some() {true} else {complete&&prod.served.len()==n&&prod.served.values().all(|x|*x>=2)
            &&max_sites>=2&&placements.len()>=n/3&&production::woodlot_counts(&w).is_some_and(|(_,f)|f>=4)
            &&w.sandbox.bodies.iter().all(|b|!b.felled_tree)};
    let walked=if passed&&complete {walk::check(&w,&mut p)?;true}else{false};
    let result=serde_json::json!({"passed":passed,"physical_residents":n,"aggregate_residents":0,"resident_activity":activity,"steps":w.farming.clock,
        "acceptance_mode":if bounded.is_some(){"bounded_run"}else{"complete_settlement"},"completed_all_projects":complete,
        "wall_seconds":started.elapsed().as_secs_f64(),"completed_projects":site(&w)?.phase,"planned_projects":initial.projects.len(),
        "paid_cells":built(B::Planks)+built(B::Chest),"planned_cells":initial.projects.iter().map(|p|p.cells.len()).sum::<usize>(),
        "maximum_concurrent_sites":max_sites,"placements_per_worker":placements,"served_per_resident":prod.served,
        "production":prod.counts,"grown_trees":grown,"forestry":production::woodlot_counts(&w),"timber_conserved":balanced,"food_conserved":food,
        "construction_cargo_reload":cargo_saved,"full_checkpoint_reload":full_reload,"scheduler_max_workers_per_tick":peak,
        "player_obstruction_recovery":interrupted&&obstruction.is_none(),
        "grounded_access_to_all_houses":walked,"housed_residents":housed,
        "housing_deficit":n.saturating_sub(housed),"housing_invalidation_and_reload":houses_complete,
        "housing_full_checkpoint_reload":housing_checkpoint_reload,
        "gameplay_and_physics_tick_ms":stats(ticks),"gameplay_tick_ms":stats(gameplay),"physics_tick_ms":stats(physics),"loose_drop_tick_ms":stats(loose_drops),
        "paid_cells_per_project":(0..initial.projects.len()).map(|i|site(&w).map_or(0,|s|s.built(&w,i))).collect::<Vec<_>>(),
        "limits":"Authored supplied site and six project plans; assigned work roles; local one-metre single-player; no offscreen or multiplayer authority claim"});
    std::fs::write(root.join("colony-check.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[city-colony-check] {result}");
    ensure!(passed,"Colony did not meet construction, participation or conservation targets");
    sidecar(&mut w,root,"finished.vxf")?;Ok(())
}

fn stats(mut values:Vec<f64>)->serde_json::Value {
    values.sort_by(f64::total_cmp);
    let at=|q:f64|values[((values.len()-1) as f64*q) as usize];
    serde_json::json!({"mean":values.iter().sum::<f64>()/values.len() as f64,
        "p50":at(0.5),"p95":at(0.95),"p99":at(0.99),"max":at(1.0)})
}

fn site(w:&World)->Result<&State> {w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing colony ledger")}
fn sidecar(w:&mut World,root:&Path,name:&str)->Result<()> {
    let before=ron::to_string(&w.farming.jobs)?;let bytes=crate::world::farming::sidecar::encode(w);
    std::fs::write(root.join(name),&bytes)?;
    let (f,_)=crate::world::farming::sidecar::decode(&bytes)?;
    ensure!(ron::to_string(&f.jobs)?==before,"Sidecar changed cargo, claims or project progress");w.farming=f;Ok(())
}
fn world_logs(w:&World,base:IVec3)->u32 {
    let mut n=0;for x in -24..=24 {for z in -24..=24 {for y in 0..=10 {
        n+=u32::from(w.get(base+IVec3::new(x,y,z))==B::Log);
    }}}n
}
