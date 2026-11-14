//! Driver pipeline cache kept on disk, so later launches skip shader compiles.
//!
//! Only Vulkan supports `Features::PIPELINE_CACHE` in wgpu 27; elsewhere this is
//! a no-op and every pipeline keeps `cache: None`.
use std::path::PathBuf;

/// Largest cache file read back; a bigger one is ignored and rebuilt.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("VOXEL_CACHE_DIR").filter(|path| !path.is_empty()).map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "LOCALAPPDATA" } else { "XDG_CACHE_HOME" })
                .filter(|path| !path.is_empty())
                .map(|path| PathBuf::from(path).join("VoxelWorkshop").join("cache"))
        })
}

fn read_cache(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len == 0 || len > MAX_BYTES { return None; }
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 == len).then_some(bytes)
}

pub struct DiskCache {
    cache: wgpu::PipelineCache,
    path: PathBuf,
}

/// The feature to request when the adapter can keep a cache on disk.
pub fn feature(adapter: &wgpu::Adapter) -> wgpu::Features {
    if wgpu::util::pipeline_cache_key(&adapter.get_info()).is_some() {
        adapter.features() & wgpu::Features::PIPELINE_CACHE
    } else {
        wgpu::Features::empty()
    }
}

impl DiskCache {
    /// Opens the cache for this adapter, seeded from the last saved file.
    pub fn open(device: &wgpu::Device, info: &wgpu::AdapterInfo) -> Option<Self> {
        if std::env::var_os("VOXEL_NO_PIPELINE_CACHE").is_some()
            || !device.features().contains(wgpu::Features::PIPELINE_CACHE)
        {
            return None;
        }
        let key = wgpu::util::pipeline_cache_key(info)?;
        let legacy_path = || crate::world::storage::save_directory().join("pipeline-cache").join(&key);
        let shared_path = cache_directory().map(|dir| dir.join("pipeline-cache").join(&key));
        // Reuse existing caches on upgrade. Future launches can use the shared
        // cache without probing the world-save directory during GPU startup.
        let (path, data) = match shared_path {
            Some(path) => {
                let data = read_cache(&path).or_else(|| read_cache(&legacy_path()));
                (path, data)
            }
            None => {
                let path = legacy_path();
                let data = read_cache(&path);
                (path, data)
            }
        };
        // SAFETY: the file is only ever written from `PipelineCache::get_data`,
        // atomically (temp file then rename). wgpu checks its header, adapter key
        // and length, the driver checks its own header, and `fallback` starts an
        // empty cache when either rejects the data.
        let cache = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("pipeline cache"),
                data: data.as_deref(),
                fallback: true,
            })
        };
        println!(
            "[pipeline-cache] path={} seeded_bytes={}",
            path.display(),
            data.as_ref().map_or(0, Vec::len)
        );
        Some(Self { cache, path })
    }

    pub fn cache(&self) -> &wgpu::PipelineCache {
        &self.cache
    }

    /// Writes the current cache contents on a background thread.
    pub fn save(&self) {
        let cache = self.cache.clone();
        let path = self.path.clone();
        let spawned = std::thread::Builder::new().name("pipeline-cache".into()).spawn(move || {
            let Some(data) = cache.get_data() else { return };
            if data.is_empty() || data.len() as u64 > MAX_BYTES { return; }
            let write = || -> std::io::Result<()> {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let temp = path.with_extension(format!("{}.tmp", std::process::id()));
                std::fs::write(&temp, &data)?;
                std::fs::rename(&temp, &path)
            };
            if let Err(error) = write() {
                eprintln!("[pipeline-cache] cannot save {}: {error}", path.display());
            }
        });
        if let Err(error) = spawned {
            eprintln!("[pipeline-cache] cannot start writer: {error}");
        }
    }
}
