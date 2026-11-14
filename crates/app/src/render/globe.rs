pub mod band;
pub mod bake;
pub mod cache;
pub mod chart;
pub mod gpu;
#[cfg(test)]
mod gpu_band_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod gpu_air_review;
pub mod gpu_tiles;
pub mod scheduler;
pub mod shell;
pub mod shell_gpu;
mod shell_noise;
pub mod shell_obstacles;
mod shell_sim;
pub mod shell_worker;
pub mod tile;
pub mod worker;
mod surface;

/// Native A/B review options; normal play uses the cached surface fields.
pub fn surface_mode() -> u32 {
    static MODE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("VOXEL_GLOBE_SURFACE").as_deref() {
        Ok("legacy") => 0,
        Ok("resolution") => 1,
        Ok("procedural") => 2,
        _ => 3,
    })
}

/// Whether the globe draws: default on, off with `VOXEL_GLOBE=0`.
///
/// On by default on every world with chart coordinates equal to world metres;
/// `VOXEL_GLOBE=0` turns it off for A/B captures.
pub fn active() -> bool {
    static ACTIVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACTIVE.get_or_init(|| {
        std::env::var("VOXEL_GLOBE").as_deref() != Ok("0")
            || std::env::args().any(|arg| arg == "--benchmark-globe")
    })
}

/// Whether `--benchmark-globe` forces the globe on the surface tier too.
///
/// `VOXEL_GLOBE=1` only enables the globe; it still skips the surface tier.
pub fn forced() -> bool {
    static FORCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FORCED.get_or_init(|| std::env::args().any(|arg| arg == "--benchmark-globe"))
}

pub fn reference_cloud_air() -> bool {
    static REFERENCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REFERENCE.get_or_init(|| std::env::args().any(|a| a == "--benchmark-reference-cloud-air"))
}
