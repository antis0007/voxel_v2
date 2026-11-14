//! Standard Anchor freight module. Local coordinates start at the floor;
//! the paired doors face +x.
use crate::BlockId as B;
use glam::Vec3;

pub(crate) const HALF: Vec3 = Vec3::new(5.8, 2.0, 1.8);
pub(crate) const HEIGHT: f32 = 4.0;

pub(crate) fn container(q: Vec3, v: f32, mat: B, open: bool) -> Option<B> {
    let edge = HALF - q.abs();
    if edge.x < 0.0 || edge.z < 0.0 || !(0.0..HEIGHT).contains(&q.y) {
        return None;
    }
    let wall = v.max(0.3);
    if q.y < wall || q.y >= HEIGHT - wall {
        return Some(B::SteelPlate);
    }
    if edge.x < wall {
        if q.x > 0.0 && open && edge.z >= wall {
            return Some(B::Air);
        }
        return Some(if q.z.abs() < v.max(0.4) {
            B::SteelPlate
        } else {
            mat
        });
    }
    if edge.z < wall {
        let ribs = q.x.rem_euclid(1.5) < (v * 0.6).max(0.24);
        return Some(if ribs || edge.x < wall * 2.0 {
            B::SteelPlate
        } else {
            mat
        });
    }
    Some(B::Air)
}
