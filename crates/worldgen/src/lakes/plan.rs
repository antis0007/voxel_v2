//! Siting and kind choice for one lattice cell. Reads only the tile-free
//! preview terrain, climate, tectonics, the valley and grove reserves and the
//! river plan, none of which read lakes, so planning cannot recurse.
use super::{BIG_CELL, Lake, LakeKind, SMALL_CELL};
use crate::{WATER_LEVEL_METERS, WorldSampleConfig, amberroot, drainage, hash2, landscape, valley};
use engine_core::VoxelScalePreset;
use glam::Vec2;

/// Water surface below the lowest ground on the rim ring.
const FREEBOARD_M: f32 = 0.3;
/// Rim ring radius as a multiple of the local shore radius.
const RING: f32 = 1.12;
/// No lake within this distance of the origin, where spawn and the yard sit.
const SPAWN_CLEAR_M: f32 = 220.0;

struct Spec {
    kind: LakeKind,
    radius: (f32, f32),
    depth: (f32, f32),
    bank: f32,
    /// Most the rim ring may vary, and the outline's lobe strength.
    relief: f32,
    lobes: f32,
    rim: f32,
}

fn spec(kind: LakeKind) -> Spec {
    let s = |radius, depth, bank, relief, lobes, rim| Spec { kind, radius, depth, bank, relief, lobes, rim };
    match kind {
        LakeKind::Lake => s((30.0, 85.0), (3.0, 7.0), 0.16, 9.0, 1.0, 0.0),
        LakeKind::Tarn => s((16.0, 36.0), (4.0, 8.0), 1.1, 16.0, 0.5, 0.0),
        LakeKind::Crater => s((38.0, 70.0), (9.0, 16.0), 1.4, 30.0, 0.25, 6.0),
        LakeKind::Oasis => s((8.0, 16.0), (1.6, 2.4), 0.22, 4.0, 0.7, 0.0),
        LakeKind::Cenote => s((5.0, 10.0), (6.0, 10.0), 0.0, 3.0, 0.15, 0.0),
        LakeKind::Pond => s((5.0, 11.0), (1.2, 2.2), 0.3, 2.5, 0.9, 0.0),
        LakeKind::MarshPool => s((2.5, 6.0), (0.8, 1.4), 0.3, 1.2, 1.2, 0.0),
    }
}

/// Kind for a big-lattice cell from height, climate and volcanism.
fn big_kind(ground: f32, heat: f32, wet: f32, volcanism: f32, roll: f32) -> Option<LakeKind> {
    Some(if volcanism > 0.45 {
        LakeKind::Crater
    } else if ground > 96.0 || (heat < 0.2 && ground > 60.0) {
        LakeKind::Tarn
    } else if heat > 0.63 && wet < 0.38 {
        if roll > 0.12 {
            return None;
        }
        LakeKind::Oasis
    } else if roll < 0.06 && wet > 0.4 {
        LakeKind::Cenote
    } else {
        LakeKind::Lake
    })
}

/// Kind for a small-lattice cell: pools cluster in wet ground, ponds are rare.
fn small_kind(heat: f32, wet: f32, roll: f32) -> Option<LakeKind> {
    if wet > 0.66 {
        Some(LakeKind::MarshPool)
    } else if wet > 0.42 && (0.28..0.72).contains(&heat) && roll < 0.08 {
        Some(LakeKind::Pond)
    } else {
        None
    }
}

pub(super) fn site(cfg: &WorldSampleConfig, small: bool, ix: i32, iz: i32) -> Option<Lake> {
    let cell = if small { SMALL_CELL } else { BIG_CELL };
    let salt = cfg.seed.wrapping_add(if small { 0x5a11_0000 } else { 0x1a4e_0000 });
    let roll = |k: i32| hash2(ix, iz, salt.wrapping_add(k as u32 * 7919));
    // Most cells reject on the hash alone, before any terrain is read.
    let chance = roll(0);
    if chance > if small { 0.5 } else { 0.3 } {
        return None;
    }
    let centre = Vec2::new(
        (ix as f32 + 0.5 + (roll(1) - 0.5) * 0.5) * cell,
        (iz as f32 + 0.5 + (roll(2) - 0.5) * 0.5) * cell,
    );
    if centre.length() < SPAWN_CLEAR_M {
        return None;
    }
    // Every voxel scale shares one plan, surveyed at one metre.
    let canon = WorldSampleConfig { voxel_scale: VoxelScalePreset::OneMeter, ..*cfg };
    let ground = |p: Vec2| landscape::pre_drainage_preview_top(&canon, p.x, p.y);
    let g0 = ground(centre);
    if g0 <= WATER_LEVEL_METERS + 2.5 {
        return None;
    }
    let (heat, wet, _) = landscape::visual_ecology(&canon, centre.x, centre.y, g0);
    let kind = if small {
        small_kind(heat, wet, chance * 2.0)?
    } else {
        let volcanism = landscape::sample_tectonics(centre.x, centre.y, cfg.seed, cfg.generator_version).volcanism;
        big_kind(g0, heat, wet, volcanism, chance / 0.3)?
    };
    let s = spec(kind);
    let lerp = |(a, b): (f32, f32), t: f32| a + (b - a) * t;
    let lobe = |k: i32, amp: f32| (amp * s.lobes * roll(k), roll(k + 1) * std::f32::consts::TAU);
    let mut lake = Lake {
        kind: s.kind,
        centre,
        radius: lerp(s.radius, roll(3)),
        level: 0.0,
        depth: lerp(s.depth, roll(4)),
        lobes: [lobe(5, 0.16), lobe(7, 0.12), lobe(9, 0.07)],
        bank: s.bank,
        rim: s.rim,
    };
    let samples = if small { 12 } else { 32 };
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for k in 0..samples {
        let angle = k as f32 / samples as f32 * std::f32::consts::TAU;
        let p = centre + Vec2::new(angle.cos(), angle.sin()) * lake.shore_radius(angle) * RING;
        // Rivers keep their own channels; a lake never sits across a wet one.
        // Dry gullies are absorbed into the basin.
        if drainage::sample(&canon, p.x, p.y).water_m.is_some() {
            return None;
        }
        let g = ground(p);
        (low, high) = (low.min(g), high.max(g));
    }
    if high - low > s.relief || g0 - low > s.relief + lake.depth || drainage::sample(&canon, centre.x, centre.y).water_m.is_some() {
        return None;
    }
    if !small && (valley::reserves(&canon, centre.x, centre.y) || amberroot::reserves(&canon, centre.x, centre.y)) {
        return None;
    }
    lake.level = match kind {
        LakeKind::Cenote => g0.min(low) - lerp((5.0, 8.0), roll(11)),
        LakeKind::Crater => low - FREEBOARD_M + 0.4 * s.rim,
        _ => low - FREEBOARD_M,
    };
    Some(lake)
}
