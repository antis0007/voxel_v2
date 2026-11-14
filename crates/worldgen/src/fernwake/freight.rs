//! Freight hall with a powered front route and a lower maintenance loop.
use crate::{BlockId as B,WorldSampleConfig};
use glam::Vec3;
pub const VERSION:u32=85;
pub const CLERK:Vec3=Vec3::new(589.5,0.02,42.5);
pub const CONTROL:Vec3=Vec3::new(564.5,1.4,44.5);
pub const NOTICE:Vec3=Vec3::new(548.5,1.4,40.5);
pub const ARCHIVE:Vec3=Vec3::new(609.5,-6.6,79.5);
pub const DOOR_MIN:Vec3=Vec3::new(567.25,0.0,46.0);
pub const DOOR_MAX:Vec3=Vec3::new(567.75,4.0,50.0);
pub const SHUTTER_LIFT:f32=5.0;
pub const BENCH:Vec3=Vec3::new(589.5,0.5,38.5);
pub fn active(cfg:&WorldSampleConfig)->bool {cfg.generator_version>=VERSION && super::active(cfg)}
pub fn point(cfg:&WorldSampleConfig,p:Vec3)->Vec3 {p+Vec3::Y*crate::habitats_region::floor(cfg)}
pub fn refuge(cfg:&WorldSampleConfig,m:Vec3)->bool {
    active(cfg) && (538.0..602.0).contains(&m.x) && (35.0..61.0).contains(&m.z)
        && (-1.0..9.0).contains(&(m.y - crate::habitats_region::floor(cfg)))
}

fn room(p:Vec3,lo:Vec3,hi:Vec3)->Option<B> {
    if p.cmplt(lo-Vec3::ONE).any() || p.cmpge(hi+Vec3::ONE).any(){return None;}
    Some(if p.cmplt(lo).any() || p.cmpge(hi).any(){B::StoneBricks}else{B::Air})
}

pub fn sample(cfg:&WorldSampleConfig,m:Vec3)->Option<B> {
    if !active(cfg) || !(509.0..619.0).contains(&m.x) || !(25.0..91.0).contains(&m.z){return None;}
    let p=m-Vec3::Y*crate::habitats_region::floor(cfg);
    if !(-13.0..14.0).contains(&p.y){return None;}
    // The inspection channel joins the river already visible under the bridge.
    if ((510.0..515.0).contains(&p.x) && (47.0..88.0).contains(&p.z)
        || (510.0..577.0).contains(&p.x) && (83.0..88.0).contains(&p.z))
        && (-12.0..-9.0).contains(&p.y) {
        return Some(if p.y< -11.0 || p.z>=87.0
            || (p.x>=515.0 && p.z<84.0) || (p.x<515.0 && (p.x<511.0 || p.x>=514.0)) {B::StoneBricks}
            else if p.y< -9.0 {B::Water}else{B::Air});
    }
    // A broad doorway replaces only the far wall of the previous crossing.
    if (532.0..541.0).contains(&p.x) && (45.0..51.0).contains(&p.z) && (-1.0..6.0).contains(&p.y) {
        return Some(if p.y<0.0 || p.y>=5.0 || p.z<46.0 || p.z>=50.0 {B::Limestone}else{B::Air});
    }
    // Two quarter-turn stair runs join the hall to a lower inspection gallery.
    // Steps are 0.5 m high and 1.5 m deep before voxel rasterization.
    for x in [551.0,592.0] {
        if (x-2.0..x+3.0).contains(&p.x) && (56.0..87.0).contains(&p.z) {
            let floor=-((p.z-59.0).max(0.0)/1.5).floor().min(16.0)*0.5;
            if (floor-1.0..floor+5.0).contains(&p.y) {
                return Some(if p.x<x-1.0 || p.x>=x+2.0 || p.y<floor || p.y>=floor+4.0 {B::Limestone}
                    else if (p.z as i32)%6==0 && p.x<x && (floor+2.0..floor+3.0).contains(&p.y) {B::Torch}
                    else {B::Air});
            }
        }
    }
    if (549.0..595.0).contains(&p.x) && (82.0..89.0).contains(&p.z) && (-10.0..-3.0).contains(&p.y) {
        return Some(if p.z<83.0 || p.z>=88.0 || p.y>=-4.0 {B::StoneBricks}
            else if p.y< -8.0 {if (571.0..576.0).contains(&p.x){B::Water}else{B::StoneBricks}}
            else {B::Air});
    }
    // A side archive is reached from the service gallery, not by a key flag.
    if (593.0..609.0).contains(&p.x) && (83.0..87.0).contains(&p.z) && (-9.0..-4.0).contains(&p.y) {
        return Some(if p.y< -8.0 || p.y>=-5.0 || p.z<84.0 || p.z>=86.0 {B::StoneBricks}else{B::Air});
    }
    if let Some(shell)=room(p,Vec3::new(604.0,-8.0,73.0),Vec3::new(617.0,-2.0,87.0)) {
        if shell!=B::Air {return Some(shell);}
        if p.x>=614.0 && p.y< -4.0 {return Some(B::Bookcase);}
        if (609.0..610.0).contains(&p.x) && (79.0..80.0).contains(&p.z) && p.y< -7.0 {return Some(B::CarvedStone);}
        if p.z<75.0 && p.y< -7.0 {return Some(B::GlazedTiles);}
        if p.x<605.0 && p.z<74.0 && p.y>=-5.0 {return Some(B::Crystal);}
        return Some(B::Air);
    }
    let shell=room(p,Vec3::new(540.0,0.0,36.0),Vec3::new(602.0,11.0,61.0))?;
    if shell!=B::Air {return Some(shell);}
    if (567.0..568.0).contains(&p.x) {
        return Some(if (46.0..50.0).contains(&p.z) && p.y<4.0 {
            if (DOOR_MIN.x..DOOR_MAX.x).contains(&p.x) {B::IronBlock}else{B::Air}
        }else if (46.0..50.0).contains(&p.z) && (5.0..9.0).contains(&p.y) {B::Air}
        else{B::StoneBricks});
    }
    if (568.0..569.0).contains(&p.x) && (46.0..50.0).contains(&p.z) && (5.0..9.0).contains(&p.y) {return Some(B::StoneBricks);}
    // Tall piers frame a legible central freight lane; counters face side aisles.
    if [543.0,555.0,578.0,599.0].iter().any(|x|(p.x-x).abs()<0.7)
        && ((p.z-37.5).abs()<0.7 || (p.z-59.5).abs()<0.7) {return Some(B::Limestone);}
    if p.x<563.0 && [548.0,555.0].iter().any(|x|(p.x-x).abs()<2.0)
        && (40.0..41.0).contains(&p.z) && p.y<1.0 {return Some(B::Planks);}
    if (564.0..565.0).contains(&p.x) && (44.0..45.0).contains(&p.z) && p.y<1.0 {return Some(B::CopperBlock);}
    if (588.0..592.0).contains(&p.x) && (38.0..39.0).contains(&p.z) && p.y<1.0 {return Some(B::Planks);}
    // Occupied receiving bay: a bunk, books, and salvage kept outside the lane.
    if (584.0..588.0).contains(&p.x) && (37.0..39.0).contains(&p.z) && p.y<1.0 {return Some(B::GlazedTiles);}
    if (596.0..599.0).contains(&p.x) && p.z<39.0 && p.y<3.0 {return Some(B::Bookcase);}
    if (579.0..582.0).contains(&p.x) && (55.0..58.0).contains(&p.z) && p.y<2.0 {return Some(B::CopperBlock);}
    // The former conveyor is now a footpath; its copper stripe leads to the door.
    if (47.0..49.0).contains(&p.z) && p.y<0.25 {return Some(B::GlazedTiles);}
    for x in [545.0,560.0,575.0,597.0] {
        if (p.x-x).abs()<0.5 && (p.z-54.5).abs()<0.5 && (3.0..4.0).contains(&p.y) {return Some(B::Torch);}
    }
    Some(B::Air)
}
