//! The construction kit: shells, piers, ceilings, balconies, stairs, doorways
//! and corridors. Pure functions of a point in metres; sample points are voxel
//! centres, so every feature is at least one voxel thick at any scale.
use super::index::CORR_BIT;
use super::{Ceiling, Complex, Corr, Kind, LEVEL, Outline, Room, dress, outline, starry};
use crate::{BlockId as B, hash3};
use glam::Vec3;

/// Balcony band depth in the reveal hall.
pub const BAND: f32 = 3.0;
/// Width of the reveal stair beside the near balcony.
pub const STAIR_W: f32 = 2.5;

/// True on a line repeating every `period`, one voxel (or `width`) thick.
pub fn near_line(a: f32, period: f32, v: f32, width: f32) -> bool {
    let d = (a - v * 0.5).rem_euclid(period);
    d.min(period - d) < width.max(v * 0.5)
}

/// Extra height a ceiling style adds over the wall height at the centre.
pub fn rise(r: &Room) -> f32 {
    let s = r.w().min(r.d());
    if r.outline == Outline::Round && r.ceiling != Ceiling::Coffered {
        return (s * 0.25).clamp(2.0, 4.0);
    }
    match r.ceiling {
        Ceiling::Beams => 0.0,
        Ceiling::Ribbed => (s * 0.18).clamp(1.5, 3.0),
        Ceiling::Corbel => (s * 0.2).clamp(1.5, 3.5),
        Ceiling::Coffered => 1.0,
    }
}

/// Ceiling height above the floor at a room-local point.
fn ceiling_at(r: &Room, rx: f32, rz: f32, bay: f32, v: f32) -> f32 {
    let (w, d) = (r.w(), r.d());
    let (across, half) = if w <= d { (rx - w * 0.5, w * 0.5) } else { (rz - d * 0.5, d * 0.5) };
    let t = (across.abs() / half).clamp(0.0, 1.0);
    if r.outline == Outline::Round && r.ceiling != Ceiling::Coffered {
        // A dome over the round plan, ribbed where the bays cross it.
        let q = ((rx - w * 0.5) / (w * 0.5)).hypot((rz - d * 0.5) / (d * 0.5)).min(1.0);
        return r.h + rise(r) * (1.0 - q * q);
    }
    match r.ceiling {
        Ceiling::Beams => r.h,
        Ceiling::Ribbed => r.h + rise(r) * (1.0 - t * t),
        Ceiling::Corbel => r.h + rise(r) * ((1.0 - t) * 4.0).floor() * 0.25,
        Ceiling::Coffered => {
            let (cx, cz) = (rx - w * 0.5, rz - d * 0.5);
            if cx.hypot(cz) < 2.5 {
                // Oculus: a light well up into the rock.
                r.h + 7.0
            } else if near_line(rx, bay, v, 0.6) || near_line(rz, bay, v, 0.6) {
                r.h
            } else {
                r.h + 1.0
            }
        }
    }
}

/// Distance round a room's wall from a fixed bearing, metres.
fn around(r: &Room, rx: f32, rz: f32) -> f32 {
    let a = (rz - r.d() * 0.5).atan2(rx - r.w() * 0.5) + std::f32::consts::PI;
    a * (r.w() + r.d()) * 0.25
}

/// Stone for walls and ceilings: alternating courses with a scatter of the
/// second stone, so large surfaces stay quiet but not flat.
pub fn stone(c: &Complex, m: Vec3, v: f32) -> B {
    let p = (m / v).floor().as_ivec3();
    let course = ((m.y / 2.0).floor() as i32).rem_euclid(2) == 1;
    if course ^ (hash3(p.x, p.y, p.z, c.seed.wrapping_add(13)) > 0.85) { c.pal.alt } else { c.pal.wall }
}

/// The shell around a room: tombstone when sealed, a trim base course and
/// cornice, door frames around corridor mouths.
pub fn shell(c: &Complex, r: &Room, m: Vec3, v: f32) -> B {
    if r.sealed {
        return B::TombStone;
    }
    let fy = m.y - r.floor;
    let (rx, rz) = (m.x - r.x0, m.z - r.z0);
    if r.kind == Kind::Orrery
        && (-rx).max(rx - r.w()).max(-rz).max(rz - r.d()) < v
        && let Some(b) = starry::constellation(r, around(r, rx, rz), fy, v)
    {
        return b;
    }
    if dress::breach_hole(r, rx, rz, fy) {
        return if fy < 0.4 { B::Gravel } else { B::Air };
    }
    let frame = c.index.at(m.x, m.z).iter().filter(|id| *id & CORR_BIT != 0).any(|&id| {
        let k = &c.corrs[(id & !CORR_BIT) as usize];
        m.x >= k.x0 - 0.8 && m.x < k.x1 + 0.8 && m.z >= k.z0 - 0.8 && m.z < k.z1 + 0.8 && {
            let f = k.floor_at(m.x, m.z);
            m.y >= f - 0.5 && m.y < f + k.h + 0.8
        }
    });
    if frame || (fy >= -v && fy < 0.6) || (fy >= r.h - 0.6 && fy < r.h) {
        c.pal.trim
    } else {
        stone(c, m, v)
    }
}

/// Air in front of a corridor mouth, 3.5 m into the room, so no pier, prop
/// or cell wall ever blocks a door.
pub fn doorway(c: &Complex, m: Vec3) -> bool {
    c.index.at(m.x, m.z).iter().filter(|id| *id & CORR_BIT != 0).any(|&id| {
        let k = &c.corrs[(id & !CORR_BIT) as usize];
        let (ax, az) = if k.along_x { (3.5, 0.5) } else { (0.5, 3.5) };
        m.x >= k.x0 - ax && m.x < k.x1 + ax && m.z >= k.z0 - az && m.z < k.z1 + az && {
            let f = k.floor_at(m.x, m.z);
            m.y >= f && m.y < f + k.h
        }
    })
}

pub fn room(c: &Complex, r: &Room, m: Vec3, v: f32) -> B {
    let (rx, rz, fy) = (m.x - r.x0, m.z - r.z0, m.y - r.floor);
    let (w, d) = (r.w(), r.d());
    let bay = c.job.bay();
    let dwall = outline::depth(r, rx, rz);
    if dwall < 0.0 {
        // Behind the outline: the room's own rock, cut only by door throats.
        if outline::throat(c, r, m) {
            return B::Air;
        }
        if r.sealed {
            return B::TombStone;
        }
        let k = dwall.abs();
        if r.kind == Kind::Orrery && k < v
            && let Some(b) = starry::constellation(r, around(r, rx, rz), fy, v)
        {
            return b;
        }
        return if r.outline != Outline::Hewn && k < v.max(0.6) && (fy < 0.6 || (fy >= r.h - 0.6 && fy < r.h)) { c.pal.trim } else { stone(c, m, v) };
    }
    if fy < 0.0 {
        if let Some(b) = dress::below(c, r, rx, rz, fy, v) {
            return b;
        }
        return if fy >= -v { floor_block(c, r, m, v) } else { stone(c, m, v) };
    }
    let ceil = ceiling_at(r, rx, rz, bay, v);
    if fy >= ceil {
        return stone(c, m, v);
    }
    // Ribs on vaults, beams on flat ceilings, one voxel deep, at every bay.
    let along = if w <= d { rz } else { rx };
    let across = if w <= d { rx } else { rz };
    if fy >= ceil - 0.6 && fy >= r.h - 0.6 {
        match r.ceiling {
            Ceiling::Ribbed | Ceiling::Corbel if near_line(along, bay, v, 0.0) => return c.pal.trim,
            Ceiling::Beams if near_line(along, bay, v, 0.0) && r.kind != Kind::Labyrinth => return c.pal.trim,
            _ => {}
        }
    }
    if r.kind == Kind::Entry && (rx - w * 0.5).hypot(rz - d * 0.5) < 1.6 {
        return B::Air;
    }
    if doorway(c, m) {
        return B::Air;
    }
    if r.kind == Kind::Reveal
        && let Some(b) = balcony(c, r, rx, rz, fy, v)
    {
        return b;
    }
    // Piers at every bay, skipping the corners; a sconce on every other one.
    let pd = v.max(0.6);
    if dwall < pd + v && !r.kind.is_puzzle() && fy < r.h {
        let wall_along = if rx.min(w - rx) < rz.min(d - rz) { rz } else { rx };
        let span = if rx.min(w - rx) < rz.min(d - rz) { d } else { w };
        if wall_along > 1.5 && wall_along < span - 1.5 && near_line(wall_along, bay, v, 0.6) {
            if dwall < pd {
                return if fy < 0.6 || (fy >= r.h - 1.0) { c.pal.trim } else { stone(c, m, v) };
            }
            let k = ((wall_along - v * 0.5) / bay).round() as i32;
            if k.rem_euclid(2) == 0 && r.kind != Kind::Cells {
                if (2.4..2.4 + v).contains(&fy) {
                    return c.pal.trim;
                }
                if (2.4 + v..2.4 + 2.0 * v).contains(&fy) {
                    return B::Torch;
                }
            }
        }
    }
    let _ = across;
    dress::furnish(c, r, rx, rz, fy, v, dwall).unwrap_or(B::Air)
}

fn floor_block(c: &Complex, r: &Room, m: Vec3, v: f32) -> B {
    let (rx, rz) = (m.x - r.x0, m.z - r.z0);
    // A border of trim and a field of floor stone.
    let dwall = rx.min(r.w() - rx).min(rz).min(r.d() - rz);
    if r.sealed && r.kind == Kind::Core {
        return B::TombStone;
    }
    if dwall < 1.0 || near_line(rx, c.job.bay() * 2.0, v, 0.0) && near_line(rz, c.job.bay() * 2.0, v, 0.0) {
        c.pal.trim
    } else {
        let p = (m / v).floor().as_ivec3();
        if hash3(p.x, p.y, p.z, c.seed.wrapping_add(17)) > 0.9 { c.pal.alt } else { c.pal.floor }
    }
}

/// Balcony ring at the upper level, a railing with lamps, and a stair along
/// the near (-z) band that lands on the +x band.
fn balcony(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let dwall = rx.min(w - rx).min(rz).min(d - rz);
    let run = LEVEL / 0.75;
    let (s0, s1) = (w - BAND - run, w - BAND);
    let on_stair = rx >= s0 && rx < s1 && (BAND..BAND + STAIR_W).contains(&rz);
    if on_stair {
        let top = ((rx - s0) * 0.75 / v).floor() * v + v;
        return (fy < top.min(LEVEL)).then_some(c.pal.trim);
    }
    if dwall < BAND && fy >= LEVEL - v && fy < LEVEL {
        return Some(c.pal.trim);
    }
    let rail = dwall >= BAND - v && dwall < BAND;
    let landing = w - rx < BAND + v && rz >= BAND - v && rz < BAND + STAIR_W;
    if rail && !landing && (LEVEL..LEVEL + 1.0).contains(&fy) {
        return Some(c.pal.trim);
    }
    // Lamps on the railing every other bay.
    if rail && !landing && fy >= LEVEL + 1.0 && fy < LEVEL + 1.0 + v {
        let along = if rx.min(w - rx) < rz.min(d - rz) { rz } else { rx };
        if near_line(along, c.job.bay() * 2.0, v, 0.0) {
            return Some(B::Torch);
        }
    }
    None
}

pub fn corridor(c: &Complex, k: &Corr, m: Vec3, v: f32) -> B {
    let f = k.floor_at(m.x, m.z);
    let fy = m.y - f;
    if fy < 0.0 {
        return if fy >= -v { c.pal.floor } else { stone(c, m, v) };
    }
    let (along, across, half) = if k.along_x {
        (m.x, m.z - (k.z0 + k.z1) * 0.5, (k.z1 - k.z0) * 0.5)
    } else {
        (m.z, m.x - (k.x0 + k.x1) * 0.5, (k.x1 - k.x0) * 0.5)
    };
    let edge = across.abs() / half.max(0.01);
    let roof = if k.pointed && edge > 0.5 { k.h - 1.0 } else { k.h };
    if fy >= roof {
        return stone(c, m, v);
    }
    if let Some(p) = k.plug {
        let mid = (k.a0 + k.a1) * 0.5;
        if (along - mid).abs() < 1.0 {
            return p;
        }
    }
    // Sconces on alternating sides every eight metres.
    if half - across.abs() < v && (k.a1 - k.a0) > 6.0 && near_line(along, 8.0, v, 0.0) {
        let side = ((along / 8.0).round() as i32).rem_euclid(2) == 0;
        if side == (across > 0.0) {
            if (1.6..1.6 + v).contains(&fy) {
                return c.pal.trim;
            }
            if (1.6 + v..1.6 + 2.0 * v).contains(&fy) {
                return B::Torch;
            }
        }
    }
    B::Air
}

/// Spiral stair down the shaft from the gatehouse to the entry floor.
pub fn shaft(c: &Complex, m: Vec3, v: f32) -> Option<B> {
    let (sx, sz, top) = c.shaft;
    let (dx, dz) = (m.x - sx, m.z - sz);
    if dx.abs() > 2.6 || dz.abs() > 2.6 {
        return None;
    }
    let entry = &c.rooms[c.entry];
    if m.y < entry.floor || m.y >= top + 0.5 {
        return None;
    }
    if dx.abs() < 1.6 && dz.abs() < 1.6 {
        let below = top - m.y;
        let turn = below * 0.85;
        let (px, pz) = (turn.cos() * 1.05, turn.sin() * 1.05);
        if (dx - px).hypot(dz - pz) < 0.55 && below.fract() < v.max(0.4) && m.y >= entry.floor + v {
            return Some(c.pal.trim);
        }
        if below.rem_euclid(8.0) < v && (dx - px).hypot(dz - pz) < 0.7 {
            return Some(B::GlowCap);
        }
        return Some(B::Air);
    }
    (m.y >= entry.floor + entry.top() && m.y < top - 1.0).then(|| stone(c, m, v))
}
