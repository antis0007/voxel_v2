//! Room plans that are not boxes. A room keeps its bounding box (the lattice,
//! corridors and the index work in boxes); the outline says how deep inside
//! the walls a point is, and a throat in front of every door cuts through
//! whatever the outline leaves between the box edge and the room.
use super::{Complex, Kind, Room};
use crate::value_noise3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outline {
    Rect,
    /// Chamfered corners.
    Octagon,
    /// Ellipse inscribed in the box.
    Round,
    /// Two crossing naves; the corners are solid.
    Cross,
    /// Rough-hewn rock walls, as if the room was dug and never dressed.
    Hewn,
}

impl Outline {
    /// Outline for a room kind from a hash. Rooms whose floor plan is part of
    /// a puzzle or a basin keep the box.
    pub fn roll(kind: Kind, x: f32) -> Self {
        use Outline as O;
        let pick: &[O] = match kind {
            Kind::Shrine => &[O::Octagon, O::Cross, O::Rect],
            Kind::Crypt => &[O::Cross, O::Rect, O::Octagon],
            Kind::Ossuary => &[O::Hewn, O::Rect, O::Cross],
            Kind::Garden => &[O::Hewn, O::Round, O::Hewn],
            Kind::Orrery | Kind::Core => &[O::Round, O::Octagon],
            Kind::Library | Kind::Forge | Kind::Foundry => &[O::Rect, O::Octagon],
            Kind::Store | Kind::Barracks => &[O::Rect, O::Rect, O::Hewn],
            _ => &[O::Rect],
        };
        pick[(x * pick.len() as f32) as usize % pick.len()]
    }
}

/// Depth inside the walls at a room-local point: the distance to the nearest
/// wall for a box, negative outside the outline.
pub fn depth(r: &Room, rx: f32, rz: f32) -> f32 {
    let (w, d) = (r.w(), r.d());
    let (u, t) = (rx.min(w - rx), rz.min(d - rz));
    let rect = u.min(t);
    match r.outline {
        Outline::Rect => rect,
        Outline::Octagon => {
            let k = w.min(d) * 0.3;
            rect.min((u + t - k) * std::f32::consts::FRAC_1_SQRT_2)
        }
        Outline::Round => {
            let (a, b) = (w * 0.5, d * 0.5);
            let q = ((rx - a) / a).hypot((rz - b) / b);
            (1.0 - q) * a.min(b)
        }
        Outline::Cross => {
            let (nw, nd) = ((w * 0.28).round(), (d * 0.28).round());
            let long_x = u.min(t - nd);
            let long_z = (u - nw).min(t);
            long_x.max(long_z)
        }
        Outline::Hewn => {
            // Ragged rock: up to two metres in, never out past the box.
            let n = value_noise3(rx * 0.35, 0.0, rz * 0.35, r.salt.wrapping_add(19));
            rect - 0.4 - n * 1.6
        }
    }
}

/// True in the passage a door cuts from the box edge into an outline that
/// stands back from it: the corridor's width and height, straight in, until
/// one metre past the outline.
pub fn throat(c: &Complex, r: &Room, m: glam::Vec3) -> bool {
    if r.outline == Outline::Rect {
        return false;
    }
    let ids = c.index.at(m.x, m.z);
    ids.iter().filter(|id| *id & super::index::CORR_BIT != 0).any(|&id| {
        let k = &c.corrs[(id & !super::index::CORR_BIT) as usize];
        let (lo, hi) = if k.along_x { (k.z0, k.z1) } else { (k.x0, k.x1) };
        let across = if k.along_x { m.z } else { m.x };
        // The corridor must end on this room's wall, and the point must be
        // between that wall and the room.
        let ends = |a: f32, wall: f32| (a - wall).abs() < 1.0;
        let (lead, toward) = if k.along_x {
            if ends(k.x1, r.x0) || ends(k.x1 - 0.6, r.x0) {
                (m.x - r.x0, true)
            } else if ends(k.x0, r.x1) || ends(k.x0 + 0.6, r.x1) {
                (r.x1 - m.x, true)
            } else {
                (0.0, false)
            }
        } else if ends(k.z1, r.z0) || ends(k.z1 - 0.6, r.z0) {
            (m.z - r.z0, true)
        } else if ends(k.z0, r.z1) || ends(k.z0 + 0.6, r.z1) {
            (r.z1 - m.z, true)
        } else {
            (0.0, false)
        };
        if !toward || across < lo || across >= hi || !(0.0..10.0).contains(&lead) {
            return false;
        }
        let f = k.floor_at(m.x, m.z);
        m.y >= f && m.y < f + k.h && depth(r, m.x - r.x0, m.z - r.z0) < 1.0
    })
}
