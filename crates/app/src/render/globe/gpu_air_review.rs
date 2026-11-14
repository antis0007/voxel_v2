//! Review frames of a synthetic planet under the shared air model; writes PNGs, asserts the limb.

use super::bake::TileData;
use super::chart::RADIUS_M;
use super::gpu_tests::{look, render_lit};
use super::tile::{TILE_SAMPLES, TileId};
use glam::{DVec3, Vec3};

/// Six root tiles: two-wave continents, sea below zero, sand coasts and snowy highs.
fn planet() -> Vec<TileData> {
    (0..6u8)
        .map(|face| {
            let id = TileId::root(face);
            let mut t = TileData::empty(id);
            for j in 0..TILE_SAMPLES {
                for i in 0..TILE_SAMPLES {
                    let p = id.sample_chart(i, j) / 1_000.0;
                    let k = (j * TILE_SAMPLES + i) as usize;
                    let f = f64::from(face) * 1.7;
                    let h = 260.0 * ((p.x * 0.21 + f).sin() + (p.y * 0.17 - f).cos()) + 90.0 * (p.x * 0.6 + p.y * 0.5).sin() - 120.0;
                    t.height_m[k] = h as i16;
                    t.water[k] = u8::from(h < 0.0);
                    t.material[k] = if h > 380.0 { 4 } else if h < 25.0 { 3 } else { 0 };
                    t.climate[k] = [0, 140, 0, 255];
                }
            }
            t
        })
        .collect()
}

fn save(name: &str, frame: &super::gpu_tests::Frame) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/visual-direction/globe-air");
    let _ = std::fs::create_dir_all(&dir);
    // Linear target; a 2.2 curve only so the PNG reads like the game.
    let bytes: Vec<u8> = frame.color.iter().flat_map(|p| {
        let g = |c: u8| ((f32::from(c) / 255.0).powf(1.0 / 2.2) * 255.0) as u8;
        [g(p[0]), g(p[1]), g(p[2]), 255]
    }).collect();
    image::save_buffer(dir.join(name), &bytes, frame.size, frame.size, image::ExtendedColorType::Rgba8).ok();
}

fn luminance(p: [u8; 4]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

#[test]
#[ignore = "needs a GPU; writes target/visual-direction/globe-air"]
fn gpu_globe_air_review_frames() {
    // Over the equator, clear of the polar aurora.
    let eye = DVec3::new(RADIUS_M + 60_000.0, 0.0, 0.0);
    let view = look(eye, DVec3::ZERO, 70.0);
    let size = 512;
    let sun_day = Vec3::X;
    // Terminator across the disc: the sun 80 degrees off the view axis.
    let sun_dusk = Vec3::new(0.17, 0.0, 0.98).normalize();
    let day = render_lit(planet(), eye, view, size, None, sun_day, [0.0; 4]);
    let dusk = render_lit(planet(), eye, view, size, None, sun_dusk, [0.0; 4]);
    let night = render_lit(planet(), eye, view, size, None, -sun_day, [1.0, 0.0, 0.0, 1.0]);
    let moonless = render_lit(planet(), eye, view, size, None, -sun_day, [0.0; 4]);
    save("day.png", &day);
    save("dusk.png", &dusk);
    save("night-moon.png", &night);
    save("night-moonless.png", &moonless);
    // The disc edge carries the blue limb: bluer than the disc centre.
    let at = |f: &super::gpu_tests::Frame, x: u32, y: u32| f.color[(y * f.size + x) as usize];
    let centre = at(&day, size / 2, size / 2);
    let disc = RADIUS_M / (RADIUS_M + 60_000.0);
    let focal = f64::from(size) * 0.5 / 35f64.to_radians().tan();
    let edge_px = (focal * disc.asin().tan()) as u32;
    let edge = at(&day, size / 2 + edge_px - 3, size / 2);
    let blue = |p: [u8; 4]| f32::from(p[2]) / (f32::from(p[0]) + 1.0);
    assert!(blue(edge) > blue(centre) * 1.3, "edge {edge:?} centre {centre:?}");
    // A risen moon lights the night side above the starlight floor.
    let (lit, dark) = (at(&night, size / 2, size / 2), at(&moonless, size / 2, size / 2));
    assert!(luminance(lit) > luminance(dark) + 1.0, "moonlit {lit:?} moonless {dark:?}");
}
