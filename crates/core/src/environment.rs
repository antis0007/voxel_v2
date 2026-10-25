//! Physical units shared by environmental solvers and their world adapter.
//! One water unit is one microlitre, one stock unit one milligram. The integer
//! lattice represents all supported voxel volumes exactly, including eighths.
use crate::VoxelScalePreset;

pub const MICROLITRES_PER_LITRE: u64 = 1_000_000;
pub const MICROLITRES_PER_CUBIC_METRE: u64 = 1_000_000_000;
pub const MILLIGRAMS_PER_KILOGRAM: u64 = 1_000_000;
/// Intrinsic mineral-grain density, excluding pore space and stored water.
pub const SEDIMENT_GRAIN_KG_M3: u64 = 2_650;
pub fn sediment_volume(milligrams: u64) -> u64 {
    (milligrams * 1_000).div_ceil(SEDIMENT_GRAIN_KG_M3)
}
pub const ENVIRONMENT_GENERATOR: u32 = 42;
pub const ENVIRONMENT_MODEL: u32 = 1;

pub fn cell_volume(scale: VoxelScalePreset) -> u64 {
    match scale {
        VoxelScalePreset::OneMeter => 1_000_000_000,
        VoxelScalePreset::HalfMeter => 125_000_000,
        VoxelScalePreset::QuarterMeter => 15_625_000,
    }
}

pub fn face_area_cm2(scale: VoxelScalePreset) -> u64 {
    match scale {
        VoxelScalePreset::OneMeter => 10_000,
        VoxelScalePreset::HalfMeter => 2_500,
        VoxelScalePreset::QuarterMeter => 625,
    }
}

/// Material laws, independent of block names. Velocities are micrometres/s;
/// density is kg/m3. Plant and erosion acceleration belongs to the model,
/// never to these units or a rendering-quality preset.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct SoilLaw {
    pub porosity_per_mille: u32,
    /// Fraction of pore capacity retained against free gravity drainage.
    /// Root uptake and evaporation still consume this water.
    pub retention_per_mille: u32,
    pub infiltration_um_s: u32,
    pub drainage_um_s: u32,
    pub erosion_resistance_kj_kg: u32,
    pub density_kg_m3: u32,
}

impl SoilLaw {
    pub fn capacity(self, scale: VoxelScalePreset) -> u64 {
        cell_volume(scale) * u64::from(self.porosity_per_mille) / 1_000
    }
    pub fn mass(self, scale: VoxelScalePreset) -> u64 {
        cell_volume(scale) * u64::from(self.density_kg_m3) / 1_000
    }
    pub fn retained_water(self, scale: VoxelScalePreset) -> u64 {
        self.capacity(scale) * u64::from(self.retention_per_mille) / 1_000
    }
}

/// A surface flux in um/s across cm2, integrated for an integer number of s.
/// Keeping tenths of microlitres in `carry` prevents small faces losing flux.
pub fn surface_flux(speed_um_s: u32, area_cm2: u64, seconds: u32, carry: &mut u64) -> u64 {
    let tenths = u64::from(speed_um_s) * area_cm2 * u64::from(seconds) + *carry;
    *carry = tenths % 10;
    tenths / 10
}
