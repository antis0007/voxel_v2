//! Prints camera locations for reviewing the same synoptic systems used in game.
use engine_core::{cube_sphere::atlas, hash::{hash01, hash_u32}, planet_frame::PlanetAxis};
use engine_worldgen::sphere::CloudSky;
use glam::DVec3;

fn main() {
    let seed = hash_u32(hash_u32(0.23_f32.to_bits() ^ 480.0_f32.to_bits().rotate_left(13)) ^ 0x9E37_79B9);
    let seconds = std::env::args().nth(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(12.0);
    let sky = CloudSky::at(seed, PlanetAxis::new(DVec3::NEG_Z, 23.4_f64.to_radians()),
        f64::from(hash01(seed ^ 0x7EA7_0001)), 40.0 + seconds * 0.12);
    println!("weather_seed={seed} seconds={seconds}");
    for s in sky.storms() {
        let p = atlas::from_body(s.centre * (atlas::RADIUS_M + 12000.0));
        println!("{:?} strength={:.3} radius_m={:.0} camera={:.0},{:.0},12000,0,-1.56 spin={:.3}",
            s.kind, s.strength, s.radius * atlas::RADIUS_M, p.x, p.z, s.spin);
    }
}
