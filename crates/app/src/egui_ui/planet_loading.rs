//! Drainage-solve preview colours: every pixel is one solve cell.
use egui::ColorImage;
use engine_worldgen::geo::Preview;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub(super) fn image(p: &Preview) -> ColorImage {
    let n = p.n;
    let mut rgb = Vec::with_capacity(n * n * 3);
    for i in 0..n * n {
        let h = p.height[i];
        let c = if h < 0.0 {
            [22.0, 44.0, 74.0]
        } else if p.discharge_km2.as_ref().is_some_and(|d| d[i] > 2.5) {
            let d = p.discharge_km2.as_ref().map_or(0.0, |d| d[i]);
            mix([70.0, 120.0, 170.0], [150.0, 205.0, 240.0], (d / 60.0).min(1.0))
        } else {
            let (x, z) = (i % n, i / n);
            let up = p.height[z.saturating_sub(1) * n + x.saturating_sub(1)].max(0.0);
            let shade = (0.8 + (up - h) * 9.0).clamp(0.35, 1.35);
            let low = match &p.wet {
                Some(w) => mix([168.0, 150.0, 102.0], [78.0, 120.0, 70.0], w[i]),
                None => [120.0, 116.0, 98.0],
            };
            let ground = mix(low, [196.0, 190.0, 178.0], (h * 1.4 - 0.25).clamp(0.0, 1.0));
            ground.map(|v| v * shade)
        };
        rgb.extend(c.map(|v| v.clamp(0.0, 255.0) as u8));
    }
    ColorImage::from_rgb([n, n], &rgb)
}
