//! GPU checks of the transition band: resident mask discard and tile skirts.

use super::band::GlobeMask;
use super::bake::TileData;
use super::chart;
use super::gpu_tests::{look, render_masked};
use super::tile::{TILE_SAMPLES, TileId};
use crate::render::coverage::ResidentCoverage;
use glam::DVec3;

const CLEAR: [u8; 4] = [255, 0, 255, 255];

/// Pixel of chart point `p` (face-local on face 0) under `view_proj`.
fn pixel(view_proj: glam::Mat4, eye_body: DVec3, p: DVec3, size: u32) -> usize {
    let clip = view_proj * (chart::to_body(p) - eye_body).as_vec3().extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * size as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * size as f32) as u32;
    (y * size + x) as usize
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_discards_inside_the_resident_mask_and_draws_outside() {
    // Face 0 chart coordinates equal world metres, so chunk column (0, 0) spans 0..32 m.
    let mut coverage = ResidentCoverage::default();
    coverage.header = [-4, -4, 9, 1];
    assert!(coverage.set_column(0, 0));
    let centre = DVec3::new(16.0, 0.0, 16.0);
    let eye_body = chart::to_body(centre + DVec3::new(0.0, 2_000.0, 0.0));
    let view_proj = look(eye_body, chart::to_body(centre), 10.0);
    let mask = GlobeMask::new(coverage, 32.0, 0.0, centre);
    let size = 256;
    let frame = render_masked(vec![TileData::empty(TileId::root(0))], eye_body, view_proj, size, Some(mask));
    let at = pixel(view_proj, eye_body, centre, size);
    let (inside, outside) = (frame.color[at], frame.color[pixel(view_proj, eye_body, DVec3::new(112.0, 0.0, 16.0), size)]);
    assert_eq!(inside, CLEAR, "resident column must show the clear colour");
    // Discarded, not merely transparent: a depth write would hide the chunks' own fallback.
    assert_eq!(frame.depth[at], 0.0, "resident column must not write depth");
    assert_ne!(outside, CLEAR, "three columns away the globe draws");
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_fills_unowned_columns_inside_the_disc_and_fades_past_it() {
    let mut coverage = ResidentCoverage::default();
    coverage.header = [-4, -4, 9, 1];
    assert!(coverage.set_column(0, 0));
    // The disc reaches x = 128 m: columns (0, 0) through (3, 0); only (0, 0) is owned.
    let eye_xz = DVec3::new(16.0, 0.0, 16.0);
    let radius = 112.0;
    let eye_body = chart::to_body(eye_xz + DVec3::new(0.0, 2_000.0, 0.0));
    let view_proj = look(eye_body, chart::to_body(eye_xz), 20.0);
    let size = 512;
    let mask = GlobeMask::new(coverage, 32.0, radius, eye_xz);
    let frame = render_masked(vec![TileData::empty(TileId::root(0))], eye_body, view_proj, size, Some(mask));
    let at = |x: f64| frame.color[pixel(view_proj, eye_body, DVec3::new(x, 0.0, 16.0), size)];
    assert_eq!(at(16.0), CLEAR, "owned column");
    let inside = at(80.0);
    assert_ne!(inside, CLEAR, "unowned column inside the disc must be globe");
    // Half way through the fade the clear colour and the globe mix about evenly.
    let half = at(16.0 + f64::from(radius) + 128.0);
    let mid = (255.0 + f64::from(inside[0])) * 0.5;
    assert!((f64::from(half[0]) - mid).abs() < 20.0, "fade midpoint {half:?}, globe {inside:?}");
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_level_mismatch_seam_shows_no_background() {
    level_mismatch_seam(-1.0);
}

#[test]
#[ignore = "needs a GPU; run with just test-gpu gpu_globe"]
fn gpu_globe_level_mismatch_seam_shows_no_background_from_the_fine_side() {
    level_mismatch_seam(1.0);
}

/// `side` -1 looks from over the coarse tile, +1 from over the fine one.
fn level_mismatch_seam(side: f64) {
    // A level-1 tile beside a level-2 tile on the shared edge x = 0 of face 2. The fine edge
    // spikes 200 m at odd samples; without skirts the coarse edge leaves slots onto the sky.
    let coarse = TileData::empty(TileId { face: 2, level: 1, x: 0, y: 0 });
    let fine_id = TileId { face: 2, level: 2, x: 2, y: 0 };
    let mut fine = TileData::empty(fine_id);
    for j in (1..TILE_SAMPLES).step_by(2) {
        fine.height_m[(j * TILE_SAMPLES) as usize] = 200;
    }
    let target = chart::join(engine_core::cube_sphere::Face::ALL[2], DVec3::new(0.0, 0.0, -12_000.0));
    let up = chart::to_body(target).normalize();
    // Looking across the edge so the slots face the eye.
    let west = (chart::to_body(target + DVec3::new(side * 100.0, 0.0, 0.0)) - chart::to_body(target)).normalize();
    let eye_body = chart::to_body(target) + (up * 0.8 + west * 0.6).normalize() * 3_000.0;
    let frame = render_masked(vec![coarse, fine], eye_body, look(eye_body, chart::to_body(target), 20.0), 256, None);
    let background = frame.color.iter().filter(|px| **px == CLEAR).count();
    assert_eq!(background, 0, "background pixels through the seam");
}
