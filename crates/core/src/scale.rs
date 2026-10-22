/// Edge length of one voxel. Every world is built at one preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[allow(clippy::enum_variant_names)] // the unit is the point of the name
pub enum VoxelScalePreset {
    OneMeter,
    HalfMeter,
    QuarterMeter,
}

impl VoxelScalePreset {
    pub fn meters(self) -> f32 {
        match self {
            Self::OneMeter => 1.0,
            Self::HalfMeter => 0.5,
            Self::QuarterMeter => 0.25,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OneMeter => "1.00 m",
            Self::HalfMeter => "0.50 m",
            Self::QuarterMeter => "0.25 m",
        }
    }
}
