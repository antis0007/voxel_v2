//! Generator 52: where a room's vault fell, the hole it left. A ragged
//! cavity opens over the rubble cone up into the shell, its rock roof lit
//! by amber caps and hung with roots, so the one warm light in the room
//! falls on the spill. Fine scales only; 1 m rooms keep their whole vault.
use super::{Damage, Room, dress::h};
use crate::{BlockId as B, hash2, value_noise3};

/// Middle of the fall, room-local metres; the rubble cone stands under it.
pub fn centre(r: &Room) -> (f32, f32) {
    let (w, d) = (r.w(), r.d());
    let qx = if h(r, 21) < 0.5 { w * 0.27 } else { w * 0.73 };
    let qz = if h(r, 22) < 0.5 { d * 0.27 } else { d * 0.73 };
    (qx, qz)
}

/// Top of the cavity over the floor: just inside the room's shell.
fn roof(r: &Room, v: f32) -> f32 {
    r.top() + super::WALL - v
}

/// The cavity's block over the vault line (`ceil` is the ceiling over this
/// column), or `None` for solid vault.
pub fn hole(r: &Room, rx: f32, rz: f32, fy: f32, ceil: f32, v: f32) -> Option<B> {
    if v > 0.5 || r.sealed || r.damage != Damage::Collapsed || fy < ceil {
        return None;
    }
    let top = roof(r, v);
    if fy >= top {
        return None;
    }
    let (qx, qz) = centre(r);
    let dist = (rx - qx).hypot(rz - qz);
    // Widest at the vault line, closing toward the rock roof.
    let rise = ((fy - ceil) / (top - ceil).max(0.5)).clamp(0.0, 1.0);
    let ragged = value_noise3(rx * 0.7, fy * 0.5, rz * 0.7, r.salt.wrapping_add(66)) * 1.1;
    let radius = r.w().min(r.d()) * 0.13 + ragged - rise * rise * 1.2;
    if dist >= radius {
        return None;
    }
    if fy >= top - v {
        return Some(if dist < 0.35 + v * 0.5 { B::AmberCap } else { B::Limestone });
    }
    // Roots down from the rock roof near the rim, ending above the vault line.
    let (px, pz) = ((rx / v).floor() as i32, (rz / v).floor() as i32);
    let root = hash2(px, pz, r.salt.wrapping_add(67));
    let length = 0.5 + root * (top - ceil + 1.0);
    if dist > radius * 0.55 && root > 0.6 && fy >= top - length {
        return Some(if root > 0.9 { B::Log } else { B::Leaves });
    }
    Some(B::Air)
}
