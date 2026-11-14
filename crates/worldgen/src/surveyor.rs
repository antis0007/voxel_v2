//! Generator 128: Surveyor ziggurats on planet surfaces.
//!
//! Stratum III works (docs/design/WORLD_LORE.md: "whoever placed the braid;
//! never explained"): stepped monuments of fused black stone with a cleft
//! stair, a sunken relic chamber under an oculus and one course of cut marks.
//! No lamps: the Surveyors left nothing that needs tending. Density comes from
//! the seed's planet profile, so some planets carry one every few kilometres
//! and others almost none.
use crate::{BlockId, ColumnSample, WorldSampleConfig, WorldType, hash2, planet_profile, sample_column};
use glam::IVec3;

pub const VERSION: u32 = crate::gates::SURVEYOR_SITES_GENERATOR;

/// Placement cell edge; at most one monument per cell.
const CELL_M: f32 = 640.0;
/// Centre jitter inside the cell. Jitter plus the widest apron stays inside
/// the cell, so a column only ever asks its own cell.
const JITTER_M: f32 = 200.0;
const TIER_H: f32 = 3.0;
/// Half-width of the top tier, which carries the chamber.
const TOP_HALF_M: f32 = 7.0;
const CHAMBER_HALF_M: f32 = 3.5;
const CHAMBER_H: f32 = 3.0;
const ROOF_H: f32 = 1.5;
const ROOF_HALF_M: f32 = 5.0;
const OCULUS_HALF_M: f32 = 1.0;
const STAIR_HALF_M: f32 = 2.0;
const APRON_M: f32 = 4.0;
const FOUNDATION_M: f32 = 6.0;
/// Air kept clear above the roof so no tree or vine roots on the monument.
const CLEARANCE_M: f32 = 14.0;
/// Largest ground spread across the footprint corners that still fits.
const MAX_SPREAD_M: f32 = 6.0;
/// Widest base half-width the plan can roll.
const MAX_BASE_HALF_M: f32 = 33.0;
const _: () = assert!(JITTER_M + MAX_BASE_HALF_M + APRON_M < CELL_M * 0.5);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Site {
    x: f32,
    z: f32,
    /// Top face of the ground at the centre, metres.
    floor: f32,
    base_half: f32,
    tiers: u32,
    turn: u32,
}

impl Site {
    fn inset(&self) -> f32 {
        (self.base_half - TOP_HALF_M) / self.tiers as f32
    }
    fn body_top(&self) -> f32 {
        self.tiers as f32 * TIER_H
    }
}

thread_local! {
    static LAST: std::cell::Cell<Option<(WorldSampleConfig, i32, i32, Option<Site>)>> = const { std::cell::Cell::new(None) };
}

pub fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && cfg.world_type == WorldType::Planet
}

fn site(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> Option<Site> {
    if let Some((key, kx, kz, s)) = LAST.with(|c| c.get())
        && key == *cfg && kx == cx && kz == cz
    {
        return s;
    }
    let s = plan(cfg, cx, cz);
    LAST.with(|c| c.set(Some((*cfg, cx, cz, s))));
    s
}

fn plan(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> Option<Site> {
    let seed = cfg.seed.wrapping_add(0x5a1b_0128);
    let chance = cfg.planet_settings().map_or_else(|| planet_profile::profile(cfg.seed).surveyor_chance(), |s| s.unit(10) as f32);
    if hash2(cx, cz, seed) >= chance {
        return None;
    }
    let x = (cx as f32 + 0.5) * CELL_M + (hash2(cx, cz, seed ^ 1) - 0.5) * 2.0 * JITTER_M;
    let z = (cz as f32 + 0.5) * CELL_M + (hash2(cx, cz, seed ^ 2) - 0.5) * 2.0 * JITTER_M;
    let base_half = (24.0 + (hash2(cx, cz, seed ^ 3) * 4.0).floor() * 3.0).min(MAX_BASE_HALF_M);
    if crate::planetary::reserves_volcanic_summit(cfg, x, z, (base_half + APRON_M) * std::f32::consts::SQRT_2) {
        return None;
    }
    let tiers = 4 + (hash2(cx, cz, seed ^ 4) * 3.0).floor().min(2.0) as u32;
    let turn = (hash2(cx, cz, seed ^ 5) * 4.0).floor().min(3.0) as u32;
    let ground = |mx: f32, mz: f32| {
        let c = sample_column(cfg, cfg.blocks_for_height(mx), cfg.blocks_for_height(mz));
        (c.height + 1) as f32 * cfg.voxel_meters()
    };
    let centre = ground(x, z);
    let water = (cfg.water_level() + 1) as f32 * cfg.voxel_meters();
    if centre <= water + 1.0 {
        return None;
    }
    let corners = [ground(x + base_half, z), ground(x - base_half, z), ground(x, z + base_half), ground(x, z - base_half)];
    let lo = corners.iter().fold(centre, |a, &b| a.min(b));
    let hi = corners.iter().fold(centre, |a, &b| a.max(b));
    if hi - lo > MAX_SPREAD_M || lo <= water {
        return None;
    }
    Some(Site { x, z, floor: lo, base_half, tiers, turn })
}

/// One course of cut marks on the top tier: every other 1.5 m along the wall.
fn marked(lx: f32, lz: f32) -> bool {
    ((lx + lz) / 1.5).floor().rem_euclid(2.0) == 0.0
}

pub(crate) fn reserves(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    enabled(cfg)
        && site(cfg, (x / CELL_M).floor() as i32, (z / CELL_M).floor() as i32)
            .is_some_and(|s| (x - s.x).abs().max((z - s.z).abs()) <= s.base_half + APRON_M)
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> Option<BlockId> {
    if !enabled(cfg) {
        return None;
    }
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + glam::Vec3::splat(0.5)) * vm;
    let s = site(cfg, (m.x / CELL_M).floor() as i32, (m.z / CELL_M).floor() as i32)?;
    let (lx, lz) = (m.x - s.x, m.z - s.z);
    let r = lx.abs().max(lz.abs());
    if r > s.base_half + APRON_M {
        return None;
    }
    let y = m.y - s.floor;
    if r > s.base_half {
        // Fused ground: only the surface cell, and only where the land meets the plinth.
        return (p.y == column.height && y.abs() < 1.5).then_some(BlockId::Basalt);
    }
    if y < 0.0 {
        return (y >= -FOUNDATION_M).then_some(BlockId::VoidStone);
    }
    let top = s.body_top();
    if y >= top + ROOF_H {
        return (y < top + ROOF_H + CLEARANCE_M).then_some(BlockId::Air);
    }
    if y >= top {
        if r >= ROOF_HALF_M {
            return Some(BlockId::Air);
        }
        return Some(if r < OCULUS_HALF_M { BlockId::Air } else { BlockId::VoidStone });
    }
    let chamber_floor = top - CHAMBER_H;
    if y >= chamber_floor && r < CHAMBER_HALF_M {
        let plinth = r < 0.5 && y < chamber_floor + 1.0;
        return Some(if plinth { BlockId::SurveyCache } else { BlockId::Air });
    }
    // Facing frame: `a` runs out from the chamber toward the stair foot.
    let (a, c) = match s.turn {
        0 => (lx, lz),
        1 => (lz, -lx),
        2 => (-lx, -lz),
        _ => (-lz, lx),
    };
    let top_half = TOP_HALF_M + s.inset();
    if c.abs() < OCULUS_HALF_M && a >= CHAMBER_HALF_M && a <= top_half && y >= chamber_floor && y < top - 1.0 {
        return Some(BlockId::Air);
    }
    if c.abs() < STAIR_HALF_M && a > TOP_HALF_M {
        let rise = chamber_floor * (s.base_half - a) / (s.base_half - TOP_HALF_M);
        return Some(if y < rise { BlockId::VoidStone } else { BlockId::Air });
    }
    let k = (y / TIER_H).floor();
    let half = s.base_half - k * s.inset();
    if r >= half {
        return Some(BlockId::Air);
    }
    if k == 0.0 && r >= s.base_half - 2.0 {
        return Some(BlockId::Basalt);
    }
    let top_tier = k as u32 + 1 == s.tiers;
    if top_tier && y - k * TIER_H < 1.0 && r >= half - 1.0 && marked(lx, lz) {
        return Some(BlockId::CarvedStone);
    }
    Some(BlockId::VoidStone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;

    fn cfg(version: u32, seed: u32) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: version, seed, world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter }
    }

    fn first_site(cfg: &WorldSampleConfig) -> Site {
        (0..64)
            .flat_map(|cx| (0..64).map(move |cz| (cx, cz)))
            .find_map(|(cx, cz)| plan(cfg, cx, cz))
            .expect("some cell in 64x64 carries a monument")
    }

    #[test]
    fn dormant_before_its_gate_and_off_planets() {
        let old = cfg(VERSION - 1, 7);
        let column = sample_column(&old, 0, 0);
        assert_eq!(block(&old, IVec3::new(0, column.height, 0), column), None);
        let hills = WorldSampleConfig { world_type: WorldType::Hills, ..cfg(VERSION, 7) };
        assert!(!enabled(&hills));
    }

    #[test]
    fn monument_has_a_relic_chamber_a_stair_and_no_lamps() {
        let cfg = cfg(VERSION, 7);
        let s = first_site(&cfg);
        let at = |mx: f32, my: f32, mz: f32| {
            let p = IVec3::new(mx.floor() as i32, my.floor() as i32, mz.floor() as i32);
            block(&cfg, p, sample_column(&cfg, p.x, p.z))
        };
        let top = s.floor + s.body_top();
        assert_eq!(at(s.x, top - CHAMBER_H + 0.5, s.z), Some(BlockId::SurveyCache));
        assert_eq!(at(s.x + 2.0, top - 1.5, s.z), Some(BlockId::Air), "chamber air");
        assert_eq!(at(s.x, top + 0.5, s.z), Some(BlockId::Air), "oculus");
        assert_eq!(at(s.x + 3.0, top + 0.5, s.z), Some(BlockId::VoidStone), "roof");
        assert_eq!(at(s.x + s.base_half - 1.0, s.floor + 0.5, s.z + s.base_half - 1.0), Some(BlockId::Basalt), "skirt");
        assert_eq!(at(s.x, s.floor - 2.0, s.z), Some(BlockId::VoidStone), "foundation");
        let mut lamps = 0;
        let mut marks = 0;
        let mut y = s.floor;
        while y < top + ROOF_H {
            let mut x = s.x - s.base_half;
            while x <= s.x + s.base_half {
                let mut z = s.z - s.base_half;
                while z <= s.z + s.base_half {
                    match at(x, y, z) {
                        Some(BlockId::Torch) | Some(BlockId::GlowCap) | Some(BlockId::Crystal) => lamps += 1,
                        Some(BlockId::CarvedStone) => marks += 1,
                        _ => {}
                    }
                    z += 1.0;
                }
                x += 1.0;
            }
            y += 1.0;
        }
        assert_eq!(lamps, 0);
        assert!(marks > 8, "cut course has {marks} marks");
    }

    #[test]
    fn stair_climbs_from_the_foot_to_the_chamber_floor() {
        let cfg = cfg(VERSION, 7);
        let s = first_site(&cfg);
        let along = |a: f32| match s.turn {
            0 => (s.x + a, s.z),
            1 => (s.x, s.z + a),
            2 => (s.x - a, s.z),
            _ => (s.x, s.z - a),
        };
        let mut last = -1.0f32;
        let mut a = s.base_half - 0.5;
        while a > TOP_HALF_M + 0.5 {
            let (mx, mz) = along(a);
            let p = IVec3::new(mx.floor() as i32, 0, mz.floor() as i32);
            let column = sample_column(&cfg, p.x, p.z);
            let mut y = s.floor;
            let mut tread = None;
            while y < s.floor + s.body_top() {
                let q = IVec3::new(p.x, y.floor() as i32, p.z);
                if block(&cfg, q, column) == Some(BlockId::Air) {
                    tread = Some(y);
                    break;
                }
                y += 1.0;
            }
            let tread = tread.expect("air above every tread");
            assert!(tread >= last, "stair drops at a={a}");
            assert!(tread - last <= 1.0 + 1e-3 || last < 0.0, "riser over 1 m at a={a}");
            last = tread;
            a -= 1.0;
        }
        assert!(last >= s.floor + s.body_top() - CHAMBER_H - 1.0, "stair top {last} misses the chamber floor");
    }

    #[test]
    fn same_seed_same_monument_at_every_scale() {
        let one = cfg(VERSION, crate::test_world::seed());
        let s = first_site(&one);
        for scale in [VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let fine = WorldSampleConfig { voxel_scale: scale, ..one };
            let (fx, fz) = ((s.x / CELL_M).floor() as i32, (s.z / CELL_M).floor() as i32);
            let f = plan(&fine, fx, fz).expect("monument survives the scale change");
            assert_eq!((f.x, f.z, f.base_half, f.tiers, f.turn), (s.x, s.z, s.base_half, s.tiers, s.turn));
            assert!((f.floor - s.floor).abs() <= 1.0, "floor moved {} vs {}", f.floor, s.floor);
        }
    }
}
