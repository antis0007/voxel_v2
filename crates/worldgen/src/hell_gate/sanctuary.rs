//! Gate 89 inside the tower ring and the sanctuary below it.
use super::approach::wall_torch;
use crate::{BlockId as B, WorldSampleConfig, hash2, value_noise2};
use glam::{IVec3, Vec2, Vec3};
use std::f32::consts::TAU;

/// The tower ring above ground: a ragged ruin up to ten metres, with arched
/// doorways on the four axes, a string course and arrow slits.
pub fn tower(cfg: &WorldSampleConfig, d: Vec3, m: Vec3, surface: f32) -> Option<B> {
    let vm = cfg.voxel_meters();
    let angle = d.z.atan2(d.x);
    let y = m.y - surface;
    let door = d.x.abs() < 3.0 || d.z.abs() < 3.0;
    if door && y > -1.0 && y < 3.6 {
        return Some(B::Air);
    }
    let rise = value_noise2(angle.cos() * 2.2 + 9.0, angle.sin() * 2.2 - 3.0, cfg.seed ^ 0x70e5);
    let height = 2.0 + 8.0 * rise * rise;
    if y >= height {
        return None;
    }
    let slit = (angle / TAU * 16.0).rem_euclid(1.0) - 0.5;
    if (5.0..6.6).contains(&y) && (slit * TAU * 14.0 / 16.0).abs() < 0.15f32.max(vm * 0.5) {
        return Some(B::Air);
    }
    if (2.9..3.3f32.max(2.9 + vm)).contains(&y) {
        return Some(B::DressedStone);
    }
    let p = (m / vm).floor().as_ivec3();
    Some(if y > height - vm && hash2(p.x, p.z, cfg.seed ^ 0x7055) < 0.5 { B::GroveMoss } else { B::WeatheredBricks })
}

/// Dark inlay in the sanctuary's top floor layer: a runner from the door and a
/// ring around the room. `y` is metres above the floor.
pub fn floor_inlay(d: Vec3, y: f32, vm: f32) -> Option<B> {
    if y < -vm {
        return None;
    }
    let radius = d.x.hypot(d.z);
    let runner = d.x.abs() < 1.1 && d.z < -0.75 && radius < 8.0;
    let band = (6.3..6.7f32.max(6.3 + vm)).contains(&radius);
    let sill = d.x.abs() < 2.5 && d.z.abs() < 0.75;
    if sill {
        return Some(B::DressedStone);
    }
    (runner || band).then_some(B::Basalt)
}

/// Pilasters, a cornice, an architrave round the arch and four wall torches.
/// `base` is the absolute floor height and `p` the cell.
pub fn sanctuary(c: Vec3, base: f32, p: IVec3, d: Vec3, vm: f32) -> Option<B> {
    let y = d.y - base + c.y;
    let (ax, z) = (d.x.abs(), d.z);
    // Architrave and plinths framing the arch, outside its posts.
    if (3.5..3.95).contains(&ax) && z.abs() < 0.95 && y < 6.95 {
        return Some(if y < 0.6 { B::Basalt } else { B::StoneBricks });
    }
    if ax < 3.95 && z.abs() < 0.95 && (6.5..6.95).contains(&y) {
        return Some(B::StoneBricks);
    }
    let radius = ax.hypot(z);
    let theta = ax.atan2(z);
    for degrees in [67.5f32, 112.5, 157.5] {
        let a = degrees.to_radians();
        let off = theta - a;
        let (radial, tangent) = (radius * off.cos(), radius * off.sin());
        let half: f32 = if y < 0.5 || (5.2..5.6).contains(&y) { 0.65 } else { 0.45 };
        if radial >= 7.1 && tangent.abs() < half.max(vm * 0.5) && y < 5.6 {
            return Some(if (5.2..5.6).contains(&y) { B::StoneBricks } else if y < 0.5 { B::Basalt } else { B::Cobblestone });
        }
    }
    let entrance = d.x.abs() < 2.5 && z < 0.0;
    if radius >= 7.55 && (5.6..6.2).contains(&y) && !entrance {
        return Some(B::StoneBricks);
    }
    for degrees in [67.5f32, 157.5] {
        let a = degrees.to_radians();
        for sx in [-1.0f32, 1.0] {
            let r = 7.1 - vm * 0.5;
            let q = Vec3::new(c.x + sx * a.sin() * r, base + 2.6, c.z + a.cos() * r);
            if (q / vm).floor().as_ivec3() == p {
                let out = Vec2::new(sx * a.sin(), a.cos());
                let support = if out.x.abs() > out.y.abs() {
                    IVec3::new(out.x.signum() as i32, 0, 0)
                } else {
                    IVec3::new(0, 0, out.y.signum() as i32)
                };
                return Some(wall_torch(support));
            }
        }
    }
    None
}
