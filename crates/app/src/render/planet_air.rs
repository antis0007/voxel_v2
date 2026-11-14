//! Game planet parameters applied to the reusable spherical optical-depth LUT.
pub fn model() -> engine_gpu::atmosphere::AirModel {
    engine_gpu::atmosphere::AirModel {
        radius_m: super::globe::chart::RADIUS_M,
        top_m: 3_000.0,
        rayleigh_height_m: 400.0,
        mie_height_m: 120.0,
    }
}
