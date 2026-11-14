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

/// Sky and sunset art controls. This is the only owner of these decisions:
/// shaders read them from `Lighting::sky_art` or the LUT uniform's `art`
/// slot and keep no private multipliers for the same choice.
///
/// Solar convention for everything that consumes them: `LightingState::sun_dir`
/// is a unit world-space vector, Y up, pointing from the surface TOWARD the sun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyArt {
    /// Low aerosol haze extinction at sea level, 1/m. Shared by the
    /// transmittance, sky-view and aerial-perspective LUTs, so distant terrain
    /// and the horizon sky behind it come from the same air. Range 0..2e-4.
    pub haze_extinction: f32,
    /// Haze scale height, m. Range 300..3000.
    pub haze_scale_height: f32,
    /// Apparent solar disc radius, degrees. The real sun is 0.27; larger reads
    /// better at game field of view. Range 0.2..1.2.
    pub solar_radius_deg: f32,
    /// Narrow forward-scatter glare around the disc, as a fraction of disc
    /// radiance at one radius out. The broad aureole comes from the LUT's
    /// aerosol phase function, not from this. Range 0..0.5.
    pub solar_glare: f32,
    /// How far the authored sky palette may replace the physical LUT chroma,
    /// 0 = physical only. Range 0..1.
    pub sky_grade: f32,
    /// Chroma gain applied to the sun-side twilight sky only. 1 = physical.
    /// Range 0.6..1.6.
    pub sunset_saturation: f32,
    /// Lit/shaded separation of clouds. 1 = marched result. Range 0.5..1.5.
    pub cloud_contrast: f32,
    /// Exposure compensation, EV stops on top of the time-of-day exposure.
    pub exposure_ev: f32,
    /// Post-process bloom strength. Range 0..0.6.
    pub bloom_strength: f32,
    /// Weight of the authored fog colour inside the physical sky: the horizon
    /// seam blend and the heavy-cover veil in the aerial-perspective table.
    /// 1 = legacy behaviour, 0 = the LUTs alone. Range 0..1.
    pub fog_seam: f32,
}

impl SkyArt {
    pub const DEFAULT: Self = Self {
        // About 50% of the view lost to haze at 12 km and 10% at 1.5 km along the
        // ground. Real clear air would show almost none of that at game distances,
        // which left ranges 10 km out as crisp as the middle distance. Above 1e-4 the
        // horizon turns into a flat grey wall.
        haze_extinction: 6.0e-5,
        haze_scale_height: 1400.0,
        solar_radius_deg: 0.65,
        solar_glare: 0.08,
        sky_grade: 0.18,
        sunset_saturation: 1.0,
        cloud_contrast: 0.85,
        exposure_ev: 0.15,
        bloom_strength: 0.08,
        fog_seam: 1.0,
    };

    /// `DEFAULT` with review overrides applied, read once: `VOXEL_FOG_SEAM`,
    /// `VOXEL_SKY_GRADE`, `VOXEL_SUNSET_SATURATION`, `VOXEL_EXPOSURE_EV`,
    /// `VOXEL_CLOUD_CONTRAST`, `VOXEL_HAZE` (1/m), `VOXEL_HAZE_HEIGHT` (m). Lets one executable capture both sides of an A/B.
    pub fn active() -> Self {
        static ACTIVE: std::sync::OnceLock<SkyArt> = std::sync::OnceLock::new();
        *ACTIVE.get_or_init(|| {
            let mut art = Self::DEFAULT;
            let read = |name: &str, lo: f32, hi: f32| {
                std::env::var(name)
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .map(|v| v.clamp(lo, hi))
            };
            if let Some(v) = read("VOXEL_HAZE", 0.0, 2.0e-4) {
                art.haze_extinction = v;
            }
            if let Some(v) = read("VOXEL_HAZE_HEIGHT", 300.0, 3000.0) {
                art.haze_scale_height = v;
            }
            if let Some(v) = read("VOXEL_FOG_SEAM", 0.0, 1.0) {
                art.fog_seam = v;
            }
            if let Some(v) = read("VOXEL_SKY_GRADE", 0.0, 1.0) {
                art.sky_grade = v;
            }
            if let Some(v) = read("VOXEL_SUNSET_SATURATION", 0.6, 1.6) {
                art.sunset_saturation = v;
            }
            if let Some(v) = read("VOXEL_EXPOSURE_EV", -3.0, 3.0) {
                art.exposure_ev = v;
            }
            if let Some(v) = read("VOXEL_CLOUD_CONTRAST", 0.5, 1.5) {
                art.cloud_contrast = v;
            }
            art
        })
    }

    /// `Lighting::sky_art` in WGSL: [radius cos, glare, grade, saturation],
    /// [cloud contrast, exposure gain, fog seam, reserved].
    pub fn lighting_words(self) -> [[f32; 4]; 2] {
        [
            [
                self.solar_radius_deg.to_radians().cos(),
                self.solar_glare,
                self.sky_grade,
                self.sunset_saturation,
            ],
            [self.cloud_contrast, self.exposure_gain(), self.fog_seam, 0.0],
        ]
    }

    pub fn exposure_gain(self) -> f32 {
        self.exposure_ev.exp2()
    }
}

impl Default for SkyArt {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// CPU-side atmosphere inputs that drive runtime LUT generation.
///
/// These are intentionally separate from the packed lighting uniform so the
/// atmosphere runtime can evolve toward its own parameter block without waiting
/// for the full render uniform refactor.
#[derive(Clone, Copy, Debug)]
pub struct AtmosphereParameters {
    pub hell: bool,
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
    /// Today's aerosol regime plus regional air, applied to the preset.
    pub air: crate::world::sky_air::SkyAir,
}

impl AtmosphereParameters {
    pub fn from_lighting(lighting: &LightingState, cloud_controls: &CloudControls) -> Self {
        Self {
            hell: lighting.hell,
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
            air: lighting.sky_air,
        }
    }

    pub fn approximately_matches(self, other: Self) -> bool {
        const COLOR_EPSILON: f32 = 0.002;
        const DIR_EPSILON: f32 = 0.0005;
        const SCALAR_EPSILON: f32 = 0.002;

        self.hell == other.hell && vec3_distance(self.sun_dir, other.sun_dir) <= DIR_EPSILON
            && vec3_distance(self.moon_dir, other.moon_dir) <= DIR_EPSILON
            && vec3_distance(self.sun_color, other.sun_color) <= COLOR_EPSILON
            && vec3_distance(self.moon_color, other.moon_color) <= COLOR_EPSILON
            && vec3_distance(self.sky_top, other.sky_top) <= COLOR_EPSILON
            && vec3_distance(self.sky_horizon, other.sky_horizon) <= COLOR_EPSILON
            && vec3_distance(self.fog_color, other.fog_color) <= COLOR_EPSILON
            && (self.daylight - other.daylight).abs() <= SCALAR_EPSILON
            && (self.twilight - other.twilight).abs() <= SCALAR_EPSILON
            && (self.cloud_cover - other.cloud_cover).abs() <= SCALAR_EPSILON
            && !self.air.differs(other.air)
    }
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn vec3_distance(a: Vec3, b: Vec3) -> f32 {
    (a - b).abs().max_element()
}

#[cfg(test)]
mod haze_tests {
    use super::SkyArt;

    /// Haze-only transmittance along the ground for `distance_m`.
    fn haze_transmittance(distance_m: f32) -> f32 {
        (-SkyArt::DEFAULT.haze_extinction * distance_m).exp()
    }

    #[test]
    fn default_haze_gives_distant_terrain_depth_without_a_grey_wall() {
        assert!(haze_transmittance(1_500.0) > 0.88, "the middle distance must stay crisp");
        let far = haze_transmittance(12_000.0);
        assert!((0.35..0.65).contains(&far), "12 km keeps {far:.2} of the view");
        assert!(haze_transmittance(40_000.0) > 0.05, "even the horizon is not opaque");
    }

    #[test]
    fn default_haze_is_inside_its_documented_range() {
        let art = SkyArt::DEFAULT;
        assert!((0.0..=2.0e-4).contains(&art.haze_extinction));
        assert!((300.0..=3000.0).contains(&art.haze_scale_height));
    }
}
