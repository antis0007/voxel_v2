//! Halls of wandering stars. An orrery room is one of three plans, picked by
//! hash: a grand orrery of tilted rings over a star floor; star-glass islands
//! over a void, joined by thin bridges; a starlit pool under a hanging sun.
//! Constellations are set into the walls of all three.
//!
//! Every plan keeps the room's aisle cross and the walkways in from its
//! doors at floor level, so each door reaches each other door, and a fall
//! into the void lands on ledges that climb back out.
use super::{Complex, Room, kit::near_line};
use crate::{BlockId as B, hash2, hash3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Plan {
    Rings,
    Islands,
    Pool,
}

pub fn plan(r: &Room) -> Plan {
    match (hash2(5, 3, r.salt) * 3.0) as u32 {
        0 => Plan::Rings,
        1 => Plan::Islands,
        _ => Plan::Pool,
    }
}

/// Depth of the void under an island room, metres.
pub const VOID: f32 = 5.0;

pub(super) fn lanes(c: &Complex, r: &Room, rx: f32, rz: f32, half: f32) -> bool {
    let (w, d) = (r.w(), r.d());
    if (rx - w * 0.5).abs() < half || (rz - d * 0.5).abs() < half {
        return true;
    }
    let (mx, mz) = (rx + r.x0, rz + r.z0);
    c.index.at(mx, mz).iter().filter(|id| *id & super::index::CORR_BIT != 0).any(|&id| {
        let k = &c.corrs[(id & !super::index::CORR_BIT) as usize];
        if k.along_x {
            mz >= k.z0 && mz < k.z1 && k.x1 > r.x0 && k.x0 < r.x1
        } else {
            mx >= k.x0 && mx < k.x1 && k.z1 > r.z0 && k.z0 < r.z1
        }
    })
}

/// The island in the quadrant of the aisle cross holding a point, if that
/// quadrant has one: centred in the space between the aisles and the wall
/// ledges, as large as fits, nudged by hash. One quadrant in five is open void.
pub(super) fn island(r: &Room, rx: f32, rz: f32) -> Option<(f32, f32, f32)> {
    const AISLE: f32 = 1.3;
    const LEDGE: f32 = 2.4;
    let (w, d) = (r.w(), r.d());
    let (qx, qz) = (rx >= w * 0.5, rz >= d * 0.5);
    let q = qx as i32 + 2 * qz as i32;
    if hash2(q, 1, r.salt.wrapping_add(61)) < 0.2 {
        return None;
    }
    let (span_x, span_z) = (w * 0.5 - AISLE - LEDGE, d * 0.5 - AISLE - LEDGE);
    let rad = (span_x.min(span_z) * 0.5 - 0.2) * (0.75 + hash2(q, 2, r.salt.wrapping_add(61)) * 0.25);
    if rad < 0.8 {
        return None;
    }
    let jit = |k: i32, span: f32| (hash2(q, k, r.salt.wrapping_add(62)) - 0.5) * (span - 2.0 * rad).max(0.0);
    let ix = if qx { w * 0.5 + AISLE + span_x * 0.5 } else { LEDGE + span_x * 0.5 } + jit(3, span_x);
    let iz = if qz { d * 0.5 + AISLE + span_z * 0.5 } else { LEDGE + span_z * 0.5 } + jit(4, span_z);
    ((rx - ix).hypot(rz - iz) < rad + 1.0 || (rx - ix).abs().min((rz - iz).abs()) < 1.0).then_some((ix, iz, rad))
}

/// Below the floor line.
pub fn below(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32, dwall: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    match plan(r) {
        Plan::Rings => {
            (fy >= -v && hash2((rx / v) as i32, (rz / v) as i32, r.salt.wrapping_add(5)) > 0.94).then_some(c.pal.accent)
        }
        Plan::Islands => {
            let bottom = |fy: f32| {
                if fy < -VOID {
                    Some(if hash3((rx / v) as i32, (fy / v) as i32, (rz / v) as i32, r.salt) > 0.85 { B::StarGlass } else { c.pal.wall })
                } else {
                    Some(B::Air)
                }
            };
            // Bridges: one metre slabs with a star-glass kerb.
            if lanes(c, r, rx, rz, 1.0) {
                return if fy >= -1.0 { None } else { bottom(fy) };
            }
            if let Some((ix, iz, rad)) = island(r, rx, rz) {
                let dist = (rx - ix).hypot(rz - iz);
                // Rock island tapering to a point below.
                let under = -1.0 - (rad - dist).max(0.0) * 1.8;
                if dist < rad && fy >= under {
                    return Some(if fy >= -v && dist > rad - v.max(0.6) { B::StarGlass } else { c.pal.wall });
                }
                // A plank span from the island to the nearer aisle.
                let to_x = (ix - w * 0.5).abs() < (iz - d * 0.5).abs();
                // The aisle at x = w/2 is reached by running along x, and so on.
                let (along, across, end) = if to_x { (rx, rz - iz, w * 0.5) } else { (rz, rx - ix, d * 0.5) };
                let start = if to_x { ix } else { iz };
                let between = (along - start) * (end - start) >= 0.0 && (along - start).abs() <= (end - start).abs();
                if between && across.abs() < v.max(0.5) && fy >= -v {
                    return Some(B::Planks);
                }
            }
            // Ledges along the walls that climb out of the void, rising to
            // meet the aisles at floor level: one step per voxel, so the low end is
            // always within a few metres of an aisle and clear of the corners.
            if dwall < 2.2 {
                let off = (rx - w * 0.5).abs().min((rz - d * 0.5).abs()) - 1.0;
                let top = -((off.max(0.0) / v).ceil() * v).min(VOID);
                return (fy < top).then_some(c.pal.trim).or(bottom(fy));
            }
            bottom(fy)
        }
        Plan::Pool => {
            let dist = (rx - w * 0.5).hypot(rz - d * 0.5);
            let pool = w.min(d) * 0.3;
            if dist < pool && fy >= -1.2 {
                return Some(if fy < -1.0 { B::StarGlass } else { B::Water });
            }
            None
        }
    }
}

/// Between floor and ceiling.
pub fn furnish(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let (dx, dz) = (rx - w * 0.5, rz - d * 0.5);
    match plan(r) {
        Plan::Rings => {
            // Four brass lamp stands round the star floor.
            let (sx, sz) = (dx.abs() - w * 0.3, dz.abs() - d * 0.3);
            if sx.abs().max(sz.abs()) < v * 0.5 + 0.01 && fy < 1.0 + v {
                return Some(if fy < 1.0 { B::CopperBlock } else { B::Torch });
            }
            orrery(c, r, dx, dz, fy, v, 1.0)
        }
        Plan::Islands => {
            if let Some((ix, iz, rad)) = island(r, rx, rz) {
                let dist = (rx - ix).hypot(rz - iz);
                // A crystal spire on each island, a torch at its foot.
                if dist < 0.5 && fy < 1.5 + rad {
                    return Some(if fy < 0.8 { c.pal.trim } else { B::Crystal });
                }
                if (dist - 1.0).abs() < v * 0.5 + 0.01 && (0.0..v).contains(&fy) && near_line((rz - iz).atan2(rx - ix) * 1.0, 3.1, v, 0.0) {
                    return Some(B::Torch);
                }
            }
            // Shards of star glass hanging over the void, well above the bridges.
            if !lanes(c, r, rx, rz, 1.6) && fy > 3.0 && fy < r.h - 0.5 {
                let (gi, gj, gk) = ((rx / 3.0).floor() as i32, (fy / 3.0).floor() as i32, (rz / 3.0).floor() as i32);
                if hash3(gi, gj, gk, r.salt.wrapping_add(71)) > 0.8 {
                    let (cx, cy, cz) = (gi as f32 * 3.0 + 1.5, gj as f32 * 3.0 + 1.5, gk as f32 * 3.0 + 1.5);
                    if (rx - cx).abs() + (fy - cy).abs() * 0.5 + (rz - cz).abs() < v.max(0.6) {
                        return Some(B::StarGlass);
                    }
                }
            }
            orrery(c, r, dx, dz, fy, v, 0.5)
        }
        Plan::Pool => {
            let dist = dx.hypot(dz);
            let pool = w.min(d) * 0.3;
            // A low kerb round the pool, broken at the aisles.
            if (dist - pool - 0.3).abs() < 0.35 && fy < 0.5 && dx.abs() > 1.2 && dz.abs() > 1.2 {
                return Some(c.pal.trim);
            }
            if (dist - pool - 0.3).abs() < 0.35 && (0.5..0.5 + v).contains(&fy) && near_line(dz.atan2(dx) * pool, 4.0, v, 0.0) && dx.abs() > 1.2 && dz.abs() > 1.2 {
                return Some(B::Torch);
            }
            orrery(c, r, dx, dz, fy, v, 0.7)
        }
    }
}

/// A brass orrery hanging from the vault: a crystal sun, three tilted rings
/// with their planets. Every part sits above head height. `scale` shrinks
/// it for smaller rooms; `dx`/`dz` are from the room centre.
pub fn orrery(c: &Complex, r: &Room, dx: f32, dz: f32, fy: f32, v: f32, scale: f32) -> Option<B> {
    let span = r.w().min(r.d()) * 0.5;
    let sun_y = (r.h * 0.6).max(4.0);
    let rel = fy - sun_y;
    let dist = dx.hypot(dz);
    let sun = (0.9 + 0.8 * scale).max(1.0);
    if dist.hypot(rel) < sun {
        return Some(B::Crystal);
    }
    // The hanger from the vault.
    if dist < v.max(0.3) && rel > 0.0 {
        return Some(c.pal.trim);
    }
    let rings = [(0.45, 0.25, B::CopperBlock), (0.7, -0.35, B::SilverBlock), (0.95, 0.5, B::GoldBlock)];
    for (k, (frac, tilt, block)) in rings.into_iter().enumerate() {
        let radius = span * frac * scale;
        if radius < sun + 1.0 {
            continue;
        }
        // Lowest point stays above head height.
        let tilt = tilt * ((sun_y - 2.4) / radius).clamp(0.0, 1.0);
        let spin = c.seed as f32 * 1e-3 + k as f32 * 2.1;
        let (s, co) = spin.sin_cos();
        let (u, w) = (dx * co + dz * s, -dx * s + dz * co);
        let (ts, tc) = tilt.sin_cos();
        let (y2, w2) = (rel * tc - w * ts, w * tc + rel * ts);
        let th = v.max(0.35);
        if (u.hypot(w2) - radius).abs() < th && y2.abs() < th {
            return Some(block);
        }
        // A planet riding each ring.
        let a = spin * 1.7 + 0.8;
        let (pu, pw) = (a.cos() * radius, a.sin() * radius);
        if (u - pu).hypot(w2 - pw).hypot(y2) < 0.6 + k as f32 * 0.25 {
            return Some(if k == 1 { B::StarGlass } else { c.pal.accent });
        }
    }
    None
}

/// Constellations in the first voxel of an orrery room's wall: star-glass
/// stars joined by thin silver lines, a few figures per wall. `along` runs
/// round the wall.
pub fn constellation(r: &Room, along: f32, fy: f32, v: f32) -> Option<B> {
    const PATCH: f32 = 7.0;
    if fy < 1.5 || fy > r.h + 2.0 {
        return None;
    }
    let pi = (along / PATCH).floor() as i32;
    if hash2(pi, 1, r.salt.wrapping_add(81)) < 0.3 {
        return None;
    }
    let star = |k: i32| {
        let x = (pi as f32 + 0.15 + hash2(pi, k * 2, r.salt.wrapping_add(82)) * 0.7) * PATCH;
        let y = 2.0 + hash2(pi, k * 2 + 1, r.salt.wrapping_add(82)) * (r.h - 1.5).max(1.0);
        (x, y)
    };
    let n = 3 + (hash2(pi, 9, r.salt.wrapping_add(83)) * 3.0) as i32;
    let th = v.max(0.3);
    for k in 0..n {
        let (x, y) = star(k);
        if (along - x).hypot(fy - y) < th * 1.6 {
            return Some(B::StarGlass);
        }
        if k + 1 < n {
            let (x2, y2) = star(k + 1);
            let (ex, ey) = (x2 - x, y2 - y);
            let t = (((along - x) * ex + (fy - y) * ey) / (ex * ex + ey * ey).max(1e-3)).clamp(0.0, 1.0);
            if (along - x - ex * t).hypot(fy - y - ey * t) < th * 0.5 {
                return Some(B::SilverBlock);
            }
        }
    }
    None
}
