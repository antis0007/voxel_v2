//! Generator 36 upper galleries: tall plain rooms (libraries, stores,
//! cisterns, sluices) get a deck along both long walls at 3.5 m, a railing
//! with lamps on its posts, and a straight stair beside the railing at one
//! end. Doorway clearances are carved before this runs, so a door under the
//! deck keeps its opening.
//!
//! From generator 48 the stair takes whichever of its four placements (either
//! long side, either end) no doorway clearance touches, chosen once at plan
//! time; a room with none gets no gallery. Before 48 a doorway could carve
//! away the stair or its landing and strand the deck.
use super::{Complex, Corr, Kind, Outline, Room, kit::near_line};
use crate::BlockId as B;
use glam::Vec3;

/// Deck height over the floor, metres.
const DECK: f32 = 3.5;
/// Deck depth from the wall, metres; the walkway is clear of the wall piers.
const DEPTH: f32 = 3.0;
/// Width of the stair that climbs beside the railing, metres.
const STAIR: f32 = 1.4;
/// `Room::stair` when no placement is clear.
pub const NO_STAIR: u8 = u8::MAX;

fn eligible(version: u32, r: &Room) -> bool {
    version >= super::RICH
        && r.outline == Outline::Rect
        && r.h >= 6.0
        && r.w().min(r.d()) >= 12.0
        && matches!(r.kind, Kind::Library | Kind::Store | Kind::Cistern | Kind::Sluice)
}

pub fn has(c: &Complex, r: &Room) -> bool {
    eligible(c.version, r) && (c.version < super::fine::VERSION || r.stair != NO_STAIR)
}

/// Room-local frame for a stair placement: bit 0 puts the stair on the
/// -across side, bit 1 at the near end.
fn frame(r: &Room, rx: f32, rz: f32, stair: u8) -> (f32, f32, f32, f32) {
    let (w, d) = (r.w(), r.d());
    let (along, across, len, wid) = if w >= d { (rx, rz, w, d) } else { (rz, rx, d, w) };
    let along = if stair & 2 != 0 { len - along } else { along };
    let across = if stair & 1 != 0 { wid - across } else { across };
    (along, across, len, wid)
}

/// The first stair placement whose steps and landing no doorway clearance
/// reaches, for generators that choose; placement 0 before them.
pub fn choose(version: u32, r: &Room, corrs: &[Corr]) -> u8 {
    if version < super::fine::VERSION || !eligible(version, r) {
        return 0;
    }
    let (w, d) = (r.w(), r.d());
    let (len, wid) = (w.max(d), w.min(d));
    let deck = DECK.round();
    let door = |m: Vec3| {
        corrs.iter().any(|k| {
            let (ax, az) = if k.along_x { (3.5, 0.5) } else { (0.5, 3.5) };
            m.x >= k.x0 - ax - 0.5 && m.x < k.x1 + ax + 0.5 && m.z >= k.z0 - az - 0.5 && m.z < k.z1 + az + 0.5
        })
    };
    (0u8..4)
        .find(|&s| {
            let mut a = len - 1.0 - deck - 1.5;
            while a < len - 0.5 {
                for deep in [0.5, 1.5, 2.5, DEPTH + 0.7] {
                    let (along, across) = (if s & 2 != 0 { len - a } else { a }, if s & 1 != 0 { deep } else { wid - deep });
                    let (rx, rz) = if w >= d { (along, across) } else { (across, along) };
                    if door(Vec3::new(r.x0 + rx, r.floor + 1.0, r.z0 + rz)) {
                        return false;
                    }
                }
                a += 0.5;
            }
            true
        })
        .unwrap_or(NO_STAIR)
}

/// From generator 53 ceiling beams and coffer rosettes stop at the gallery.
/// A beam one voxel under a 6 m ceiling left a metre over the 4 m deck and the
/// top step, so the deck could not be climbed to.
pub const HEADROOM: u32 = 53;

/// Whether a room-local column lies over the deck or its stair.
pub fn under(c: &Complex, r: &Room, rx: f32, rz: f32) -> bool {
    if c.version < HEADROOM || !has(c, r) {
        return false;
    }
    let (along, across, len, wid) = frame(r, rx, rz, r.stair);
    let deep = across.min(wid - across);
    (along >= 1.0 && along <= len - 1.0) && (deep < DEPTH || (across > wid * 0.5 && deep < DEPTH + STAIR))
}

/// Whether a room-local point lies in the clear apron along the stair's open
/// side (generator 53): shelves and pier sconces used to wall off its foot.
pub fn approach(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32) -> bool {
    if c.version < HEADROOM || !has(c, r) || !(0.0..3.0).contains(&fy) {
        return false;
    }
    let (along, across, len, wid) = frame(r, rx, rz, r.stair);
    let deep = wid - across;
    let deck = DECK.round();
    across > wid * 0.5 && (DEPTH + STAIR..DEPTH + STAIR + 1.5).contains(&deep)
        && along >= len - 2.0 - deck && along <= len - 1.0
}

/// The gallery's blocks at a room-local point, if any.
pub fn sample(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let stair = if c.version >= super::fine::VERSION { r.stair } else { 0 };
    let (along, across, len, wid) = frame(r, rx, rz, stair);
    let deep = across.min(wid - across);
    if deep >= DEPTH + STAIR || along < 1.0 || along > len - 1.0 {
        return None;
    }
    let deck = (DECK / v).round() * v;
    // The stair: on the room side of the +across railing, rising from the far
    // end toward the middle, one step a voxel; it lands through a gap in the
    // rail onto the deck.
    let (s0, s1) = (len - 1.0 - deck, len - 1.0);
    let plus = across > wid * 0.5;
    if deep >= DEPTH {
        if plus && along >= s0 && along < s1 {
            // Steps, and clear headroom over them: no column or shelf stands
            // on a stair.
            let top = ((s1 - along) / v).ceil() * v;
            // From generator 53 the stair is clear to the ceiling: a column
            // over it left 2.5 m, one voxel short of a climb at 1 m.
            let clear = c.version >= HEADROOM || fy < top.min(deck) + 2.5;
            return clear.then_some(if fy < top.min(deck) { c.pal.trim } else { B::Air });
        }
        return None;
    }
    let gap = plus && along >= s0 - 1.5 && along < s0 + v;
    if (deck - v..deck).contains(&fy) {
        return Some(if deep >= DEPTH - v { c.pal.trim } else { c.pal.floor });
    }
    // Railing with a lamp on every other post.
    let rail = deep >= DEPTH - v && !gap;
    if rail && (deck..deck + 1.0).contains(&fy) {
        return Some(c.pal.trim);
    }
    if rail && (deck + 1.0..deck + 1.0 + v).contains(&fy) && near_line(along, 4.0, v, 0.0) {
        return Some(B::Torch);
    }
    None
}
