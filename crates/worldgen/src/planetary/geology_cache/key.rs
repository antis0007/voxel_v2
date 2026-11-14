//! Complete input binding; a different executable conservatively misses.
use std::io::{Read, BufReader};
use std::sync::OnceLock;
use sha2::{Digest, Sha256};
use crate::{WorldSampleConfig, WorldType};
use engine_core::{VoxelScalePreset, cube_sphere::atlas};
const SCHEMA: u32 = 1;
static EXECUTABLE: OnceLock<Option<[u8; 32]>> = OnceLock::new();

fn executable_digest() -> Option<[u8; 32]> {
    *EXECUTABLE.get_or_init(|| {
        let file = std::fs::File::open(std::env::current_exe().ok()?).ok()?;
        let mut file = BufReader::new(file);
        let mut digest = Sha256::new();
        let mut bytes = [0u8; 65536];
        loop {
            let n = file.read(&mut bytes).ok()?;
            if n == 0 { break; }
            digest.update(&bytes[..n]);
        }
        Some(digest.finalize().into())
    })
}

pub(super) fn for_config(cfg: &WorldSampleConfig, shelf: bool) -> Option<Vec<u8>> {
    // Unsupported old/new worlds continue through their original solver.
    if cfg.world_type != WorldType::Planet || !crate::planetary::deep_time(cfg.generator_version)
        || cfg.planet.is_some_and(|p| !p.valid())
        // The snapshot format is fixed at 720 Myr; a shortened history solves live.
        || !crate::planetary::full_history(cfg) { return None; }
    Some(canonical(cfg, shelf, &executable_digest()?))
}

pub(super) fn canonical(cfg: &WorldSampleConfig, shelf: bool, executable: &[u8; 32]) -> Vec<u8> {
    // Exhaustive destructuring makes newly added inputs a compile-time review.
    let WorldSampleConfig { generator_version, seed, world_type, voxel_scale, planet } = *cfg;
    let mut key = b"AXIOM-COMPLETE-GEOLOGY".to_vec();
    key.extend_from_slice(&SCHEMA.to_le_bytes());
    key.extend_from_slice(executable);
    key.extend_from_slice(&generator_version.to_le_bytes());
    key.extend_from_slice(&seed.to_le_bytes());
    key.push(world_type.code());
    key.push(match voxel_scale { VoxelScalePreset::OneMeter => 0,
        VoxelScalePreset::HalfMeter => 1, VoxelScalePreset::QuarterMeter => 2 });
    key.push(shelf as u8);
    key.extend_from_slice(&atlas::RADIUS_M.to_bits().to_le_bytes());
    key.push(planet.is_some() as u8);
    if let Some(settings @ engine_voxel::PlanetSettings { values, civilization, .. }) = planet {
        key.extend_from_slice(&values);
        key.extend_from_slice(&civilization.code().to_le_bytes());
        // Appended only when chosen, so default-history keys keep their bytes.
        if let Some(myr) = settings.history_myr() { key.extend_from_slice(&myr.to_le_bytes()); }
    }
    key
}
