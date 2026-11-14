//! Reference-led masonry, inhabited roofs and planted ruins. All masses are
//! ordinary editable voxels in the same radial field as the streets below.
use super::{HALF_LENGTH, OUTER_RADIUS, RADIUS, weave};
use crate::{BlockId, WorldSampleConfig};
use glam::{IVec3, Vec3};

pub const VERSION: u32 = 84;

pub(super) fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let (district, x, h, z) = weave::coordinates(p);
    let radius = RADIUS - h;
    let old = || weave::block(cfg, cell);
    if radius > OUTER_RADIUS || z.abs() >= HALF_LENGTH || h < -0.85 {
        return old();
    }
    // Keep seal courts, their anchors and bypasses, and fold landing volumes
    // exactly as authored. New architecture never conceals a working inscription.
    if z < -4.0 || (x.abs() < 5.8 && (z - weave::GATE_Z).abs() < 1.5) {
        return old();
    }
    if h <= 0.0 {
        if z.abs() < 2.8 {
            return if (z.abs() - 2.35).abs() < 0.25 {
                BlockId::GlazedTiles
            } else { BlockId::Limestone };
        }
        return old();
    }

    // An annular elevated walk: deep openings, slender upper coping, and
    // broad quiet stone. The axial streets run through its six large arches.
    if (z - 8.5).abs() < 1.65 && h < 8.2 {
        let pier = x.abs() > 10.2;
        let spring = 4.0 + 2.0 * (1.0 - (x / 10.2).powi(2)).max(0.0).sqrt();
        if h < 6.8 && (pier || h > spring) {
            return if h < 0.8 || h > 6.25 { BlockId::Sandstone } else { BlockId::DressedStone };
        }
        // Low parapets retain a clear two-metre deck. Repeated openings keep
        // the upper silhouette light when seen from the opposite district.
        if (6.8..7.65).contains(&h) && (z - 8.5).abs() > 1.15
            && (x.abs() % 3.0) < 1.8
        {
            return BlockId::CarvedStone;
        }
        return BlockId::Air;
    }
    // Broad quarter-metre treads climb from the rear lane to the arcade. At one
    // metre scale the voxel raster naturally produces one-metre steps.
    // Only the side bays are occupied; all axial streets stay open below.
    if (x - 8.0).abs() < 1.4 && (10.15..22.15).contains(&z) {
        let tread = (((22.15 - z) / 12.0 * 6.8) / 0.25).floor() * 0.25;
        if h < tread { return BlockId::Limestone; }
        return BlockId::Air;
    }

    // Six readable roof families in a shared construction language: garden
    // pergola, stepped shrine, archive gable, copper workshop, overgrown ruin,
    // and a tall open belfry. Central walk-through aisles remain 4.8 m wide.
    if x.abs() < 4.4 && (z - 15.0).abs() < 3.5 {
        let depth = (z - 15.0).abs();
        let roof = match district {
            0 => 5.2,
            1 => 5.4 + ((4.4 - x.abs()) / 1.2).floor() * 0.45,
            2 => 5.1 + (4.4 - x.abs()) * 0.55,
            3 => 5.3 + (3.5 - depth) * 0.4,
            4 => 5.3,
            _ => 8.8 + (4.4 - x.abs()) * 0.65,
        };
        let roof_material = match district {
            0 | 4 => BlockId::Log,
            2 => BlockId::RoofTiles,
            3 => BlockId::CopperBlock,
            _ => BlockId::Sandstone,
        };
        let open_roof = (district == 0 && depth > 0.45 && (x % 2.0).abs() > 0.45)
            || (district == 4 && x > 0.0 && z > 15.0);
        if h >= roof && h < roof + 0.65 && !open_roof { return roof_material; }
        if h < roof && x.abs() > 3.15 && depth > 2.25 {
            return if h < 0.8 || (h - 4.4).abs() < 0.35 {
                BlockId::CarvedStone
            } else { BlockId::DressedStone };
        }
        // Sheltered fixture: one real crystal cell, placed below the roof.
        if cell == lamp(cfg, district) { return BlockId::Crystal; }
        if x.abs() < 0.35 && depth < 0.35 && h > 4.5 && h < roof {
            return BlockId::CopperBlock;
        }
        if x.abs() > 2.8 && depth < 1.7 {
            if district == 2 && h < 3.1 { return BlockId::Planks; }
            if district == 3 && h < 1.0 { return BlockId::Planks; }
        }
        if district == 5 && (h - 7.6).abs() < 0.4 && x.abs() < 0.9 && depth < 0.9 {
            return BlockId::CopperBlock;
        }
        return BlockId::Air;
    }

    // Grouped canopy masses wrap around a planted ruin, leaving the street
    // and bridge approaches quiet. No random foliage sprinkled into paths.
    if district == 0 || district == 4 {
        let tx = x + 7.5;
        let tz = z - 15.0;
        if tx.abs() < 0.65 && tz.abs() < 0.7 && h < 6.7 { return BlockId::Log; }
        let crown = (tx / 3.2).powi(2) + (tz / 2.7).powi(2) + ((h - 6.5) / 2.3).powi(2);
        let side = ((tx - 1.8) / 2.0).powi(2) + ((tz + 1.2) / 2.0).powi(2)
            + ((h - 5.6) / 1.7).powi(2);
        if crown < 1.0 || side < 1.0 { return BlockId::Leaves; }
        if tx.abs() < 3.2 && tz.abs() < 2.9 && h < 0.7 {
            return if tx.abs() > 2.6 || tz.abs() > 2.3 { BlockId::WeatheredBricks } else { BlockId::GroveMoss };
        }
    }
    // A recessed end-wall facade gives the finite tube a built termination.
    // The original outer bedrock boundary remains intact behind this lining.
    if z > 22.5 && h < 15.0 {
        let opening = x.abs() < 3.0 && h < 6.0 + (3.0 - x.abs()) * 0.8;
        let upper_opening = x.abs() < 1.8 && (10.0..13.0).contains(&h);
        if z > 23.4 || (!opening && !upper_opening) {
            return if x.abs() > 10.0 || (h - 9.0).abs() < 0.4 {
                BlockId::Sandstone
            } else { BlockId::LimePlaster };
        }
        return BlockId::Air;
    }
    old()
}

fn lamp(cfg: &WorldSampleConfig, district: usize) -> IVec3 {
    (weave::point(district, 0.0, 4.5, 15.0) / cfg.voxel_meters()).floor().as_ivec3()
}

pub(super) fn lights(cfg: &WorldSampleConfig) -> impl Iterator<Item = IVec3> + '_ {
    (0..6).map(|d| lamp(cfg, d))
}
