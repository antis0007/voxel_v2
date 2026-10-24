//! Generator 36 upper galleries: tall plain rooms (libraries, stores,
//! cisterns, sluices) get a deck along both long walls at 3.5 m, a railing
//! with lamps on its posts, and a straight stair beside the railing at one
//! end. Doorway clearances are carved before this runs, so a door under the
//! deck keeps its opening.
use super::{Complex, Kind, Outline, Room, kit::near_line};
use crate::BlockId as B;

/// Deck height over the floor, metres.
const DECK: f32 = 3.5;
/// Deck depth from the wall, metres; the walkway is clear of the wall piers.
const DEPTH: f32 = 3.0;
/// Width of the stair that climbs beside the railing, metres.
const STAIR: f32 = 1.4;

pub fn has(c: &Complex, r: &Room) -> bool {
    c.version >= super::RICH
        && r.outline == Outline::Rect
        && r.h >= 6.0
        && r.w().min(r.d()) >= 12.0
        && matches!(r.kind, Kind::Library | Kind::Store | Kind::Cistern | Kind::Sluice)
}

/// The gallery's blocks at a room-local point, if any.
pub fn sample(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (w, d) = (r.w(), r.d());
    let (along, across, len, wid) = if w >= d { (rx, rz, w, d) } else { (rz, rx, d, w) };
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
            return (fy < top.min(deck) + 2.5).then_some(if fy < top.min(deck) { c.pal.trim } else { B::Air });
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
