use engine_core::VoxelScalePreset;

/// New-world generator and maximum readable version. Keep historical version
/// gates literal: bumping this must never change how an older save generates.
pub const CURRENT_GENERATOR_VERSION: u32 = 30;
pub const WORLD_MIN_Y_METERS: f32 = -16.0;
pub const WORLD_MAX_Y_METERS: f32 = 512.0;
pub const WATER_LEVEL_METERS: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
pub enum WorldType {
    Superflat,
    Hills,
    Ridged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldSampleConfig {
    pub generator_version: u32,
    pub seed: u32,
    pub world_type: WorldType,
    pub voxel_scale: VoxelScalePreset,
}

impl WorldSampleConfig {
    pub fn voxel_meters(self) -> f32 {
        self.voxel_scale.meters()
    }

    pub fn inv_voxel_meters(self) -> f32 {
        self.voxel_meters().recip()
    }

    pub fn world_x(self, x: i32) -> f32 {
        x as f32 * self.voxel_meters()
    }

    pub fn world_y(self, y: i32) -> f32 {
        y as f32 * self.voxel_meters()
    }

    pub fn world_z(self, z: i32) -> f32 {
        z as f32 * self.voxel_meters()
    }

    pub fn blocks_for_height(self, meters: f32) -> i32 {
        (meters * self.inv_voxel_meters()).round() as i32
    }

    pub fn blocks_for_span(self, meters: f32) -> i32 {
        ((meters * self.inv_voxel_meters()).ceil() as i32).max(1)
    }

    pub fn min_world_y(self) -> i32 {
        (if self.generator_version >= 2 {
            -64.0
        } else {
            WORLD_MIN_Y_METERS
        } * self.inv_voxel_meters())
        .floor() as i32
    }

    pub fn max_world_y(self) -> i32 {
        (WORLD_MAX_Y_METERS * self.inv_voxel_meters()).ceil() as i32
    }

    pub fn water_level(self) -> i32 {
        self.blocks_for_height(WATER_LEVEL_METERS)
    }

    pub fn clamp_height(self, y: i32) -> i32 {
        y.clamp(self.min_world_y() + 4, self.max_world_y() - 3)
    }
}
