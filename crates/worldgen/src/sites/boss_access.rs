//! Generator 32 boss complexes. Every boss reliquary stands in the arena of a
//! multilevel complex: a puzzle courtyard and sealed entry hall, a bridge over
//! a chasm, a family-themed arena with galleries, and three treasure rooms
//! (behind a cracked wall, behind a lever gate, and a warded reward vault the
//! app opens after the boss falls).
//!
//! Shrine-local metres: `u` across the door, `y` up from the plaza base, `w`
//! away from the ruin (the door faces the ruin at `w = -4`). Every box uses
//! whole metres and the centre is snapped to the voxel grid, so each box covers
//! whole cells at every voxel scale. Layouts are built once per site variant
//! and cached; a voxel tests only the shapes bucketed over its 4 m column.
use super::{B, WorldSampleConfig, WorldType, cavern_floor, reward_kind, sample_column, site};
use crate::sites::modifiers::{self, SiteModifier};
use engine_core::fast_hash::FastMap;
use engine_core::hash::hash_u32;
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::rc::Rc;

pub const VERSION: u32 = crate::gates::DUNGEON_V2_GENERATOR;
/// Half-width of the square around a region centre that the site sampler
/// offers to the complex: 15 m site jitter + 26 m to the shrine + 29 m of
/// complex, rounded up.
pub const REACH: f32 = 72.0;

mod approach;
mod arenas;
mod layout;
mod plan;
mod points;
pub use points::{TreasurePoint, treasure_points};
pub use layout::{Box3, CLEAR, DOOR, Family, Opens, Rule, Treasure, WEAK_WALL};
#[cfg(test)]
use layout::{HOPPER_COAL, WALL_CRYSTALS};
use layout::RELIC;
pub use plan::BOUNDS;
use plan::{Plan, inside};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shrine {
    pub family: Family,
    /// World metres: plaza base under the shrine centre, snapped to the voxel grid.
    pub center: Vec3,
    turn: u32,
    voxel: f32,
    /// Variant seed: mirroring, damage, heights, bridges.
    seed: u32,
    pub modifier: SiteModifier,
    ossuary: bool,
}

type PlanKey = (Family, u32, SiteModifier, bool);

thread_local! {
    static PLANS: RefCell<(Option<PlanKey>, Option<Rc<Plan>>, FastMap<PlanKey, Rc<Plan>>)> =
        RefCell::new((None, None, FastMap::default()));
}

impl Shrine {
    /// Where the reliquary (and its boss) stands: the middle of the arena floor.
    pub fn relic_point(&self) -> Vec3 {
        self.world_of(Vec3::from_array(RELIC))
    }

    /// Whether the arena half of this complex is mirrored across `u`.
    pub fn mirrored(&self) -> bool {
        self.seed & 1 == 1
    }

    /// The complex's three treasure rooms, index 0..3, tiers 1..=3.
    pub fn treasures(&self) -> [Treasure; 3] {
        approach::treasures(self.family, self.mirrored())
    }

    /// Generated block at a shrine-local point (for tests and tools).
    pub fn local_block(&self, l: Vec3) -> Option<B> {
        self.with_plan(|p| p.sample(l))
    }

    fn with_plan<T>(&self, f: impl FnOnce(&Plan) -> T) -> T {
        let key = (self.family, self.seed, self.modifier, self.ossuary);
        let plan = PLANS.with(|c| {
            let mut c = c.borrow_mut();
            if c.0 == Some(key)
                && let Some(p) = &c.1
            {
                return p.clone();
            }
            let p = match c.2.get(&key) {
                Some(p) => p.clone(),
                None => {
                    if c.2.len() >= 64 {
                        c.2.clear();
                    }
                    let p = Rc::new(arenas::build(self.family, self.seed, self.modifier, self.ossuary));
                    c.2.insert(key, p.clone());
                    p
                }
            };
            c.0 = Some(key);
            c.1 = Some(p.clone());
            p
        });
        f(&plan)
    }

    pub fn local_of(&self, m: Vec3) -> Vec3 {
        let d = m - self.center;
        let (u, w) = match self.turn {
            1 => (-d.z, d.x),
            2 => (-d.x, -d.z),
            3 => (d.z, -d.x),
            _ => (d.x, d.z),
        };
        Vec3::new(u, d.y, w)
    }

    pub fn world_of(&self, l: Vec3) -> Vec3 {
        let (x, z) = match self.turn {
            1 => (l.z, -l.x),
            2 => (-l.x, -l.z),
            3 => (-l.z, l.x),
            _ => (l.x, l.z),
        };
        self.center + Vec3::new(x, l.y, z)
    }

    /// Voxel cells whose centres lie in a shrine-local box.
    pub fn cells(&self, b: Box3) -> Vec<IVec3> {
        let v = self.voxel;
        let a = self.world_of(Vec3::new(b[0] as f32, b[2] as f32, b[4] as f32));
        let c = self.world_of(Vec3::new(b[1] as f32, b[3] as f32, b[5] as f32));
        let lo = (a.min(c) / v).floor().as_ivec3();
        let hi = (a.max(c) / v).ceil().as_ivec3();
        let mut out = Vec::new();
        for x in lo.x..hi.x {
            for y in lo.y..hi.y {
                for z in lo.z..hi.z {
                    let p = IVec3::new(x, y, z);
                    if inside(b, self.local_of((p.as_vec3() + Vec3::splat(0.5)) * v)) {
                        out.push(p);
                    }
                }
            }
        }
        out
    }

    /// Generated block at a world point, or `None` outside the complex.
    pub fn sample(&self, m: Vec3) -> Option<B> {
        let l = self.local_of(m);
        if !inside(BOUNDS, l) {
            return None;
        }
        self.local_block(l)
    }
}

type Cache = RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), Option<Shrine>>)>;

thread_local! {
    static GROUNDS: Cache = RefCell::new((None, FastMap::default()));
    static CAVERNS: Cache = RefCell::new((None, FastMap::default()));
}

fn cached(cache: &'static std::thread::LocalKey<Cache>, cfg: &WorldSampleConfig, key: (i32, i32), make: impl FnOnce() -> Option<Shrine>) -> Option<Shrine> {
    if let Some(hit) = cache.with(|g| {
        let mut g = g.borrow_mut();
        if g.0 != Some(*cfg) {
            *g = (Some(*cfg), FastMap::default());
        }
        g.1.get(&key).copied()
    }) {
        return hit;
    }
    let shrine = make();
    cache.with(|g| {
        let mut g = g.borrow_mut();
        if g.1.len() >= 256 {
            g.1.clear();
        }
        g.1.insert(key, shrine);
    });
    shrine
}

fn variant(cfg: &WorldSampleConfig, x: i32, z: i32, salt: u32) -> u32 {
    hash_u32(cfg.seed ^ hash_u32((x as u32).wrapping_mul(0x27d4_eb2d) ^ salt) ^ hash_u32((z as u32).wrapping_mul(0x1656_67b1)))
}

/// Complex behind a boss ruin, its door 22 m out from the ruin centre on the
/// far side of the courtyard, so the building, descent shaft and valves stay
/// untouched. The arena runs on away from the ruin.
pub fn site_shrine(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Shrine> {
    if cfg.generator_version < VERSION || cfg.world_type == WorldType::Superflat {
        return None;
    }
    cached(&GROUNDS, cfg, (rx, rz), || {
        let s = site(cfg, rx, rz)?;
        let family = match s.kind {
            _ if s.starter => return None,
            1 => Family::Toll,
            3 => Family::Foundry,
            5 => Family::Archive,
            _ => return None,
        };
        let v = cfg.voxel_meters();
        let at = super::site_point(s, Vec3::new(0.0, 0.0, 26.0));
        let x = (at.x / v).floor() * v;
        let z = (at.z / v).floor() * v;
        let ground = if cfg.generator_version >= 39 { s.ground } else {
            sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height as f32 * v
        };
        Some(Shrine {
            family,
            center: Vec3::new(x, ground, z),
            turn: s.turn,
            voxel: v,
            seed: variant(cfg, rx, rz, 0x5a17),
            modifier: modifiers::site_modifier(cfg, (rx, rz)),
            ossuary: cfg.generator_version >= 59,
        })
    })
}

/// Complex at the centre of a Prism Hart cavern (the broken-road chamber).
pub fn cavern_shrine(cfg: &WorldSampleConfig, gx: i32, gz: i32) -> Option<Shrine> {
    if cfg.generator_version < VERSION
        || cfg.world_type == WorldType::Superflat
        || (gx as f32 * 72.0).hypot(gz as f32 * 72.0) < 100.0
    {
        return None;
    }
    cached(&CAVERNS, cfg, (gx, gz), || {
        if reward_kind(cfg, IVec3::new(gx, gz, 1)) != 8 {
            return None;
        }
        let floor = cavern_floor(cfg, gx, gz)?;
        Some(Shrine {
            family: Family::Hart,
            center: Vec3::new(gx as f32 * 72.0, floor, gz as f32 * 72.0),
            turn: 0,
            voxel: cfg.voxel_meters(),
            seed: variant(cfg, gx, gz, 0xca7e),
            modifier: modifiers::site_modifier(cfg, (gx, gz)),
            ossuary: false,
        })
    })
}

/// The shrine holding a reliquary, by its encounter id and placed position.
/// The position disambiguates dungeon anchors that share the `z == 1` id space.
pub fn shrine(cfg: &WorldSampleConfig, id: IVec3, near: Vec3) -> Option<Shrine> {
    let s = match id.z {
        0 => site_shrine(cfg, id.x, id.y),
        1 => cavern_shrine(cfg, id.x, id.y),
        _ => None,
    }?;
    let d = near - s.relic_point();
    (d.x.abs() < 3.0 && d.z.abs() < 3.0 && d.y.abs() < 3.0).then_some(s)
}

/// Surface sampler hook for the region site at `(rx, rz)`.
pub(super) fn site_block(cfg: &WorldSampleConfig, rx: i32, rz: i32, m: Vec3) -> Option<B> {
    let shrine = site_shrine(cfg, rx, rz)?;
    let built = shrine.sample(m);
    if cfg.generator_version < 39 || built.is_some() { return built; }
    let q = shrine.local_of(m);
    let dx = (BOUNDS[0] as f32 - q.x).max(q.x - BOUNDS[1] as f32).max(0.0);
    let dz = (BOUNDS[4] as f32 - q.z).max(q.z - BOUNDS[5] as f32).max(0.0);
    let distance = dx.hypot(dz);
    if distance >= 10.0 || q.y < -12.0 || q.y > 18.0 { return None; }
    let terrain = cfg.world_y(sample_column(cfg, cfg.blocks_for_height(m.x),
        cfg.blocks_for_height(m.z)).height + 1) - shrine.center.y;
    let v = cfg.voxel_meters();
    let top = v + (terrain - v) * crate::smoothstep(0.0, 10.0, distance);
    if q.y < top && q.y >= terrain - 2.0 {
        return Some(if distance == 0.0 { B::Cobblestone }
            else if q.y >= top - v { B::Grass } else { B::Dirt });
    }
    if q.y >= top && q.y < if distance == 0.0 { terrain.max(16.0) } else { terrain } {
        return Some(B::Air);
    }
    None
}

/// Cavern sampler hook. The complex spans `w -10..29` from the chamber centre.
pub(super) fn cavern_block(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let gx = (m.x / 72.0).round() as i32;
    let gz = ((m.z - 9.5) / 72.0).round() as i32;
    let (dx, dz) = (m.x - gx as f32 * 72.0, m.z - gz as f32 * 72.0);
    if dx.abs() > BOUNDS[1] as f32 || dz < BOUNDS[4] as f32 || dz > BOUNDS[5] as f32 {
        return None;
    }
    cavern_shrine(cfg, gx, gz)?.sample(m)
}

#[cfg(test)]
#[path = "boss_access_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "boss_access_walk_tests.rs"]
mod walk_tests;

/// Disposable review uses the same sampler and canonical arena as new worlds.
pub fn ossuary_review(voxel: f32) -> Shrine {
    Shrine { family: Family::Foundry, center: Vec3::ZERO, turn: 0, voxel,
        seed: 0, modifier: SiteModifier::None, ossuary: true }
}
