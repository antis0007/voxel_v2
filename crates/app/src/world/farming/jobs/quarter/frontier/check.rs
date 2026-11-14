//! Fixed gameplay verifies paid expansion, interruptions and grounded access.
use super::*;
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub(in super::super) fn run(root:&Path)->Result<()> {
    if super::colony::enabled() {return super::colony::check::run(root);}
    if super::scene::building_check::enabled() {return super::scene::building_check::run(root);}
    std::fs::create_dir_all(root)?;
    super::movement::check(root)?;
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,424242,root);
    let mut p=crate::player::Player::new();
    install(&mut w,&mut p).map_err(anyhow::Error::msg)?;
    let initial=site(&w)?.clone();
    ensure!(initial.valid()&&initial.projects.len()==3&&initial.timber.len()==100,"Initial site ledger");
    ensure!(initial.projects.iter().all(|p|p.cells.iter().all(|o|known(&w,o.cell())==Some(B::Air))),"Prebuilt project cells");
    reload(&mut w,root,"initial.vxf")?;
    let mut input=crate::player::Input::default();
    let mut checkpointed=false;
    let mut obstructed=false;
    let mut obstruction:Option<(IVec3,u64)>=None;
    let mut last_phase=0;
    let mut phase_steps=Vec::new();
    let started=std::time::Instant::now();
    let mut peak_speed=0.0f32;
    let mut diagonal_motion=0u64;
    for step in 0..900_000 {
        let current=site(&w)?;
        if !obstructed&&current.phase==1&&w.farming.jobs.build.is_empty() {
            let prod=w.farming.jobs.production.as_ref().context("Production missing")?;
            let pile=w.farming.jobs.stockpile.context("Stockpile missing")?;
            if production::count(&w.block_data.chest(pile.into()),prod.meal)>=current.projects[1].food_reserve
                && prod.counts.eaten>=current.projects[1].meals_eaten {
                let at=current.projects[1].cells[0].cell();
                w.set(at,B::Stone);obstruction=Some((at,w.farming.clock));obstructed=true;
            }
        }
        let before:Vec<_>=w.adventure.enemies.iter().map(|e|e.pos).collect();
        crate::simulation::fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|crate::simulation::fixed::Actions::default());
        for (from,e) in before.into_iter().zip(&w.adventure.enemies) {
            let d=e.pos-from;let flat=d.x.hypot(d.z);
            peak_speed=peak_speed.max(flat/crate::simulation::STEP_SECONDS);
            if d.x.abs()>0.001&&d.z.abs()>0.001 {diagonal_motion+=1;}
        }
        w.adventure.dirty_voxels.clear();
        if let Some((at,clock))=obstruction {
            ensure!(known(&w,at)==Some(B::Stone),"Project erased a player obstruction");
            if w.farming.clock>=clock+120 {
                ensure!(w.farming.jobs.build.is_empty(),"Obstructed layer admitted work");
                reload(&mut w,root,"obstructed.vxf")?;
                w.set(at,B::Air);obstruction=None;
            }
        }
        if !checkpointed&&site(&w)?.phase==1&&w.farming.jobs.building_material&&w.farming.jobs.carry.is_some() {
            reload(&mut w,root,"carried-construction.vxf")?;checkpointed=true;
        }
        if step%120==0 {
            ensure!(w.farming.jobs.valid(),"Invalid commitments at step {step}");
            conservation(&w,&initial)?;
        }
        if site(&w)?.phase!=last_phase {
            last_phase=site(&w)?.phase;phase_steps.push((last_phase,w.farming.clock));
        }
        if step%12000==0 {
            println!("[city-site-check] step={step} phase={} status={} {}",site(&w)?.phase,
                w.farming.jobs.quarter.as_ref().map_or("",|q|q.status.as_str()),production::summary(&w).unwrap_or_default());
            if step>120000&&w.farming.jobs.build.len()<=3 {
                for o in &w.farming.jobs.build {
                    println!("[city-site-check] pending={o:?} ready={} unoccupied={}",o.ready(&w),super::super::super::construction::unoccupied(&w,*o,p.pos));
                    for (id,feet) in super::super::super::crew::saved_workers(&w) {
                        let task=super::super::super::Task::Build(*o);
                        println!("[city-site-check] worker={id} feet={feet:?} reachable={} route={}",
                            super::super::super::reachable(&w,feet,task,1.0),
                            super::super::super::path_with_limit(&w,feet,task,4096).len());
                    }
                }
            }
        }
        if site(&w)?.phase==initial.projects.len() {break;}
    }
    if site(&w)?.phase!=initial.projects.len() {
        std::fs::write(root.join("incomplete-jobs.ron"),ron::to_string(&w.farming.jobs)?)?;
        println!("[city-site-check] stock={:?}",w.block_data.chest(w.farming.jobs.stockpile.context("Stockpile missing")?.into()));
        for (id,feet) in super::super::super::crew::saved_workers(&w) {
            if let Some(a)=super::super::super::crew::view(&w.farming.jobs,id) {
                println!("[city-site-check] worker={id} feet={feet:?} task={:?} cargo={:?} status={}",a.task,a.carry,a.status);
            }
            for o in w.farming.jobs.build.iter().take(4) {
                println!("[city-site-check] order={o:?} ready={} clear={} route={}",o.ready(&w),
                    super::super::super::construction::unoccupied(&w,*o,p.pos),
                    super::super::super::path_with_limit(&w,feet,super::super::super::Task::Build(*o),4096).len());
            }
        }
    }
    ensure!(site(&w)?.phase==initial.projects.len(),"Expansion incomplete: phase={} status={}",site(&w)?.phase,
        w.farming.jobs.quarter.as_ref().map_or("",|q|q.status.as_str()));
    ensure!(checkpointed&&obstructed&&obstruction.is_none(),"Missing interruption or recovery check");
    ensure!(w.farming.jobs.valid(),"Invalid final commitments");
    conservation(&w,&initial)?;
    ensure!(peak_speed>2.0&&peak_speed<4.6&&diagonal_motion>100,"Resident movement speed or diagonal motion missing: {peak_speed} / {diagonal_motion}");
    for (i,project) in initial.projects.iter().enumerate() {
        ensure!(site(&w)?.built(&w,i)==project.cells.len(),"Incomplete physical project: {}",project.name);
    }
    walk(&w,&mut p)?;
    reload(&mut w,root,"finished.vxf")?;
    let result=serde_json::json!({"passed":true,"steps":w.farming.clock,"wall_seconds":started.elapsed().as_secs_f64(),
        "residents":super::super::super::crew::saved_workers(&w).len(),"projects":initial.projects.iter().map(|p|(&p.name,p.cells.len())).collect::<Vec<_>>(),
        "phase_steps":phase_steps,"input_logs":initial.timber.len(),"production":w.farming.jobs.production.as_ref().map(|p|&p.counts),
        "peak_walk_speed_mps":peak_speed,"diagonal_motion_ticks":diagonal_motion,
        "material_conservation":true,"construction_cargo_reload":checkpointed,"player_obstruction_recovery":obstructed,
        "initial_reload":true,"bounded_timber_queue":true,
        "limits":"Authored finite timber and serviced site; three residents; one-metre local simulation; no forestry, regional growth or multiplayer"});
    std::fs::write(root.join("site-check.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[city-site-check] PASS {result}");
    Ok(())
}

fn site(w:&World)->Result<&State> {
    w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing site ledger")
}

fn reload(w:&mut World,root:&Path,name:&str)->Result<()> {
    let before=ron::to_string(&w.farming.jobs)?;
    let bytes=crate::world::farming::sidecar::encode(w);
    std::fs::write(root.join(name),&bytes)?;
    if !w.farming.jobs.valid() {
        let j=&w.farming.jobs;
        std::fs::write(root.join(format!("{name}.ron")),&before)?;
        anyhow::bail!("Invalid live jobs at {name}: production={} quarter={} helpers={} identities={} claims={} state={j:?}",
            j.production.as_ref().is_none_or(production::State::valid),
            j.quarter.as_ref().is_none_or(super::super::State::valid),
            j.helpers.iter().all(super::super::super::crew::Worker::valid),
            super::super::super::crew::identities_valid(j),j.claims_valid());
    }
    let (loaded,_)=crate::world::farming::sidecar::decode(&bytes).with_context(||format!("Reload {name}"))?;
    ensure!(ron::to_string(&loaded.jobs)?==before&&loaded.jobs.valid(),"Reload changed obligations or cargo");
    w.farming=loaded;
    Ok(())
}

fn conservation(w:&World,initial:&State)->Result<()> {
    let jobs=&w.farming.jobs;
    let prod=jobs.production.as_ref().context("Production missing")?;
    let pile=jobs.stockpile.context("Stockpile missing")?;
    let slots=w.block_data.chest(pile.into());
    let held=|item|production::count(&slots,item)+super::super::super::crew::cargo_item(jobs,item);
    let logs=initial.timber.iter().filter(|p|known(w,IVec3::from(**p))==Some(B::Log)).count() as u32;
    let made=|name|prod.made.get(name).copied().unwrap_or(0);
    let built=|block|initial.projects.iter().flat_map(|p|&p.cells).filter(|o|o.block()==block&&known(w,o.cell())==Some(block)).count() as u32;
    ensure!((logs+held(Item::Block(B::Log)))*4+made("Oak planks")==initial.timber.len() as u32*4,"Timber units changed");
    ensure!(held(Item::Block(B::Planks))+built(B::Planks)+made("Chest")*8==made("Oak planks"),"Plank units changed");
    ensure!(held(Item::Block(B::Chest))+built(B::Chest)==made("Chest"),"Crafted containers changed");
    ensure!(held(prod.meal)+prod.counts.eaten==prod.counts.meals,"Meal units changed");
    Ok(())
}

fn walk(w:&World,p:&mut crate::player::Player)->Result<()> {
    let base=IVec3::from(w.farming.jobs.quarter.as_ref().context("Missing settlement")?.origin).as_vec3();
    p.pos=base+Vec3::new(0.5,1.0,11.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
    let points=[(0.5,1.0,1.5),(1.5,1.0,-0.5),(5.5,2.0,-0.5),(5.5,2.0,-4.5),
        (5.5,2.0,-0.5),(1.5,2.0,1.5),(5.5,2.0,1.5),(5.5,2.0,4.5),
        (5.5,2.0,1.5),(1.5,2.0,1.5),(0.5,2.0,2.5),(0.5,2.0,4.5),(-4.5,2.0,4.5),(-4.5,2.0,7.5)];
    let distance=super::super::walk::route(w,p,points.into_iter().map(|(x,y,z)|base+Vec3::new(x,y,z)).collect())?;
    ensure!(!p.flying&&p.pos.y>=base.y+1.9,"Grounded interior not reached");
    println!("[city-site-check] PASS grounded boardwalk and three interiors distance_m={distance:.2}");
    Ok(())
}
