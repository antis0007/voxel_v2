//! Camera/material diagnostic for native vista captures; never opens a save.
use engine_worldgen::{WorldSampleConfig,WorldType};
use engine_core::VoxelScalePreset;
use glam::Vec3;
fn main(){
    let cfg=WorldSampleConfig{ planet: None,generator_version:std::env::args().nth(1).and_then(|v|v.parse().ok()).unwrap_or(97),seed:424242,world_type:WorldType::Hills,voxel_scale:VoxelScalePreset::OneMeter};
    let(eye,target)=engine_worldgen::vistas::review_view(&cfg,"arch").expect("arch camera");
    let forward=(target-eye).normalize();let right=forward.cross(Vec3::Y).normalize();let up=right.cross(forward);
    let half=35.0_f32.to_radians().tan();
    for(x,y)in[(800.0,720.0),(600.0,720.0),(1000.0,705.0),(775.0,690.0)]{
        let direction=(forward+right*((x/1600.0*2.0-1.0)*half*1600.0/900.0)+up*((1.0-y/900.0*2.0)*half)).normalize();
        for i in 0..1200 {
            let point=eye+direction*(i as f32*0.25);
            let b=engine_worldgen::sample_block(&cfg,point.floor().as_ivec3());
            if b.is_solid(){
                let p=point.floor().as_ivec3();
                let column=engine_worldgen::sample_column(&cfg,p.x,p.z);
                println!("pixel=({x},{y}) distance_m={} point={point:?} block={b:?} site={:?} cavern={:?} vista={:?}",i as f32*0.25,
                    engine_worldgen::sites::block(&cfg,p,column.height),engine_worldgen::sites::cavern(&cfg,p,column.height),engine_worldgen::vistas::block(&cfg,p,column));break;
            }
        }
    }
}
