/// Renderer-wide debug surfaces exposed through the existing UI -> uniform -> shader path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderDebugView {
    #[default]
    None,
    WeatherClipmapNear,
    WeatherClipmapMid,
    WeatherClipmapFar,
    WeatherComposite,
    WeatherShadowField,
    CloudAlpha,
    AtmosphereTransmittance,
    AtmosphereSkyView,
    AtmosphereMultiscatter,
    CloudLayerDensity,
    CloudLayerAabb,
    CloudStepCount,
    CloudShadowDensity,
    CloudWeatherDecode,
    CloudTemporalWeight,
}

impl RenderDebugView {
    pub fn as_u32(self) -> u32 {
        match self {
            Self::None => 0,
            Self::WeatherClipmapNear => 1,
            Self::WeatherClipmapMid => 2,
            Self::WeatherClipmapFar => 3,
            Self::WeatherComposite => 4,
            Self::WeatherShadowField => 5,
            Self::CloudAlpha => 6,
            Self::AtmosphereTransmittance => 7,
            Self::AtmosphereSkyView => 8,
            Self::AtmosphereMultiscatter => 9,
            Self::CloudLayerDensity => 10,
            Self::CloudLayerAabb => 11,
            Self::CloudStepCount => 12,
            Self::CloudShadowDensity => 13,
            Self::CloudWeatherDecode => 14,
            Self::CloudTemporalWeight => 15,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Final",
            Self::WeatherClipmapNear => "Weather Clipmap Near",
            Self::WeatherClipmapMid => "Weather Clipmap Mid",
            Self::WeatherClipmapFar => "Weather Clipmap Far",
            Self::WeatherComposite => "Weather Composite",
            Self::WeatherShadowField => "Weather Shadow Field",
            Self::CloudAlpha => "Cloud Alpha",
            Self::AtmosphereTransmittance => "Atmosphere Transmittance",
            Self::AtmosphereSkyView => "Atmosphere Sky View",
            Self::AtmosphereMultiscatter => "Atmosphere Multiscatter",
            Self::CloudLayerDensity => "Cloud Layer Density",
            Self::CloudLayerAabb => "Cloud Layer AABB",
            Self::CloudStepCount => "Cloud Step Count",
            Self::CloudShadowDensity => "Cloud Shadow Density",
            Self::CloudWeatherDecode => "Cloud Weather Decode",
            Self::CloudTemporalWeight => "Cloud Temporal Weight",
        }
    }

    pub fn all() -> [Self; 16] {
        [
            Self::None,
            Self::WeatherClipmapNear,
            Self::WeatherClipmapMid,
            Self::WeatherClipmapFar,
            Self::WeatherComposite,
            Self::WeatherShadowField,
            Self::CloudAlpha,
            Self::AtmosphereTransmittance,
            Self::AtmosphereSkyView,
            Self::AtmosphereMultiscatter,
            Self::CloudLayerDensity,
            Self::CloudLayerAabb,
            Self::CloudStepCount,
            Self::CloudShadowDensity,
            Self::CloudWeatherDecode,
            Self::CloudTemporalWeight,
        ]
    }
}
