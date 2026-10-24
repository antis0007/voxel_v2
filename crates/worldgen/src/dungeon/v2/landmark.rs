//! The one thing worth remembering in each complex: a landmark in the middle
//! of the reveal hall, chosen by the job. Landmarks stay inside a quarter of
//! the hall's short side from its centre, so the fight anchors (at 0.38 of
//! the width) and the balcony stair stay clear.
use super::{Complex, Job, LEVEL, Room, kit::near_line, starry};
use crate::BlockId as B;

fn local(r: &Room, rx: f32, rz: f32) -> (f32, f32, f32, f32) {
    let (dx, dz) = (rx - r.w() * 0.5, rz - r.d() * 0.5);
    (dx, dz, dx.hypot(dz), r.w().min(r.d()) * 0.25)
}

pub fn below(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (_, _, dist, rad) = local(r, rx, rz);
    match c.job {
        Job::Undercroft if dist < rad => Some(if dist > rad - v.max(0.6) && fy >= -v {
            c.pal.trim
        } else if fy < -0.4 {
            B::Water
        } else {
            B::Air
        }),
        Job::HollowFloor if dist < rad => Some(if fy >= -v && dist < rad - v {
            B::MirageBrick
        } else if fy < -3.0 {
            B::Water
        } else if dist >= rad - v {
            c.pal.trim
        } else {
            B::Air
        }),
        Job::ForgeHalls if dist < rad * 0.5 && fy >= -1.0 => Some(if fy < -0.3 { B::Lava } else { B::Air }),
        _ => None,
    }
}

pub fn furnish(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (dx, dz, dist, rad) = local(r, rx, rz);
    if dist >= rad + 0.5 {
        return None;
    }
    match c.job {
        Job::Catacombs => {
            // Ossuary tower: a ring of niches, a brazier on top.
            let top = LEVEL + 2.0;
            if dist < 2.6 && fy < top {
                let niche = dist > 1.8 && fy.rem_euclid(1.5) < 0.7 && near_line(dz.atan2(dx) * 2.6, 1.6, v, 0.0);
                return Some(if niche { B::Gravel } else if dist > 1.8 { c.pal.alt } else { c.pal.wall });
            }
            (dist < v && (top..top + v).contains(&fy)).then_some(B::Torch)
        }
        Job::Undercroft => {
            // A fountain column in the basin.
            (dist < 1.0 && fy < LEVEL).then_some(if fy > LEVEL - 1.0 { c.pal.accent } else { c.pal.trim })
        }
        Job::ForgeHalls => {
            // Furnace stack over a lava hearth, open on all four sides.
            let sq = dx.abs().max(dz.abs());
            let arch = fy < 2.0 && (dx.abs() < 1.0 || dz.abs() < 1.0);
            ((1.5..2.5).contains(&sq) && !arch && fy < r.h + 1.0).then_some(if fy.rem_euclid(3.0) < v { c.pal.trim } else { c.pal.alt })
        }
        Job::Warren => {
            // A giant mushroom: stem, cap, glowing gills.
            let cap = LEVEL + 3.0;
            if dist < 1.3 && fy < cap {
                return Some(B::MushroomStem);
            }
            let dome = cap + 1.5 - (dist / rad).powi(2) * 1.5;
            if dist < rad && fy >= cap && fy < dome {
                return Some(if fy < cap + v { B::GlowCap } else { B::RedCap });
            }
            None
        }
        Job::Quarantine => {
            // A barred cage on a plinth: whatever was kept here is gone.
            if dist < rad && fy < 0.6 {
                return Some(c.pal.trim);
            }
            let bars = (dist - 2.5).abs() < 0.4 && near_line(dz.atan2(dx) * 2.5, 1.0, v, 0.0);
            (bars && fy < 4.5 || (dist < 2.9 && (4.5..4.5 + v).contains(&fy))).then_some(c.pal.trim)
        }
        Job::Orrery => starry::orrery(c, r, dx, dz, fy, v, 0.5),
        Job::HollowFloor => {
            // A crystal spine rising from the pit through the false floor.
            (dist < 0.8 && fy < LEVEL + 3.0).then_some(c.pal.accent)
        }
        Job::Sanctum => {
            // Stepped dais: three steps up to an altar and a gold offering.
            let step = ((rad - dist) / (rad / 3.0)).floor().clamp(0.0, 3.0);
            if dist < rad && fy < step * 0.5 {
                return Some(c.pal.trim);
            }
            if dist < 1.0 && (1.5..2.5).contains(&fy) {
                return Some(if fy >= 2.5 - v { c.pal.accent } else { c.pal.alt });
            }
            None
        }
    }
}
