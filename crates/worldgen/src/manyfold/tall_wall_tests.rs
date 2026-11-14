//! White plaster partitions on the straight tube, in both grid directions.
use super::{block, spawn, CENTER, RADIUS, TALL_WHITE_WALLS};
use crate::{BlockId, WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;
use glam::Vec3;
use std::f32::consts::{PI, TAU};

fn world(version: u32, scale: VoxelScalePreset, seed: u32) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed,
        world_type: WorldType::Manyfold,
        voxel_scale: scale,
    }
}

fn cell_at(cfg: &WorldSampleConfig, angle: f32, z: f32, height: f32) -> glam::IVec3 {
    let radius = RADIUS - height;
    let p = CENTER + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z);
    (p / cfg.voxel_meters()).floor().as_ivec3()
}

fn quadrant(angle: f32) -> usize {
    ((angle.rem_euclid(TAU) / (PI / 2.0)).floor() as usize).min(3)
}

#[test]
fn tall_white_walls_stand_in_every_direction() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        for seed in [424242, 7] {
            let cfg = world(TALL_WHITE_WALLS, scale, seed);
            let old = world(80, scale, seed);
            let mut count = [0u32; 4];
            let mut along = [false; 4];
            let mut around = [false; 4];
            for i in 0..64 {
                let angle = i as f32 * TAU / 64.0;
                let q = quadrant(angle);
                for z_i in 0..12 {
                    let z = -18.0 + z_i as f32 * 3.0;
                    for height in [5.0, 7.5, 9.2] {
                        let cell = cell_at(&cfg, angle, z, height);
                        if block(&cfg, cell) != BlockId::WhitePlaster {
                            continue;
                        }
                        if block(&old, cell) != BlockId::Air {
                            continue;
                        }
                        count[q] += 1;
                        let centre = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters() - CENTER;
                        let arc = (centre.x.atan2(-centre.y) * RADIUS + TAU * RADIUS / 12.0)
                            .rem_euclid(TAU * RADIUS);
                        let bay = TAU * RADIUS / 24.0;
                        let u = arc % bay;
                        let v = (centre.z + 24.0).rem_euclid(8.0);
                        // Sector edges run along the tube. Row edges run around it.
                        along[q] |= u < 0.85 || u > bay - 0.85;
                        around[q] |= v < 0.85 || v > 8.0 - 0.85;
                    }
                }
            }
            assert!(
                count.iter().all(|n| *n > 0),
                "seed {seed} {scale:?} missing a quadrant: {count:?}"
            );
            assert!(
                along.iter().all(|hit| *hit) && around.iter().all(|hit| *hit),
                "seed {seed} {scale:?} walls do not run both ways: along {along:?} around {around:?}"
            );
        }
    }
}

#[test]
fn tall_walls_leave_the_hollow_and_the_ring_open() {
    let cfg = world(TALL_WHITE_WALLS, VoxelScalePreset::OneMeter, 424242);
    for i in 0..8 {
        let angle = i as f32 * TAU / 8.0;
        let hollow = cell_at(&cfg, angle, 8.0, 14.0);
        assert_eq!(block(&cfg, hollow), BlockId::Air, "hollow closed at {angle}");
        let street = cell_at(&cfg, angle, 0.0, 2.0);
        assert_eq!(block(&cfg, street), BlockId::Air, "ring closed at {angle}");
    }
    let spawn_cell = spawn(&cfg).floor().as_ivec3();
    assert!(
        !block(&cfg, spawn_cell).is_solid(),
        "spawn {spawn_cell} is inside a wall"
    );
}

#[test]
fn older_generators_keep_the_short_coloured_walls() {
    let old = world(80, VoxelScalePreset::OneMeter, 424242);
    let previous = world(100, VoxelScalePreset::OneMeter, 424242);
    for i in 0..32 {
        let angle = i as f32 * TAU / 32.0;
        for z in [-12.0, -6.0, 6.0, 12.0] {
            for height in [1.2, 5.0, 8.0] {
                let cell = cell_at(&old, angle, z, height);
                assert_ne!(block(&old, cell), BlockId::WhitePlaster);
                assert_ne!(block(&previous, cell), BlockId::WhitePlaster);
            }
        }
    }
}
