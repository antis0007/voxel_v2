//! Small, persistent bank dwellers. Never load terrain to create an animal.
use super::*;
use crate::{mesh::models::Model, world::chunk_coord_from_world_block};

pub(super) fn loaded(w:&World,p:Vec3)->bool {
    let s=w.voxel_scale.meters();
    [-2.0,2.0].into_iter().all(|x| [-2.0,2.0].into_iter().all(|z|
        [-2.0,3.0].into_iter().all(|y| w.chunks.contains_key(&chunk_coord_from_world_block(((p+Vec3::new(x,y,z))/s).floor().as_ivec3())))))
}

pub(super) fn spawn(a: &mut Adventure, world: &World, feet: Vec3, count: &mut usize) {
    if a.enemies.iter().filter(|e| e.kind == SALAMANDER).count() >= 4 { return; }
    let cfg = world.sample_config();
    let s = cfg.voxel_meters();
    let cx = (feet.x / 160.0).floor() as i32;
    let cz = (feet.z / 160.0).floor() as i32;
    for z in cz-1..=cz+1 { for x in cx-1..=cx+1 {
        let Some(home) = engine_worldgen::karst::garden(&cfg, x, z) else { continue; };
        if home.distance_squared(feet) > 48.0*48.0 { continue; }
        for slot in 2..4 {
            if *count >= ACTIVE_LIMIT || a.wildlife.residents.len() >= RECORD_LIMIT { return; }
            let key = (x,z,slot);
            if a.wildlife.residents.contains_key(&key) { continue; }
            let bank = home + Vec3::new(-4.0, 2.0, if slot == 2 { 5.0 } else { -5.0 });
            let top = (bank / s).floor().as_ivec3();
            for dy in 0..cfg.blocks_for_span(4.0) {
                let cell = top - IVec3::Y * dy;
                // Include the whole collision neighbourhood: World::get may read
                // disk for nonresident chunks, which must never stall a spawn.
                let loaded = [-2,2].into_iter().all(|dx| [-2,2].into_iter().all(|dz|
                    [0,2].into_iter().all(|dy| world.chunks.contains_key(&chunk_coord_from_world_block(cell+IVec3::new(dx,dy,dz))))));
                if !loaded { continue; }
                if !matches!(world.get(cell), BlockId::FungalSoil | BlockId::GroveMoss) { continue; }
                let p = (cell.as_vec3()+Vec3::new(0.5,1.0,0.5))*s + Vec3::Y*0.02;
                if !free(world,p,SALAMANDER) || !supported(world,p,0.2) { continue; }
                let r = Resident { kind:SALAMANDER, home:p, pos:p, health:super::super::enemy_health(SALAMANDER), yaw:slot as f32 };
                a.enemies.push(r.actor(key));
                a.wildlife.residents.insert(key,r);
                *count += 1;
                break;
            }
        }
    }}
}

pub(super) fn draw(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    let pale = [0.60,0.57,0.68];
    let mut part = |at:Vec3,size:Vec3,color| m.material_box(e.pos+at,size,Quat::IDENTITY,color,70.0,0.0);
    part(Vec3::new(0.0,0.18,0.0),Vec3::new(0.32,0.22,0.60),pale);
    part(Vec3::new(0.0,0.20,-0.38),Vec3::new(0.40,0.18,0.30),pale);
    for side in [-1.0,1.0] {
        part(Vec3::new(side*0.15,0.26,-0.50),Vec3::splat(0.045),[0.07,0.06,0.10]);
        for z in [-0.20,0.22] {
            let offset = if side * z < 0.0 { 0.0 } else { std::f32::consts::PI };
            let (stride, lift) = super::super::enemy_presentation::walk(e, 0.08, 0.03, offset, 1.0);
            part(Vec3::new(side*0.23,0.08+lift,z+stride),Vec3::new(0.22,0.08,0.10),pale);
        }
        for i in 0..3 {
            part(Vec3::new(side*(0.24+i as f32*0.035),0.22,-0.35+i as f32*0.065),
                Vec3::new(0.12,0.045,0.035),[0.62,0.32,0.38]);
        }
    }
    for i in 0..4 {
        let f=i as f32;
        part(Vec3::new((time*1.8-f*0.5).sin()*0.025*(f+1.0),0.14-f*0.015,0.36+f*0.14),
            Vec3::new(0.18-f*0.035,0.13-f*0.025,0.18),pale);
    }
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    m.rotate_since(mark, e.pos, Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(e.yaw));
}
