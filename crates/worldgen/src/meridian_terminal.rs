//! First braid ground exchange. Real editable blocks own circulation and cover;
//! the app adds thin fittings. Version gated to preserve existing worlds.
use crate::{WorldSampleConfig,WorldType,terrain_height};
use engine_content::BlockId as B;
use glam::{IVec3,Vec3};
pub const VERSION:u32=64;
pub mod monument;
pub mod earthport;
pub const X:f32=1100.0;
pub const Z:f32=-900.0;
pub const PASSENGER_SHAFT_HALF_M: f32 = 4.0;
pub const PASSENGER_GUIDE_SIDE_M: f32 = 5.0;

fn passenger_shaft(x: f32, z: f32) -> bool {
    x.abs() < PASSENGER_SHAFT_HALF_M && (-PASSENGER_GUIDE_SIDE_M..PASSENGER_SHAFT_HALF_M).contains(&z)
}
thread_local! {static DATUM:std::cell::Cell<Option<(WorldSampleConfig,f32)>>=const {std::cell::Cell::new(None)};}
pub fn floor(cfg:&WorldSampleConfig)->f32 {
    DATUM.with(|cache| {
        if let Some((key,y))=cache.get() {if key==*cfg {return y;}}
        let s=cfg.inv_voxel_meters();
        let terrain=(terrain_height(cfg,(X*s).floor() as i32,(Z*s).floor() as i32)+1) as f32*cfg.voxel_meters();
        // One datum owns the editable tower, its fittings, boarding berth and
        // freight anchors. Ocean sites need a dry landing, not a seabed hall.
        let sea=cfg.world_y(cfg.water_level()+1);
        let y=terrain.max(sea)+2.0;
        cache.set(Some((*cfg,y)));y
    })
}
pub fn console(cfg:&WorldSampleConfig)->IVec3 {
    let (x,z)=if cfg.generator_version>=monument::VERSION {(-31.0,26.0)}else{(-12.0,10.0)};
    cell(cfg,x,z)
}
fn cell(cfg:&WorldSampleConfig,x:f32,z:f32)->IVec3 {(Vec3::new(X+x,floor(cfg),Z+z)*cfg.inv_voxel_meters()).floor().as_ivec3()}
pub fn workbench(cfg:&WorldSampleConfig)->IVec3 {if cfg.generator_version>=monument::VERSION {cell(cfg,-40.0,-6.0)}else{cell(cfg,-30.0,-7.0)}}
pub fn smelter(cfg:&WorldSampleConfig)->IVec3 {if cfg.generator_version>=monument::VERSION {cell(cfg,-40.0,-12.0)}else{cell(cfg,-30.0,-12.0)}}
pub fn enabled(cfg:&WorldSampleConfig)->bool {cfg.generator_version>=VERSION && cfg.world_type!=WorldType::Superflat}
pub fn block(cfg:&WorldSampleConfig,p:IVec3,height:i32)->Option<B> {
    if !enabled(cfg) {return None;}
    if cfg.generator_version>=earthport::VERSION {return earthport::block(cfg,p,height);}
    if cfg.generator_version>=monument::VERSION {return monument::block(cfg,p,height);}
    let m=(p.as_vec3()+Vec3::splat(0.5))*cfg.voxel_meters();
    let x=m.x-X;let z=m.z-Z;
    if x.abs()>52.0 || !(-46.0..70.0).contains(&z) {return None;}
    let y=m.y-floor(cfg);
    if !(-36.0..64.0).contains(&y) {return None;}
    let v=cfg.voxel_meters();
    if z>=42.0 {
        if x.abs()>8.0 {return None;}
        let top=-((z-42.0)/2.0).floor()-v;
        return if y<top-v*2.0 {None} else if y<top+v {Some(B::StoneBricks)} else {Some(B::Air)};
    }
    if y<0.0 {return Some(if y> -2.0 {B::StoneBricks} else {B::Basalt});}
    if passenger_shaft(x, z) {
        return Some(B::Air);
    }
    if p==console(cfg) {return Some(B::Workbench);}
    let local_cell=|dx:f32,dz:f32| (Vec3::new(X+dx,floor(cfg),Z+dz)*cfg.inv_voxel_meters()).floor().as_ivec3();
    if p==local_cell(-30.0,-12.0) {return Some(B::Smelter);}
    if p==local_cell(-30.0,-7.0) {return Some(B::Workbench);}
    let bay=(z+36.0).rem_euclid(18.0);
    let pier=bay<3.0;
    let damaged=x>0.0 && z< -12.0;
    let crown=if damaged {28.0+(z+42.0)*0.45} else {52.0};
    if x.abs()>35.0 && x.abs()<44.0 && pier && y<crown {
        if x.abs()>40.0-y*0.06 || y<6.0 {return Some(B::Limestone);}
        return Some(B::Basalt);
    }
    if pier && y>43.0 && y<48.0 && !damaged && (z>= -12.0 || x< -12.0) {return Some(B::StoneBricks);}
    if (1.0..2.0).contains(&bay) && (x.abs()-33.5).abs()<v*0.6 && z<10.0 && !damaged {
        if (13.0..14.0).contains(&y) {return Some(B::Torch);}
        if (10.0..13.0).contains(&y) {return Some(B::Basalt);}
    }
    if x.abs()>20.0 && x.abs()<35.0 && z<22.0 {
        if (9.0..10.0).contains(&y) && !(damaged && z< -22.0) {return Some(B::StoneBricks);}
        let stair=if z>=10.0 {((22.0-z)/1.3).floor().clamp(0.0,9.0)} else { -1.0 };
        if stair>=0.0 && x.abs()<24.0 && y<stair+v {return Some(B::StoneBricks);}
        let wall=(x.abs()-34.0).abs()<0.7 || (z+40.0).abs()<0.7;
        let window=(z+35.0).rem_euclid(9.0)>2.0;
        if wall && y<19.0 && !(y>11.0 && y<16.0 && window) && !(z>10.0 && x.abs()<35.0) {
            if !damaged || z> -24.0 || y<5.0 {return Some(B::Limestone);}
        }
        if (19.0..20.0).contains(&y) && !damaged {return Some(B::StoneBricks);}
    }
    if damaged && x>26.0 && z< -20.0 && y<3.0+(x as i32+z as i32).rem_euclid(5) as f32 {
        return Some(B::Cobblestone);
    }
    if (1.0..2.0).contains(&bay) && (x.abs()-34.0).abs()<v && (3.0..4.0).contains(&y) {return Some(B::Torch);}
    if pier && (x.abs()-34.0).abs()<v && y<3.0 {return Some(B::Basalt);}
    if y<v && x.abs()>44.0 && ((x+z) as i32).rem_euclid(11)<4 {return Some(B::GroveMoss);}
    Some(B::Air)
}
