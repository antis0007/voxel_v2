//! A vertical anchor monument with subordinate earthworks and service wings.
use crate::{WorldSampleConfig,BlockId as B};
use glam::{IVec3,Vec3};
use super::{X,Z,passenger_shaft,floor,console,workbench,smelter};
pub const VERSION:u32=65;
pub const SHAFT_HEIGHT:f32=256.0;
pub fn block(cfg:&WorldSampleConfig,p:IVec3,height:i32)->Option<B> {
    let v=cfg.voxel_meters();let m=(p.as_vec3()+Vec3::splat(0.5))*v;
    let q=m-Vec3::new(X,floor(cfg),Z);let r=q.x.hypot(q.z);
    let approach=q.x.abs()<24.0 && (95.0..278.0).contains(&q.z);
    if r>128.0 && !approach {return None;}
    let terrain=(height+1) as f32*v-floor(cfg);
    // The outer earthwork joins each natural column, including high slopes.
    // A broad cut-and-fill terrace replaces the old abrupt rectangular valley cut.
    let u=if approach {((q.z-210.0)/68.0).clamp(0.0,1.0)} else {((r-88.0)/40.0).clamp(0.0,1.0)};
    let longitudinal=u*u*(3.0-2.0*u);
    let edge=if approach {((q.x.abs()-14.0)/10.0).clamp(0.0,1.0)}else{0.0};
    let blend=1.0-(1.0-longitudinal)*(1.0-edge*edge*(3.0-2.0*edge));
    let grade=terrain*blend;
    if q.y<grade {
        if q.y<grade-8.0 && r>25.0 && q.y<terrain {return None;}
        return Some(if q.y>grade-2.0 {if r<88.0 || approach {B::StoneBricks}else{B::Grass}}
            else if r<25.0 {B::Basalt}else{B::Dirt});
    }
    if q.y>SHAFT_HEIGHT+2.0 {return Some(B::Air);}
    if approach || r>88.0 {return Some(B::Air);}
    if passenger_shaft(q.x, q.z) {
        return Some(B::Air);
    }
    if p==console(cfg) || p==workbench(cfg) {return Some(B::Workbench);}
    if p==smelter(cfg) {return Some(B::Smelter);}
    let a=q.z.atan2(q.x);let sector=(a/std::f32::consts::FRAC_PI_3).round()*std::f32::consts::FRAC_PI_3;
    let delta=a-sector;let radial=r*delta.cos();let lateral=r*delta.sin();
    // Six broad, curved buttresses resolve into deep sockets, not temporary legs.
    if q.y<136.0 {
        let t=(q.y/136.0).clamp(0.0,1.0);
        let centre=48.0-37.0*t.powf(0.62);
        let width=6.0-2.5*t;
        if (radial-centre).abs()<width && lateral.abs()<5.5-2.0*t {
            return Some(if lateral.abs()>3.0-0.8*t {B::Basalt}else{B::StoneBricks});
        }
        if q.y<6.0 && (radial-48.0).abs()<10.0 && lateral.abs()<9.0 {return Some(B::Basalt);}
    }
    // Annular anchor chamber with a generous axial entry. The central guide
    // well and flanking walkways expose the base of the tension machinery.
    if r>12.0 && r<23.0 && q.y<18.0 {
        let entrance=q.x.abs()<6.0 && q.y<11.0;
        if !entrance {return Some(if q.y>15.0 {B::StoneBricks}else{B::Basalt});}
    }
    if (24.0..SHAFT_HEIGHT).contains(&q.y) {
        let radius=18.0-q.y*0.043;
        if (radial-radius).abs()<2.4 && lateral.abs()<1.8 {return Some(B::StoneBricks);}
        // Deep collar floors bind the shaft, keeping the central cable well open.
        let collar=(q.y-24.0).rem_euclid(40.0)<2.0;
        if collar && r>7.0 && r<radius+3.0 {return Some(B::Basalt);}
        if q.y>244.0 && r>5.0 && r<11.0 {return Some(B::StoneBricks);}
    }
    // Low control/workshop wing. No roof or bridge crosses the arrival axis.
    let control=(-50.0.. -26.0).contains(&q.x) && (-22.0..40.0).contains(&q.z);
    if control {
        let wall=q.x< -48.0 || q.x> -28.0 || q.z< -20.0 || q.z>38.0;
        let door=q.x> -29.0 && q.z>22.0 && q.z<30.0 && q.y<4.0;
        let window=q.x> -29.0 && (2.0..5.0).contains(&q.y) && (q.z+16.0).rem_euclid(10.0)>2.0;
        if q.y<7.0 && wall && !door && !window {return Some(B::WeatheredBricks);}
        if (7.0..8.0).contains(&q.y) {return Some(B::StoneBricks);}
    }
    // A roofed cargo berth, recessed below the tower's foothills.
    let lamp=(Vec3::new(X+52.0,floor(cfg),Z+18.0)*cfg.inv_voxel_meters()).floor().as_ivec3();
    if p.x==lamp.x && p.z==lamp.z {
        if q.y<2.0 {return Some(B::Basalt);}
        if q.y<3.0 {return Some(B::Torch);}
    }
    let cargo=(28.0..58.0).contains(&q.x) && (-12.0..26.0).contains(&q.z);
    if cargo {
        let post=(q.x<31.0 || q.x>55.0) && (q.z< -9.0 || q.z>23.0);
        if post && q.y<9.0 {return Some(B::Basalt);}
        if (9.0..10.0).contains(&q.y) {return Some(B::StoneBricks);}
    }
    // Remains belong to one failed service annex, away from the functioning axis.
    if q.x>53.0 && q.x<71.0 && q.z< -29.0 && q.z> -48.0 {
        if q.y<2.0+((q.x+q.z) as i32).rem_euclid(4) as f32 {return Some(B::Cobblestone);}
    }
    // Supported route lights are deliberately subordinate to the shaft.
    for (x,z) in [(-13.0,66.0),(13.0,66.0),(-13.0,40.0),(13.0,40.0),(-28.0,32.0)] {
        if (q.x-x).abs()<v*0.6 && (q.z-z).abs()<v*0.6 {
            if q.y<2.0 {return Some(B::Basalt);}
            if q.y<3.0 {return Some(B::Torch);}
        }
    }
    Some(B::Air)
}
