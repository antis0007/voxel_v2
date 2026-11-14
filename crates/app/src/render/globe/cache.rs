//! Per-seed on-disk tile cache; any mismatch is a miss so a bad file is regenerated.

use std::path::{Path, PathBuf};

use super::bake::TileData;
use super::tile::TileId;
use engine_voxel::WorldSampleConfig;

pub const MAGIC: &[u8; 8] = b"VXGT0005";

const SAMPLES: usize = 65 * 65;
const HEADER: usize = 8 + 16;
const FILE_LEN: usize = HEADER + SAMPLES * 10;

pub struct TileCache {
    root: PathBuf,
    writable: bool,
}

/// Folder name for `cfg` and the planet size: each changes the baked ground.
pub fn folder_name(cfg: &WorldSampleConfig) -> String {
    let (half, radius) = (super::chart::HALF_M as u64, super::chart::RADIUS_M as u64);
    let mut name = format!("{}-g{}-{:?}-{:?}-h{half}-r{radius}", cfg.seed, cfg.generator_version, cfg.world_type, cfg.voxel_scale);
    if cfg.planet_settings().is_some() { name.push_str(&format!("-p{:016x}", cfg.planet_fingerprint())); }
    name.to_lowercase().replace(' ', "")
}

impl TileCache {
    /// Opens `<save_dir>/globe/<folder_name(cfg)>/`; a failed create leaves it read-only.
    pub fn open(save_dir: &Path, cfg: &WorldSampleConfig) -> TileCache {
        let root = save_dir.join("globe").join(folder_name(cfg));
        let writable = std::fs::create_dir_all(&root).is_ok();
        TileCache { root, writable }
    }

    fn path(&self, id: TileId) -> PathBuf {
        self.root.join(format!("{:016x}.gtl", id.key()))
    }

    pub fn load(&self, id: TileId) -> Option<TileData> {
        let bytes = std::fs::read(self.path(id)).ok()?;
        decode(&bytes, id)
    }

    /// Writes via a temp file so a crash never leaves a half-written tile under the real name.
    pub fn store(&self, data: &TileData) -> bool {
        if !self.writable {
            return false;
        }
        let path = self.path(data.id);
        // The pid keeps two game processes on one save from writing the same temp file.
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let written = std::fs::write(&tmp, encode(data)).is_ok() && {
            // Windows rename does not replace an existing file.
            let _ = std::fs::remove_file(&path);
            std::fs::rename(&tmp, &path).is_ok()
        };
        if !written {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }
}

fn encode(d: &TileData) -> Vec<u8> {
    let mut out = Vec::with_capacity(FILE_LEN);
    out.extend_from_slice(MAGIC);
    for v in [d.id.face as u32, d.id.level as u32, d.id.x, d.id.y] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for h in &d.height_m {
        out.extend_from_slice(&h.to_le_bytes());
    }
    out.extend_from_slice(&d.material);
    out.extend_from_slice(&d.water);
    for c in &d.climate {
        out.extend_from_slice(c);
    }
    for n in &d.normals { out.extend_from_slice(n); }
    out
}

fn decode(b: &[u8], id: TileId) -> Option<TileData> {
    if b.len() != FILE_LEN || &b[..8] != MAGIC {
        return None;
    }
    let word = |i: usize| u32::from_le_bytes([b[8 + i * 4], b[9 + i * 4], b[10 + i * 4], b[11 + i * 4]]);
    if (word(0), word(1), word(2), word(3)) != (id.face as u32, id.level as u32, id.x, id.y) {
        return None;
    }
    let h_end = HEADER + SAMPLES * 2;
    let height_m = b[HEADER..h_end].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    let material = b[h_end..h_end + SAMPLES].to_vec();
    let water = b[h_end + SAMPLES..h_end + SAMPLES * 2].to_vec();
    let appearance = h_end + SAMPLES * 2;
    let normal_start = appearance + SAMPLES * 4;
    let climate = b[appearance..normal_start].chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    let normals = b[normal_start..].chunks_exact(2).map(|n| [n[0], n[1]]).collect();
    Some(TileData { id, height_m, material, water, climate, normals })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u32, generator_version: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version,
            seed,
            world_type: engine_voxel::WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        }
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("globe-cache-{}-{}", std::process::id(), name))
    }

    fn sample_data() -> TileData {
        let mut d = TileData::empty(TileId { face: 2, level: 3, x: 5, y: 6 });
        for k in 0..SAMPLES {
            d.height_m[k] = (k as i16).wrapping_mul(7);
            d.material[k] = (k % 251) as u8;
            d.water[k] = (k % 13) as u8;
            d.climate[k] = [(k % 17) as u8, (k % 19) as u8, 0, 255];
        }
        d
    }

    #[test]
    fn round_trip_restores_every_field() {
        let dir = temp("round_trip");
        let cache = TileCache::open(&dir, &cfg(74, 53));
        let d = sample_data();
        assert!(cache.store(&d));
        assert!(cache.store(&d), "overwrite must work");
        let got = cache.load(d.id).expect("hit");
        assert_eq!(got.id, d.id);
        assert_eq!(got.height_m, d.height_m);
        assert_eq!(got.material, d.material);
        assert_eq!(got.water, d.water);
        assert_eq!(got.climate, d.climate);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_generator_bump_misses_the_old_cache() {
        let dir = temp("bump");
        let a = TileCache::open(&dir, &cfg(74, 53));
        a.store(&sample_data());
        let b = TileCache::open(&dir, &cfg(74, 54));
        assert!(b.load(sample_data().id).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_different_scale_misses_the_same_seed_and_version() {
        let dir = temp("scale");
        let a = TileCache::open(&dir, &cfg(74, 53));
        assert!(a.store(&sample_data()));
        let mut other = cfg(74, 53);
        other.voxel_scale = engine_core::VoxelScalePreset::HalfMeter;
        let b = TileCache::open(&dir, &other);
        assert!(b.load(sample_data().id).is_none());
        let mut world = cfg(74, 53);
        world.world_type = engine_voxel::WorldType::Superflat;
        assert!(TileCache::open(&dir, &world).load(sample_data().id).is_none());
        assert!(a.load(sample_data().id).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_leaves_no_temp_file_behind() {
        let dir = temp("tmp");
        let cache = TileCache::open(&dir, &cfg(74, 53));
        assert!(cache.store(&sample_data()));
        let stray = std::fs::read_dir(&cache.root).unwrap().flatten().filter(|e| e.path().to_string_lossy().ends_with(".tmp")).count();
        assert_eq!(stray, 0);
        assert!(folder_name(&cfg(74, 53)).contains("-g53-hills-onemeter-h"), "{}", folder_name(&cfg(74, 53)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_truncated_file_is_a_miss_not_a_panic() {
        let dir = temp("trunc");
        let cache = TileCache::open(&dir, &cfg(74, 53));
        let id = sample_data().id;
        std::fs::write(cache.path(id), [0u8; 20]).unwrap();
        assert!(cache.load(id).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_read_only_root_degrades_to_misses() {
        let cache = TileCache::open(Path::new("Z:/nonexistent/globe"), &cfg(74, 53));
        assert!(!cache.writable);
        assert!(!cache.store(&sample_data()));
        assert!(cache.load(sample_data().id).is_none());
    }
}
