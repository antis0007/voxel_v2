//! Disposable cave camera and generated-water comparison.
use crate::{player::Player, world::{World, BlockId as B}};
use glam::{IVec3, Vec3};

pub(super) fn cenote(w:&World,p:&mut Player,view:&str) {
    let cfg=w.sample_config();
    let s=cfg.voxel_meters();
    for z in -8..=8 { for x in -8..=8 {
        let Some(home)=engine_worldgen::karst::garden(&cfg,x,z) else {continue;};
        let pool=((home+Vec3::new(-4.0,0.0,0.0))/s).floor().as_ivec3();
        if engine_worldgen::sample_block(&cfg,pool)!=B::Water {continue;}
        let mut feet=home+Vec3::new(3.0,2.0,6.0);
        let mut target=home+Vec3::new(-3.0,0.8,0.0);
        if view=="cenote-entry" {
            let mouth=engine_worldgen::karst::entrance(&cfg,x,z).unwrap();
            feet=mouth+Vec3::new(3.0,3.0,3.0);
            target=mouth-Vec3::Y*5.0;
        }
        p.pos=feet/s;p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=true;
        let d=target-(feet+Vec3::Y*1.6);
        p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(d.x.hypot(d.z));
        println!("[cenote-review] view={view} garden_m={home:?} pool={pool:?} camera_m={feet:?}");
        return;
    }}
    println!("[cenote-review] no eligible pool found");
}

pub(super) fn inspect(w:&World,p:&mut Player) {
    let cfg=w.sample_config();
    let mut wet=[0usize;2];
    let mut cave_cells=0;
    let mut camera=None;
    for (index,version) in [72,cfg.generator_version].into_iter().enumerate() {
    let mut cfg=cfg;cfg.generator_version=version;
    for z in (-96..=96).step_by(12) { for x in (-96..=96).step_by(12) {
        let column=engine_worldgen::sample_column(&cfg,x,z);
        for y in cfg.min_world_y()+2..column.height-cfg.blocks_for_span(14.0) {
            let at=IVec3::new(x,y,z);
            if !engine_worldgen::cave_carves(&cfg,at,&column) {continue;}
            if index==1 {cave_cells+=1;}
            let after=engine_worldgen::sample_block_in_column(&cfg,at,column);
            wet[index]+=usize::from(after==B::Water);
            if index==1 && camera.is_none() && after==B::Air
                && crate::world::cosmos_at(&cfg,at.as_vec3()*cfg.voxel_meters())<0.001
                && engine_worldgen::sample_block_in_column(&cfg,at+IVec3::Y,column)==B::Air
                && engine_worldgen::sample_block_in_column(&cfg,at-IVec3::Y,column).is_solid() {
                camera=Some(at);
            }
        }
    }}}
    if let Some(at)=camera {
        p.pos=at.as_vec3()+Vec3::new(0.5,0.1,0.5);
        p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
        p.yaw=0.7;p.pitch=0.0;p.flying=true;
    }
    println!("[cave-review] cells={cave_cells} water_v72={} water_current={} camera={camera:?}",wet[0],wet[1]);
}
