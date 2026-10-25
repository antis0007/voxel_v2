//! Exercise the complete production model batch: habitat -> enemies -> tool.
use super::*;
use crate::{mesh::models, player::Player, world::{DayNightCycle, WorldType}};
use engine_core::VoxelScalePreset;

#[test]
fn model_materials_keep_terrain_off_creatures_and_held_tools() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        let mut world = World::new(WorldType::Superflat, scale);
        world.generator_version = 1;
        let mut player = Player::new();
        player.pos = Vec3::new(0.0, 80.0, 7.0) / scale.meters();
        player.precise_pos = player.pos.as_dvec3();
        world.adventure.enemies = [0, 3, 5, 11, 12].into_iter().enumerate()
            .map(|(i, kind)| Enemy::new(IVec3::ZERO, i as u32, kind,
                Vec3::new(i as f32 * 2.0, 80.0, 0.0), 0)).collect();
        world.adventure.melee_cd = world.adventure.swing_duration() * 0.9;
        world.adventure.wand = 0;
        for weapon in [0, 5] {
            world.adventure.weapon = weapon;
            let vertices = models::build(&world, &player, &DayNightCycle::new(0.5, 600.0), 0.001);
            assert!(!vertices.is_empty());
            for v in &vertices {
                let family = v.surface[2].round() as u32 % 64;
                assert!(!matches!(family, 0..=4 | 7 | 16 | 33 | 36),
                    "terrain family {family} leaked into a model at {:?}", v.pos);
            }
            let held: Vec<_> = vertices.iter().filter(|v| v.surface[2] >= 262_144.0).collect();
            assert!(held.iter().any(|v| v.surface[2] as u32 % 64 == 59), "missing metal head");
            assert!(held.iter().any(|v| v.surface[2] as u32 % 64 == 54), "missing skin material");
            assert!(held.iter().any(|v| v.surface[2] as u32 % 64 == models::AUTHORED_SURFACE as u32), "missing authored haft");
            models::recycle(vertices);
        }
    }
}
