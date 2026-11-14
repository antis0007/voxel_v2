//! Top-down and side ASCII of the arch site in local (a,b) metres; diagnostics only.
use engine_worldgen::{WorldSampleConfig,WorldType};
use engine_core::VoxelScalePreset;
fn main(){
    let version:u32=std::env::args().nth(1).and_then(|v|v.parse().ok()).unwrap_or(96);
    let cfg=WorldSampleConfig{ planet: None,generator_version:version,seed:424242,world_type:WorldType::Hills,voxel_scale:VoxelScalePreset::OneMeter};
    let(o,ax,bx)=engine_worldgen::vistas::review_frame(&cfg).expect("arch site");
    let top=|a:f32,b:f32|{let p=o+ax*a+bx*b;
        let mut y=200;while y>-20&&!engine_worldgen::sample_block(&cfg,glam::IVec3::new(p.x.floor()as i32,y,p.z.floor()as i32)).is_solid(){y-=1;}y as f32-o.y};
    println!("height above site base (m), rows b=-70..70 step 4, cols a=-70..30 step 3; '.'=<5 digit=h/5");
    for bi in (-70..=70).step_by(4){
        let mut line=String::new();
        for ai in (-70..=30).step_by(3){
            let h=top(ai as f32,bi as f32);
            line.push(if h<5.0{'.'}else{char::from_digit(((h/5.0)as u32).min(35),36).unwrap()});
        }
        println!("b={bi:4} {line}");
    }
    println!("side view along b at a=-29 centre: rows y 0..50 step 2, cols b=-60..60 step 2 ('#' solid)");
    for yi in (0..=50).rev().step_by(2){
        let mut line=String::new();
        for bi in (-60..=60).step_by(2){
            let p=o+ax*-29.0+bx*(bi as f32);
            let s=engine_worldgen::sample_block(&cfg,glam::IVec3::new(p.x.floor()as i32,(o.y+yi as f32).floor()as i32,p.z.floor()as i32)).is_solid();
            line.push(if s{'#'}else{' '});
        }
        println!("y={yi:3} {line}");
    }
}
