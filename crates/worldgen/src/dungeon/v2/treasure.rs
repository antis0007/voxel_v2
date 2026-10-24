//! What a complex pays out: the sealed core's offering and the caches that
//! reward a walk to a dead end.
use super::{Complex, Room, dress::h, kit};
use crate::BlockId as B;

/// Where a dead end keeps its cache: just off the aisle cross, in the first
/// quadrant (from a hashed one) that no doorway keeps clear.
pub fn cache_spot(c: &Complex, r: &Room) -> (f32, f32) {
    let first = (h(r, 41) * 4.0) as u32;
    let spot = |q: u32| {
        let (sx, sz) = (if q & 1 == 0 { -1.0 } else { 1.0 }, if q & 2 == 0 { -1.0 } else { 1.0 });
        (r.w() * 0.5 + sx * 2.2, r.d() * 0.5 + sz * 2.2)
    };
    let clear = |(x, z): (f32, f32)| {
        [(-1.3f32, -1.3f32), (1.3, -1.3), (-1.3, 1.3), (1.3, 1.3)]
            .into_iter()
            .all(|(ox, oz)| !kit::doorway(c, glam::Vec3::new(r.x0 + x + ox, r.floor + 0.8, r.z0 + z + oz)))
    };
    (0..4).map(|k| spot((first + k) % 4)).find(|&p| clear(p)).unwrap_or(spot(first))
}

/// A salvage crate on a stone plinth, a torch beside it.
pub fn cache(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (cx, cz) = cache_spot(c, r);
    let (dx, dz) = ((rx - cx).abs(), (rz - cz).abs());
    if dx.max(dz) < 0.6 {
        if fy < 0.5 {
            return Some(B::Cobblestone);
        }
        return (fy < 0.5 + v).then_some(B::SalvageCrate);
    }
    // The torch stands on the floor on the side away from the aisle.
    let (tx, tz) = (cx + (cx - r.w() * 0.5).signum() * 1.2, cz);
    ((rx - tx).abs() < v * 0.5 + 0.01 && (rz - tz).abs() < v * 0.5 + 0.01 && fy < v).then_some(B::Torch)
}

/// The objective: a raised dais, a cache on a plinth, offerings around it.
pub fn core(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (cx, cz) = (r.w() * 0.5, r.d() * 0.5);
    let dist = (rx - cx).hypot(rz - cz);
    if dist < 3.5 && fy < 0.5 {
        return Some(c.pal.trim);
    }
    if dist < 0.8 && fy < 0.5 + v {
        return Some(B::SurveyCache);
    }
    let ring = (dist - 2.6).abs() < 0.5 && (rx - cx).atan2(rz - cz).rem_euclid(std::f32::consts::FRAC_PI_2) < 0.35;
    if ring && fy < 0.5 + 2.0 * v {
        return Some(if fy < 0.5 + v { c.pal.alt } else { c.pal.accent });
    }
    None
}
