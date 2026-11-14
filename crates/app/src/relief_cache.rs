//! Disposable regional relief, separate from geology and player saves.
use std::path::PathBuf;

pub fn configure() {
    if std::env::var_os("VOXEL_NO_RELIEF_CACHE").is_some() { return; }
    let explicit = env_path("VOXEL_RELIEF_CACHE_DIR");
    // Generic cache roots do not turn performance runs into warm-cache runs.
    let benchmark = explicit.is_none() && crate::parse_benchmark_config().enabled;
    let root = select_root(false, benchmark, explicit, env_path("VOXEL_CACHE_DIR"),
        env_path(if cfg!(windows) { "LOCALAPPDATA" } else { "XDG_CACHE_HOME" }));
    if let Some(root) = root { engine_worldgen::planetary::configure_relief_cache(root); }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|p| !p.is_empty()).map(PathBuf::from)
}

// No filesystem access or environment mutation in precedence tests.
fn select_root(disabled: bool, benchmark: bool, explicit: Option<PathBuf>,
    shared: Option<PathBuf>, platform: Option<PathBuf>) -> Option<PathBuf>
{
    if disabled { return None; }
    let nonempty = |p: &PathBuf| !p.as_os_str().is_empty();
    let root = match explicit.filter(nonempty) {
        Some(root) => root,
        None if benchmark => return None,
        None => shared.filter(nonempty)
            .or_else(|| platform.filter(nonempty).map(|p| p.join("VoxelWorkshop/cache")))?
            .join("relief-v1"),
    };
    // Never silently redirect an invalid explicit root or use player saves.
    root.is_absolute().then_some(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf { std::env::current_dir().unwrap().join(name) }

    #[test]
    fn relief_cache_precedence_and_independent_namespace() {
        let explicit = root("explicit-relief");
        let shared = root("shared-cache");
        let platform = root("platform-cache");
        assert_eq!(select_root(false, false, Some(explicit.clone()), Some(shared.clone()),
            Some(platform.clone())), Some(explicit.clone()));
        assert_eq!(select_root(false, false, None, Some(shared.clone()), Some(platform.clone())),
            Some(shared.join("relief-v1")));
        assert_eq!(select_root(false, false, None, None, Some(platform.clone())),
            Some(platform.join("VoxelWorkshop/cache/relief-v1")));
        assert_eq!(select_root(false, false, None, None, None), None);
        for benchmark in [false, true] {
            assert_eq!(select_root(true, benchmark, Some(explicit.clone()), Some(shared.clone()),
                Some(platform.clone())), None);
        }
    }

    #[test]
    fn relief_cache_benchmarks_require_dedicated_nonempty_override() {
        let explicit = root("explicit-relief");
        let shared = root("shared-cache");
        let platform = root("platform-cache");
        assert_eq!(select_root(false, true, None, Some(shared.clone()), Some(platform.clone())), None);
        assert_eq!(select_root(false, true, Some(PathBuf::new()), Some(shared.clone()),
            Some(platform.clone())), None);
        assert_eq!(select_root(false, true, Some(explicit.clone()), Some(shared),
            Some(platform)), Some(explicit));
    }

    #[test]
    fn relief_cache_empty_and_relative_roots_fail_closed() {
        let platform = root("platform-cache");
        assert_eq!(select_root(false, false, Some(PathBuf::new()), Some(PathBuf::new()),
            Some(platform.clone())), Some(platform.join("VoxelWorkshop/cache/relief-v1")));
        assert_eq!(select_root(false, false, Some(PathBuf::from("relative-explicit")),
            Some(root("valid-shared")), Some(platform)), None);
        assert_eq!(select_root(false, false, None, Some(PathBuf::from("relative-shared")),
            Some(root("valid-platform"))), None);
        assert_eq!(select_root(false, false, None, None, Some(PathBuf::from("relative-platform"))), None);
    }
}
