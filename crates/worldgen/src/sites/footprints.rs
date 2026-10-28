//! Process-wide footprints for the reservation test. Horizon samplers run on
//! scoped threads, so the per-thread site and homestead caches start cold on
//! every pass and a tree query rebuilt whole campuses to read their extents.
use super::{Site, WorldSampleConfig};
use engine_core::fast_hash::{FastBuild, FastMap};
use std::sync::Mutex;

/// A site and its campus extent in site-local metres.
pub(super) type SiteFootprint = Option<(Site, Option<(f32, f32, f32, f32)>)>;
/// Homestead centre and reservation reach in metres, before terrain decides
/// whether the home is built. A planned home has exactly this footprint.
pub(super) type HomeBound = Option<(f32, f32, f32)>;

const SHARDS: usize = 16;
// An 8.4 km horizon square holds about 11k homestead cells and 2k site regions.
const LIMIT: usize = 4096;

struct Shard<T> {
    cfg: Option<WorldSampleConfig>,
    values: FastMap<(i32, i32), T>,
}

struct Table<T>([Mutex<Shard<T>>; SHARDS]);

impl<T: Copy> Table<T> {
    const fn new() -> Self {
        Self([const { Mutex::new(Shard { cfg: None, values: FastMap::with_hasher(FastBuild::new()) }) }; SHARDS])
    }
    /// The lock is released while `calculate` runs; a race only repeats pure work.
    fn get(&self, cfg: &WorldSampleConfig, key: (i32, i32), calculate: impl FnOnce() -> T) -> T {
        let shard = &self.0[(key.0.wrapping_mul(31).wrapping_add(key.1) as u32 as usize) % SHARDS];
        if let Ok(s) = shard.lock()
            && s.cfg == Some(*cfg)
            && let Some(&value) = s.values.get(&key)
        {
            return value;
        }
        let value = calculate();
        if let Ok(mut s) = shard.lock() {
            if s.cfg != Some(*cfg) {
                s.cfg = Some(*cfg);
                s.values.clear();
            }
            if s.values.len() >= LIMIT {
                s.values.clear();
            }
            s.values.insert(key, value);
        }
        value
    }
}

static SITES: Table<SiteFootprint> = Table::new();
static BOUNDS: Table<HomeBound> = Table::new();
static HOMES: Table<bool> = Table::new();

pub(super) fn site(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> SiteFootprint {
    SITES.get(cfg, (rx, rz), || {
        super::site(cfg, rx, rz).map(|s| {
            let building = super::super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version);
            (s, building.map(|b| b.extent()))
        })
    })
}

pub(super) fn home_bound(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> HomeBound {
    BOUNDS.get(cfg, (ix, iz), || super::home_layout(cfg, ix, iz).map(|(_, x, z, b)| (x, z, super::home_reach(&b))))
}

/// Whether the cell's homestead survives its terrain tests.
pub(super) fn home(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> bool {
    HOMES.get(cfg, (ix, iz), || super::homestead(cfg, ix, iz).is_some())
}
