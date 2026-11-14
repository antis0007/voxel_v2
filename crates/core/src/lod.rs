//! Screen-space quality selection for distant textures and derived fields.

/// Projected sphere diameter, conservative when the observer is inside it.
pub fn sphere_diameter_px(radius: f64, distance: f64, focal_px: f64) -> f64 {
    let tangent = (distance * distance - radius * radius).max(1.0).sqrt();
    (2.0 * focal_px.max(1.0) * radius.max(0.0) / tangent).max(1.0)
}

/// Power-of-two sample count with hysteresis to avoid repeated resource changes.
/// `current` is the previous selection, zero on first use.
pub fn texture_resolution(pixels: f64, minimum: u32, maximum: u32, current: u32) -> u32 {
    let maximum = maximum.max(1).next_power_of_two();
    let minimum = minimum.max(1).next_power_of_two().min(maximum);
    let pixels = pixels.clamp(f64::from(minimum), f64::from(maximum));
    if current >= minimum
        && current <= maximum
        && current.is_power_of_two()
        && pixels > f64::from(current) * 0.4
        && pixels <= f64::from(current) * 1.15
    {
        return current;
    }
    (pixels.ceil() as u32)
        .next_power_of_two()
        .clamp(minimum, maximum)
}
