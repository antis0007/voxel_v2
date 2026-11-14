//! Opt-in paid construction and grounded upper-floor acceptance.
use super::*;
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub(in super::super) fn enabled()->bool {
    std::env::var("VOXEL_CITY_MULTISTOREY").is_ok_and(|s|s=="1")
}

pub(in super::super) fn run(root:&Path)->Result<()> {
    std::fs::create_dir_all(root)?;
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter,424242,root);
    let mut p=crate::player::Player::new();
    install(&mut w,&mut p).map_err(anyhow::Error::msg)?;
    let initial=site(&w)?.clone();
    ensure!(initial.valid() && initial.projects.len()==3,"Invalid two-storey site");
    ensure!(initial.projects.iter().flat_map(|p|&p.cells).all(|o|known(&w,o.cell())==Some(B::Air)),"Prebuilt project");
    let before=ron::to_string(&w.farming.jobs)?;
    let (loaded,_)=crate::world::farming::sidecar::decode(&crate::world::farming::sidecar::encode(&w))?;
    ensure!(ron::to_string(&loaded.jobs)?==before,"Initial project reload changed");
    let mut input=crate::player::Input::default();
    let started=std::time::Instant::now();
    let mut cargo_reload=false;
    for step in 0..1_200_000 {
        crate::simulation::fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|crate::simulation::fixed::Actions::default());
        w.adventure.dirty_voxels.clear();
        if step%120==0 {
            ensure!(w.farming.jobs.valid(),"Invalid paid commitments at {step}");
            conservation(&w,&initial)?;
        }
        if !cargo_reload && site(&w)?.phase==1 && w.farming.jobs.building_material && w.farming.jobs.carry.is_some() {
            let before=ron::to_string(&w.farming.jobs)?;
            let (loaded,_)=crate::world::farming::sidecar::decode(&crate::world::farming::sidecar::encode(&w))?;
            ensure!(ron::to_string(&loaded.jobs)?==before,"Construction cargo reload changed");
            w.farming=loaded; cargo_reload=true;
        }
        if step%24_000==0 {println!("[city-multistorey] step={step} phase={} pending={}",site(&w)?.phase,w.farming.jobs.build.len());}
        if site(&w)?.phase==initial.projects.len() {break;}
    }
    ensure!(site(&w)?.phase==initial.projects.len(),"Two-storey paid construction incomplete");
    conservation(&w,&initial)?;
    let base=IVec3::from(w.farming.jobs.quarter.as_ref().context("Missing quarter")?.origin).as_vec3();
    p.pos=base+Vec3::new(2.5,2.0,2.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
    let points=[(2.5,2.0,3.5),(2.5,3.0,4.5),(2.5,4.0,5.5),(2.5,5.0,6.5),
        (4.5,5.0,6.5),(2.5,5.0,6.5),(-0.5,5.0,6.5),(-0.5,6.0,5.5),
        (-0.5,7.0,4.5),(-0.5,8.0,3.5),(4.5,8.0,3.5)];
    let distance=super::super::super::walk::route(&w,&mut p,points.into_iter()
        .map(|(x,y,z)|base+Vec3::new(x,y,z)).collect())?;
    ensure!(!p.flying && (p.pos.y-base.y-8.0).abs()<0.15,"Roof not reached while grounded");
    let home=initial.projects[1].home.as_ref().context("Missing upper home")?;
    ensure!(home.destination[1]==base.y as i32+5,"Upper home remained downstairs");
    let report=serde_json::json!({"passed":true,"ticks":w.farming.clock,"wall_seconds":started.elapsed().as_secs_f64(),
        "projects":initial.projects.iter().map(|p|(&p.name,p.cells.len())).collect::<Vec<_>>(),"input_logs":initial.timber.len(),
        "cargo_reload":cargo_reload,"conservation":true,"grounded_walk_m":distance,"supported_scale_m":1.0,
        "limits":"Three physical workers; one upper home destination; no functional market or trade district"});
    std::fs::write(root.join("multistorey-check.json"),serde_json::to_vec_pretty(&report)?)?;
    println!("[city-multistorey] PASS {report}");
    Ok(())
}

fn site(w:&World)->Result<&State> {
    w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing site")
}

fn conservation(w:&World,initial:&State)->Result<()> {
    let jobs=&w.farming.jobs;
    let prod=jobs.production.as_ref().context("Missing production")?;
    let pile=jobs.stockpile.context("Missing stockpile")?;
    let stock=w.block_data.chest(pile.into());
    let held=|item|production::count(&stock,item)+super::super::super::super::crew::cargo_item(jobs,item);
    let made=|name|prod.made.get(name).copied().unwrap_or(0);
    let built=|block|initial.projects.iter().flat_map(|p|&p.cells).filter(|o|o.block()==block&&known(w,o.cell())==Some(block)).count() as u32;
    let logs=initial.timber.iter().filter(|p|known(w,IVec3::from(**p))==Some(B::Log)).count() as u32;
    ensure!((logs+held(Item::Block(B::Log)))*4+made("Oak planks")==initial.timber.len() as u32*4,"Timber changed");
    ensure!(held(Item::Block(B::Planks))+built(B::Planks)+made("Chest")*8==made("Oak planks"),"Planks changed");
    ensure!(held(Item::Block(B::Chest))+built(B::Chest)==made("Chest"),"Chests changed");
    ensure!(held(prod.meal)+prod.counts.eaten==prod.counts.meals,"Meals changed");
    Ok(())
}
