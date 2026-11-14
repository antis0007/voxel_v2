//! Planet shell sea floor: shelf, shelf break, slope and abyssal plain.
//!
//! Shelves are drowned lowland, so channels that cross them deepen into
//! canyons where the slope begins. The world floor sits about 70 m under the
//! sea surface, so the profile keeps Earth's shape, not its depths.
use crate::BlockId;
use crate::planet::noise_at;
use glam::DVec3;

/// Shaped sea floor. Older planets keep the clamped continental ramp.
pub const VERSION: u32 = 135;

/// Old ramp gradient in metres per unit of `continent`; offshore distance is measured in it.
const RAMP_M: f64 = 620.0;
const SHELF_EDGE_M: f64 = 18.0;
const PLAIN_M: f64 = 68.0;
/// Offshore distance the slope spans, in continent units.
const SLOPE_U: f64 = 0.03;

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Shelf width in continent units: wide off flat coasts, narrow off mountain coasts.
fn shelf_width(seed: u32, dir: DVec3, alpine: f64) -> f64 {
    let margin = noise_at(dir * 4.1, seed.wrapping_add(701));
    (0.04 + 0.16 * margin * margin) * (1.0 - 0.7 * alpine)
}

/// Thin ridged lines; one field serves shelf channels and slope canyons so they connect.
fn channel(seed: u32, dir: DVec3) -> f64 {
    let n = noise_at(dir * 70.0, seed.wrapping_add(709)) * 0.8 + noise_at(dir * 190.0, seed.wrapping_add(719)) * 0.2;
    (1.0 - (n * 2.0 - 1.0).abs()).powi(10)
}

/// Depth below the sea surface for a column whose ramp height `base` lies under `sea_m`.
pub fn depth(seed: u32, dir: DVec3, base: f64, sea_m: f64, alpine: f64) -> f64 {
    let u = ((sea_m - base) / RAMP_M).max(0.0);
    let w = shelf_width(seed, dir, alpine);
    let profile = if u < w {
        SHELF_EDGE_M * (u / w).powf(1.3)
    } else {
        SHELF_EDGE_M + (PLAIN_M - SHELF_EDGE_M) * smoothstep(w, w + SLOPE_U, u)
    };
    // Abyssal hills stay low so the plain reads flat.
    let plain = smoothstep(w + SLOPE_U * 0.7, w + SLOPE_U * 1.5, u);
    let hills = (noise_at(dir * 60.0, seed.wrapping_add(727)) - 0.5) * 3.0 * plain;
    // Channels start at zero on the coast, deepen toward the break and fade into fans on the plain.
    let cut = 3.0 + 22.0 * smoothstep(w * 0.6, w + SLOPE_U * 0.5, u);
    let reach = smoothstep(0.0, 0.015, u) * (1.0 - plain);
    (profile + hills + channel(seed, dir) * cut * reach).max(0.5)
}

/// Floor cover by depth; `None` leaves the shallow shelf to the climate cover.
pub fn floor_cover(depth: f64) -> Option<(BlockId, BlockId, f32)> {
    use BlockId::*;
    if depth < 24.0 {
        None
    } else if depth < PLAIN_M - 8.0 {
        Some((Gravel, Stone, 0.0))
    } else {
        Some((Mud, Clay, 0.0))
    }
}

#[cfg(test)]
#[path = "planet_ocean_tests.rs"]
mod tests;
