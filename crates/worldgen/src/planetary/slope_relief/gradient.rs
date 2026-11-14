//! Gradients of blended relief in a tangent cube chart.
use glam::{DVec2, DVec3};

pub(super) fn blended_gradient(
    base: DVec2, correction: DVec2, weight: f64,
) -> DVec2 {
    base + correction * weight
}

/// Convert chart height derivatives through the local surface metric.
pub(super) fn physical_slope(gradient: DVec2, east: DVec3, north: DVec3) -> f64 {
    let xx = east.length_squared();
    let zz = north.length_squared();
    let xz = east.dot(north);
    let determinant = xx * zz - xz * xz;
    if determinant <= 1e-12 { return gradient.length(); }
    ((zz * gradient.x * gradient.x - 2.0 * xz * gradient.x * gradient.y
        + xx * gradient.y * gradient.y) / determinant).max(0.0).sqrt()
}

pub(super) fn body_gradient(gradient: DVec2, east: DVec3, north: DVec3) -> DVec3 {
    let xx = east.length_squared();
    let zz = north.length_squared();
    let xz = east.dot(north);
    (east * (zz * gradient.x - xz * gradient.y) + north * (xx * gradient.y - xz * gradient.x))
        / (xx * zz - xz * xz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposited_footslope_can_flatten_an_existing_slope() {
        let base = DVec2::new(0.8, 0.0);
        let fill = DVec2::new(-0.6, 0.0);
        assert!((blended_gradient(base, fill, 1.0).length() - 0.2).abs() < 1e-12);
        assert!((blended_gradient(base, -fill, 1.0).length() - 1.4).abs() < 1e-12);
        assert_eq!(blended_gradient(base, fill, 0.0), base);
    }

    #[test]
    fn physical_gradient_respects_stretched_and_oblique_chart_axes() {
        let east = DVec3::new(2.0, 0.0, 0.0);
        let north = DVec3::new(1.0, 0.0, 1.0);
        // Physical h = 0.3*x + 0.4*z gives chart derivatives 0.6 and 0.7.
        assert!((physical_slope(DVec2::new(0.6, 0.7), east, north) - 0.5).abs() < 1e-12);
    }
}
