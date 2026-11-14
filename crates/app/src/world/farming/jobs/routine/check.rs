//! Disposable acceptance is invoked after paid colony construction.
use super::*;
mod reload;
mod negative;
mod home_profile;
mod materialize;
pub(crate) mod growth;
pub(crate) mod neighborhood;
pub(crate) mod services;
pub(crate) fn pure()->anyhow::Result<()> {
    anyhow::ensure!(!arrived(Vec3::ZERO,Vec3::X,1.0),"Routine accepted work interaction radius");
    let mut r=Resident::default(); r.enter(Phase::Rest,10); r.observe(10000,false,PULSE);
    anyhow::ensure!(r.dwell==0,"Empty destination accrued rest");
    r.observe(20000,true,PULSE); anyhow::ensure!(r.dwell==PULSE,"Unobserved time accrued rest");
    let encoded=ron::to_string(&r)?; let restored:Resident=ron::from_str(&encoded)?;
    anyhow::ensure!(restored==r,"Routine reload mismatch");
    Ok(())
}

use anyhow::{Context,Result,ensure};
use crate::{player::Input,simulation::{Budget,WorldSimulation,fixed},world::storage};
use std::{collections::{BTreeMap,HashSet},path::Path,time::Instant};

pub(crate) fn run(root:&Path)->Result<()> {
    pure()?;
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Daily check requires a disposable save root");
    let load=Instant::now();
    let loaded=storage::load_review_checkpoint()?;
    let (mut w,mut p,mut day)=(loaded.world,loaded.player,loaded.day_night);
    ensure!(w.farming.jobs.stockpile.is_some(),"Missing stockpile");
    let admitted_chunks=materialize::admit(&mut w)?;
    let load_ms=load.elapsed().as_secs_f64()*1000.0;
    let site=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing colony")?;
    ensure!(site.phase==site.projects.len(),"Daily check requires completed paid colony");
    let eligible:Vec<_>=site.households.iter().flat_map(|h|h.members.iter().copied()).collect();
    let identities=w.farming.jobs.production.as_ref().context("Missing production")?.workers.clone();
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut input=Input::default();let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    let mut game=Vec::new();let mut drop_cost=Vec::new();let mut finish=Vec::new();let mut sample_cost=Vec::new();
    let mut pending=Some(materialize::Pending::new(&w,0,"initial"));
    let mut materializations=Vec::new();
    let mut discontinuities=Vec::new();let mut excluded=0u32;
    let mut moved=BTreeMap::<u32,f32>::new();let mut previous=BTreeMap::<u32,Vec3>::new();
    negative::interruptions(&mut w)?;
    let mut full_reload=false;
    let mut route_reload=false;let mut rest_reload=false;let mut cargo_reload=false;
    let mut reload_ms=Vec::new();let mut min_live=identities.len();
    let before_food=food_balance(&w);
    let before_timber=timber_balance(&w);let mut grown=0u32;
    let roots:Vec<_>=w.farming.jobs.production.as_ref().context("Missing forestry")?.beds.iter()
        .filter(|b|b.crop==B::Sapling).map(|b|IVec3::from(b.at)).collect();
    let before_served=w.farming.jobs.production.as_ref().context("Missing meals")?.served.clone();
    let limit=std::env::var("VOXEL_DAILY_STEPS").ok().and_then(|s|s.parse::<usize>().ok()).unwrap_or(120000).clamp(120,600000);
    let full_duration=std::env::var("VOXEL_DAILY_FULL_DURATION").is_ok_and(|s|s=="1");
    let cycles=std::env::var("VOXEL_DAILY_CYCLES").ok().and_then(|s|s.parse::<u32>().ok()).unwrap_or(1).clamp(1,8);
    let started=Instant::now();
    for step in 0..limit {
        let growing:Vec<_>=roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        ensure!(sim.prepare_step(),"Simulation unavailable");
        let at=Instant::now();
        let mut drops=std::mem::take(&mut w.inventory.dropped);
        for drop in &mut drops {
            let next=drop.pos-Vec3::Y*(crate::simulation::STEP_SECONDS*2.5).min(0.1);
            if w.get(((next-Vec3::Y*0.12)/w.voxel_scale.meters()).floor().as_ivec3()).is_empty() {drop.pos=next;}
        }
        w.inventory.dropped=drops;
        drop_cost.push(at.elapsed().as_secs_f64()*1000.0);
        let at=Instant::now();
        let mut materialized=Ok(None);
        fixed::gameplay(&mut w,&mut p,&mut input,false,|hook,w,_,_| {
            if matches!(hook,fixed::Hook::BeforeAdventure)&&let Some(stage)=&mut pending {
                materialized=stage.observe(root,w,step);
            }
            fixed::Actions::default()
        });
        if let Some(report)=materialized? {materializations.push(report);pending=None;previous.clear();}
        for at in growing {if w.get(at)==B::Log&&!w.farming.plots.contains_key(&(at.x,at.y,at.z)){grown+=1;}}
        game.push(at.elapsed().as_secs_f64()*1000.0);
        let at=Instant::now();
        fixed::finish(&mut sim,&mut w,&p,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        finish.push(at.elapsed().as_secs_f64()*1000.0);
        blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        if pending.is_none()&&step%PULSE as usize==0 {
            let at=Instant::now();let mut live=HashSet::new();let mut current=BTreeMap::new();
            for &id in &identities {
                if let Some(a)=vehicle::villager_actor(&w,id).filter(|a|a.health>0.0) {
                    ensure!(live.insert(id),"Duplicate resident");
                    if let Some(last)=previous.get(&id) {
                        let distance=last.distance(a.pos);
                        if distance<3.0 { *moved.entry(id).or_default()+=distance; } else {
                            excluded+=1;
                            if discontinuities.len()<128 {discontinuities.push(serde_json::json!({"id":id,"step":step,"from":last.to_array(),"to":a.pos.to_array(),"distance":distance}));}
                        }
                    }
                    current.insert(id,a.pos);
                }
            }
            previous=current;
            ensure!(live.len()==identities.len(),"Active resident missing outside rematerialization stage");
            min_live=min_live.min(live.len());
            ensure!(w.farming.jobs.valid(),"Invalid routine or job state");
            let residents:Vec<_>=eligible.iter().filter_map(|&id|crew::view(&w.farming.jobs,id)).collect();
            let route=residents.iter().any(|a|matches!(a.task,Some(Task::Routine(_)))&&!a.path.is_empty());
            let rest=residents.iter().any(|a|a.routine.phase==Phase::Rest&&a.routine.dwell>0);
            let cargo=residents.iter().any(|a|a.carry.is_some());
            let checkpoint=(!route_reload&&route)||(!rest_reload&&rest)||(!cargo_reload&&cargo);
            if checkpoint {
                let at=Instant::now();
                let encoded=crate::world::farming::sidecar::encode(&w);
                let before=ron::to_string(&w.farming.jobs)?;
                let (restored,_)=crate::world::farming::sidecar::decode(&encoded)?;
                ensure!(ron::to_string(&restored.jobs)?==before,"Daily reload changed needs, goals or cargo");
                w.farming=restored;
                route_reload|=route;rest_reload|=rest;cargo_reload|=cargo;
                if rest&&!full_reload {
                    reload::full(root,&mut w,&mut p,&mut day)?;
                    sim.reset_for_world(w.voxel_scale);full_reload=true;
                    pending=Some(materialize::Pending::new(&w,step+1,"rest-reload"));previous.clear();
                }
                reload_ms.push(at.elapsed().as_secs_f64()*1000.0);
            }
            sample_cost.push(at.elapsed().as_secs_f64()*1000.0);
        }
        if step%12000==0 {
            write(root,&w,&eligible,&moved)?;
            println!("[daily-life] step={step} {}",summary(&w));
        }
        if !full_duration&&step>12000&&eligible.iter().all(|&id|crew::view(&w.farming.jobs,id)
            .is_some_and(|a|a.routine.rests>=cycles&&a.routine.socials>=cycles&&a.routine.resumed>0)) {break;}
    }
    write(root,&w,&eligible,&moved)?;
    let rows:Vec<_>=eligible.iter().filter_map(|&id|crew::view(&w.farming.jobs,id)).collect();
    let rested=rows.iter().filter(|a|a.routine.rests>=cycles).count();
    let social=rows.iter().filter(|a|a.routine.socials>=cycles).count();
    let resumed=rows.iter().filter(|a|a.routine.resumed>0).count();
    let prod=w.farming.jobs.production.as_ref().context("Lost production")?;
    let ate=eligible.iter().filter(|id|prod.served.get(id).copied().unwrap_or(0)>before_served.get(id).copied().unwrap_or(0)).count();
    let food=food_balance(&w)==before_food;
    let timber=timber_balance(&w)==before_timber+grown*16;
    let physically_moved=eligible.iter().filter(|id|moved.get(id).copied().unwrap_or(0.0)>=2.0).count();
    let passed=pending.is_none()&&excluded==0&&physically_moved==eligible.len()&&rested==eligible.len()&&social==eligible.len()&&resumed==eligible.len()&&ate==eligible.len()
        &&min_live==identities.len()&&food&&timber&&route_reload&&rest_reload&&full_reload;
    let steps=game.len();
    let result=serde_json::json!({"passed":passed,"steps":steps,"simulated_seconds":steps as f64*crate::simulation::STEP_SECONDS as f64,"required_cycles":cycles,"residents":identities.len(),"housed":eligible.len(),
        "unhoused":identities.len()-eligible.len(),"rested":rested,"socialized":social,"productive_after_social":resumed,
        "physical_meals":ate,"physically_moved":physically_moved,"excluded_discontinuities":excluded,"discontinuity_samples":discontinuities,"minimum_live":min_live,"food_conserved":food,"timber_conserved":timber,"grown_trees":grown,"route_sidecar_reload":route_reload,
        "rest_sidecar_reload":rest_reload,"full_rest_checkpoint_reload":full_reload,"obstruction_and_manual_interruptions":true,"cargo_sidecar_reload":cargo_reload,"elapsed_seconds":started.elapsed().as_secs_f64(),
        "initial_checkpoint_load_ms":load_ms,"admitted_chunks":admitted_chunks,"materialization_stages":materializations,"reload_ms":reload_ms,"gameplay_ms":stats(game),"finish_ms":stats(finish),
        "sampling_ms":stats(sample_cost),"drop_settle_ms":stats(drop_cost),"housing_demand":w.farming.jobs.quarter.as_ref().map(|q|q.housing_demand),
        "scope":"Existing paid houses and physical workers; no new construction, population, or multiplayer authority"});
    std::fs::write(root.join("daily-life-check.json"),serde_json::to_vec_pretty(&result)?)?;
    home_profile::run(root,&w)?;
    println!("[daily-life] passed={passed} meals={ate} rested={rested} social={social} resumed={resumed} live={min_live} jumps={excluded} food={food} timber={timber}");ensure!(passed,"Daily-life acceptance incomplete");Ok(())
}
fn food_balance(w:&World)->i64 {
    let j=&w.farming.jobs;let Some(s)=&j.production else{return 0;};
    let stock=j.stockpile.map(|p|w.block_data.chest(p.into())).unwrap_or_default();
    let held=|item|production::count(&stock,item)+crew::cargo_item(j,item)
        +w.inventory.dropped.iter().filter(|d|d.stack.item==item).map(|d|d.stack.count).sum::<u32>();
    use crate::world::inventory::Item;
    i64::from(held(Item::Block(B::CarrotCrop))+held(Item::Block(B::BeetCrop)))
        +2*i64::from(held(s.meal)+s.counts.eaten)+i64::from(s.counts.planted)-i64::from(s.counts.harvested)
}
fn write(root:&Path,w:&World,ids:&[u32],moved:&BTreeMap<u32,f32>)->Result<()> {
    let rows:Vec<_>=ids.iter().filter_map(|&id|crew::view(&w.farming.jobs,id).map(|a|serde_json::json!({
        "id":id,"saved_position":a.pos,"routine":a.routine,"reason":a.status,"feet":vehicle::villager_actor(w,id).map(|e|e.pos.to_array()),
        "path":a.path.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"cargo":a.carry,"distance_m":moved.get(&id)}))).collect();
    std::fs::write(root.join("daily-residents.json"),serde_json::to_vec_pretty(&rows)?)?;Ok(())
}
fn stats(mut values:Vec<f64>)->serde_json::Value {
    if values.is_empty(){return serde_json::json!({});}values.sort_by(f64::total_cmp);
    let at=|q:f64|values[((values.len()-1) as f64*q) as usize];
    serde_json::json!({"mean":values.iter().sum::<f64>()/values.len() as f64,"p95":at(0.95),"p99":at(0.99),"max":at(1.0)})
}

fn timber_balance(w:&World)->u32 {
    use crate::world::inventory::Item;
    let j=&w.farming.jobs;let Some(pile)=j.stockpile else{return 0;};
    let slots=w.block_data.chest(pile.into());let base=IVec3::from(pile)-IVec3::Y;
    let held=|item|production::count(&slots,item)+crew::cargo_item(j,item)
        +w.inventory.dropped.iter().filter(|d|d.stack.item==item).map(|d|d.stack.count).sum::<u32>();
    let published:HashSet<_>=w.sandbox.bodies.iter().filter(|b|b.felled_tree)
        .flat_map(|b|b.occupied.iter().map(|(at,_)|*at)).collect();
    let mut logs=0u32;
    for x in -24..=24 {for z in -24..=24 {for y in 0..=10 {
        let at=base+IVec3::new(x,y,z);logs+=u32::from(!published.contains(&at)&&w.get(at)==B::Log);
    }}}
    logs+=w.sandbox.bodies.iter().filter(|b|b.felled_tree).map(|b|b.voxels.iter().filter(|(_,b)|*b==B::Log).count() as u32).sum::<u32>();
    let paid=j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).into_iter().flat_map(|s|s.projects.iter())
        .flat_map(|p|p.cells.iter()).filter(|o|known(w,o.cell())==Some(o.block()))
        .map(|o|if o.block()==B::Planks {1}else if o.block()==B::Chest {8}else{0}).sum::<u32>();
    (logs+held(Item::Block(B::Log)))*4+held(Item::Block(B::Planks))+held(Item::Block(B::Chest))*8+paid
}
