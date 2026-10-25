//! Generator 48 detail that resolves with the voxel size. Every measure is in
//! metres and keys off voxel centres, so a moulding a quarter metre deep is
//! one voxel at 0.25 m, rounds to one at 0.5 m, and simply is not there at
//! 1 m: coarse worlds keep their clean shapes, fine worlds gain mouldings,
//! inlaid panels, jointed flagstones, chandeliers and rib bosses.
use super::{Ceiling, Complex, Kind, Outline, Room, kit::near_line};
use crate::{BlockId as B, hash2};

/// First generator with scale-resolved detail.
pub const VERSION: u32 = 48;
/// Height the chandelier plate hangs at over the floor, metres.
const LAMP: f32 = 3.0;

/// Rooms whose middle belongs to a landmark, a puzzle or a void.
fn busy_middle(r: &Room) -> bool {
    matches!(r.kind, Kind::Reveal | Kind::Orrery | Kind::Core | Kind::Chasm | Kind::Pitfall | Kind::Labyrinth | Kind::Entry)
}

/// Mouldings on the walls and the chandelier, inside a room's void. Runs
/// after doorways, balconies and piers, before galleries and furniture.
pub fn room(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32, dwall: f32) -> Option<B> {
    if v > 0.5 {
        return None;
    }
    // A plinth that steps out from the wall foot, a cornice under the
    // ceiling line: two courses at a quarter metre, one at a half.
    let plinth: f32 = if fy < 0.3 { 0.45 } else if fy < 0.6 { 0.3 } else { 0.0 };
    let cornice = if r.h >= 4.0 && fy < r.h {
        let below = r.h - fy;
        if below < 0.3 { 0.45 } else if below < 0.6 { 0.3 } else { 0.0 }
    } else {
        0.0
    };
    if dwall < plinth.max(cornice) && r.outline != Outline::Hewn {
        return Some(c.pal.trim);
    }
    chandelier(c, r, rx, rz, fy, v)
}

/// A ring of four candles on a plate, hung on a chain from the vault over
/// the middle of a large room, well above head height.
fn chandelier(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    if busy_middle(r) || r.w().min(r.d()) < 12.0 || r.h < LAMP + 1.5 || fy < LAMP - v {
        return None;
    }
    let (dx, dz) = (rx - r.w() * 0.5, rz - r.d() * 0.5);
    let ring = dx.hypot(dz);
    let plate = (LAMP / v).round() * v;
    if fy < plate {
        return (ring < 1.45).then_some(if ring < 0.9 && ring > 0.5 { B::Air } else { c.pal.accent });
    }
    // Candles at the four compass points of the ring, on the plate.
    let candle = |a: f32, b: f32| a.abs() < v * 0.5 + 0.01 && (b.abs() - 1.05).abs() < v * 0.5 + 0.01;
    if fy < plate + v && (candle(dx, dz) || candle(dz, dx)) {
        return Some(B::Torch);
    }
    // The chain: one voxel up the middle, links alternating metals.
    if dx.abs() < v * 0.5 + 0.01 && dz.abs() < v * 0.5 + 0.01 && fy >= plate {
        let link = ((fy / v) as i32).rem_euclid(2) == 0;
        return Some(if link { B::SilverBlock } else { c.pal.trim });
    }
    None
}

/// Floor stone: flagstones with joints at fine scales and a worn path down
/// the centre cross; `None` keeps the generator's own floor.
pub fn floor(c: &Complex, r: &Room, rx: f32, rz: f32, v: f32, dwall: f32) -> Option<B> {
    if v > 0.5 || dwall < 1.0 || matches!(r.kind, Kind::Core) {
        return None;
    }
    let lane = (rx - r.w() * 0.5).abs() < 1.2 || (rz - r.d() * 0.5).abs() < 1.2;
    // Flags a metre by a metre and a half, every other course offset half a flag.
    let course = (rz / 1.5).floor() as i32;
    let shift = if course.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
    let joint = near_line(rx + shift, 1.0, v, 0.0) || near_line(rz, 1.5, v, 0.0);
    if joint {
        return Some(c.pal.trim);
    }
    let flag = hash2(((rx + shift) / 1.0).floor() as i32, course, r.salt.wrapping_add(48));
    Some(if lane && flag > 0.5 {
        B::Cobblestone
    } else if flag > 0.85 {
        c.pal.alt
    } else {
        c.pal.floor
    })
}

/// Inlaid panels on the first voxel of wall between piers: a trim frame
/// round a field of the alternate stone (accent stone glows in some jobs and
/// shouts across a whole wall). `along` runs along the wall.
pub fn panel(c: &Complex, r: &Room, along: f32, fy: f32, v: f32) -> Option<B> {
    if v > 0.5 || r.outline == Outline::Hewn || !(1.0..r.h - 1.0).contains(&fy) {
        return None;
    }
    let bay = c.job.bay();
    let local = along.rem_euclid(bay);
    let inside = local > 1.0 && local < bay - 1.0;
    if !inside {
        return None;
    }
    let edge = local < 1.0 + v || local >= bay - 1.0 - v || fy < 1.0 + v || fy >= r.h - 1.0 - v;
    Some(if edge { c.pal.trim } else { c.pal.alt })
}

/// Voussoirs round a doorway: alternate stones, a keystone at the crown.
/// `across` is from the opening's centre line, `rise` above its lintel.
pub fn voussoir(c: &Complex, across: f32, rise: f32, v: f32) -> B {
    if across.abs() < v.max(0.5) * 0.5 + 0.01 && rise >= 0.0 {
        return c.pal.accent;
    }
    if v > 0.5 {
        return c.pal.trim;
    }
    let stone = (across / 0.5).floor() as i32;
    if stone.rem_euclid(2) == 0 { c.pal.trim } else { c.pal.alt }
}

/// A boss where a vault rib crosses the room's centre line.
pub fn boss(c: &Complex, r: &Room, across: f32, v: f32) -> Option<B> {
    let mid = if r.w() <= r.d() { r.w() * 0.5 } else { r.d() * 0.5 };
    (v <= 0.5 && matches!(r.ceiling, Ceiling::Ribbed | Ceiling::Corbel) && (across - mid).abs() < 0.5).then_some(c.pal.accent)
}
