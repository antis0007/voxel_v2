//! Native route checks cover open diagonals, corners and fine-cell bodies.
use super::*;
use anyhow::{Result,ensure};
use crate::fluid::VoxelScalePreset;
use std::path::Path;

pub(super) fn check(root:&Path)->Result<()> {
    for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
        let mut w=World::with_seed_in(crate::world::WorldType::Superflat,scale,424242,root);
        let v=scale.meters();let factor=v.recip() as i32;
        let y=engine_worldgen::terrain_height(&w.sample_config(),0,0)+1;
        let from=IVec3::new(-3*factor,y,-3*factor);
        let to=from+IVec3::new(1,0,1);
        let mut edits=Vec::new();
        for x in -5*factor..=6*factor {for z in -5*factor..=6*factor {
            edits.push((IVec3::new(x,y-1,z),B::Dirt));
            for h in 0..3*factor {edits.push((IVec3::new(x,y+h,z),B::Air));}
        }}
        w.set_many(&edits);
        ensure!(super::super::super::navigation::diagonal_clear(&w,from,to,v),"Open diagonal rejected at {v} m");
        for corner in [IVec3::new(to.x,y,from.z),IVec3::new(from.x,y,to.z)] {
            w.set(corner,B::Stone);
            ensure!(!super::super::super::navigation::diagonal_clear(&w,from,to,v),"Corner cut at {v} m");
            w.set(corner,B::Air);
        }
        w.set(to+IVec3::Y,B::Stone);
        ensure!(!super::super::super::navigation::diagonal_clear(&w,from,to,v),"Head collision at {v} m");
        w.set(to+IVec3::Y,B::Air);
        ensure!(!super::super::super::navigation::diagonal_clear(&w,from,to+IVec3::Y,v),"Diagonal climb admitted");
        if scale==VoxelScalePreset::OneMeter {
            let goal=IVec3::new(4,y,4);w.set(goal,B::Chest);
            let start=super::super::super::metres(from,v);
            let task=super::super::super::Task::Store(goal);
            let route=super::super::super::path_with_limit(&w,start,task,4096);
            ensure!(!route.is_empty(),"Open field route missing");
            let mut point=start;let mut diagonals=0;let mut diagonal_metres=0.0;
            for next in route {
                let a=(point/v).floor().as_ivec3();let b=(next/v).floor().as_ivec3();
                if a.x!=b.x&&a.z!=b.z {
                    diagonals+=1;
                    diagonal_metres+=(next-point).length();
                    ensure!(super::super::super::navigation::diagonal_clear(&w,a,b,v),"Unsafe diagonal in route");
                }
                point=next;
            }
            ensure!(diagonals>0&&diagonal_metres>=5.0&&super::super::super::reachable(&w,point,task,v),"Route did not use the open diagonal");
            println!("[city-site-check] PASS actual diagonal route steps={diagonals}");
            let pile=IVec3::new(0,y,0);let bench=pile+IVec3::Z;
            w.set(pile,B::Chest);w.set(bench,B::Workbench);w.set(bench+IVec3::Z,B::Planks);
            let top=super::super::super::navigation::work_point(&w,pile+IVec3::Y,v)
                .ok_or_else(||anyhow::anyhow!("Missing chest support"))?;
            ensure!((top.y-(y as f32+0.875)).abs()<0.001,"Chest treated as a full cube");
            let start=top+Vec3::new(0.0,0.0,-0.08);
            let task=super::super::super::Task::Produce(production::Task::Craft(bench,0));
            let route=super::super::super::path_with_limit(&w,start,task,4096);
            let goal=route.back().copied().ok_or_else(||anyhow::anyhow!("No route from chest to workbench"))?;
            ensure!([-0.1,0.1].into_iter().all(|x|[-0.1,0.1].into_iter().all(|z|
                super::super::super::reachable(&w,goal+Vec3::new(x,0.0,z),task,v))),"Unusable chest work endpoint");
            println!("[city-site-check] PASS shaped chest support and reachable crafting endpoint");
        }
    }
    println!("[city-site-check] PASS diagonal clearance at 1 m, 0.5 m, 0.25 m; blocked corners, headroom and diagonal climbs");
    Ok(())
}
