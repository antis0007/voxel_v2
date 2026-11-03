pub mod bake;
pub mod cache;
pub mod chart;
pub mod gpu;
#[cfg(test)]
mod gpu_tests;
pub mod gpu_tiles;
pub mod scheduler;
pub mod tile;
pub mod worker;

/// Whether the globe draws: `VOXEL_GLOBE=1` or `--benchmark-globe`.
///
/// No planet world type exists yet, so the globe draws on any world with chart
/// coordinates equal to world metres. When `WorldType::Planet` lands this becomes
/// `world_type == Planet || active()`.
pub fn active() -> bool {
    static ACTIVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACTIVE.get_or_init(|| {
        std::env::var("VOXEL_GLOBE").as_deref() == Ok("1")
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
