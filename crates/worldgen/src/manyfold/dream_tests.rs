use super::{dream, shell};
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
    let angle = sector as f32 * shell::pitch() + along;
    let dist = shell::radius(z) + above;
    let p = super::CENTER + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z);
    let cell = (p / cfg.voxel_meters()).floor().as_ivec3();
    super::block(&cfg, cell)
}

#[test]
fn the_districts_are_different_and_the_white_shell_stays() {
    let mid = shell::pitch() * 0.5;
    assert_eq!(at(shell::VERSION, 1, 12.0, 0.8, mid), BlockId::WhitePlaster);
    assert_eq!(at(dream::VERSION, 1, 12.0, 0.8, mid), BlockId::Limestone);
    assert_eq!(at(dream::VERSION, 1, 92.0, 0.8, mid), BlockId::WeatheredBricks);
    assert_eq!(at(dream::VERSION, 1, 172.0, 0.8, mid), BlockId::Clay);
    assert_eq!(at(dream::VERSION, 1, 252.0, 0.8, mid), BlockId::CarvedStone);
}

#[test]
fn three_illusions_keep_their_promises() {
    let mid = shell::pitch() * 0.5;
    let dark = at(dream::VERSION, 3, 20.0, 0.4, mid);
    let under = at(dream::VERSION, 3, 20.0, 2.2, mid);
    assert_eq!(dark, BlockId::MirageVoid);
    assert!(!dark.is_solid());
    assert!(under.is_solid(), "the dark skin needs a floor under it");
    let skin = at(dream::VERSION, 0, 6.3, 0.4, mid);
    let room = at(dream::VERSION, 0, 6.3, 2.5, mid);
    let room_floor = at(dream::VERSION, 0, 6.3, 5.2, mid);
    assert_eq!(skin, BlockId::MirageBrick);
    assert!(!skin.is_solid());
    assert_eq!(room, BlockId::Air);
    assert!(room_floor.is_solid());
    assert_eq!(at(dream::VERSION, 8, 20.0, -1.0, 0.02), BlockId::MirageBrick);
    assert_eq!(at(dream::VERSION, 8, 20.0, -1.0, 0.07), BlockId::Air);
    assert!(at(dream::VERSION, 8, 20.0, -1.0, 0.13).is_solid());
}

#[test]
fn stillcourt_and_the_cistern_and_the_garden() {
    let mid = shell::pitch() * 0.5;
    assert_eq!(at(dream::VERSION, 0, 1.4, 0.4, mid), BlockId::Water);
    assert_eq!(at(dream::VERSION, 0, 4.0, 0.4, mid), BlockId::Planks);
    assert_eq!(at(dream::VERSION, 0, 172.0, 0.4, mid), BlockId::Water);
    let cfg = world(dream::VERSION);
    let feet = super::spawn(&cfg).floor().as_ivec3();
    assert!(!super::block(&cfg, feet).is_solid(), "spawn is inside a wall");
    let mut short = false;
    for sector in 0..16 {
        let low = at(dream::VERSION, sector, 500.0, -1.0, 0.01);
        let high = at(dream::VERSION, sector, 500.0, -8.0, 0.01);
        short |= low.is_solid() && high == BlockId::Air;
    }
    assert!(short, "the garden walls stay low");
}
