//! The borough's houses: a row of two-storey house volumes, one every seven
//! metres along the box, backed onto the street's wall. Each has a front
//! wall with a framed door and windows, party walls, an upper floor, a
//! pitched roof rising to the back, a table inside, and a step at the door.
//! Some have fallen in. A house whose door lines up with a passage is left
//! hollow so the passage runs through it.
use super::furnish::Here;
use super::palette;
use crate::undercrown::plan::Style;
use crate::{BlockId as B, hash2};

/// Bay width, metres.
const BAY: f32 = 7.0;
/// Depth in front of the front wall that steps and sills may use.
const STOOP: f32 = 1.0;

pub(super) fn houses(at: &Here, floor: f32, style: Style, seed: u32, passages: [f32; 2]) -> Option<B> {
    let (l, size) = (at.l(), at.size());
    let i = (l.x / BAY).floor() as i32;
    let u = l.x - i as f32 * BAY;
    let width = (size.x - i as f32 * BAY).min(BAY);
    let door = width * 0.5;
    // Depth behind the front face: negative in front of the house.
    let fz = (size.z - STOOP) - l.z;
    let fy = at.m.y - floor;
    let v = at.v;
    let [wall, alt, trim, _] = palette(style);
    let through = passages.iter().any(|&p| (p - (i as f32 * BAY + door)).abs() < 1.0);
    let ruin = hash2(i, 0, seed);
    let body = match hash2(i, 1, seed) {
        r if r < 0.33 => wall,
        r if r < 0.66 => alt,
        _ => B::Planks,
    };
    let fallen = ruin < 0.15 && !through;
    let eave = if fallen { 2.5 + hash2(i, 2, seed) * 2.5 } else { 6.2 + hash2(i, 3, seed) * 1.2 };
    let roofline = eave + fz.max(0.0) * 0.5;

    if fz < 0.0 {
        // The step at the door, and window sills at the finer scales.
        if !through && (u - door).abs() < 1.0 && fz >= -0.8 && fy < at.snap(0.4) {
            return Some(trim);
        }
        let sill = ((u - 1.6).abs() < 0.7 || (u - (width - 1.6)).abs() < 0.7) && fz >= -0.25 && (1.15..1.15 + v.max(0.15)).contains(&fy);
        let eaves = !fallen && fz >= -0.6 && fy >= eave - 0.25 && fy < eave - 0.25 + v.max(0.5);
        return if eaves { Some(B::WeatheredBricks) } else { sill.then_some(trim) };
    }
    if fy >= roofline + v.max(0.5) {
        return None;
    }
    if !fallen && fy >= roofline {
        return Some(B::WeatheredBricks);
    }
    let party = u < v.max(0.4);
    if party {
        return Some(alt);
    }
    if fz < 0.8 {
        let opening = (u - door).abs() < 0.8 && fy < 2.8;
        let windows = ((u - 1.6).abs() < 0.6 || (u - (width - 1.6)).abs() < 0.6) && ((fy - 1.8).abs() < 0.5 || (fy - 5.0).abs() < 0.6);
        if opening || windows {
            return None;
        }
        let frame = (u - door).abs() < 1.2 && fy < 3.3;
        let corner = u < 0.6 || u > width - 0.6;
        return Some(if frame || corner {
            trim
        } else if body == B::Planks && ((fy - 3.4).abs() < 0.3 || u.rem_euclid(2.0) < v.min(0.3)) {
            B::Log
        } else {
            body
        });
    }
    if through || fallen {
        return None;
    }
    // Upper floor, open over a stair corner at the back.
    if fy >= 3.4 && fy < 3.4 + v.max(0.25) && !(u > width - 2.0 && fz > 3.0) {
        return Some(B::Planks);
    }
    let table = (u - door).abs() < 1.0 && (fz - 2.8).abs() < 0.6 && fy < at.snap(0.9);
    table.then_some(B::Planks)
}
