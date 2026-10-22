//! One placed waterworks. Older generators do not sample it.
use super::{BlockId as B, WorldSampleConfig};
use engine_content::SizeClass;
use glam::Vec3;

pub fn landing_offset() -> f32 {
    SizeClass::Normal.height()
}

pub fn tunnel_width() -> f32 {
    0.70
}

#[cfg(test)]
pub fn plate_accepts(weight: u8) -> bool {
    weight >= 2
}

/// Local layout in metres, origin at the garden floor.
/// Stair landing sits at the normal cap height.
pub fn sample(cfg: &WorldSampleConfig, metres: Vec3) -> Option<B> {
    if cfg.generator_version < 20 {
        return None;
    }
    let origin = Vec3::new(240.0, 0.0, 48.0);
    let d = metres - origin;
    if d.x.abs() > 28.0 || d.z.abs() > 22.0 || d.y < -4.0 || d.y > 16.0 {
        return None;
    }
    // Maintenance tunnel: only the small class fits.
    if (4.0..18.0).contains(&d.x) && d.z.abs() < tunnel_width() * 0.5 && (0.0..2.2).contains(&d.y) {
        return Some(B::Air);
    }
    // Broken stair gap, then a landing at the normal cap.
    if (-2.0..6.0).contains(&d.x) && d.z.abs() < 2.0 && d.y < landing_offset() && d.y > 0.0 {
        return Some(B::Air);
    }
    if (5.5..8.0).contains(&d.x)
        && d.z.abs() < 2.0
        && (landing_offset()..landing_offset() + 0.4).contains(&d.y)
    {
        return Some(B::Limestone);
    }
    // Garden floor and a spore cell.
    if d.x.abs() < 6.0 && d.z.abs() < 6.0 && d.y < 0.25 && d.y > -0.2 {
        if d.x.abs() < 0.5 && d.z.abs() < 0.5 {
            return Some(B::GlowCap);
        }
        return Some(B::FungalSoil);
    }
    // Shell.
    if d.x.abs() < 24.0 && d.z.abs() < 18.0 && d.y < 12.0 && d.y > -1.0 {
        let inside = d.x.abs() < 22.0 && d.z.abs() < 16.0 && d.y < 10.0 && d.y > 0.2;
        if inside {
            return Some(B::Air);
        }
        return Some(B::Limestone);
    }
    None
}

#[cfg(test)]
pub fn garden_spore_local() -> Vec3 {
    Vec3::new(240.0, 0.1, 48.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use crate::{WorldSampleConfig, WorldType};

    fn cfg(version: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: version,
            seed: 1,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn stair_plate_and_tunnel_match_the_size_table() {
        assert_eq!(landing_offset(), SizeClass::Normal.height());
        assert!(tunnel_width() + 0.001 >= SizeClass::Small.half_width() * 2.0);
        assert!(tunnel_width() < SizeClass::Normal.half_width() * 2.0);
        assert!(!plate_accepts(SizeClass::Normal.weight()));
        assert!(plate_accepts(SizeClass::Large.weight()));
        assert!(sample(&cfg(20), garden_spore_local()).is_some());
        assert!(sample(&cfg(19), garden_spore_local()).is_none());
    }
}
