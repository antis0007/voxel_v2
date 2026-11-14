//! Seeded crystal posts on open bay floors. The maze bay is left alone.
use super::shell::{self, Class};
use crate::{hash_u32, BlockId, WorldSampleConfig, SEEDED_REALMS};
use glam::{IVec3, Vec3};

fn chosen(seed: u32, sector: i32, row: i32) -> bool {
    if row == 1 || (sector == 0 && row == 0) {
        return false;
    }
    hash_u32(seed ^ (sector as u32).wrapping_mul(0x9E37_79B9) ^ (row as u32).wrapping_mul(0x85EB_CA6B)) % 4 == 0
}

pub fn mark(cfg: &WorldSampleConfig, cell: IVec3) -> Option<BlockId> {
    if cfg.generator_version < SEEDED_REALMS {
        return None;
    }
    let s = shell::sample(cfg, cell);
    if !matches!(s.class, Class::Hollow) || !chosen(cfg.seed, s.sector, s.row) {
        return None;
    }
    let mid_a = shell::pitch_for(cfg.generator_version) * 0.5 + 0.03;
    let mid_z = shell::row_len(cfg.generator_version) * 0.5;
    let post = (s.local_a - mid_a).abs() < 0.004
        && (s.local_z - mid_z).abs() < 0.6
        && (-1.05..-0.35).contains(&s.above);
    post.then_some(BlockId::Crystal)
}

fn feet(cfg: &WorldSampleConfig, sector: i32, row: i32) -> Vec3 {
    let z = (row as f32 + 0.5) * shell::row_len(cfg.generator_version);
    let mut angle = (sector as f32 + 0.5) * shell::pitch_for(cfg.generator_version);
    if cfg.generator_version >= shell::GRAND {
        angle -= super::grand::twist(z);
    }
    let radius = shell::shell_radius(cfg.generator_version, z) - 1.6;
    let center = shell::center_of(cfg.generator_version);
    center + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z)
}

pub fn encounters(cfg: &WorldSampleConfig, player: Vec3) -> Vec<(IVec3, Vec3, u32)> {
    if cfg.generator_version < SEEDED_REALMS {
        return Vec::new();
    }
    let center = shell::center_of(cfg.generator_version);
    let row = ((player.z - center.z) / shell::row_len(cfg.generator_version)).floor() as i32;
    let sectors = if cfg.generator_version >= shell::GRAND { 6 } else { 16 };
    let mut out = Vec::new();
    for dz in -1..=1 {
        for sector in 0..sectors {
            let at = row + dz;
            if !chosen(cfg.seed, sector, at) {
                continue;
            }
            let stand = feet(cfg, sector, at);
            if (stand - player).length() > 90.0 {
                continue;
            }
            let kind = hash_u32(cfg.seed ^ sector as u32 ^ (at as u32).wrapping_mul(13)) % 6;
            let id = IVec3::new(sector.wrapping_mul(1024).wrapping_add(at), 500, 1);
            out.push((id, stand, kind));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorldSampleConfig, WorldType, sample_block};
    use engine_core::VoxelScalePreset;

    fn cfg(seed: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: SEEDED_REALMS,
            seed,
            world_type: WorldType::Manyfold,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn the_same_seed_repeats_and_another_seed_can_differ() {
        let mut first = None;
        for seed in 1..48 {
            let world = cfg(seed);
            for sector in 0..6 {
                for row in [2, 3, 4] {
                    let stand = feet(&world, sector, row);
                    let list = encounters(&world, stand);
                    if list.iter().any(|e| (e.1 - stand).length() < 3.0) {
                        assert_eq!(list, encounters(&world, stand));
                        let body = (stand / world.voxel_meters()).floor().as_ivec3();
                        assert!(!sample_block(&world, body).is_solid(), "stand blocked");
                        first = Some((seed, list));
                        break;
                    }
                }
                if first.is_some() { break; }
            }
            if first.is_some() { break; }
        }
        let (seed, list) = first.expect("no seeded bay");
        let other = encounters(&cfg(seed.wrapping_add(17)), list[0].1);
        assert_ne!(list, other);
    }
}
