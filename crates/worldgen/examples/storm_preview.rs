//! Tropical cyclones are the only storms under 0.08 rad with cyclonic spin.
//! Writes a top-down greyscale PGM of the strongest tropical cyclone's cloud density.
//!
//! `cargo run -p engine_worldgen --example storm_preview -- out.pgm [seed]`
use engine_core::planet_frame::PlanetAxis;
use engine_worldgen::sphere::CloudSky;
use glam::DVec3;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else { eprintln!("usage: storm_preview out.pgm [seed]"); return };
    let seed = args.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(7);
    let axis = PlanetAxis::new(DVec3::NEG_Z, 23.4_f64.to_radians());
    // Scan model time for the strongest tropical storm.
    let mut best: Option<(f64, f64)> = None;
    for i in 0..400 {
        let t = 40.0 + f64::from(i) * 0.5;
        let sky = CloudSky::at(seed, axis, 0.3, t);
        for s in sky.storms().iter().filter(|s| s.radius < 0.08 && s.spin > 0.0) {
            if best.is_none_or(|(_, strength)| s.strength > strength) { best = Some((t, s.strength)); }
        }
    }
    let Some((t, strength)) = best else { eprintln!("no tropical storm"); return };
    let sky = CloudSky::at(seed, axis, 0.3, t);
    let Some(s) = sky.storms().iter().filter(|s| s.radius < 0.08 && s.spin > 0.0)
        .max_by(|a, b| a.strength.total_cmp(&b.strength)) else { return };
    let size = 768usize;
    let span = 4.5 * s.radius;
    let mut out = format!("P5\n{size} {size}\n255\n").into_bytes();
    for j in 0..size {
        for i in 0..size {
            let u = (i as f64 / size as f64 - 0.5) * 2.0 * span;
            let v = (0.5 - j as f64 / size as f64) * 2.0 * span;
            let p = (s.centre + s.east * u + s.pole * v).normalize();
            out.push((sky.density(p).clamp(0.0, 1.0) * 255.0) as u8);
        }
    }
    if let Err(e) = std::fs::write(&path, out) { eprintln!("{e}"); return; }
    println!("time={t} strength={strength:.3} radius={:.4} -> {path}", s.radius);
}
