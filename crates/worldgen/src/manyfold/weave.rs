//! Physical inscriptions and fold thresholds. State is the edited anchor voxel,
//! not a quest flag: erase, mine, or rebuild it using ordinary world persistence.
use super::{CENTER, HALF_LENGTH, OUTER_RADIUS, RADIUS, districts};
use crate::{BlockId, WorldSampleConfig};
use glam::{IVec3, Vec3};
use std::f32::consts::{PI, TAU};

pub const VERSION: u32 = crate::gates::OUTCROPS_AND_WEAVE_GENERATOR;
pub const DISTRICTS: usize = 6;
pub const TRAP_Z: f32 = -10.5;
pub const GATE_Z: f32 = 20.0;
pub const FRAME_HEIGHT: f32 = 3.0;
pub const APERTURE_WIDTH: f32 = 2.5;
pub const APERTURE_HEIGHT: f32 = 2.9;

/// Metres, with arc measured along the nominal floor and height toward the void.
pub fn point(district: usize, arc: f32, height: f32, z: f32) -> Vec3 {
    let angle = district as f32 * TAU / DISTRICTS as f32 + arc / RADIUS;
    CENTER
        + Vec3::new(
            angle.sin() * (RADIUS - height),
            -angle.cos() * (RADIUS - height),
            z,
        )
}

pub fn coordinates(point_m: Vec3) -> (usize, f32, f32, f32) {
    let p = point_m - CENTER;
    let angle = p.x.atan2(-p.y);
    let district = (((angle + PI / 6.0).rem_euclid(TAU) / (TAU / 6.0)) as usize).min(5);
    let local = (angle - district as f32 * TAU / 6.0 + PI).rem_euclid(TAU) - PI;
    (
        district,
        local * RADIUS,
        RADIUS - p.truncate().length(),
        p.z,
    )
}

pub fn anchor(cfg: &WorldSampleConfig, district: usize) -> IVec3 {
    (point(district, 4.4, 1.25, -7.4) / cfg.voxel_meters())
        .floor()
        .as_ivec3()
}

pub fn keystone(cfg: &WorldSampleConfig, district: usize) -> IVec3 {
    (point(district, 0.0, 7.0, GATE_Z) / cfg.voxel_meters())
        .floor()
        .as_ivec3()
}

pub fn partner(district: usize) -> usize {
    (district + 3) % DISTRICTS
}

pub fn seal_name(district: usize) -> &'static str {
    match district % 3 {
        0 => "Return seal",
        1 => "Misroute seal",
        _ => "Reversal seal",
    }
}

pub(super) fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let (district, local, height, z) = coordinates(p);
    let radius = RADIUS - height;
    if radius > OUTER_RADIUS || z.abs() >= HALF_LENGTH || radius >= OUTER_RADIUS - 2.0 {
        return districts::block(cfg, cell);
    }
    if cell == anchor(cfg, district) {
        return BlockId::Crystal;
    }
    if cell == keystone(cfg, district) {
        return BlockId::CopperBlock;
    }
    let court = local.abs() < 6.5 && (-15.0..-5.0).contains(&z);
    let threshold = local.abs() < 5.0 && (z - GATE_Z).abs() < 1.1;
    if height <= 0.0 {
        if height > -0.85 && court {
            let r = Vec3::new(local, 0.0, z - TRAP_Z).length();
            let inscription = (r - 2.8).abs() < 0.4
                || (r - 4.0).abs() < 0.3
                || (local.abs() < 0.35 && r < 4.0)
                || ((z - TRAP_Z).abs() < 0.35 && r < 4.0);
            return if inscription {
                BlockId::CopperBlock
            } else {
                BlockId::Limestone
            };
        }
        if height > -0.85 && threshold {
            return BlockId::GlazedTiles;
        }
        return districts::block(cfg, cell);
    }
    if court {
        // Preserve the brass-weight mechanism's existing wall and opening.
        if local > 5.3
            && let Some((angle, gate_z)) = districts::shortcut(cfg.seed)
        {
            let here = (p - CENTER).x.atan2(-(p - CENTER).y);
            let distance = ((here - angle + PI).rem_euclid(TAU) - PI).abs() * RADIUS;
            if distance < 0.9 && (z - gate_z).abs() < 1.95 {
                return districts::block(cfg, cell);
            }
        }
        // An open outer lane permits walking around a line without invoking it.
        // The anchor has a visible stone support; deleting it leaves a real gap.
        if (local - 4.4).abs() < 0.85 && (z + 7.4).abs() < 0.85 && height < 1.2 {
            return BlockId::CarvedStone;
        }
        // Broken buttresses bracket the inscription without closing its bypass.
        if local.abs() > 5.3 && (z + 14.0).abs() < 0.65 && height < 5.0 {
            return BlockId::WeatheredBricks;
        }
        return BlockId::Air;
    }
    if threshold {
        let roof = 5.8 + (1.0 - local.abs() / 5.0) * 1.8;
        if (local.abs() > 3.3 && height < roof + 0.8) || (height > roof && height < roof + 0.8) {
            return if local.abs() > 4.3 {
                BlockId::Basalt
            } else {
                BlockId::CarvedStone
            };
        }
        return BlockId::Air;
    }
    // Tall, broken ribs frame each axial street. Their gaps leave the same
    // navigable floor intact; they are voxel architecture, not a sky backdrop.
    if (local.abs() - 9.5).abs() < 0.75 && (z + 18.0).abs() < 0.9 && height < 10.0 {
        let notch = (height - 5.5).abs() < 0.75 && district % 2 == 0;
        if !notch {
            return BlockId::WeatheredBricks;
        }
    }
    districts::block(cfg, cell)
}
