//! Opt-in, unsaved in-engine movement review. Uses the normal player update.
use crate::{
    player::{FlyMode, Input, Player},
    world::{BlockId, World},
};
use glam::{IVec3, Vec3};

#[derive(Default)]
pub struct WaterReview {
    elapsed: f32,
    reset_flight: bool,
    jump_max: f32,
    flew: bool,
    reported: u8,
    captured_jump: bool,
    /// Open-air block scene instead of the walled pool.
    block: bool,
}
/// Metres above the local ground. Above the streamed surface, so the scene's
/// chunks are meshed fresh; ground-level edits to resident chunks did not show.
pub const OPEN_BLOCK_ELEVATION: i32 = 79;

impl WaterReview {
    pub fn open_block() -> Self {
        Self {
            block: true,
            ..Self::default()
        }
    }

    pub fn install(&mut self, world: &mut World, player: &mut Player) {
        if self.block {
            return Self::install_block(world, player);
        }
        let n = (1.0 / world.voxel_scale.meters()).round() as i32;
        let mut edits = Vec::new();
        for y in 95 * n..108 * n {
            for z in -5 * n..6 * n {
                for x in -5 * n..6 * n {
                    let b = if y < 96 * n
                        || x == -5 * n
                        || x == 6 * n - 1
                        || z == -5 * n
                        || z == 6 * n - 1
                    {
                        BlockId::StoneBricks
                    } else if y < 100 * n {
                        BlockId::Water
                    } else {
                        BlockId::Air
                    };
                    edits.push((IVec3::new(x, y, z), b));
                }
            }
        }
        world.set_many(&edits);
        Self::reset(player, world.voxel_scale.meters());
    }

    /// A stone floor with one free-standing 3×2 water block in open air: the
    /// case where a water cell's side faces are exposed to the sky.
    fn install_block(world: &mut World, player: &mut Player) {
        let n = (1.0 / world.voxel_scale.meters()).round() as i32;
        // Sit on the local ground, not at a fixed Y: a floor below the grass is
        // invisible and the block then looks like it never spawned.
        let base = world.terrain_height_at(0, 0) + OPEN_BLOCK_ELEVATION;
        let mut edits = Vec::new();
        for z in -5 * n..6 * n {
            for x in -5 * n..6 * n {
                edits.push((IVec3::new(x, base * n, z), BlockId::Stone));
            }
        }
        world.set_many(&edits);
        for z in -n..n {
            for x in 0..3 * n {
                world.set_water_volume(IVec3::new(x, (base + 1) * n, z), 8);
            }
        }
        let scale = world.voxel_scale.meters();
        *player = Player::new();
        player.pos = Vec3::new(-2.5, (base + 2) as f32 + 0.4, 0.5) / scale;
        player.precise_pos = player.pos.as_dvec3();
        player.fly_mode = FlyMode::Creative;
        player.pitch = -0.12;
    }
    fn reset(player: &mut Player, scale: f32) {
        *player = Player::new();
        player.pos = Vec3::new(0.5, 97.0, 0.5) / scale;
        player.precise_pos = player.pos.as_dvec3();
        player.fly_mode = FlyMode::Creative;
        player.pitch = 0.15;
    }
    pub fn input(&mut self, player: &mut Player, input: &mut Input, scale: f32, dt: f32) {
        self.elapsed += dt;
        if self.elapsed >= 6.0 && !self.reset_flight {
            Self::reset(player, scale);
            self.reset_flight = true;
        }
        input.clear();
        input.jump = (2.0..4.5).contains(&self.elapsed)
            || (6.5..6.58).contains(&self.elapsed)
            || (6.68..10.5).contains(&self.elapsed);
    }
    pub fn observe(&mut self, player: &Player, scale: f32) -> Option<&'static str> {
        let y = player.pos.y * scale;
        if self.elapsed < 6.0 {
            self.jump_max = self.jump_max.max(y);
        } else {
            self.flew |= player.flying && y > 101.0 && !player.swimming;
        }
        if self.elapsed < 6.0 && y > 100.2 && !self.captured_jump {
            self.captured_jump = true;
            return Some("jump-exit");
        }
        if self.elapsed >= 4.5 && self.reported == 0 {
            self.reported = 1;
            println!(
                "[water-review] jump={} scale={scale} peak_feet_m={:.3} surface_m=100 flying={}",
                if self.jump_max > 100.2 && !player.flying {
                    "PASS"
                } else {
                    "FAIL"
                },
                self.jump_max,
                player.flying
            );
            return None;
        }
        if self.elapsed >= 10.5 && self.reported == 1 {
            self.reported = 2;
            println!(
                "[water-review] flight={} scale={scale} feet_m={y:.3} swimming={} flying={}",
                if self.flew { "PASS" } else { "FAIL" },
                player.swimming,
                player.flying
            );
            return Some("flight-exit");
        }
        None
    }
}

#[cfg(test)]
mod block_scene_tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;

    #[test]
    fn open_block_scene_has_water_in_air() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut player = Player::new();
        WaterReview::open_block().install(&mut world, &mut player);
        let y = world.terrain_height_at(0, 0) + OPEN_BLOCK_ELEVATION + 1;
        assert_eq!(world.get(IVec3::new(1, y, 0)), BlockId::Water);
        assert_eq!(world.water_volume(IVec3::new(1, y, 0)), 8);
    }
}
