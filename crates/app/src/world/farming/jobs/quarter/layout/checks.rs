//! Layout invariants and grounded native traversal for review fixtures.
use super::*;
use anyhow::{ensure, Result};
use crate::{player::Player, world::{World, WorldType}, fluid::VoxelScalePreset};
use std::path::Path;

pub(in super::super) fn run(root:&Path)->Result<()> {
    if district::enabled() {return super::district_checks::run(root);}
    std::fs::create_dir_all(root)?;
    let profiles=profiles().map_err(anyhow::Error::msg)?;
    ensure!(ron::from_str::<Vec<Profile>>(&ron::to_string(&profiles)?)?==profiles,"Layout content round trip");
    let mut report=Vec::new();
    let mut digests=BTreeSet::new();
    let styles=traditions().map_err(anyhow::Error::msg)?;
    for profile in 0..3 {for seed in [7,424242,8675309] {
        for complexity in 1..=3 {
            let candidate=generate(seed,profile,profile as usize,complexity,&scene::height).map_err(anyhow::Error::msg)?;
            let lot=candidate.parcels[0].lot(IVec3::ZERO,true);
            ensure!(geometry::plan(&lot,&styles[lot.tradition]).iter().all(|&(p,_)|
                (geometry::at(&lot,p)-STOCKPILE).as_vec3().length_squared() <= super::super::super::RANGE.powi(2)),
                "Project exceeds the stockpile service radius");
        }
        let plan=generate(seed,profile,profile as usize,1,&scene::height).map_err(anyhow::Error::msg)?;
        ensure!(plan==generate(seed,profile,profile as usize,1,&scene::height).map_err(anyhow::Error::msg)?,"Nondeterministic plan");
        ensure!(plan==ron::from_str::<Plan>(&ron::to_string(&plan)?)?,"Plan provenance changed on reload");
        let paving=plan.paving();
        ensure!(plan.roads.iter().skip(1).take(6).zip(&plan.parcels).all(|(r,p)|r.first()==Some(&p.door())&&r.last()==Some(&[0,0])),"Unconnected frontage");
        for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
            let factor=scale.meters().recip() as i32;
            for (i,parcel) in plan.parcels.iter().enumerate() {
                // The real plan conversion is tested at a negative world offset too.
                let lot=parcel.lot(IVec3::new(-96,20,-64),i==0);
                let mut fine_lot=lot.clone(); fine_lot.origin=(IVec3::from(lot.origin)*factor).to_array();
                let cells=geometry::plan_for_scale(&fine_lot,&styles[lot.tradition],scale);
                let coarse=geometry::plan(&lot,&styles[lot.tradition]);
                let reference:std::collections::BTreeMap<_,_>=coarse.iter().map(|&(p,b)|(geometry::at(&lot,p).to_array(),b)).collect();
                ensure!(cells.len()==coarse.len()*factor.pow(3) as usize,"Scaled parcel volume changed");
                for &(p,b) in &cells {
                    let at=geometry::at(&fine_lot,p);
                    let parent=[at.x.div_euclid(factor),at.y.div_euclid(factor),at.z.div_euclid(factor)];
                    ensure!(reference.get(&parent)==Some(&b),"Fine cell escaped its rotated coarse footprint");
                    let x=at.x.div_euclid(factor)+96;
                    let z=at.z.div_euclid(factor)+64;
                    ensure!(!paving.contains(&[x,z]),"Street intersects parcel: profile={profile} seed={seed} parcel={i} at={x},{z}");
                }
            }
        }
        let text=ron::to_string(&plan)?;
        let fingerprint=text.bytes().fold(0xcbf29ce484222325u64,|h,b|(h^b as u64).wrapping_mul(0x100000001b3));
        ensure!(digests.insert(fingerprint),"Duplicate layouts");
        report.push(serde_json::json!({"profile":profiles[profile as usize].name,"seed":seed,
            "plan":plan,"diagonal_steps":plan.diagonal_steps(),"paved_square_metres":paving.len(),"fingerprint":format!("{fingerprint:016x}"),"scales_checked_m":[1.0,0.5,0.25]}));
    }}
    let flat=|_:Point|Some(0);
    let clear=|p:Point|inside(p);
    let (ordered,_)=route::find([-16,-12],[16,12],&profiles[0],&flat,&clear).map_err(anyhow::Error::msg)?;
    let (organic,_)=route::find([-16,-12],[16,12],&profiles[2],&flat,&clear).map_err(anyhow::Error::msg)?;
    let diagonal=|r:&[Point]|r.windows(2).filter(|s|s[0][0]!=s[1][0]&&s[0][1]!=s[1][1]).count();
    ensure!(diagonal(&ordered)<diagonal(&organic),"Alignment preference did not affect routing");
    let corridor=|p:Point|inside(p)&&(p[0]-p[1]).abs()<=1;
    let (deviation,_)=route::find([-10,-10],[10,10],&profiles[0],&flat,&corridor).map_err(anyhow::Error::msg)?;
    ensure!(diagonal(&deviation)>0,"Ordered culture forbids diagonals");
    let terrain=|p:Point| if p[0].abs()<=2&&p[1].abs()<=4 {None} else {Some(0)};
    let (detour,_)=route::find([-12,0],[12,0],&profiles[0],&terrain,&|p|inside(p)&&terrain(p).is_some()).map_err(anyhow::Error::msg)?;
    ensure!(detour.iter().any(|p|p[1].abs()>4),"Road crossed surveyed exclusion");
    let legacy:super::super::State=ron::from_str("(lots:[],housing_demand:4,residents:20,paused:true,completed:0,origin:(-96,20,-64))")?;
    ensure!(legacy.layout.is_none()&&legacy.paused&&legacy.origin==[-96,20,-64],"Legacy quarter changed");
    if let Ok(path)=std::env::var("VOXEL_CITY_LEGACY_CHECKPOINT") {
        let (loaded,_)=crate::world::farming::sidecar::decode(&std::fs::read(path)?)?;
        ensure!(loaded.jobs.quarter.as_ref().is_some_and(|q|q.layout.is_none()),"Legacy JOB1 changed");
        ensure!(loaded.jobs.worker.is_some(),"Legacy worker lost");
        println!("[city-layout] PASS legacy interrupted JOB1 file loaded");
    }
    std::fs::write(root.join("layout-check.json"),serde_json::to_vec_pretty(&report)?)?;
    println!("[city-layout] PASS 9 deterministic plans, 27 scale cases, 27 project service-radius cases, road/parcel separation, finite alignment preference, terrain detour, old-state defaults");
    Ok(())
}

pub(in super::super) fn native(w:&World,p:&mut Player)->Result<()> {
    let Some(q)=w.farming.jobs.quarter.as_ref() else{return Ok(());};
    let Some(plan)=&q.layout else{return Ok(());};
    let base=IVec3::from(q.origin);
    let mut distance=0.0;
    for (i,road) in plan.roads.iter().skip(1).take(6).enumerate() {
        p.pos=base.as_vec3()+super::super::Vec3::new(0.5,1.0,0.5);p.precise_pos=p.pos.as_dvec3();
        p.vel=super::super::Vec3::ZERO;p.flying=false;
        let mut points:Vec<_>=road.iter().rev().map(|&[x,z]|(base+IVec3::new(x,1+scene::height([x,z]).unwrap_or(0),z)).as_vec3()+super::super::Vec3::new(0.5,0.0,0.5)).collect();
        if i>0 {
            let lot=plan.parcels[i].lot(base,false);
            for z in [-2,3] {points.push(geometry::at(&lot,IVec3::new(4,1,z)).as_vec3()+super::super::Vec3::new(0.5,0.0,0.5));}
        }
        distance+=super::super::walk::route(w,p,points)?;
        ensure!(!p.flying,"Layout walk used flight");
    }
    p.pos=base.as_vec3()+super::super::Vec3::new(0.5,1.0,20.5);p.precise_pos=p.pos.as_dvec3();p.vel=super::super::Vec3::ZERO;
    println!("[city-layout] PASS grounded public-street walks profile={} seed={} frontages=6 existing-interiors=5 distance_m={distance:.2}",plan.profile,plan.seed);
    Ok(())
}

pub(in super::super) fn native_matrix(root:&Path)->Result<()> {
    if district::enabled() {return super::district_checks::native_matrix(root);}
    for profile in 0..3 {for seed in [7,424242,8675309] {
        let mut w=World::with_seed_in(WorldType::Superflat,VoxelScalePreset::OneMeter,seed,&root.join(format!("walk-{profile}-{seed}")));
        let base=IVec3::new(0,engine_worldgen::terrain_height(&w.sample_config(),0,0)+1,0);
        let plan=generate(seed,profile,profile as usize,1,&scene::height).map_err(anyhow::Error::msg)?;
        w.set_many(&scene::edits(&plan,base).map_err(anyhow::Error::msg)?);
        w.farming.jobs.quarter=Some(super::super::State {frontier:None,lots:vec![plan.parcels[0].lot(base,true)],layout:Some(plan),housing_demand:4,residents:20,paused:true,completed:0,origin:base.to_array(),status:String::new()});
        native(&w,&mut Player::new())?;
    }}
    Ok(())
}
