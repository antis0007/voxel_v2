//! Shared Creative cruise and approach assistance, in metres and seconds.
use super::{FlightSettings, Input};
use glam::Vec3;

pub fn migrate(settings: &mut FlightSettings) {
    if settings.speed == 24.0 && settings.boost == 3.0 {
        settings.speed = 10.8;
        settings.boost = 2.0;
    }
}

pub fn base_speed(settings: &FlightSettings, input: &Input) -> f32 {
    let speed = if settings.speed.is_finite() { settings.speed.clamp(4.0, 64.0) } else { 10.8 };
    let boost = if settings.boost.is_finite() { settings.boost.clamp(1.0, 4.0) } else { 2.0 };
    speed * if input.sprint { boost } else { 1.0 }
}

/// The same cruise envelope on home and on a foreign body. Alt always gives
/// precise building/docking movement, independent of altitude or body frame.
pub fn speed(settings: &FlightSettings, input: &Input, altitude_m: f32) -> f32 {
    let base = base_speed(settings, input);
    if input.alt_down { return base * 0.2; }
    let cruise = (1.0 + (altitude_m - 250.0).max(0.0) / 80.0).min(180.0);
    (base * cruise).min(3000.0)
}

/// Braking distance, rather than a sudden switch at the collision ceiling,
/// limits a descending Creative pilot. Home uses the top of the voxel shell
/// as a conservative reference; lunar landing uses the sampled ground.
pub fn approach(velocity_mps: Vec3, up: Vec3, clearance_m: f32, settings: &FlightSettings, input: &Input) -> Vec3 {
    if velocity_mps.dot(up) >= 0.0 { return velocity_mps; }
    let base = base_speed(settings, input) * if input.alt_down { 0.2 } else { 1.0 };
    let clearance = clearance_m.max(0.0);
    // Blend out before the atmosphere boundary: crossing 6 km must not snap
    // cruise velocity down to the inner approach limit in a single step.
    let outer = (clearance - 3000.0).max(0.0);
    let limit = (base * base + 2.0 * 60.0 * clearance).sqrt() + outer * outer / 2000.0;
    velocity_mps.clamp_length_max(limit)
}

/// Sparse chunk lookups avoid terrain collision sampling in empty space, but
/// retain swept collision when a Creative pilot approaches an edited hull.
pub fn structure_on_path(world: &crate::world::World, from: glam::DVec3, to: glam::DVec3) -> bool {
    let size = crate::world::CHUNK_SIZE as f64;
    let lo = (from.min(to) / size).floor().as_ivec3() - glam::IVec3::ONE;
    let hi = (from.max(to) / size).floor().as_ivec3() + glam::IVec3::ONE;
    for z in lo.z..=hi.z { for y in lo.y..=hi.y { for x in lo.x..=hi.x {
        let q = glam::IVec3::new(x, y, z);
        if world.chunks.contains_key(&q) || world.regions.view.records.contains_key(&q) { return true; }
    } } }
    false
}
