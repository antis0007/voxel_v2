use super::*;
use crate::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;

const SEED: u32 = 7;
const SEA: f64 = 12.0;

fn dirs() -> impl Iterator<Item = DVec3> {
    (0..400).map(|i| {
        let t = f64::from(i);
        DVec3::new((t * 0.37).sin(), (t * 0.11).cos(), (t * 0.23).sin() + 0.1).normalize()
    })
}

#[test]
fn the_floor_meets_the_coast_without_a_step() {
    for dir in dirs() {
        assert!(depth(SEED, dir, SEA - 0.01, SEA, 0.0) < 0.6, "{dir:?}");
    }
}

#[test]
fn shelves_stay_shallow_and_the_plain_sits_deep() {
    let mut shelf = 0;
    let mut plain = 0;
    for dir in dirs() {
        let w = shelf_width(SEED, dir, 0.0);
        let at = |u: f64| depth(SEED, dir, SEA - u * RAMP_M, SEA, 0.0);
        let inner = at(w * 0.4);
        let far = at(w + SLOPE_U * 3.0);
        shelf += u32::from(inner < 12.0);
        plain += u32::from((PLAIN_M - 4.0..PLAIN_M + 4.0).contains(&far));
    }
    assert!(shelf > 370 && plain > 390, "shelf {shelf} plain {plain} of 400");
}

#[test]
fn the_slope_is_steeper_than_the_shelf() {
    for dir in dirs().take(50) {
        let w = shelf_width(SEED, dir, 0.0);
        let at = |u: f64| depth(SEED, dir, SEA - u * RAMP_M, SEA, 0.0);
        let shelf_grad = (at(w * 0.8) - at(w * 0.2)) / (w * 0.6);
        let slope_grad = (at(w + SLOPE_U * 0.7) - at(w + SLOPE_U * 0.3)) / (SLOPE_U * 0.4);
        assert!(slope_grad > shelf_grad * 3.0, "{dir:?} shelf {shelf_grad} slope {slope_grad}");
    }
}

#[test]
fn mountain_coasts_have_narrow_shelves() {
    for dir in dirs().take(50) {
        assert!(shelf_width(SEED, dir, 1.0) < shelf_width(SEED, dir, 0.0));
    }
}

#[test]
fn channels_cut_the_outer_shelf_and_slope() {
    let mut cut = 0;
    for dir in dirs() {
        if channel(SEED, dir) > 0.5 {
            let w = shelf_width(SEED, dir, 0.0);
            let u = w + SLOPE_U * 0.3;
            let smooth = SHELF_EDGE_M + (PLAIN_M - SHELF_EDGE_M) * smoothstep(w, w + SLOPE_U, u);
            cut += u32::from(depth(SEED, dir, SEA - u * RAMP_M, SEA, 0.0) > smooth + 5.0);
        }
    }
    assert!(cut > 0, "no canyon found on the slope");
}

#[test]
fn planet_columns_gain_a_shelf_only_from_the_gate() {
    let cfg = |generator_version| WorldSampleConfig {
        planet: None,
        generator_version,
        seed: SEED,
        world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let (old, new) = (cfg(VERSION - 1), cfg(VERSION));
    let step = engine_core::cube_sphere::atlas::HALF_M / 8.0;
    let (mut sea, mut shallow_old, mut shallow_new, mut land_diff) = (0, 0, 0, 0);
    // Face NegX keeps clear of the solved spawn face.
    let face_x = engine_core::cube_sphere::atlas::PITCH_M as i32;
    for gz in -7..8 {
        for gx in -7..8 {
            let (x, z) = (face_x + (f64::from(gx) * step) as i32, (f64::from(gz) * step) as i32);
            if crate::basin::covers(&new, x, z) {
                continue;
            }
            let (a, b) = (crate::sample_column(&old, x, z), crate::sample_column(&new, x, z));
            if a.height >= old.water_level() {
                land_diff += u32::from(a.height != b.height && b.height < new.water_level());
                continue;
            }
            sea += 1;
            let wl = new.water_level();
            shallow_old += u32::from(a.height >= wl - 20);
            shallow_new += u32::from(b.height >= wl - 20);
        }
    }
    assert!(sea > 10, "only {sea} sea columns");
    assert!(shallow_new > shallow_old, "shelf columns old {shallow_old} new {shallow_new}");
    assert_eq!(land_diff, 0, "land columns sank under the sea");
}
