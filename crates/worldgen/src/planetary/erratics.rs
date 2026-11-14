//! Opt-in glacial erratic relief in body metres, independent of cube charts.
//! Bounded neighbour support keeps the same rock on either side of a cell.
use glam::DVec3;
use engine_core::cube_sphere::atlas;

pub const ERRATIC_RELIEF_GENERATOR: u32 = 307;
/// Solid rock deposited after erosion; historical307 relief stays unchanged.
pub const ERRATIC_DEPOSIT_GENERATOR: u32 = 308;
const PITCH: f64 = 32.0;

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
fn unit(key: u64, salt: u64) -> f64 {
    (mix(key ^ salt) >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}
fn ease(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// No allocation, cache, chart selection or per-frame state. The shell filter
/// and 3x3x3 neighbourhood are fixed for a candidate, not the sample position.
/// Support radius <= 7.2m and radial projection moves a centre <=16m: a supporting
/// centre cannot be more than one 32m cell away on any axis.
pub fn height(seed: u32, dir: DVec3, ice: f64, footprint: f32) -> f64 {
    let amplitude = ease(0.08, 0.30, ice)
        * (1.0 - ease(3.0, 12.0, f64::from(footprint)));
    if amplitude <= 0.0 { return 0.0; }
    let p = dir * atlas::RADIUS_M;
    let cell = (p / PITCH).floor();
    let mut height = 0.0_f64;
    for z in -1..=1 { for y in -1..=1 { for x in -1..=1 {
        let c = cell + DVec3::new(x as f64, y as f64, z as f64);
        let key = mix(seed as u64 ^ (c.x as i64 as u64).wrapping_mul(0x9e3779b185ebca87)
            ^ (c.y as i64 as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
            ^ (c.z as i64 as u64).wrapping_mul(0x165667b19e3779f9));
        if unit(key, 1) > 0.42 { continue; }
        let centre = (c + DVec3::new(0.2 + 0.6 * unit(key, 2),
            0.2 + 0.6 * unit(key, 3), 0.2 + 0.6 * unit(key, 4))) * PITCH;
        let radius_to_centre = centre.length();
        if (radius_to_centre - atlas::RADIUS_M).abs() > PITCH * 0.5 { continue; }
        let up = centre / radius_to_centre;
        let delta = p - up * atlas::RADIUS_M;
        let radius = 2.0 + 4.0 * unit(key, 5).powi(2);
        if delta.length_squared() > 1.44 * radius * radius { continue; }
        // Random body-space orientation, projected onto this rock's tangent.
        let axis = DVec3::new(unit(key, 6) - 0.5, unit(key, 7) - 0.5, unit(key, 8) - 0.5);
        let tangent = up.cross(axis).normalize_or_zero();
        if tangent == DVec3::ZERO { continue; }
        let side = up.cross(tangent);
        let u = delta.dot(tangent) / radius;
        let v = delta.dot(side) / (radius * (0.62 + 0.25 * unit(key, 9)));
        // Rounded angular shoulders, unequal axes and a sloping crown. The
        // squared edge envelope is C1 at zero: no needles at a scatter mask.
        let q = u.abs().powi(3) + v.abs().powi(3);
        if q >= 1.0 { continue; }
        let crown = (1.0 - q).powi(2) * (1.0 + 0.18 * u - 0.12 * v);
        height = height.max(radius * (0.55 + 0.18 * unit(key, 10)) * crown);
    } } }
    height * amplitude
}
