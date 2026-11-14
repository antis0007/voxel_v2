use super::{expedition, grand, shell};
use crate::{BlockId, WorldType};
use engine_core::VoxelScalePreset;
use glam::Vec3;

fn world(version: u32) -> crate::WorldSampleConfig {
    crate::WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed: 424242,
        world_type: WorldType::Manyfold,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

fn at(version: u32, sector: i32, z: f32, above: f32, along: f32) -> BlockId {
    let cfg = world(version);
    let twist = if version >= shell::GRAND {
        grand::twist(z)
    } else {
        0.0
    };
    let angle = sector as f32 * shell::pitch_for(version) + along - twist;
    let dist = shell::shell_radius(version, z) + above;
    let p = shell::center_of(version) + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z);
    let cell = (p / cfg.voxel_meters() - Vec3::splat(0.5))
        .floor()
        .as_ivec3();
    super::block(&cfg, cell)
}

fn promises(version: u32) {
    let row = shell::row_len(version);
    let mid = row * 0.5;
    let pitch = shell::pitch_for(version);
    let prior = shell::VAST;

    let book_z = mid * 0.25 + 4.5;
    let book_a = pitch * 0.18;
    assert_ne!(at(prior, 2, book_z, -1.8, book_a), BlockId::Bookcase);
    assert_eq!(at(version, 2, book_z, -1.8, book_a), BlockId::Bookcase);
    assert_eq!(
        at(
            version,
            2,
            (mid * 0.25 + mid * 0.55) * 0.5,
            -3.0,
            pitch * 0.20
        ),
        BlockId::Air
    );
    assert_eq!(
        at(version, 2, mid * 0.25 + 1.2, -4.0, pitch * 0.10),
        BlockId::Limestone
    );
    assert_eq!(
        at(version, 2, mid * 0.55 - 1.2, -2.0, pitch * 0.18),
        BlockId::Air
    );

    assert_eq!(at(version, 2, mid, -21.5, pitch * 0.65), BlockId::Limestone);
    assert_eq!(at(version, 2, mid, -18.0, pitch * 0.65), BlockId::Air);

    let stair_z = (mid * 0.55 + (mid - 2.0)) * 0.5;
    assert_eq!(
        at(version, 0, stair_z, -12.0, pitch * 0.65),
        BlockId::Limestone
    );

    let pool_z = row + (mid - 34.0);
    assert_ne!(at(prior, 4, pool_z, 0.8, pitch * 0.22), BlockId::Water);
    assert_eq!(at(version, 4, pool_z, 0.8, pitch * 0.22), BlockId::Water);
    assert_eq!(
        at(version, 4, row + (mid - 22.0), -1.0, pitch * 0.16),
        BlockId::Planks
    );

    let plug_z = row + mid;
    assert!(at(prior, 4, plug_z, -30.0, pitch * 0.5).is_solid() || version >= shell::GRAND);
    assert!(at(version, 4, plug_z, -30.0, shell::pitch_for(version) * 0.5).is_solid());
    assert_eq!(at(version, 3, mid, -2.0, 0.01), BlockId::Air);

    let cfg = world(version);
    let feet = super::spawn(&cfg).floor().as_ivec3();
    assert!(
        !super::block(&cfg, feet).is_solid(),
        "spawn {feet} is inside a wall"
    );
    let home = super::cradle(&cfg).expect("expedition cradle");
    let cell = (home / cfg.voxel_meters() - Vec3::splat(0.5))
        .floor()
        .as_ivec3();
    assert!(
        !super::block(&cfg, cell).is_solid(),
        "cradle {home} is inside a block"
    );
    let center = shell::center_of(version);
    let outward = Vec3::new(home.x - center.x, home.y - center.y, 0.0).normalize_or_zero();
    let under = (home + outward * 2.0) / cfg.voxel_meters() - Vec3::splat(0.5);
    assert!(super::block(&cfg, under.floor().as_ivec3()).is_solid());
    assert!(super::cradle(&world(prior)).is_none());
    assert!(!super::shortcut_cells(&cfg).is_empty());
}

#[test]
fn the_vast_shell_gains_the_expedition() {
    assert!(expedition::VERSION > shell::VAST);
    promises(expedition::VERSION);
}

#[test]
fn the_grand_drum_keeps_the_expedition() {
    assert!(shell::GRAND > expedition::VERSION);
    promises(shell::GRAND);
}
