//! A metre-scale playable control from the original USGS elevation samples.
use super::{SEA, geo, review::catchment::Frame, smooth};
use crate::{BlockId, ColumnSample, WorldSampleConfig, WorldType};
use engine_core::cube_sphere::atlas;
use glam::{DVec2, DVec3, IVec3};
use std::sync::OnceLock;

pub const VERSION: u32 = 268;
pub const SIDE: usize = 1000;
pub const SPACING_M: f64 = 4.0;
pub const LOW_M: f64 = 48.0;
pub const SPAN_M: f64 = SIDE as f64 * SPACING_M;
const APRON_M: f64 = 384.0;
const DATA: &[u8] = include_bytes!("../../../../assets/terrain/black-dragon-wash-4m.f32");

struct Heightfield {
    heights: Vec<f32>,
    datum: f64,
}

fn heightfield() -> &'static Heightfield {
    static FIELD: OnceLock<Heightfield> = OnceLock::new();
    FIELD.get_or_init(|| {
        assert_eq!(DATA.len(), SIDE * SIDE * 4);
        let heights: Vec<_> = DATA.chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap())).collect();
        assert!(heights.iter().all(|h| h.is_finite()));
        let low = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let high = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(LOW_M + f64::from(high - low) < 504.0);
        Heightfield { heights, datum: f64::from(low) }
    })
}

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && cfg.generator_version == VERSION
}

pub fn frame() -> Frame {
    Frame { origin: DVec2::new(343_584.0, 16_000.0), across: DVec2::X, downstream: DVec2::Y }
}

pub fn contains(x: f64, z: f64) -> bool {
    let p = frame().local(DVec2::new(x, z));
    p.min_element() >= 0.0 && p.max_element() <= SPAN_M
}

fn grid() -> crate::geo::grid::Grid {
    crate::geo::grid::Grid { n: SIDE, cell_m: SPACING_M, min_m: [0.0; 2] }
}

/// Original samples are retained verbatim. Only the elevation datum moves.
pub fn height(x: f64, z: f64) -> f64 {
    let field = heightfield();
    grid().bilinear(&field.heights, x, z) - field.datum + LOW_M
}

pub fn normal(x: f64, z: f64) -> glam::Vec3 {
    let d = SPACING_M;
    glam::Vec3::new(
        ((height(x - d, z) - height(x + d, z)) / (2.0 * d)) as f32,
        1.0,
        ((height(x, z - d) - height(x, z + d)) / (2.0 * d)) as f32,
    ).normalize()
}

pub(super) fn ground(dir: DVec3) -> Option<geo::Ground> {
    let chart = atlas::from_body(dir * atlas::RADIUS_M);
    if !contains(chart.x, chart.z) { return None; }
    let p = frame().local(DVec2::new(chart.x, chart.z));
    let n = normal(p.x, p.y);
    Some(geo::Ground { deposit_material: crate::BlockId::Stone, deposit_bed_top: 0.0, deposit_bed_slope: 0.0, deposit_m: 0.0,
        top_m: height(p.x, p.y) as f32, slope: n.x.hypot(n.z) / n.y.max(0.0001),
        temperature: 0.65, moisture: 0.2, sediment: 0.0, placer: 0.0,
        river_gravel: false, river_bedrock: false, maat_c: 14.0, permafrost: false,
        water_m: None, riparian: 0.0, delta: 0.0, delta_mud: 0.0,
        suspended: 0.0, intertidal: 0.0, scree_m: 0.0, outlook_clearing: 0.0,
    })
}

pub(super) fn apply(dir: DVec3, ground: &mut geo::Ground) {
    let chart = atlas::from_body(dir * atlas::RADIUS_M);
    let p = frame().local(DVec2::new(chart.x, chart.z));
    let outside = (-p.x).max(-p.y).max(p.x - SPAN_M).max(p.y - SPAN_M).max(0.0);
    if outside >= APRON_M { return; }
    let weight = 1.0 - smooth(0.0, APRON_M, outside);
    let elevation = height(p.x, p.y);
    let normal = normal(p.x, p.y);
    let grade = f64::from(normal.x.hypot(normal.z) / normal.y.max(0.0001));
    ground.top_m = (f64::from(ground.top_m) + weight * (elevation - f64::from(ground.top_m))) as f32;
    ground.slope = (f64::from(ground.slope) + weight * (grade - f64::from(ground.slope))) as f32;
    if outside == 0.0 {
        ground.water_m = None;
        ground.delta = 0.0;
        ground.suspended = 0.0;
        ground.intertidal = 0.0;
        ground.scree_m = 0.0;
        ground.riparian = 0.0;
        ground.placer = 0.0;
        ground.river_gravel = false;
        ground.river_bedrock = false;
        ground.outlook_clearing = 0.0;
    }
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> Option<BlockId> {
    if !active(cfg) || !contains(f64::from(cfg.world_x(p.x)), f64::from(cfg.world_z(p.z))) {
        return None;
    }
    Some(if p.y > column.height { BlockId::Air }
        else if p.y == cfg.min_world_y() { BlockId::Bedrock }
        else { BlockId::Sandstone })
}

pub(super) fn spawn() -> DVec3 {
    static SPAWN: OnceLock<DVec3> = OnceLock::new();
    *SPAWN.get_or_init(|| {
        let mut best = (f64::NEG_INFINITY, DVec2::splat(SPAN_M * 0.5));
        for z in (600..3400).step_by(24) {
            for x in (600..3400).step_by(24) {
                let p = DVec2::new(f64::from(x), f64::from(z));
                let h = height(p.x, p.y);
                if h < SEA + 8.0 || normal(p.x, p.y).y < 0.96 { continue; }
                let relief = height(p.x - 160.0, p.y).max(height(p.x + 160.0, p.y)) - h;
                if relief > best.0 { best = (relief, p); }
            }
        }
        let chart = frame().chart(best.1.x, best.1.y);
        DVec3::new(chart.x, height(best.1.x, best.1.y).floor() + 2.0, chart.y)
    })
}

pub(super) fn checksum(base: u64) -> u64 {
    let frame = frame();
    let hash = [frame.origin.x, frame.origin.y, frame.across.x, frame.across.y,
        frame.downstream.x, frame.downstream.y, LOW_M, SPACING_M].into_iter()
        .fold(base, |hash, value| (hash ^ value.to_bits()).wrapping_mul(0x100000001b3));
    DATA.iter().fold(hash, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
