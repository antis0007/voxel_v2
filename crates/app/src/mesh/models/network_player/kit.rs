//! Original delver kit. Broad silhouette and connected patches, no status glow.
use super::{Model,Quat,Vec3};
pub(super) fn draw(m:&mut Model,origin:Vec3,body:Quat,head:Quat,id:u32) {
    let cloth=[0.18,0.26,0.22];let canvas=[0.48,0.43,0.30];
    let leather=[0.14,0.075,0.04];let brass=[0.51,0.29,0.075];let ceramic=[0.60,0.58,0.44];
    let mut part=|at:Vec3,size:Vec3,q:Quat,color:[f32;3]| {
        m.cuboid(origin+body*at,size,body*q,color,0.0);
    };
    let i=Quat::IDENTITY;
    // Common working harness; the back is recognizable as well as the face.
    part(Vec3::new(-0.13,1.18,0.155),Vec3::new(0.055,0.43,0.025),i,leather);
    part(Vec3::new(0.13,1.18,0.155),Vec3::new(0.055,0.43,0.025),i,leather);
    part(Vec3::new(0.0,1.14,0.20),Vec3::new(0.27,0.30,0.12),i,cloth);
    part(Vec3::new(0.0,1.30,0.265),Vec3::new(0.29,0.06,0.045),i,canvas);
    for patch in 0..=id%4 {
        part(Vec3::new(-0.08+patch as f32*0.05,1.24,0.267),Vec3::new(0.035,0.04,0.012),i,brass);
    }
    match id%4 {
        0=>{
            // Surveyor: deep hood, split canvas tails and a rolled field map.
            for side in [-1.0,1.0] {
                let offset=head*Vec3::new(side*0.205,0.22,0.0);
                part(Vec3::new(0.0,1.43,0.0)+offset,Vec3::new(0.075,0.38,0.40),head,cloth);
                part(Vec3::new(side*0.13,0.73,0.15),Vec3::new(0.22,0.30,0.065),Quat::from_rotation_x(-0.1),canvas);
            }
            part(Vec3::new(0.0,1.43,0.0)+head*Vec3::Y*0.43,Vec3::new(0.48,0.10,0.43),head,cloth);
            part(Vec3::new(0.0,1.02,0.30),Vec3::new(0.45,0.10,0.12),i,canvas);
            for x in [-0.14,0.14] {part(Vec3::new(x,1.02,0.37),Vec3::new(0.035,0.12,0.025),i,leather);}
        },
        1=>{
            // Foundry worker: overlapping apron and ceramic shoulder.
            part(Vec3::new(0.0,1.06,-0.155),Vec3::new(0.32,0.37,0.045),i,leather);
            part(Vec3::new(0.0,0.74,-0.16),Vec3::new(0.38,0.30,0.05),i,leather);
            part(Vec3::new(-0.31,1.40,0.0),Vec3::new(0.29,0.11,0.29),Quat::from_rotation_z(-0.18),ceramic);
        },
        2=>{
            // Courier: asymmetric shoulder cape and a deep flap satchel.
            part(Vec3::new(-0.15,1.25,0.23),Vec3::new(0.38,0.46,0.08),Quat::from_rotation_z(-0.15),cloth);
            part(Vec3::new(0.28,0.92,0.08),Vec3::new(0.18,0.29,0.19),i,leather);
            part(Vec3::new(0.28,1.06,0.02),Vec3::new(0.19,0.075,0.21),i,canvas);
            part(Vec3::new(0.28,0.97,-0.025),Vec3::new(0.045,0.065,0.015),i,brass);
        },
        _=>{
            // Tether maintainer: protected spool, restraint belt and service cans.
            part(Vec3::new(0.0,1.18,0.29),Vec3::new(0.36,0.35,0.10),i,ceramic);
            for y in [1.07,1.14,1.21,1.28] {part(Vec3::new(0.0,y,0.36),Vec3::new(0.28,0.035,0.045),i,leather);}
            for x in [-0.14,0.14] {
                part(Vec3::new(x,0.97,-0.18),Vec3::new(0.07,0.18,0.075),i,ceramic);
                part(Vec3::new(x,1.065,-0.18),Vec3::new(0.08,0.035,0.085),i,brass);
            }
        }
    }
}
