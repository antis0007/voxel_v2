//! CPU ray-march preview of a vista review pose to a PPM; diagnostics only, not a native capture.
//! Usage: vista_render <version> <view name|local> <out.ppm> [ea ey eb ta ty tb in site metres]
//! Env: VISTA_SEED=n (default 424242), VISTA_VOXEL=0.5|0.25 picks the voxel scale; VISTA_PULL=m backs the eye away and up by m.
use engine_worldgen::{WorldSampleConfig,WorldType};
use engine_content::BlockId;
use engine_core::VoxelScalePreset;
use glam::{IVec3,Vec3};
fn colour(b:BlockId)->Vec3{
    let h=(format!("{b:?}").bytes().fold(7u32,|a,c|a.wrapping_mul(31).wrapping_add(c as u32)))as f32;
    let r=(h*0.37).sin()*0.5+0.5;let g=(h*0.71).sin()*0.5+0.5;let bl=(h*0.13).sin()*0.5+0.5;
    match b{BlockId::Air=>Vec3::ZERO,BlockId::Water=>Vec3::new(0.2,0.4,0.7),BlockId::Limestone=>Vec3::new(0.78,0.74,0.62),
        BlockId::Sandstone=>Vec3::new(0.80,0.62,0.40),BlockId::Grass=>Vec3::new(0.30,0.55,0.22),BlockId::Sand=>Vec3::new(0.90,0.82,0.58),
        _=>Vec3::new(0.35+0.4*r,0.35+0.4*g,0.35+0.4*bl)}
}
fn main(){
    let a:Vec<String>=std::env::args().collect();
    let version:u32=a[1].parse().unwrap();
    let voxel_scale=match std::env::var("VISTA_VOXEL").as_deref(){Ok("0.5")=>VoxelScalePreset::HalfMeter,Ok("0.25")=>VoxelScalePreset::QuarterMeter,_=>VoxelScalePreset::OneMeter};
    let seed=std::env::var("VISTA_SEED").ok().and_then(|v|v.parse().ok()).unwrap_or(424242);
    let cfg=WorldSampleConfig{ planet: None,generator_version:version,seed,world_type:WorldType::Hills,voxel_scale};
    let vm=voxel_scale.meters();
    let(eye,target)=if a[2]=="local"{
        let f:Vec<f32>=a[4..10].iter().map(|v|v.parse().unwrap()).collect();
        let(o,ax,bx)=engine_worldgen::vistas::review_frame(&cfg).expect("site");
        let m=|a:f32,y:f32,b:f32|o+ax*a+bx*b+Vec3::Y*y;
        (m(f[0],f[1],f[2]),m(f[3],f[4],f[5]))
    }else{engine_worldgen::vistas::review_view(&cfg,&a[2]).expect("view")};
    let pull:f32=std::env::var("VISTA_PULL").ok().and_then(|v|v.parse().ok()).unwrap_or(0.0);
    let eye=eye+((eye-target)*Vec3::new(1.0,0.0,1.0)).normalize_or_zero()*pull+Vec3::Y*pull*0.35;
    let (w,h)=(480usize,270usize);
    let forward=(target-eye).normalize();let right=forward.cross(Vec3::Y).normalize();let up=right.cross(forward);
    let half=35.0_f32.to_radians().tan();
    let sun=Vec3::new(0.4,0.8,0.3).normalize();
    let mut out=Vec::with_capacity(w*h*3);
    for py in 0..h{for px in 0..w{
        let d=(forward+right*((px as f32/w as f32*2.0-1.0)*half*w as f32/h as f32)+up*(1.0-py as f32/h as f32*2.0)*half).normalize();
        let mut col=Vec3::new(0.55,0.72,0.92)*(0.8+0.2*d.y.max(0.0));
        let far=450.0+pull*1.5;
        let mut prev=(eye/vm).floor().as_ivec3();let mut t=0.0_f32;
        while t<far{
            t+=0.4*vm;let p=((eye+d*t)/vm).floor().as_ivec3();
            if p==prev{continue;}
            let b=engine_worldgen::sample_block(&cfg,p);
            if b.is_solid(){
                let n=(prev-p).as_vec3();
                let lit=0.45+0.55*n.dot(sun).max(0.0);
                let fog=(t/far).powf(1.5);
                col=(colour(b)*lit).lerp(Vec3::new(0.65,0.78,0.92),fog);
                break;
            }
            prev=p;
        }
        for c in [col.x,col.y,col.z]{out.push((c.clamp(0.0,1.0)*255.0)as u8);}
    }}
    let mut f=format!("P6\n{w} {h}\n255\n").into_bytes();f.extend(out);
    std::fs::write(&a[3],f).unwrap();
    let _=IVec3::ZERO;
}
