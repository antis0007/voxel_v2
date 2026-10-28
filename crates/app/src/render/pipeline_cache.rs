//! Driver pipeline cache kept on disk, so later launches skip shader compiles.
//!
//! Only Vulkan supports `Features::PIPELINE_CACHE` in wgpu 27; elsewhere this is
//! a no-op and every pipeline keeps `cache: None`.
use std::path::PathBuf;

/// Largest cache file read back; a bigger one is ignored and rebuilt.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

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
        let path = crate::world::storage::save_directory().join("pipeline-cache").join(key);
        let data = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= MAX_BYTES)
            .and_then(|_| std::fs::read(&path).ok());
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
        let Some(data) = self.cache.get_data() else { return };
        let path = self.path.clone();
        let spawned = std::thread::Builder::new().name("pipeline-cache".into()).spawn(move || {
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
