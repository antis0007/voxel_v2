//! Generator 53 standing water: lakes, ponds, alpine tarns, crater lakes,
//! oases, cenotes and marsh pools. Each sits on a jittered lattice cell and is
//! a pure function of seed and position. Its level is the lowest ground on a
//! ring just outside the shore, less a freeboard, so the surrounding ground
//! always holds the water; the basin is then carved below that level.
use crate::{WorldSampleConfig, WorldType, drainage};
use engine_core::fast_hash::FastMap;
use glam::Vec2;
use std::cell::RefCell;
use std::sync::{OnceLock, RwLock};

mod plan;
#[cfg(test)]
mod tests;

pub const LAKES_GENERATOR: u32 = crate::gates::LAKES_AND_CLIMATE_GENERATOR;
/// Lattice for lakes, tarns, craters, oases and cenotes.
const BIG_CELL: f32 = 288.0;
/// Lattice for ponds and marsh pools.
const SMALL_CELL: f32 = 56.0;
/// Water depth at the shoreline.
const LIP_M: f32 = 0.35;
/// Bank crest above the water and its width beyond the shore.
const CREST_M: f32 = 0.25;
const CREST_WIDTH_M: f32 = 2.5;
/// Bank distance beyond the shore that a lake can still carve.
const BANK_REACH_M: f32 = 14.0;
const CACHE_CAP: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LakeKind {
    Lake,
    Pond,
    Tarn,
    Crater,
    Oasis,
    Cenote,
    MarshPool,
}

#[derive(Clone, Copy, Debug)]
struct Lake {
    kind: LakeKind,
    centre: Vec2,
    radius: f32,
    level: f32,
    depth: f32,
    /// Outline lobes: amplitude and phase for one, two and three lobes.
    lobes: [(f32, f32); 3],
    /// Bank slope beyond the shore, metres up per metre out.
    bank: f32,
    /// Crater rim height built above the surrounding ground.
    rim: f32,
}

impl Lake {
    fn shore_radius(&self, angle: f32) -> f32 {
        let lobe: f32 = self.lobes.iter().enumerate().map(|(k, &(a, phase))| a * ((k + 1) as f32 * angle + phase).sin()).sum();
        self.radius * (1.0 + lobe)
    }

    /// Metres past the shore (negative inside) and the normalised radius.
    fn edge(&self, p: Vec2) -> (f32, f32) {
        let d = p - self.centre;
        let shore = self.shore_radius(d.y.atan2(d.x));
        (d.length() - shore, d.length() / shore)
    }

    fn reach(&self) -> f32 {
        self.radius * 1.6 + BANK_REACH_M + self.rim * 2.0
    }
}

/// What standing water asks of one column, in ground-top metres.
#[derive(Clone, Copy, Debug)]
pub struct LakeSample {
    /// Highest ground top allowed here.
    pub ceiling_m: f32,
    /// Lowest ground top allowed here, and how far above the terrain it may build.
    pub floor_m: Option<f32>,
    pub floor_cap_m: f32,
    pub water_m: Option<f32>,
    pub kind: Option<LakeKind>,
    /// Metres past the nearest shore; negative under water.
    pub edge_m: f32,
}

pub const NONE: LakeSample =
    LakeSample { ceiling_m: f32::MAX, floor_m: None, floor_cap_m: 0.0, water_m: None, kind: None, edge_m: f32::MAX };

type Key = (u32, u8, u32, u64, bool, i32, i32);
thread_local! {
    static LOCAL: RefCell<FastMap<Key, Option<Lake>>> = RefCell::new(FastMap::default());
}
static SHARED: OnceLock<RwLock<FastMap<Key, Option<Lake>>>> = OnceLock::new();

fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= LAKES_GENERATOR && cfg.world_type != WorldType::Superflat && !drainage::standing_aside()
}

/// The lake a lattice cell owns, memoised per thread and across threads.
fn lake(cfg: &WorldSampleConfig, small: bool, ix: i32, iz: i32) -> Option<Lake> {
    let key = (cfg.seed, cfg.world_type.code(), cfg.generator_version, cfg.planet_fingerprint(), small, ix, iz);
    if let Some(v) = LOCAL.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let shared = SHARED.get_or_init(|| RwLock::new(FastMap::default()));
    let found = shared.read().unwrap_or_else(|e| e.into_inner()).get(&key).copied();
    let v = found.unwrap_or_else(|| {
        let v = plan::site(cfg, small, ix, iz);
        let mut m = shared.write().unwrap_or_else(|e| e.into_inner());
        if m.len() >= 8 * CACHE_CAP {
            m.clear();
        }
        m.insert(key, v);
        v
    });
    LOCAL.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= CACHE_CAP {
            m.clear();
        }
        m.insert(key, v);
    });
    v
}

fn nearby(cfg: &WorldSampleConfig, p: Vec2) -> impl Iterator<Item = Lake> + '_ {
    [(false, BIG_CELL), (true, SMALL_CELL)].into_iter().flat_map(move |(small, cell)| {
        let (ix, iz) = ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32);
        (-1..=1).flat_map(move |dz| (-1..=1).filter_map(move |dx| lake(cfg, small, ix + dx, iz + dz)))
    })
}

/// Ground and water limits from every lake that reaches `(x, z)` metres.
pub fn sample(cfg: &WorldSampleConfig, x: f32, z: f32) -> LakeSample {
    // Same reason as the river carve: the grove's pools keep their own walls.
    if !active(cfg) || crate::amberroot::reserves(cfg, x, z) {
        return NONE;
    }
    let p = Vec2::new(x, z);
    let mut out = NONE;
    let mut nearest: Option<(f32, Lake, f32)> = None;
    for l in nearby(cfg, p) {
        if (p - l.centre).length() > l.reach() {
            continue;
        }
        let (edge, r) = l.edge(p);
        let ceiling = profile(&l, edge, r);
        out.ceiling_m = out.ceiling_m.min(ceiling);
        if nearest.is_none_or(|(e, _, _)| edge < e) {
            nearest = Some((edge, l, ceiling));
        }
    }
    if let Some((edge, l, ceiling)) = nearest {
        out.kind = Some(l.kind);
        out.edge_m = edge;
        out.water_m = (edge <= CREST_WIDTH_M).then_some(l.level);
        let (floor, cap) = floor(&l, edge, ceiling);
        out.floor_m = floor.map(|f| f.min(out.ceiling_m));
        out.floor_cap_m = cap;
    }
    out
}

/// Highest ground top a lake allows at `edge` metres past its shore.
fn profile(l: &Lake, edge: f32, r: f32) -> f32 {
    if edge <= 0.0 {
        let r = r.clamp(0.0, 1.0);
        let shape = match l.kind {
            LakeKind::Cenote => return l.level - l.depth,
            LakeKind::Tarn | LakeKind::Crater => 1.0 - r.powi(4),
            _ => 1.0 - r * r,
        };
        return l.level - LIP_M - (l.depth - LIP_M) * shape;
    }
    match l.kind {
        // A cenote is a shaft: nothing outside it is cut.
        LakeKind::Cenote | LakeKind::Crater => f32::MAX,
        _ => l.level - LIP_M + l.bank * edge + 0.5 * (edge - 6.0).max(0.0).powi(2),
    }
}

/// Lowest ground top a lake asks for, and the most it may build above terrain.
fn floor(l: &Lake, edge: f32, ceiling: f32) -> (Option<f32>, f32) {
    match l.kind {
        LakeKind::Cenote => (None, 0.0),
        LakeKind::Crater if edge > 0.0 => {
            let bump = (-((edge - l.rim) / (1.5 * l.rim)).powi(2)).exp();
            (Some(l.level + CREST_M + l.rim * bump), l.rim + 1.0)
        }
        _ if edge <= 0.0 => (Some(ceiling), drainage::LEVEE_CAP_M),
        _ => (Some(ceiling.min(l.level + CREST_M - 0.8 * (edge - CREST_WIDTH_M).max(0.0))), drainage::LEVEE_CAP_M),
    }
}

/// True when a lake's shore lies within `margin` metres of `(x, z)`, so
/// structures placed afterwards can stand aside.
pub fn reserves(cfg: &WorldSampleConfig, x: f32, z: f32, margin: f32) -> bool {
    active(cfg) && nearby(cfg, Vec2::new(x, z)).any(|l| l.edge(Vec2::new(x, z)).0 < margin)
}

/// Surface and filler for a lake's bed and its shore band.
pub fn shore_blocks(kind: LakeKind) -> (crate::BlockId, crate::BlockId) {
    use crate::BlockId as B;
    match kind {
        LakeKind::Lake | LakeKind::Oasis => (B::Sand, B::Sand),
        LakeKind::Pond | LakeKind::MarshPool => (B::Mud, B::Clay),
        LakeKind::Tarn => (B::Gravel, B::Stone),
        LakeKind::Crater => (B::Basalt, B::Basalt),
        LakeKind::Cenote => (B::Stone, B::Stone),
    }
}
