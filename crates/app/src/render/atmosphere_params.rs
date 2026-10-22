use crate::world::{CloudControls, LightingState};
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AtmosphereMode {
    AnalyticFallback,
    TransmittanceLut,
    #[default]
    SkyViewLut,
}

impl AtmosphereMode {
    pub fn as_u32(self) -> u32 {
        match self {
            Self::AnalyticFallback => 0,
            Self::TransmittanceLut => 1,
            Self::SkyViewLut => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AnalyticFallback => "Analytic Fallback",
            Self::TransmittanceLut => "Transmittance LUT",
            Self::SkyViewLut => "Sky-View LUT",
        }
    }

    pub fn all() -> [Self; 3] {
        [
            Self::AnalyticFallback,
            Self::TransmittanceLut,
            Self::SkyViewLut,
        ]
    }
}

/// CPU-side atmosphere inputs that drive runtime LUT generation.
///
/// These are intentionally separate from the packed lighting uniform so the
/// atmosphere runtime can evolve toward its own parameter block without waiting
/// for the full render uniform refactor.
#[derive(Clone, Copy, Debug)]
pub struct AtmosphereParameters {
    pub sun_dir: Vec3,
    pub moon_dir: Vec3,
    pub sun_color: Vec3,
    pub moon_color: Vec3,
    pub sky_top: Vec3,
    pub sky_horizon: Vec3,
    pub fog_color: Vec3,
    pub daylight: f32,
    pub twilight: f32,
    pub cloud_cover: f32,
}

impl AtmosphereParameters {
    pub fn from_lighting(lighting: &LightingState, cloud_controls: &CloudControls) -> Self {
        Self {
            sun_dir: lighting.sun_dir.normalize_or_zero(),
            moon_dir: lighting.moon_dir.normalize_or_zero(),
            sun_color: Vec3::from_array(lighting.sun_color),
            moon_color: Vec3::from_array(lighting.moon_color),
            sky_top: Vec3::from_array(lighting.sky_top),
            sky_horizon: Vec3::from_array(lighting.sky_horizon),
            fog_color: Vec3::from_array(lighting.fog_color),
            daylight: saturate(lighting.daylight),
            twilight: saturate(lighting.twilight),
            cloud_cover: saturate(cloud_controls.coverage * cloud_controls.density),
        }
    }

    pub fn approximately_matches(self, other: Self) -> bool {
        const COLOR_EPSILON: f32 = 0.002;
        const DIR_EPSILON: f32 = 0.0005;
        const SCALAR_EPSILON: f32 = 0.002;

        vec3_distance(self.sun_dir, other.sun_dir) <= DIR_EPSILON
            && vec3_distance(self.moon_dir, other.moon_dir) <= DIR_EPSILON
            && vec3_distance(self.sun_color, other.sun_color) <= COLOR_EPSILON
            && vec3_distance(self.moon_color, other.moon_color) <= COLOR_EPSILON
            && vec3_distance(self.sky_top, other.sky_top) <= COLOR_EPSILON
            && vec3_distance(self.sky_horizon, other.sky_horizon) <= COLOR_EPSILON
            && vec3_distance(self.fog_color, other.fog_color) <= COLOR_EPSILON
            && (self.daylight - other.daylight).abs() <= SCALAR_EPSILON
            && (self.twilight - other.twilight).abs() <= SCALAR_EPSILON
            && (self.cloud_cover - other.cloud_cover).abs() <= SCALAR_EPSILON
    }
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn vec3_distance(a: Vec3, b: Vec3) -> f32 {
    (a - b).abs().max_element()
}
