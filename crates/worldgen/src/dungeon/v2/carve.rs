//! Generator 52 carved architecture at fine scales: wall piers get a
//! stepped base and a capital under the cornice, fluted shafts at a quarter
//! metre, and flat beamed ceilings become coffered, each coffer stepped back
//! and centred on a rosette. Wall fields between piers stay plain.
use super::{Outline, Room, kit::near_line};
use crate::BlockId as B;

/// Bottom of the cornice moulding over the floor, metres.
fn cornice(r: &Room) -> f32 {
    if r.h >= 4.0 { r.h - 0.6 } else { r.h }
}

/// How far a pier's base and capital reach past its shaft, metres.
pub const SPREAD: f32 = 0.5;

/// A pier's mouldings and flutes. `off` is the distance along the wall from
/// the pier's centre line, `dwall` the depth from the wall, `pd` the shaft's
/// depth. `Some(Air)` is a flute.
pub fn pier(r: &Room, trim: B, off: f32, dwall: f32, fy: f32, v: f32, pd: f32) -> Option<B> {
    if v > 0.5 || r.outline == Outline::Hewn || fy < 0.0 {
        return None;
    }
    let shaft = 0.6;
    // Two courses: a plinth block, then a torus a quarter metre narrower.
    let lower = (0.0..0.25).contains(&fy) && off < shaft + SPREAD && dwall < pd + SPREAD;
    let upper = (0.25..0.5).contains(&fy) && off < shaft + SPREAD * 0.5 && dwall < pd + SPREAD * 0.5;
    if lower || upper {
        return Some(trim);
    }
    let c0 = cornice(r);
    if c0 - 0.5 >= 3.0 {
        let abacus = (c0 - 0.25..c0).contains(&fy) && off < shaft + SPREAD && dwall < pd + SPREAD;
        let echinus = (c0 - 0.5..c0 - 0.25).contains(&fy) && off < shaft + SPREAD * 0.5 && dwall < pd + SPREAD * 0.5;
        if abacus || echinus {
            return Some(trim);
        }
    }
    // Flutes: every other quarter-metre column of the shaft's face is cut back.
    let face = dwall < pd && dwall >= pd - v && off > v * 0.5 && off < shaft;
    let groove = ((off / v).round() as i32).rem_euclid(2) == 1;
    if v <= 0.25 && face && groove && fy >= 0.75 && fy < c0 - 0.75 {
        return Some(B::Air);
    }
    None
}

/// Distance from `a` to the nearest line of a grid of `period` through `origin`.
fn gap(a: f32, period: f32, origin: f32) -> f32 {
    let d = (a - origin).rem_euclid(period);
    d.min(period - d)
}

/// Extra height of a coffered beam ceiling over its beams, metres: half a
/// metre inside each coffer, a quarter at its stepped rim (quarter metre
/// voxels only). `along` runs across the beams, `wid` is the room's width
/// along them.
pub fn coffer(along: f32, across: f32, wid: f32, bay: f32, dwall: f32, v: f32) -> f32 {
    if v > 0.5 || dwall < 0.75 {
        return 0.0;
    }
    let da = gap(along, bay, v * 0.5);
    let dc = gap(across, bay * 0.5, wid * 0.5);
    if da <= 0.5 || dc <= 0.4 {
        return 0.0;
    }
    if v <= 0.25 && (da <= 0.75 || dc <= 0.65) { 0.25 } else { 0.5 }
}

/// A rosette at the middle of each coffer, hung from its recess.
pub fn rosette(along: f32, across: f32, wid: f32, bay: f32, v: f32) -> bool {
    let da = (gap(along, bay, v * 0.5) - bay * 0.5).abs();
    let dc = (gap(across, bay * 0.5, wid * 0.5) - bay * 0.25).abs();
    v <= 0.5 && da + dc < v * 1.01 && !near_line(along, bay, v, 0.0)
}
