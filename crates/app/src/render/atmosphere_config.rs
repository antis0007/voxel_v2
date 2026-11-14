//! Physical atmosphere coefficients as data.
//!
//! `assets/atmosphere.ron` holds named presets; this module parses and
//! validates them and packs the active one for the LUT generation uniform in
//! `atmosphere_lut_generation.wgsl`. Time-of-day colour (sun, moon, sky) stays
//! in `AtmosphereParameters`; art decisions stay in `SkyArt`. Flow:
//! RON -> `AtmosphereConfig` -> `AtmospherePreset::uniform_words` -> uniform.

use serde::Deserialize;

const ATMOSPHERE_RON: &str = include_str!("../../../../assets/atmosphere.ron");

/// Radius the runtime sky lookups (`sky_shader.wgsl`, `atmosphere_shared.wgsl`)
/// still hard-code. Presets must match it until those read the uniform.
pub const RUNTIME_PLANET_RADIUS_M: f32 = 6_360_000.0;
/// Height of the transmittance table's altitude axis.
pub const MAX_ATMOSPHERE_HEIGHT_M: f32 = 60_000.0;

#[derive(Clone, Debug, Deserialize)]
pub struct AtmosphereConfig {
    pub active: String,
    pub presets: Vec<AtmospherePreset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AtmospherePreset {
    pub name: String,
    pub planet_radius_m: f32,
    pub atmosphere_height_m: f32,
    pub rayleigh_scattering: (f32, f32, f32),
    pub rayleigh_scale_height_m: f32,
    pub mie_scattering: (f32, f32, f32),
    pub mie_absorption: (f32, f32, f32),
    pub mie_scale_height_m: f32,
    pub mie_g: f32,
    pub ozone_absorption: (f32, f32, f32),
    pub ozone_center_m: f32,
    pub ozone_width_m: f32,
}

impl AtmosphereConfig {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self =
            ron::from_str(text).map_err(|error| format!("assets/atmosphere.ron: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if !self.presets.iter().any(|p| p.name == self.active) {
            return Err(format!("active preset {:?} is not defined", self.active));
        }
        self.presets.iter().try_for_each(AtmospherePreset::validate)
    }

    /// The preset named by `active`. Parsed configurations always contain it;
    /// a manually constructed configuration may be empty or name no preset.
    pub fn active_preset(&self) -> Option<&AtmospherePreset> {
        self.presets.iter().find(|p| p.name == self.active)
    }

    /// The preset selected in the shipped file. A malformed shipped file is a
    /// build defect caught by the tests below; the fallback keeps release safe.
    pub fn shipped_active() -> AtmospherePreset {
        match Self::parse(ATMOSPHERE_RON) {
            Ok(config) => config.active_preset().cloned().unwrap_or_else(AtmospherePreset::earthlike),
            Err(error) => {
                eprintln!("{error}; using built-in earthlike coefficients");
                AtmospherePreset::earthlike()
            }
        }
    }
}

impl AtmospherePreset {
    /// Dense volcanic aerosol absorbs blue/green preferentially. This is a
    /// transport preset, not a red filter over the ordinary planet's sky.
    pub fn hell() -> Self {
        Self { name: "hell".into(), planet_radius_m: RUNTIME_PLANET_RADIUS_M,
            atmosphere_height_m: 40000.0, rayleigh_scattering: (2.2e-6, 3.0e-6, 4.0e-6),
            rayleigh_scale_height_m: 6000.0, mie_scattering: (12.0e-6, 9.0e-6, 6.0e-6),
            mie_absorption: (2.0e-6, 11.0e-6, 25.0e-6), mie_scale_height_m: 1800.0, mie_g: 0.82,
            ozone_absorption: (0.0, 0.0, 0.0), ozone_center_m: 25000.0, ozone_width_m: 15000.0 }
    }
    pub fn earthlike() -> Self {
        Self {
            name: "earthlike".into(),
            planet_radius_m: RUNTIME_PLANET_RADIUS_M,
            atmosphere_height_m: MAX_ATMOSPHERE_HEIGHT_M,
            rayleigh_scattering: (5.8e-6, 13.5e-6, 33.1e-6),
            rayleigh_scale_height_m: 8000.0,
            mie_scattering: (3.0e-6, 3.0e-6, 3.0e-6),
            mie_absorption: (0.0, 0.0, 0.0),
            mie_scale_height_m: 1200.0,
            mie_g: 0.76,
            ozone_absorption: (0.650e-6, 1.881e-6, 0.085e-6),
            ozone_center_m: 25000.0,
            ozone_width_m: 15000.0,
        }
    }

    fn validate(&self) -> Result<(), String> {
        let name = &self.name;
        if !self.planet_radius_m.is_finite()
            || (self.planet_radius_m - RUNTIME_PLANET_RADIUS_M).abs() > 1.0
        {
            return Err(format!(
                "{name}: planet_radius_m must be {RUNTIME_PLANET_RADIUS_M} for now"
            ));
        }
        if !(1_000.0..=MAX_ATMOSPHERE_HEIGHT_M).contains(&self.atmosphere_height_m) {
            return Err(format!("{name}: atmosphere_height_m outside 1000..=60000"));
        }
        if !(-0.99..=0.99).contains(&self.mie_g) {
            return Err(format!("{name}: mie_g outside -0.99..=0.99"));
        }
        if !self.ozone_center_m.is_finite() || self.ozone_center_m < 0.0 {
            return Err(format!("{name}: ozone_center_m must be finite and non-negative"));
        }
        let positive = [
            self.rayleigh_scale_height_m,
            self.mie_scale_height_m,
            self.ozone_width_m,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(format!("{name}: scale heights and ozone width must be positive"));
        }
        let coefficients = [
            self.rayleigh_scattering,
            self.mie_scattering,
            self.mie_absorption,
            self.ozone_absorption,
        ];
        if coefficients
            .iter()
            .any(|c| [c.0, c.1, c.2].iter().any(|v| !v.is_finite() || *v < 0.0))
        {
            return Err(format!("{name}: coefficients must be finite and non-negative"));
        }
        Ok(())
    }

    /// Five vec4s matching `planet`, `beta_r_g`, `beta_m_width`,
    /// `beta_o_center`, `beta_ma` in `AtmosphereLutUniform`.
    pub fn uniform_words(&self) -> [[f32; 4]; 5] {
        let (r, m, a, o) = (
            self.rayleigh_scattering,
            self.mie_scattering,
            self.mie_absorption,
            self.ozone_absorption,
        );
        [
            [
                self.planet_radius_m,
                self.atmosphere_height_m,
                self.rayleigh_scale_height_m,
                self.mie_scale_height_m,
            ],
            [r.0, r.1, r.2, self.mie_g],
            [m.0, m.1, m.2, self.ozone_width_m],
            [o.0, o.1, o.2, self.ozone_center_m],
            [a.0, a.1, a.2, 0.0],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_file_parses_and_validates() {
        let config = AtmosphereConfig::parse(ATMOSPHERE_RON).expect("shipped atmosphere.ron");
        assert!(config.presets.iter().any(|p| p.name == "earthlike"));
        assert!(config.presets.iter().any(|p| p.name == "alien_default"));
    }

    #[test]
    fn shipped_earthlike_equals_the_pre_config_shader_constants() {
        let config = AtmosphereConfig::parse(ATMOSPHERE_RON).unwrap();
        let file = config.presets.iter().find(|p| p.name == "earthlike").unwrap();
        assert_eq!(file.uniform_words(), AtmospherePreset::earthlike().uniform_words());
    }

    #[test]
    fn invalid_presets_are_rejected() {
        let mut bad = AtmospherePreset::earthlike();
        bad.mie_g = 1.0;
        assert!(bad.validate().is_err());
        let mut bad = AtmospherePreset::earthlike();
        bad.planet_radius_m = 1.0e6;
        assert!(bad.validate().is_err());
    }
}
