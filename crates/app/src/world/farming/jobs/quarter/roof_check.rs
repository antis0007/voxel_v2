//! Permanent courtyard gates preserve construction and grounded roof egress.
use super::*;
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path};
use crate::fluid::VoxelScalePreset as Scale;

pub(super) fn matrix(root: &Path) -> Result<()> {
    let styles=traditions().map_err(anyhow::Error::msg)?;
    let mut routes=Vec::new();
    for (style,row) in styles.iter().enumerate() {for levels in 1..=3 {for turn in [false,true] {
        let old=Lot {origin:[-64,18,-64],tradition:style,complexity:levels,turn,layer:0,
            occupied:false,market_cover:false,frontage:true,variant:7,roof_access:false};
        let encoded=ron::to_string(&old)?;
        ensure!(!encoded.contains("roof_access"),"Legacy lot serialization changed");
        ensure!(!ron::from_str::<Lot>(&encoded)?.roof_access,"Legacy lot gained a roof gate");
        let mut lot=old.clone();lot.roof_access=true;
        let original:BTreeMap<_,_>=geometry::plan(&old,row).into_iter().map(|(p,b)|(p.to_array(),b)).collect();
        let current:BTreeMap<_,_>=geometry::plan(&lot,row).into_iter().map(|(p,b)|(p.to_array(),b)).collect();
        let gate=IVec3::new(geometry::WIDTH-1,i32::from(levels)*4+1,geometry::DEPTH-i32::from(levels)*4);
        let removed=usize::from(row.form==Roof::Courtyard);
        ensure!(original.len()==current.len()+removed,"Roof gate changed another material cell");
        ensure!(current.iter().all(|(p,b)|original.get(p)==Some(b)),"Roof gate added or changed material");
        if removed==1 {ensure!(original.contains_key(&gate.to_array())&&!current.contains_key(&gate.to_array()),"Gate not aligned with exterior stair");}
        for scale in [Scale::OneMeter,Scale::HalfMeter,Scale::QuarterMeter] {
            let factor=scale.meters().recip() as i32;
            let before=geometry::plan_for_scale(&old,row,scale);
            let after=geometry::plan_for_scale(&lot,row,scale);
            ensure!(before.len()==after.len()+removed*factor.pow(3) as usize,"Scaled roof gate bill changed");
            if removed==1 {
                let blocked:std::collections::BTreeSet<_>=after.iter().map(|(p,_)|p.to_array()).collect();
                let sign=if turn {-1}else{1};
                for x in 0..factor {for z in 0..factor {for y in 0..factor*2 {
                    ensure!(!blocked.contains(&(gate*factor+IVec3::new(sign*x,y,sign*z)).to_array()),"Scaled roof gate lacks headroom");
                }}}
            }
        }
        if row.form!=Roof::Courtyard {continue;}
        let mut w=World::with_seed_in(crate::world::WorldType::Superflat,Scale::OneMeter,7,
            &root.join(format!("roof-{levels}-{turn}")));
        let y=engine_worldgen::terrain_height(&w.sample_config(),-64,-64)+2;
        lot.origin=[-64,y,-64];
        let mut cells=Vec::new();
        for x in -3..=13 {for z in -4..=18 {
            cells.push((geometry::at(&lot,IVec3::new(x,-1,z)),B::Stone));
        }}
        cells.extend(geometry::plan(&lot,row).into_iter().map(|(p,b)|(geometry::at(&lot,p),b)));
        w.set_many(&cells);
        let distance=roof_round_trip(&w,&lot,&mut crate::player::Player::new())?;
        routes.push(serde_json::json!({"levels":levels,"turned":turn,"walk_m":distance}));
    }}}
    let legacy=layout::generate(7,0,1,1,&layout::scene::height).map_err(anyhow::Error::msg)?;
    ensure!(!ron::to_string(&legacy)?.contains("roof_access"),"Legacy parcel serialization changed");
    if let Ok(path)=std::env::var("VOXEL_CITY_ROOF_LEGACY_CHECKPOINT") {
        let (loaded,_)=crate::world::farming::sidecar::decode(&std::fs::read(path)?)?;
        ensure!(loaded.jobs.valid(),"Legacy courtyard checkpoint invalid");
        let q=loaded.jobs.quarter.as_ref().context("Legacy courtyard state missing")?;
        ensure!(q.lots.iter().all(|l|!l.roof_access),"Existing courtyard acquired a new bill");
        ensure!(q.layout.as_ref().is_some_and(|p|p.parcels.iter().all(|p|!p.roof_access)),"Existing parcels changed");
        println!("[city-roof] PASS old courtyard checkpoint retains its original geometry and bill");
    }
    std::fs::write(root.join("roof-access-matrix.json"),serde_json::to_vec_pretty(&routes)?)?;
    println!("[city-roof] PASS legacy geometry delta,54 scaled bills/headroom checks and6 grounded roof round trips");
    Ok(())
}

fn point(lot:&Lot,p:IVec3)->Vec3 {geometry::at(lot,p).as_vec3()+Vec3::new(0.5,0.0,0.5)}

fn grounded(w:&World,p:&mut crate::player::Player,target:Vec3)->Result<f32> {
    let mut distance=0.0;
    for _ in 0..2400 {
        let delta=(target-p.pos)*Vec3::new(1.0,0.0,1.0);
        if delta.length()<0.05&&p.on_ground&&(p.pos.y-target.y).abs()<0.15 {
            ensure!(!p.flying,"Roof route used flight");return Ok(distance);
        }
        let mut input=crate::player::Input::default();
        if delta.length()>=0.04 {p.yaw=delta.x.atan2(-delta.z);input.forward=true;}
        let before=p.pos;
        crate::player::update_player(p,&mut input,w,crate::simulation::STEP_SECONDS);
        distance+=before.distance(p.pos);
    }
    anyhow::bail!("Grounded roof waypoint blocked: feet={:?} target={target:?}",p.pos)
}

fn roof_round_trip(w:&World,lot:&Lot,p:&mut crate::player::Player)->Result<f32> {
    let top=i32::from(lot.complexity)*4;
    let gate=geometry::at(lot,IVec3::new(geometry::WIDTH-1,top+1,geometry::DEPTH-top));
    ensure!(w.get(gate)==B::Air&&w.get(gate+IVec3::Y)==B::Air,"Permanent roof gate is obstructed");
    let start=point(lot,IVec3::new(9,0,16));
    p.pos=start;p.precise_pos=start.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
    let mut steps=vec![IVec3::new(9,0,15)];
    for i in 0..top {steps.push(IVec3::new(9,i+1,geometry::DEPTH-1-i));}
    for x in [8,7,6] {steps.push(IVec3::new(x,top+1,geometry::DEPTH-top));}
    let mut distance=0.0;
    for step in &steps {distance+=grounded(w,p,point(lot,*step))?;}
    ensure!((p.pos.y-start.y-(top+1) as f32).abs()<0.15,"Roof not reached");
    for step in steps.iter().rev().skip(1) {distance+=grounded(w,p,point(lot,*step))?;}
    distance+=grounded(w,p,start)?;
    ensure!((p.pos-start).length()<0.35,"Roof route did not return to ground");
    Ok(distance)
}

pub(super) fn completed(w:&World,p:&mut crate::player::Player,root:&Path)->Result<()> {
    let city=w.farming.jobs.quarter.as_ref().context("Missing completed quarter")?;
    let Some(lot)=city.lots.first().filter(|l|l.roof_access) else{return Ok(());};
    ensure!(lot.occupied&&city.completed==1,"Roof proof ran before paid completion");
    let distance=roof_round_trip(w,lot,p)?;
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing paid stockpile")?);
    let task=super::super::Task::Store(pile);
    let mut returns=Vec::new();
    for (id,feet) in super::super::crew::saved_workers(w) {
        let path=super::super::path(w,feet,task);
        ensure!(super::super::reachable(w,feet,task,1.0)||!path.is_empty(),"Completed worker {id} has no stockpile return route");
        p.pos=feet;p.precise_pos=feet.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
        let mut walked=0.0;
        for target in path {walked+=grounded(w,p,target)?;}
        ensure!(!p.flying&&super::super::reachable(w,p.pos,task,1.0),"Completed worker {id} return route is blocked");
        returns.push(serde_json::json!({"worker":id,"start":feet.to_array(),"end":p.pos.to_array(),"walk_m":walked}));
    }
    std::fs::write(root.join("paid-roof-access.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"grounded_roof_round_trip_m":distance,"completed_worker_route_replays":returns,
        "scale_m":1.0,"permanent_gate":true}))?)?;
    println!("[city-roof] PASS paid roof round trip distance_m={distance:.2} and all completed-worker stockpile routes");
    Ok(())
}
