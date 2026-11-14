//! Dedicated disposable geology cache, never colocated with player saves.
use std::path::PathBuf;
pub fn configure() {
    if std::env::var_os("VOXEL_NO_GEOLOGY_CACHE").is_some() { return; }
    let explicit = std::env::var_os("VOXEL_GEOLOGY_CACHE_DIR")
        .filter(|p| !p.is_empty()).map(PathBuf::from);
    // Normal performance runs remain cold; warm-cache experiments opt in.
    if explicit.is_none() && crate::parse_benchmark_config().enabled { return; }
    let root = explicit.or_else(|| {
        std::env::var_os("VOXEL_CACHE_DIR").filter(|p| !p.is_empty()).map(PathBuf::from)
            .or_else(|| std::env::var_os(if cfg!(windows) { "LOCALAPPDATA" } else { "XDG_CACHE_HOME" })
                .filter(|p| !p.is_empty()).map(|p| PathBuf::from(p).join("VoxelWorkshop/cache")))
            .map(|p| p.join("geology-v1"))
    });
    if let Some(root) = root { engine_worldgen::planetary::configure_geology_cache(root); }
}
