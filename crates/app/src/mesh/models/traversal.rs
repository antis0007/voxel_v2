//! Articulated, metre-sized working limbs; world geometry uses the collision contacts.
use super::{AUTHORED_SURFACE, Model};
use crate::player::Player;
use glam::{Quat, Vec3};

pub(super) fn draw(m: &mut Model, p: &Player, time: f32) {
    let t = &p.traversal;
    let feet = p.pos * m.scale_meters();
    let yaw = p.gravity_frame * Quat::from_rotation_y(-p.yaw);
    let brass = [0.52, 0.34, 0.17];
    let iron = [0.25, 0.29, 0.28];
    let gel = [0.35, 0.54, 0.31];
    if p.glide.available {
        m.with_surface_kind(AUTHORED_SURFACE, |m| {
            if p.glide.deployed {
                let frame=yaw*Quat::from_rotation_x(-p.pitch*0.35);
                let at=feet+p.up_dir()*1.75;
                let nose=at+frame*Vec3::new(0.0,0.08,-0.9);
                let tail=at+frame*Vec3::new(0.0,0.12,0.65);
                for side in [-1.0,1.0] {
                    let tip=at+frame*Vec3::new(side*2.1,-0.12,0.8);
                    let points=[nose,tail,tip];
                    let normal=(tail-nose).cross(tip-nose).normalize_or_zero();
                    m.triangle(points,normal,[0.76,0.64,0.36],0.0);
                    m.triangle(points,-normal,[0.56,0.45,0.25],0.0);
                    m.material_beam(nose,tip,0.045,brass,AUTHORED_SURFACE);
                    m.material_beam(tail,tip,0.025,iron,AUTHORED_SURFACE);
                }
                m.material_beam(nose,tail,0.045,brass,AUTHORED_SURFACE);
                m.material_beam(at,feet+p.up_dir()*1.0,0.035,iron,AUTHORED_SURFACE);
            } else {
                m.cuboid(feet+yaw*Vec3::new(0.0,1.1,0.25),Vec3::new(0.22,0.85,0.2),yaw,[0.66,0.55,0.32],0.0);
            }
        });
    }
    if t.jet != 0 {
        m.with_surface_kind(AUTHORED_SURFACE, |m| {
            let colour = if t.jet == 1 {
                [0.64, 0.30, 0.13]
            } else {
                [0.17, 0.46, 0.53]
            };
            for side in [-1.0, 1.0] {
                let tank = feet + yaw * Vec3::new(side * 0.22, 1.05, 0.27);
                m.cuboid(tank, Vec3::new(0.22, 0.65, 0.25), yaw, colour, 0.0);
                m.cuboid(
                    tank - p.up_dir() * 0.37,
                    Vec3::new(0.17, 0.13, 0.19),
                    yaw,
                    iron,
                    0.0,
                );
                if t.thrust {
                    let pulse = 0.18 + (time * 45.0).sin().abs() * 0.09;
                    m.cuboid(
                        tank - p.up_dir() * (0.44 + pulse * 0.5),
                        Vec3::new(0.09, pulse, 0.09),
                        yaw,
                        if t.jet == 1 {
                            [1.0, 0.48, 0.08]
                        } else {
                            [0.24, 0.82, 1.0]
                        },
                        1.0,
                    );
                }
            }
        });
    }
    if !t.octo {
        return;
    }
    m.with_surface_kind(AUTHORED_SURFACE, |m| {
        let back = feet + yaw * Vec3::new(0.0, 1.05, 0.23);
        m.cuboid(back, Vec3::new(0.42, 0.52, 0.20), yaw, iron, 0.0);
        for i in 0..8 {
            let side = if i % 4 < 2 { -1.0 } else { 1.0 };
            let row = (i / 2) as f32;
            let root = back + yaw * Vec3::new(side * 0.22, 0.20 - row * 0.12, 0.0);
            let folded = yaw
                * Vec3::new(
                    side * (0.58 + (i % 2) as f32 * 0.18),
                    0.1 + row * 0.22,
                    0.38,
                );
            let tip = if t.attached {
                t.contacts[i].map(|v| feet + v)
            } else {
                None
            }
            .unwrap_or(feet + folded);
            let gait = if t.attached {
                (time * 5.0 + i as f32 * 2.4).sin() * 0.045
            } else {
                0.0
            };
            let elbow = root.lerp(tip, 0.48) + yaw * Vec3::new(side * 0.38, 0.22 + gait, 0.15);
            m.material_beam(root, elbow, 0.065, brass, AUTHORED_SURFACE);
            m.material_beam(elbow, tip, 0.045, iron, AUTHORED_SURFACE);
            m.cuboid(elbow, Vec3::splat(0.16), yaw, brass, 0.0);
            m.cuboid(tip, Vec3::new(0.16, 0.10, 0.16), yaw, gel, 0.0);
        }
    });
}
