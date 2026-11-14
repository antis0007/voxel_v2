// Opaque tuft blades: physical height above the root is stored in surface.y.
// At most 8 cm displacement (4 cm in a breeze); roots stay fixed. Color/depth and shadow compose
// this exact function. Placement membership and stationary pebbles never move.
// Displacement only, in voxels; the phase reads the absolute position `p`.
fn tuft_wind_offset(p: vec3<f32>, height_m: f32, scale: f32, time: f32, wind: vec2<f32>) -> vec3<f32> {
    let speed = length(wind);
    let direction = wind / max(speed, 0.0001);
    // Anchor the phase in the world so weather turning does not jump the gust.
    let phase = dot(p.xz * scale, vec2<f32>(0.73, 0.41));
    let tip = clamp(height_m / 0.40, 0.0, 1.0);
    // Wind is real m/s: a breeze reaches the 4 cm bend by ~8 m/s, a gale to an
    // eyewall bends up to twice as far.
    let force = clamp(speed * 0.12, 0.0, 1.0) + clamp((speed - 8.3) / 40.0, 0.0, 1.0);
    let bend = tip * tip * 0.04 * force
        * (0.75 * sin(time * 1.25 - phase * 0.28) + 0.25 * sin(time * 2.9 + phase * 1.3));
    return vec3<f32>(direction.x, 0.0, direction.y) * (bend / max(scale, 0.0001));
}
