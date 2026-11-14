//! Structural side aisles. The central nave, doors and stair approaches win.
use super::{Complex, Kind, Room};
use crate::BlockId as B;

pub(super) fn sample(c: &Complex, r: &Room, x: f32, z: f32, y: f32, v: f32) -> Option<B> {
    if !matches!(r.kind, Kind::Library | Kind::Scriptorium | Kind::Shrine
        | Kind::Refectory | Kind::Forge | Kind::Foundry) || y < 0.0 {
        return None;
    }
    let (along, across, width, length) = if r.w() > r.d() {
        (x,z,r.d(),r.w())
    } else { (z,x,r.w(),r.d()) };
    if width < 12.0 || along < 2.0 || along > length - 2.0 { return None; }
    let side = across.min(width - across);
    if (side - 2.2).abs() >= v.max(0.65) { return None; }
    let period = 6.0;
    let bay = (along - 2.0).rem_euclid(period);
    let pier = bay.min(period - bay) < v.max(0.65);
    let spring = 4.0;
    let arch = spring + 1.8 * (1.0 - ((bay - 3.0) / 3.0).powi(2)).max(0.0).sqrt();
    if pier && y < spring || y >= arch && y < arch + v.max(0.75) {
        return Some(if y < 1.0 || y > spring - 0.5 { c.pal.trim } else { c.pal.wall });
    }
    None
}
