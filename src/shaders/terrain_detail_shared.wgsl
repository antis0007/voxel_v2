// Opaque tuft blades: physical height above the root is stored in surface.y.
// At most 4 cm displacement; roots stay fixed. Color/depth and shadow compose
// this exact function. Placement membership and stationary pebbles never move.
fn tuft_wind_position(p: vec3<f32>, height_m: f32, scale: f32, time: f32, wind: vec2<f32>) -> vec3<f32> {
    let speed = length(wind);
    let direction = wind / max(speed, 0.0001);
    let phase = dot(p.xz * scale, vec2<f32>(0.73, 0.41));
    let tip = clamp(height_m / 0.40, 0.0, 1.0);
    let bend = tip * tip * 0.04 * clamp(speed * 0.12, 0.0, 1.0)
        * (0.65 * sin(time * 1.7 + phase) + 0.35 * sin(time * 2.9 + phase * 1.3));
    return p + vec3<f32>(direction.x, 0.0, direction.y) * (bend / max(scale, 0.0001));
}
