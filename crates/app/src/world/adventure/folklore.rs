//! Original folklore bodies and combat identities. Kinds are append-only.
use super::{Enemy, enemy_presentation::{walk, travel, strike}};
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub(super) const SATYR: u32 = 20;
pub(super) const IMP: u32 = 21;
pub(super) const REDCAP: u32 = 22;
pub(super) const DRAUGR: u32 = 23;
pub(super) const BOGGART: u32 = 24;
pub(super) const CORPSE_LIGHT: u32 = 25;

pub(super) fn charges(kind: u32) -> bool { matches!(kind, SATYR | REDCAP | super::monsters::GNOLL) }
pub(super) fn attack(kind: u32, sequence: u32) -> Option<u32> {
    match kind {
        SATYR | REDCAP | DRAUGR => Some(3),
        BOGGART => Some(if sequence.is_multiple_of(2) { 2 } else { 3 }),
        IMP => Some(0),
        CORPSE_LIGHT => Some(2),
        _ => None,
    }
}
pub(super) fn melee_damage(kind: u32) -> f32 {
    if let Some(melee) = super::bestiary::species(kind).and_then(|s| s.melee) { return melee; }
    match kind { 10 => 18.0, DRAUGR => 22.0, BOGGART => 16.0, _ => 10.0 }
}

fn p(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 { e.pos + Vec3::new(x,y,z) }
fn block(m: &mut Model, at: Vec3, size: Vec3, color: [f32;3], material: f32) {
    m.material_box(at,size,Quat::IDENTITY,color,material,0.0);
}
fn beam(m: &mut Model, a: Vec3, b: Vec3, width: f32, color: [f32;3], material: f32) {
    m.material_beam(a,b,width,color,material);
}
const HORN: [f32;3] = [0.72,0.62,0.41];
const IRON: [f32;3] = [0.25,0.29,0.30];
const DARK: [f32;3] = [0.12,0.11,0.10];

/// Digitigrade legs have knees and backward hocks; feet remain flat in stance.
fn legs(e: &Enemy, m: &mut Model, hip_y: f32, width: f32, color: [f32;3], goat: bool, brace: f32) {
    for side in [-1.0_f32,1.0] {
        let offset = if side < 0.0 {0.0} else {std::f32::consts::PI};
        let (distance,lift) = walk(e, if goat {0.28} else {0.23},0.14,offset,1.0-brace);
        let stride = travel(e, distance);
        let hip=p(e,side*width,hip_y-brace*0.09,0.0);
        let knee=p(e,side*(width+0.025),hip_y*0.57+lift*0.35,-0.11)+stride*0.45;
        let hock=p(e,side*(width+0.035),0.24+lift*0.6,if goat {0.16} else {0.0})+stride*0.75;
        let foot=p(e,side*(width+0.04),0.08+lift,-0.045)+stride;
        beam(m,hip,knee,0.09,color,70.0);
        beam(m,knee,hock,0.065,color,70.0);
        beam(m,hock,foot,0.05,DARK,70.0);
        block(m,foot,Vec3::new(0.20,0.16,0.28),if goat {DARK} else {IRON},if goat {70.0} else {59.0});
        if goat {
            block(m,foot+Vec3::new(0.0,0.03,-0.14),Vec3::new(0.025,0.10,0.025),HORN,70.0);
        }
    }
}

fn eyes(m: &mut Model, head: Vec3, spacing: f32, front: f32, color: [f32;3], energy: f32) {
    for side in [-1.0_f32,1.0] {
        m.material_box(head+Vec3::new(side*spacing,0.015,front),Vec3::new(0.075,0.04,0.035),Quat::IDENTITY,color,60.0,energy);
    }
}

pub(super) fn body(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    match e.kind {
        SATYR => satyr(e,m,wind),
        IMP => imp(e,m,time,wind),
        REDCAP => redcap(e,m,wind),
        DRAUGR => draugr(e,m,wind),
        BOGGART => boggart(e,m,time,wind),
        CORPSE_LIGHT => corpse_light(e,m,time,wind),
        _ => {},
    }
}

pub(in crate::world::adventure) fn satyr(e: &Enemy, m: &mut Model, wind: f32) {
    let fur=[0.29,0.22,0.15]; let skin=[0.59,0.42,0.29];
    let release=strike(e);
    legs(e,m,0.85,0.16,fur,true,wind);
    let chest=p(e,0.0,1.16-wind*0.14,-wind*0.15-release*0.12);
    m.material_box(chest,Vec3::new(0.48,0.53,0.29),Quat::from_rotation_x(-wind*0.4),skin,70.0,0.0);
    block(m,p(e,0.0,0.86,0.015),Vec3::new(0.48,0.22,0.34),fur,70.0);
    let head=chest+Vec3::new(0.0,0.44,-0.06-wind*0.17);
    block(m,head,Vec3::new(0.32,0.33,0.30),skin,70.0);
    eyes(m,head,0.085,-0.16,[0.76,0.69,0.36],0.25);
    block(m,head+Vec3::new(0.0,-0.23,0.0),Vec3::new(0.17,0.22,0.15),fur,70.0);
    for side in [-1.0_f32,1.0] {
        // Four angular segments curl forward from the crown.
        let horn=[Vec3::new(side*0.13,0.12,0.04),Vec3::new(side*0.25,0.29,0.13),
            Vec3::new(side*0.31,0.36,-0.02),Vec3::new(side*0.24,0.24,-0.19)];
        for i in 0..3 {beam(m,head+horn[i],head+horn[i+1],0.055-i as f32*0.012,HORN,70.0);}
        block(m,head+Vec3::new(side*0.23,0.0,0.0),Vec3::new(0.18,0.085,0.12),fur,70.0);
        let hand=chest+Vec3::new(side*0.38,-0.22+wind*0.18,-0.15-wind*0.22);
        let shoulder=chest+Vec3::new(side*0.27,0.16,0.0);
        let elbow=chest+Vec3::new(side*0.38,-0.03,0.04+wind*0.15);
        beam(m,shoulder,elbow,0.07,skin,70.0); beam(m,elbow,hand,0.065,skin,70.0);
        block(m,hand,Vec3::splat(0.14),skin,70.0);
    }
    // Unequal reed pipes hang at the hip, distinct from the charge silhouette.
    for i in 0..5 {block(m,p(e,-0.30+i as f32*0.055,0.81+i as f32*0.025,-0.20),Vec3::new(0.04,0.24+i as f32*0.035,0.05),HORN,69.0);}
}

fn imp(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let skin=[0.43,0.21,0.15]; let wing=[0.24,0.15,0.18];
    legs(e,m,0.42,0.12,skin,true,wind);
    let chest=p(e,0.0,0.63-wind*0.07,0.0);
    block(m,chest,Vec3::new(0.33,0.36,0.27),skin,70.0);
    let head=chest+Vec3::new(0.0,0.28,-0.12);
    block(m,head,Vec3::new(0.36,0.28,0.29),skin,70.0);
    eyes(m,head,0.105,-0.16,[0.98,0.61,0.19],0.4+wind);
    let cast=strike(e);
    for side in [-1.0_f32,1.0] {
        beam(m,head+Vec3::new(side*0.13,0.10,0.02),head+Vec3::new(side*0.21,0.30,0.05),0.045,HORN,70.0);
        let hand=chest+Vec3::new(side*(0.24+wind*0.10),-0.10+wind*0.20,-0.12-cast*0.35);
        beam(m,chest+Vec3::new(side*0.18,0.10,0.0),hand,0.055,skin,70.0);
        block(m,hand,Vec3::splat(0.12),HORN,70.0);
        let mark=m.mark();
        for i in 0..3 {
            let end=chest+Vec3::new(side*(0.37+i as f32*0.09),0.15-i as f32*0.12,0.20);
            beam(m,chest+Vec3::new(side*0.12,0.10,0.12),end,0.025,HORN,70.0);
            block(m,end+Vec3::new(-side*0.045,0.0,0.0),Vec3::new(0.12,0.14,0.035),wing,72.0);
        }
        m.rotate_since(mark,chest,Quat::from_rotation_y(side*((time*8.0).sin()*0.10+wind*0.7)));
    }
    let tail=[chest+Vec3::new(0.0,-0.08,0.10),chest+Vec3::new(0.1,-0.28,0.36),chest+Vec3::new((time*3.0).sin()*0.12,0.0,0.55)];
    beam(m,tail[0],tail[1],0.035,skin,70.0);beam(m,tail[1],tail[2],0.025,skin,70.0);
    block(m,tail[2],Vec3::new(0.10,0.10,0.055),HORN,70.0);
    m.material_box(head+Vec3::new(0.0,-0.08,-0.16),Vec3::new(0.17,0.045,0.04),Quat::IDENTITY,[0.95,0.37,0.12],60.0,wind);
}

fn redcap(e: &Enemy, m: &mut Model, wind: f32) {
    let cloth=[0.29,0.20,0.20]; let skin=[0.54,0.48,0.34]; let cap=[0.55,0.11,0.12];
    legs(e,m,0.55,0.14,cloth,false,wind);
    let chest=p(e,0.0,0.77-wind*0.13,0.0);
    block(m,chest,Vec3::new(0.42,0.40,0.29),cloth,72.0);
    let head=chest+Vec3::new(0.0,0.32,-0.08);
    block(m,head,Vec3::new(0.34,0.29,0.30),skin,70.0);
    eyes(m,head,0.09,-0.16,[0.82,0.71,0.43],0.2);
    block(m,head+Vec3::new(0.0,-0.02,-0.22),Vec3::new(0.12,0.10,0.18),skin,70.0);
    block(m,head+Vec3::new(0.0,0.15,0.0),Vec3::new(0.50,0.065,0.40),cap,72.0);
    for i in 0..3 {block(m,head+Vec3::new(i as f32*0.035,0.21+i as f32*0.075,0.0),Vec3::new(0.34-i as f32*0.075,0.10,0.31-i as f32*0.065),cap,72.0);}
    for side in [-1.0_f32,1.0] {
        let hand=chest+Vec3::new(side*0.32,-0.16+wind*0.40,-0.14-strike(e)*0.40);
        beam(m,chest+Vec3::new(side*0.22,0.12,0.0),hand,0.065,cloth,72.0);
        block(m,hand,Vec3::splat(0.13),skin,70.0);
        if side > 0.0 {
            beam(m,hand,hand+Vec3::new(0.0,0.1,-0.34),0.035,DARK,69.0);
            block(m,hand+Vec3::new(0.0,0.15,-0.36),Vec3::new(0.06,0.20,0.30),IRON,59.0);
        }
    }
}

fn draugr(e: &Enemy, m: &mut Model, wind: f32) {
    let skin=[0.36,0.45,0.42]; let cloth=[0.18,0.26,0.29];
    legs(e,m,0.86,0.18,cloth,false,wind);
    let chest=p(e,0.0,1.19-wind*0.06,0.01);
    block(m,chest,Vec3::new(0.56,0.62,0.34),cloth,72.0);
    for i in 0..3 {block(m,chest+Vec3::new(0.0,0.17-i as f32*0.17,-0.18),Vec3::new(0.49,0.10,0.065),IRON,59.0);}
    let head=chest+Vec3::new(0.0,0.49,-0.02);
    block(m,head,Vec3::new(0.34,0.36,0.31),skin,70.0);
    block(m,head+Vec3::new(0.0,0.16,0.015),Vec3::new(0.42,0.15,0.37),IRON,59.0);
    block(m,head+Vec3::new(0.0,0.03,-0.18),Vec3::new(0.05,0.27,0.05),IRON,59.0);
    eyes(m,head,0.095,-0.17,[0.42,0.75,0.77],0.6);
    for side in [-1.0_f32,1.0] {
        let hand=chest+Vec3::new(side*(0.39+wind*0.1),-0.28+wind*0.70,-0.18+wind*0.12-strike(e)*0.65);
        let shoulder=chest+Vec3::new(side*0.32,0.19,0.0);
        let elbow=chest+Vec3::new(side*0.45,0.0+wind*0.45,0.05);
        beam(m,shoulder,elbow,0.09,cloth,72.0);beam(m,elbow,hand,0.07,skin,70.0);
        block(m,hand,Vec3::splat(0.16),skin,70.0);
        if side > 0.0 {
            let top=hand+Vec3::new(0.0,0.56-wind*0.22,-0.12-strike(e)*0.14);
            beam(m,hand,top,0.035,[0.29,0.21,0.13],69.0);
            block(m,top+Vec3::new(0.10,0.0,-0.05),Vec3::new(0.34,0.27,0.07),IRON,59.0);
            block(m,top+Vec3::new(0.25,-0.05,-0.05),Vec3::new(0.06,0.33,0.09),HORN,59.0);
        }
    }
    for i in 0..4 {block(m,p(e,-0.23+i as f32*0.15,0.91-(i%2) as f32*0.08,0.23),Vec3::new(0.12,0.88,0.035),cloth,72.0);}
}

fn boggart(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let skin=[0.28,0.36,0.21]; let moss=[0.19,0.25,0.14];
    legs(e,m,0.50,0.22,skin,true,wind);
    let chest=p(e,0.0,0.82-wind*0.14,0.09);
    block(m,chest,Vec3::new(0.64,0.62,0.42),skin,70.0);
    let head=chest+Vec3::new(0.0,0.27,-0.30);
    block(m,head,Vec3::new(0.48,0.30,0.38),moss,70.0);
    block(m,head+Vec3::new(0.0,-0.11,-0.20),Vec3::new(0.45,0.10,0.10),DARK,70.0);
    eyes(m,head,0.14,-0.21,[0.83,0.68,0.27],0.35);
    for side in [-1.0_f32,1.0] {
        let shoulder=chest+Vec3::new(side*0.35,0.17,0.0);
        let elbow=chest+Vec3::new(side*0.51,-0.10+wind*0.48,-0.19);
        let hand=chest+Vec3::new(side*0.52,-0.58+wind*0.88,-0.32-strike(e)*0.48);
        beam(m,shoulder,elbow,0.095,skin,70.0);beam(m,elbow,hand,0.075,skin,70.0);
        for finger in 0..3 {beam(m,hand+Vec3::new((finger as f32-1.0)*0.07,0.0,0.0),hand+Vec3::new((finger as f32-1.0)*0.09,-0.09,-0.16),0.025,HORN,70.0);}
    }
    for i in 0..5 {
        let x=(i as f32-2.0)*0.12;
        beam(m,chest+Vec3::new(x,0.35,0.19),chest+Vec3::new(x+(time*1.6+i as f32).sin()*0.025,-0.20,0.29),0.035,moss,70.0);
    }
}

fn corpse_light(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let c=e.center()+Vec3::Y*((time*2.3+e.slot as f32).sin()*0.055);
    let cloth=[0.19,0.28,0.29]; let light=[0.47,0.81,0.70];
    block(m,c,Vec3::new(0.25,0.35,0.23),HORN,70.0);
    eyes(m,c,0.065,-0.13,light,0.7+wind);
    for side in [-1.0_f32,1.0] {
        let root=c+Vec3::new(side*0.17,0.11,0.07);
        let tip=c+Vec3::new(side*(0.36+wind*0.14),0.39+wind*0.18,0.02);
        beam(m,root,tip,0.035,HORN,70.0);
        for i in 0..3 {
            let a=c+Vec3::new(side*(0.13+i as f32*0.055),-0.17,0.02+i as f32*0.04);
            let b=a+Vec3::new((time*2.0+i as f32).sin()*0.07,-0.28-i as f32*0.10,0.06);
            beam(m,a,b,0.028,cloth,72.0);
        }
    }
    m.glow_orb(c+Vec3::new(0.0,-0.05,-0.14),0.06+wind*0.025,light,0.8+wind,0.8);
}
