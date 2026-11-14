//! Generator 3 exploration sites. Every feature is a pure function of seed and
//! metre coordinates; edits always override it through World/WorldSnapshot.
use super::{BlockId as B, WorldSampleConfig, WorldType, hash2, sample_column, value_noise2};
use glam::{IVec3, Vec2, Vec3};
use std::cell::RefCell;
use engine_core::fast_hash::FastMap;
use std::collections::HashMap;

pub mod boss_access;
pub mod modifiers;
mod workings;
pub mod kind;
pub use kind::SiteKind;
mod footprints;
mod review;
pub use review::place_review_view;
mod workshop_route;
mod workshop_supplies;

pub(crate) fn region_span(cfg: &WorldSampleConfig) -> f32 {
    if cfg.generator_version >= 9 {
        192.0
    } else {
        224.0
    }
}
#[derive(Clone, Copy)]
struct Site {
    x: f32,
    z: f32,
    ground: f32,
    turn: u32,
    kind: u32,
    seed: u32,
    starter: bool,
}
thread_local! {
    static SITES: RefCell<(Option<WorldSampleConfig>, FastMap<(i32,i32),Option<Site>>)> = RefCell::new((None,FastMap::default()));
    static GALLERIES: RefCell<(Option<WorldSampleConfig>, FastMap<(i32,i32),Option<f32>>)> = RefCell::new((None,FastMap::default()));
}

fn chamber_rarity(version: u32) -> f32 {
    if version >= 17 { 0.22 } else { 0.36 }
}

fn cavern_floor(cfg: &WorldSampleConfig, gx: i32, gz: i32) -> Option<f32> {
    let n = hash2(gx, gz, cfg.seed.wrapping_add(1101));
    if n < chamber_rarity(cfg.generator_version) {
        return None;
    }
    let ground = sample_column(
        cfg,
        cfg.blocks_for_height(gx as f32 * 72.0),
        cfg.blocks_for_height(gz as f32 * 72.0),
    )
    .height as f32
        * cfg.voxel_meters();
    Some(((ground - 18.0 - n * 25.0) / cfg.voxel_meters()).floor() * cfg.voxel_meters())
}

fn gallery_floor(cfg: &WorldSampleConfig, p: IVec3, m: Vec3) -> Option<f32> {
    let key = (p.x, p.z);
    if let Some(cached) = GALLERIES.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != Some(*cfg) {
            *c = (Some(*cfg), FastMap::default());
        }
        c.1.get(&key).copied()
    }) {
        return cached;
    }
    let gx = (m.x / 72.0).round() as i32;
    let gz = (m.z / 72.0).round() as i32;
    let mut result = None;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let (Some(a), Some(b)) = (
            cavern_floor(cfg, gx, gz),
            cavern_floor(cfg, gx + dx, gz + dz),
        ) else {
            continue;
        };
        let along = if dx != 0 {
            (m.x - gx as f32 * 72.0) * dx as f32
        } else {
            (m.z - gz as f32 * 72.0) * dz as f32
        };
        if !(0.0..=72.0).contains(&along) {
            continue;
        }
        let t = along / 72.0;
        let side = if dx != 0 {
            m.z - gz as f32 * 72.0
        } else {
            m.x - gx as f32 * 72.0
        };
        // Symmetric bow: both directions evaluate the same connection.
        let bend = (t * std::f32::consts::PI).sin() * 3.0;
        if (side - bend).abs() < 2.0 {
            result = Some(((a + (b - a) * t) / cfg.voxel_meters()).floor() * cfg.voxel_meters());
            break;
        }
    }
    GALLERIES.with(|c| {
        let mut c = c.borrow_mut();
        if c.1.len() >= 8192 {
            c.1.clear();
        }
        c.1.insert(key, result);
    });
    result
}

fn site_roll(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> (u32, Vec2, bool) {
    let starter = rx == 0 && rz == 0;
    let seed = cfg
        .seed
        .wrapping_add((rx as u32).wrapping_mul(73856093))
        .wrapping_add((rz as u32).wrapping_mul(19349663));
    let curated = cfg.generator_version >= engine_voxel::CURATED_SURFACE_GENERATOR;
    let admitted = starter || !curated || crate::hash3(rx, rz, 41, seed) < 0.28;
    let x = if starter {
        0.0
    } else {
        rx as f32 * region_span(cfg)
            + (hash2(rx, rz, seed) - 0.5)
                * if cfg.generator_version >= 9 {
                    30.0
                } else {
                    70.0
                }
    };
    let z = if starter {
        -42.0
    } else {
        rz as f32 * region_span(cfg)
            + (hash2(rx, rz, seed.wrapping_add(1)) - 0.5)
                * if cfg.generator_version >= 9 {
                    30.0
                } else {
                    70.0
                }
    };
    (seed, Vec2::new(x, z), admitted)
}

/// Potential legacy site centres without probing columns during the Planet bake.
pub(crate) fn trail_keepouts(cfg: &WorldSampleConfig, centre: Vec2) -> Vec<Vec2> {
    let cell = (centre / region_span(cfg)).round().as_ivec2();
    (-5..=5).flat_map(|z| (-5..=5).filter_map(move |x| {
        let (ix, iz) = (cell.x + x, cell.y + z);
        let (seed, at, admitted) = site_roll(cfg, ix, iz);
        (admitted && ((ix == 0 && iz == 0) || hash2(ix, iz, seed.wrapping_add(3)) > 0.18))
            .then_some(at)
    })).collect()
}

fn site(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Site> {
    let cached = SITES.with(|s| {
        let mut c = s.borrow_mut();
        if c.0 != Some(*cfg) {
            *c = (Some(*cfg), FastMap::default());
        }
        c.1.get(&(rx, rz)).copied()
    });
    if let Some(s) = cached {
        return s;
    }
    let starter = rx == 0 && rz == 0;
    let curated = cfg.generator_version >= engine_voxel::CURATED_SURFACE_GENERATOR;
    let (seed, Vec2 { x, y: z }, admitted) = site_roll(cfg, rx, rz);
    let ground = if admitted {
        sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height as f32
            * cfg.voxel_meters()
    } else {
        f32::NEG_INFINITY
    };
    let result = if admitted && (starter
        || (ground > 17.0 && ground < if cfg.generator_version>=91 {460.0}else{105.0} && hash2(rx, rz, seed.wrapping_add(3)) > 0.18))
        && !super::undercrown::reserved(cfg, Vec3::new(x, ground, z))
        && !super::ruins::reserves(cfg, x, z, 70.0)
    {
        let kind = if starter {
            Some(0)
        } else {
            let biome = || super::landscape::sample(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).biome;
            if curated {
                kind::pick_curated(biome(), hash2(rx, rz, seed.wrapping_add(5)))
            } else {
                Some(kind::pick(cfg, rx, rz, seed, biome))
            }
        };
        kind.map(|kind| Site {
            x,
            z,
            ground,
            turn: if starter {
                0
            } else {
                (hash2(rx, rz, seed.wrapping_add(4)) * 4.0) as u32 % 4
            },
            kind,
            seed,
            starter,
        })
    } else {
        None
    };
    let result = if cfg.generator_version >= 37 {
        result.and_then(|mut s| {
            if cfg.generator_version >= 92 && !s.starter && matches!(s.kind, 1 | 3 | 5) {
                let (hx,hz) = super::households::district_center(s.x,s.z);
                // Ruined boss campuses belong outside the inhabited district.
                if (s.x-hx).hypot(s.z-hz) < 160.0 { return None; }
            }
            let building = super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version);
            let mut bounds = building.as_ref().map_or((-14.0, -14.0, 14.0, 14.0), |b| b.extent());
            if cfg.generator_version >= 39 && matches!(s.kind, 1 | 3 | 5) {
                // The ruin and its attached boss court are one campus: survey
                // the whole court and its apron at the same floor elevation.
                bounds.0 = bounds.0.min(-26.0);
                bounds.2 = bounds.2.max(26.0);
                bounds.3 = bounds.3.max(65.0);
            }
            // Survey the entire plot and its apron, not just its centre.
            // Reject a cliff site rather than manufacturing a forty-metre plinth.
            if cfg.generator_version >= 51 && matches!(s.kind, 1 | 3 | 5) {
                // A boss campus that misses at its rolled facing may fit at
                // another. The Foundry plot is the widest, so it gets the
                // starter's allowance; at 7 m it almost never fit at all.
                let relief = if s.kind == 3 { 10.0 } else { 7.0 };
                let (turn, ground) = (0..4).find_map(|i| {
                    let turn = (s.turn + i) % 4;
                    fit_ground(cfg, s.x, s.z, turn, bounds, relief).map(|g| (turn, g))
                })?;
                s.turn = turn;
                s.ground = ground;
                return Some(s);
            }
            s.ground = fit_ground(cfg, s.x, s.z, s.turn, bounds, if s.starter { 10.0 } else { 7.0 })?;
            Some(s)
        })
    } else {
        result
    };
    SITES.with(|s| {
        let mut c = s.borrow_mut();
        if c.1.len() >= 256 {
            c.1.clear();
        }
        c.1.insert((rx, rz), result);
    });
    result
}

/// Once per cached site. Includes the apron so a doorway cannot face a cliff.
fn fit_ground(
    cfg: &WorldSampleConfig, x: f32, z: f32, turn: u32,
    (x0, z0, x1, z1): (f32, f32, f32, f32), max_relief: f32,
) -> Option<f32> {
    let margin = x0.abs().max(x1.abs()).hypot(z0.abs().max(z1.abs()));
    if super::planetary::reserves_volcanic_summit(cfg, x, z, margin) {
        return None;
    }
    if cfg.generator_version >= 39 {
        // Sample at a fixed spatial interval: nine points per axis missed
        // narrow ravines in large castle plots. This runs only when planning.
        let nx = ((x1 - x0) / 3.0).ceil() as usize;
        let nz = ((z1 - z0) / 3.0).ceil() as usize;
        let mut heights = Vec::with_capacity((nx + 1) * (nz + 1));
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for iz in 0..=nz {
            for ix in 0..=nx {
                let lx = x0 + (x1 - x0) * ix as f32 / nx.max(1) as f32;
                let lz = z0 + (z1 - z0) * iz as f32 / nz.max(1) as f32;
                let (dx, dz) = match turn {
                    1 => (lz, -lx), 2 => (-lx, -lz), 3 => (-lz, lx), _ => (lx, lz),
                };
                let (wx, wz) = (x + dx, z + dz);
                if super::valley::reserves(cfg, wx, wz) || super::amberroot::reserves(cfg, wx, wz) {
                    return None;
                }
                let column = sample_column(cfg, cfg.blocks_for_height(wx), cfg.blocks_for_height(wz));
                let h = cfg.world_y(column.height);
                if h <= 14.0 || column.cave.opening.carves(1.0, h - super::WATER_LEVEL_METERS)
                    || super::undercrown::reserved(cfg, Vec3::new(wx, h, wz)) {
                    return None;
                }
                // Relief only grows as the survey continues, and every other
                // failure is also None, so stop at the first excess.
                (low, high) = (low.min(h), high.max(h));
                if high - low > max_relief {
                    return None;
                }
                heights.push(h);
            }
        }
        heights.sort_by(f32::total_cmp);
        if heights.last()? - heights[0] > max_relief { return None; }
        return Some(heights[heights.len() / 2]);
    }
    let mut heights = [0.0f32; 81];
    for iz in 0..9 {
        for ix in 0..9 {
            let (lx, lz) = (x0 + (x1 - x0) * ix as f32 / 8.0, z0 + (z1 - z0) * iz as f32 / 8.0);
            let (dx, dz) = match turn {
                1 => (lz, -lx), 2 => (-lx, -lz), 3 => (-lz, lx), _ => (lx, lz),
            };
            heights[iz * 9 + ix] = cfg.world_y(sample_column(cfg,
                cfg.blocks_for_height(x + dx), cfg.blocks_for_height(z + dz)).height);
        }
    }
    heights.sort_by(f32::total_cmp);
    if heights[0] <= 14.0 || heights[80] - heights[0] > max_relief { return None; }
    Some(heights[40])
}

fn local(s: Site, p: Vec3) -> Vec3 {
    let x = p.x - s.x;
    let z = p.z - s.z;
    let (x, z) = match s.turn {
        1 => (-z, x),
        2 => (-x, -z),
        3 => (z, -x),
        _ => (x, z),
    };
    Vec3::new(x, p.y - s.ground, z)
}

const HOME_CELL: f32 = 80.0;

/// A small roofed outpost between the large sites (cottage, farmstead, watch post, chapel, inn).
struct Homestead {
    x: f32,
    z: f32,
    ground: f32,
    turn: u32,
    reach: f32,
    building: super::architecture::Building,
}

type HomeSlot = Option<std::rc::Rc<Homestead>>;

thread_local! {
    static HOMES: RefCell<(Option<WorldSampleConfig>, FastMap<(i32, i32), HomeSlot>)> = RefCell::new((None, FastMap::default()));
    static HOME_LAST: RefCell<Option<(WorldSampleConfig, (i32, i32), HomeSlot)>> = const { RefCell::new(None) };
}

/// Seed, centre and building of a homestead cell, before any terrain test.
fn home_layout(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<(u32, f32, f32, super::architecture::Building)> {
    let seed = cfg
        .seed
        .wrapping_add((ix as u32).wrapping_mul(0x9E37_79B1))
        .wrapping_add((iz as u32).wrapping_mul(0x85EB_CA6B));
    let (hx,hz) = super::households::district_center(ix as f32*HOME_CELL,iz as f32*HOME_CELL);
    let threshold = if cfg.generator_version >= 92 {
        // Concentrated village buildings, with occasional isolated farmsteads.
        if (ix as f32*HOME_CELL-hx).hypot(iz as f32*HOME_CELL-hz)<120.0 {
            0.20
        } else if cfg.generator_version >= engine_voxel::CURATED_SURFACE_GENERATOR {
            return None;
        } else {
            0.96
        }
    } else { 0.68 };
    if hash2(ix, iz, seed.wrapping_add(900)) < threshold {
        return None;
    }
    // Leave the entire graded apron inside the owning 80 m cell.
    let jitter = if cfg.generator_version >= 39 { 6.0 } else { 24.0 };
    let x = ix as f32 * HOME_CELL + (hash2(ix, iz, seed.wrapping_add(901)) - 0.5) * jitter;
    let z = iz as f32 * HOME_CELL + (hash2(ix, iz, seed.wrapping_add(902)) - 0.5) * jitter;
    let variety = cfg.generator_version >= super::architecture::CONNECTED_ROOF_VERSION;
    let mut building = super::architecture::homestead_building_versioned(seed, variety);
    if variety {
        building = building.connected();
    }
    if cfg.generator_version >= 39 {
        let (x0, z0, x1, z1) = building.extent();
        building = super::architecture::Building::new(building.wings, building.curtains,
            Some((x0 - 1.0, x1 + 1.0, z0 - 1.0, z1 + 1.0, B::Cobblestone, 12.0)));
    }
    Some((seed, x, z, building))
}

fn home_reach(building: &super::architecture::Building) -> f32 {
    let (x0, z0, x1, z1) = building.extent();
    x0.abs().max(z0.abs()).max(x1.abs()).max(z1.abs()) * 1.5 + 2.0
}

fn plan_homestead(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> HomeSlot {
    let (seed, x, z, building) = home_layout(cfg, ix, iz)?;
    if x.hypot(z) < 90.0 || super::valley::reserves(cfg, x, z) || super::amberroot::reserves(cfg, x, z)
        || super::ruins::reserves(cfg, x, z, 30.0) {
        return None;
    }
    // Leave room around the big ruins.
    let span = region_span(cfg);
    let (rx, rz) = ((x / span).round() as i32, (z / span).round() as i32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            if let Some(s) = site(cfg, rx + dx, rz + dz)
                && (s.x - x).hypot(s.z - z) < if cfg.generator_version >= 39 { 78.0 } else { 44.0 } {
                    return None;
                }
            if cfg.generator_version >= 39 && let Some(s) = site(cfg, rx + dx, rz + dz) {
                let q = local(s, Vec3::new(x, s.ground, z));
                if matches!(s.kind, 1 | 3 | 5) && q.x.abs() < 62.0 && (-22.0..101.0).contains(&q.z) {
                    return None;
                }
            }
        }
    }
    let v = cfg.voxel_meters();
    let height = |px: f32, pz: f32| {
        sample_column(cfg, cfg.blocks_for_height(px), cfg.blocks_for_height(pz)).height as f32 * v
    };
    let mut ground = height(x, z);
    let mut low = ground;
    for (ox, oz) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
        let g = height(x + ox, z + oz);
        ground = ground.max(g);
        low = low.min(g);
    }
    let ceiling = if cfg.generator_version >= 92 { 300.0 } else { 110.0 };
    if !(14.5..ceiling).contains(&ground) || ground - low > 4.5 {
        return None;
    }
    if super::undercrown::reserved(cfg, Vec3::new(x, ground, z)) {
        return None;
    }
    let mut turn = (hash2(ix, iz, seed.wrapping_add(903)) * 4.0) as u32 % 4;
    if cfg.generator_version >= 92 && let Some(door) = building.entrance() {
        let (hx,hz) = super::households::district_center(x,z);
        let mut best = f32::MIN;
        for candidate in 0..4 {
            let (dx,dz) = match candidate {
                1 => (door.z,-door.x), 2 => (-door.x,-door.z),
                3 => (-door.z,door.x), _ => (door.x,door.z),
            };
            let score = dx*(hx-x)+dz*(hz-z);
            if score>best { best=score; turn=candidate; }
        }
    }
    if cfg.generator_version >= 37 {
        ground = fit_ground(cfg, x, z, turn, building.extent(), 6.0)?;
        if let Some(door) = building.entrance() {
            let (dx, dz) = match turn {
                1 => (door.z, -door.x), 2 => (-door.x, -door.z),
                3 => (-door.z, door.x), _ => (door.x, door.z),
            };
            ground = height(x + dx, z + dz);
        }
    }
    let reach = home_reach(&building);
    Some(std::rc::Rc::new(Homestead {
        x,
        z,
        ground,
        turn,
        reach,
        building,
    }))
}

fn homestead(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> HomeSlot {
    if let Some(hit) = HOME_LAST.with(|l| {
        l.borrow()
            .as_ref()
            .filter(|(c, k, _)| c == cfg && *k == (ix, iz))
            .map(|(_, _, h)| h.clone())
    }) {
        return hit;
    }
    let found = HOMES.with(|h| {
        let mut h = h.borrow_mut();
        if h.0 != Some(*cfg) {
            *h = (Some(*cfg), FastMap::default());
        }
        h.1.get(&(ix, iz)).cloned()
    });
    let slot = match found {
        Some(slot) => slot,
        None => {
            let built = plan_homestead(cfg, ix, iz);
            HOMES.with(|h| {
                let mut h = h.borrow_mut();
                if h.1.len() >= 512 {
                    h.1.clear();
                }
                h.1.insert((ix, iz), built.clone());
            });
            built
        }
    };
    HOME_LAST.with(|l| *l.borrow_mut() = Some((*cfg, (ix, iz), slot.clone())));
    slot
}

fn homestead_block(cfg: &WorldSampleConfig, m: Vec3, terrain_top: f32) -> Option<B> {
    let home = homestead(
        cfg,
        (m.x / HOME_CELL).round() as i32,
        (m.z / HOME_CELL).round() as i32,
    )?;
    let (dx, dz) = (m.x - home.x, m.z - home.z);
    if dx.abs() > home.reach || dz.abs() > home.reach {
        return None;
    }
    let (x, z) = match home.turn {
        1 => (-dz, dx),
        2 => (-dx, -dz),
        3 => (dz, -dx),
        _ => (dx, dz),
    };
    home.building.sample_sited(
        Vec3::new(x, m.y - home.ground, z),
        cfg.voxel_meters(),
        terrain_top - home.ground,
        cfg.generator_version,
    )
}

fn site_point(s: Site, p: Vec3) -> Vec3 {
    let (x, z) = match s.turn {
        1 => (p.z, -p.x),
        2 => (-p.x, -p.z),
        3 => (-p.z, p.x),
        _ => (p.x, p.z),
    };
    Vec3::new(s.x + x, s.ground + p.y, s.z + z)
}

/// Visits settled architecture once per placement cell, without sampling its voxels.
pub fn visit_skylines(
    cfg: &WorldSampleConfig,
    center: glam::Vec2,
    radius: f32,
    cancel: &std::sync::atomic::AtomicBool,
    mut emit: impl FnMut([Vec3; 9], f32, B, B),
) {
    if cfg.generator_version < 39
        || !matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
    {
        return;
    }
    let cells = |span: f32| {
        let lo = ((center - radius) / span).round().as_ivec2();
        let hi = ((center + radius) / span).round().as_ivec2();
        (lo.x..=hi.x, lo.y..=hi.y)
    };
    let nearby = |x, z| center.distance_squared(glam::Vec2::new(x, z)) <= radius * radius;
    let mut building = |s: Site, plan: &super::architecture::Building| {
        for wing in &plan.wings {
            let (roof, wall, cap) = wing.skyline();
            emit(roof.map(|p| site_point(s, p)), s.ground, wall, cap);
        }
    };
    let (xs, zs) = cells(region_span(cfg));
    for z in zs {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) { return; }
        for x in xs.clone() {
            if let Some(s) = site(cfg, x, z)
                && nearby(s.x, s.z)
                && (s.starter || !super::valley::reserves(cfg, s.x, s.z))
                && let Some(plan) = super::architecture::site_building_version(
                    s.kind, s.seed, s.starter, cfg.generator_version,
                )
            {
                building(s, &plan);
            }
        }
    }
    let (xs, zs) = cells(HOME_CELL);
    for z in zs {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) { return; }
        for x in xs.clone() {
            if let Some(h) = homestead(cfg, x, z) && nearby(h.x, h.z) {
                let s = Site { x: h.x, z: h.z, ground: h.ground, turn: h.turn,
                    kind: 0, seed: 0, starter: false };
                building(s, &h.building);
            }
        }
    }
}

/// These fixtures share the ruin's orientation and metre scale across voxel sizes.
pub fn foundry_stations(cfg: &WorldSampleConfig, id: IVec3) -> Vec<Vec3> {
    let Some(s) = site(cfg, id.x, id.y) else {
        return Vec::new();
    };
    if id.z != 0 || s.kind != 3 {
        return Vec::new();
    }
    [
        (-5.0, -6.0),
        (0.0, -6.0),
        (5.0, -6.0),
        (0.0, 8.0),
        (-12.0, -7.0),
    ]
    .into_iter()
    .map(|(x, z)| site_point(s, Vec3::new(x, cfg.voxel_meters(), z)))
    .collect()
}

pub fn foundry_rotation(cfg: &WorldSampleConfig, id: IVec3) -> f32 {
    site(cfg, id.x, id.y).map_or(0.0, |s| s.turn as f32 * std::f32::consts::FRAC_PI_2)
}

/// World-space target of the starter workshop's mine heart. Shared by the
/// compass so the return trip after collecting all three sigils has the same
/// destination as the interaction itself.
/// True when a surface fixture would intrude on an authored site or homestead.
/// Kept in this module so the trail layer can respect private site planning
/// without duplicating the footprints.
pub(super) fn surface_reserved(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    if cfg.generator_version >= 39 {
        return building_reserved(cfg, x, z) || super::dungeon::reserved(cfg, Vec3::new(x, 0.0, z));
    }
    let span = region_span(cfg);
    let rx = (x / span).round() as i32;
    let rz = (z / span).round() as i32;
    if let Some(s) = site(cfg, rx, rz) {
        let reach = 18.0 + site_variation(s);
        if (s.x - x).hypot(s.z - z) < reach {
            return true;
        }
    }
    if let Some(h) = homestead(
        cfg,
        (x / HOME_CELL).round() as i32,
        (z / HOME_CELL).round() as i32,
    )
        && (h.x - x).hypot(h.z - z) < h.reach + 4.0 {
            return true;
        }
    super::amberroot::reserves(cfg, x, z) || super::dungeon::reserved(cfg, Vec3::new(x, 0.0, z))
}

/// Approach side of the starter decline, before the floor starts sloping.
pub fn workshop_mine_entrance(cfg: &WorldSampleConfig) -> Option<Vec3> {
    let starter = site(cfg, 0, 0)?;
    Some(Vec3::new(starter.x, starter.ground + cfg.voxel_meters(), starter.z + 0.5))
}

/// The supply gallery owns one crate at each voxel scale.
pub fn workshop_supply_cache(cfg: &WorldSampleConfig, p: IVec3) -> bool {
    if !workshop_supplies::enabled(cfg) {
        return false;
    }
    let v = cfg.voxel_meters();
    let centre = (p.as_vec3() + Vec3::splat(0.5)) * v;
    if !(11.0..12.0).contains(&centre.x) || !(-54.0..-53.0).contains(&centre.z) {
        return false;
    }
    site(cfg, 0, 0).is_some_and(|s| workshop_supplies::sample(local(s, centre), v) == Some(B::SurveyCache))
}

pub fn workshop_heart(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.world_type == WorldType::Superflat {
        return None;
    }
    let starter = site(cfg, 0, 0)?;
    Some(Vec3::new(11.0, starter.ground - 18.0, -90.0))
}

/// Edge of one surface-site region in metres; `site_seat` takes region indices on this grid.
pub fn site_region_span(cfg: &WorldSampleConfig) -> f32 {
    region_span(cfg)
}

/// A clear spot beside the surface site of region `(rx, rz)` for an app-side overlay
/// (mana hedrons): `(x, z)` in metres, 16-28 m from the site centre, with the overlay and a
/// 4 m cross around it off the site building and homesteads. Reads only; places no blocks.
pub fn site_seat(cfg: &WorldSampleConfig, rx: i32, rz: i32, salt: u32) -> Option<(f32, f32)> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let s = site(cfg, rx, rz)?;
    let turn = hash2(rx, rz, s.seed.wrapping_add(salt));
    for step in 0..8 {
        let angle = (turn + step as f32 / 8.0) * std::f32::consts::TAU;
        for ring in [16.0, 22.0, 28.0] {
            let (x, z) = (s.x + ring * angle.cos(), s.z + ring * angle.sin());
            let clear = [(0.0, 0.0), (4.0, 0.0), (-4.0, 0.0), (0.0, 4.0), (0.0, -4.0)]
                .iter()
                .all(|&(dx, dz)| !trail_obstructed(cfg, x + dx, z + dz));
            if clear {
                return Some((x, z));
            }
        }
    }
    None
}

/// Returns `(world_pos_meters, kind, turn, size_variation)` for surface sites within `max_dist` metres.
pub fn horizon_landmarks(
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    max_dist: f32,
) -> Vec<(Vec3, u32, u32, f32)> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return Vec::new();
    }
    let span = region_span(cfg);
    let radius = (max_dist / span).ceil() as i32 + 1;
    let crx = (center.0 as f32 / span).round() as i32;
    let crz = (center.1 as f32 / span).round() as i32;
    let mut out = Vec::new();
    for rz in crz - radius..=crz + radius {
        for rx in crx - radius..=crx + radius {
            let Some(s) = site(cfg, rx, rz) else {
                continue;
            };
            let dist = (s.x - center.0 as f32).hypot(s.z - center.1 as f32);
            if dist > max_dist {
                continue;
            }
            let variation = if s.starter {
                0.0
            } else {
                hash2(0, 0, s.seed.wrapping_add(67)) * 3.0
            };
            out.push((Vec3::new(s.x, s.ground, s.z), s.kind, s.turn, variation));
        }
    }
    out
}

/// Trail endpoints use the actual door and the site's rotation, not its centre.
pub(super) fn trail_entrances(cfg: &WorldSampleConfig, reach: f32) -> Vec<Vec3> {
    let radius = (reach / region_span(cfg)).ceil() as i32;
    let mut out = Vec::new();
    for rz in -radius..=radius {
        for rx in -radius..=radius {
            let Some(s) = site(cfg, rx, rz) else { continue };
            if s.x.hypot(s.z) > reach || super::valley::reserves(cfg, s.x, s.z) { continue; }
            let door = super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version)
                .and_then(|b| b.entrance()).unwrap_or(Vec3::new(0.0, 0.0, 12.0));
            let mut p = site_point(s, door);
            p.y += cfg.voxel_meters();
            out.push(p);
        }
    }
    out.sort_by(|a, b| (a.x * a.x + a.z * a.z).total_cmp(&(b.x * b.x + b.z * b.z)));
    out
}

/// The ruin door in region (rx, rz) as a road endpoint, if the region has a ruin.
pub(crate) fn region_door(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<Vec3> {
    let s = site(cfg, rx, rz)?;
    if super::valley::reserves(cfg, s.x, s.z) {
        return None;
    }
    let door = super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version)
        .and_then(|b| b.entrance())
        .unwrap_or(Vec3::new(0.0, 0.0, 12.0));
    let mut p = site_point(s, door);
    p.y += cfg.voxel_meters();
    Some(p)
}

/// The boss court behind a ruin (kinds 1, 3, 5). Regional roads go around it.
pub(crate) fn campus_reserved(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    let span = region_span(cfg);
    let (rx, rz) = ((x / span).round() as i32, (z / span).round() as i32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let Some(s) = site(cfg, rx + dx, rz + dz) else { continue };
            if !matches!(s.kind, 1 | 3 | 5) {
                continue;
            }
            let q = local(s, Vec3::new(x, s.ground, z));
            if q.x.abs() < 30.0 && (10.0..69.0).contains(&q.z) {
                return true;
            }
        }
    }
    false
}

/// Homestead doors whose home cell centre lies in the box, as road endpoints.
pub(crate) fn homestead_doors(cfg: &WorldSampleConfig, x0: f32, z0: f32, x1: f32, z1: f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    let (ix0, ix1) = ((x0 / HOME_CELL).ceil() as i32, (x1 / HOME_CELL).floor() as i32);
    let (iz0, iz1) = ((z0 / HOME_CELL).ceil() as i32, (z1 / HOME_CELL).floor() as i32);
    for iz in iz0..=iz1 {
        for ix in ix0..=ix1 {
            if (ix as f32 * HOME_CELL) >= x1 || (iz as f32 * HOME_CELL) >= z1 {
                continue;
            }
            let Some(home) = homestead(cfg, ix, iz) else { continue };
            let Some(door) = home.building.entrance() else { continue };
            let (dx, dz) = match home.turn {
                1 => (door.z, -door.x), 2 => (-door.x, -door.z),
                3 => (-door.z, door.x), _ => (door.x, door.z),
            };
            out.push(Vec3::new(home.x + dx, home.ground + cfg.voxel_meters(), home.z + dz));
        }
    }
    out
}

/// Only the occupied footprint, not the surrounding plot. Roads may reach a
/// door across its courtyard but must not take a shortcut through the house.
pub(super) fn trail_obstructed(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    let span = region_span(cfg);
    if let Some(s) = site(cfg, (x / span).round() as i32, (z / span).round() as i32) {
        let q = local(s, Vec3::new(x, s.ground, z));
        if let Some(b) = super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version) {
            if b.occupies(q.x, q.z) { return true; }
        } else if q.x.hypot(q.z) < 6.0 { return true; }
    }
    if let Some(h) = homestead(cfg, (x / HOME_CELL).round() as i32, (z / HOME_CELL).round() as i32) {
        let (dx, dz) = (x - h.x, z - h.z);
        let (lx, lz) = match h.turn {
            1 => (-dz, dx), 2 => (-dx, -dz), 3 => (dz, -dx), _ => (dx, dz),
        };
        return h.building.occupies(lx, lz);
    }
    false
}

type RelicSurveyKey = (WorldSampleConfig, u32, bool, i32, i32, i32, i32);
thread_local! {
    static RELIC_SURVEY: RefCell<Option<(RelicSurveyKey, Vec<(u32, Vec3)>)>> = const { RefCell::new(None) };
}

/// Cache the terrain survey, not the chosen target: movement still selects
/// the exact nearest candidate and updates bearing/distance every frame.
pub fn next_relic(cfg: &WorldSampleConfig, origin: Vec3, collected: u32) -> Option<(u32, Vec3)> {
    if cfg.generator_version < 4 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let target = if collected & 1 == 0 {
        3
    } else if collected & 2 == 0 {
        5
    } else {
        8
    };
    let rx = (origin.x / region_span(cfg)).round() as i32;
    let rz = (origin.z / region_span(cfg)).round() as i32;
    let caverns = target == 8 && collected & 4 == 0;
    let gx = if caverns { (origin.x / 72.0).round() as i32 } else { 0 };
    let gz = if caverns { (origin.z / 72.0).round() as i32 } else { 0 };
    let key = (*cfg, target, caverns, rx, rz, gx, gz);
    RELIC_SURVEY.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|(old, _)| *old != key) {
            *cache = Some((key, relic_candidates(cfg, target, caverns, rx, rz, gx, gz)));
        }
        cache.as_ref()?.1.iter().copied().min_by(|(_, a), (_, b)|
            a.distance_squared(origin).total_cmp(&b.distance_squared(origin)))
    })
}

fn relic_candidates(cfg: &WorldSampleConfig, target: u32, caverns: bool,
    rx: i32, rz: i32, gx: i32, gz: i32) -> Vec<(u32, Vec3)> {
    let mut candidates = Vec::new();
    for x in rx - 5..=rx + 5 {
        for z in rz - 5..=rz + 5 {
            if let Some(s) = site(cfg, x, z)
                && s.kind == target {
                    candidates.push((s.kind, site_point(s, Vec3::Y)));
                }
        }
    }
    if caverns {
        for x in gx - 8..=gx + 8 {
            for z in gz - 8..=gz + 8 {
                if (x as f32 * 72.0).hypot(z as f32 * 72.0) < 100.0
                    || reward_kind(cfg, IVec3::new(x, z, 1)) != 8
                {
                    continue;
                }
                if let Some(floor) = cavern_floor(cfg, x, z) {
                    candidates.push((8, Vec3::new(x as f32 * 72.0, floor + 1.0, z as f32 * 72.0)));
                }
            }
        }
    }
    candidates
}

/// Three stacked rooms under a ruin, joined by one stair shaft at the back wall.
/// Finding the ruin is the entrance; each landing is the next layer down.
fn ruin_descent(s: Site, cfg: &WorldSampleConfig, q: Vec3) -> Option<B> {
    if cfg.generator_version >= 41 {
        return workings::pit(s, cfg, q);
    }
    if cfg.generator_version < 16 || s.starter {
        return None;
    }
    let v = cfg.voxel_meters();
    let sx = q.x;
    let sz = q.z + 6.0;
    let below = -q.y;
    if !(-1.0..74.0).contains(&below) || sx.abs() > 8.0 || sz.abs() > 7.0 {
        return None;
    }
    let shaft = sx.abs() < 1.55 && sz.abs() < 1.55 && below < 70.0 && below > -0.2;
    let band = (below / 22.0).floor();
    let rel = below.rem_euclid(22.0);
    let room = (1.0..=3.0).contains(&band) && sx.abs() < 7.2 && sz.abs() < 6.2 && rel < 5.2;
    if shaft {
        let turn = below * 0.85;
        let step = (sx - turn.cos() * 0.85).hypot(sz - turn.sin() * 0.85) < 0.42
            && below.fract() < v.max(0.4);
        if step && below > 0.4 {
            return Some(B::StoneBricks);
        }
        if below.rem_euclid(8.0) < v && below > 1.0 {
            return Some(B::GlowCap);
        }
        return Some(B::Air);
    }
    if !room {
        return None;
    }
    if rel < v {
        return Some(match band as u32 {
            1 => B::FungalSoil,
            2 => B::Ice,
            _ => B::Basalt,
        });
    }
    if (4.6..5.6).contains(&sx) && sz.abs() < 0.55 && (1.0..2.0).contains(&rel) {
        return Some(match band as u32 {
            1 => B::CultivatorCache,
            2 => B::SurveyCache,
            _ => B::SalvageCrate,
        });
    }
    if sx.abs() < 0.6 && (3.5..4.5).contains(&sz) && (1.0..2.2).contains(&rel) {
        return Some(B::Crystal);
    }
    Some(B::Air)
}

fn sample_site(s: Site, cfg: &WorldSampleConfig, p: Vec3, terrain_top: f32) -> Option<B> {
    let q = local(s, p);
    let v = cfg.voxel_meters();
    if s.starter && workshop_supplies::enabled(cfg)
        && let Some(b) = workshop_supplies::sample(q, v) { return Some(b); }
    if s.starter && cfg.generator_version >= 48
        && let Some(b) = workshop_route::sample(q, v) { return Some(b); }
    if s.kind == 1 && cfg.generator_version >= 26 {
        return super::toll_crossing::block_versioned(q,v,cfg.generator_version >= super::toll_crossing::CONNECTED_STAIRS_GENERATOR);
    }
    if let Some(b) = ruin_descent(s, cfg, q) {
        return Some(b);
    }
    if cfg.generator_version >= 41 && s.kind == 0
        && let Some(b) = workings::mine(q, v, cfg.generator_version) { return Some(b); }
    // A dry, walkable descent connects each mine to its first fungal chamber.
    // Positive local depth is forward into the hillside (negative local z).
    let depth = -q.z;
    if s.kind == 0 && q.x.abs() < 2.6 && (-1.0..=44.0).contains(&depth) {
        let floor = -(depth.max(0.0) * 0.45).min(18.0);
        let floor = (floor / v).floor() * v;
        if q.y >= floor && q.y < floor + v {
            return Some(B::Cobblestone);
        }
        if q.y >= floor + v && q.y < floor + 4.4 {
            if depth > 4.0 && depth.rem_euclid(7.0) < v && (q.x.abs() > 1.7 || q.y > floor + 3.5) {
                return Some(B::Log);
            }
            return Some(B::Air);
        }
        // Overlap the lining across the stepped roof so loose overburden
        // cannot settle into the starter haulage lane.
        if s.starter && depth >= 7.0 && workshop_supplies::enabled(cfg)
            && (floor + 4.4..floor + 4.4 + 2.0 * v).contains(&q.y)
        {
            return Some(B::Limestone);
        }
        if q.y >= floor - v && q.y < floor {
            return Some(B::Limestone);
        }
    }
    // Broad chamber, side galleries and physically sized mushroom clusters.
    let cx = q.x;
    let cz = q.z + 48.0;
    let chamber = (cx / 17.0).powi(2) + (cz / 19.0).powi(2);
    if s.kind == 0 && chamber < 1.12 && q.y >= -20.0 && q.y < 0.0 {
        let floor = -19.0;
        let roof = -6.0 - chamber * 3.0 + (cx * 0.25).sin() * 0.8;
        if chamber < 1.0 && q.y >= floor && q.y < roof {
            if q.y < floor + v {
                return Some(B::FungalSoil);
            }
            if let Some(b) = fungus(cfg, s.seed, Vec3::new(cx, q.y - floor, cz), None) {
                return Some(b);
            }
            // Ore seams are exposed in the chamber floor, not random treasure.
            if q.y < floor + 2.0 * v && cx.abs() > 11.0 {
                return Some(if cz > 3.0 { B::TinOre } else { B::CopperOre });
            }
            if (cx - 10.0).abs() < 1.0 && cz.abs() < 1.0 && q.y < floor + 3.0 {
                return Some(B::Crystal);
            }
            return Some(B::Air);
        }
        if chamber < 1.12 && q.y < roof + 1.0 {
            return Some(B::Limestone);
        }
    }
    // Generator 16 raises roofed, partitioned buildings; the watchtower keeps its round keep.
    if cfg.generator_version >= 16 && s.kind != 1 {
        let building = super::architecture::site_building_version(s.kind, s.seed, s.starter, cfg.generator_version)?;
        return match building.sample_sited(q, v, terrain_top - s.ground, cfg.generator_version) {
            None => None,
            Some(B::Air) => fixtures(s, cfg, q, v)
                .or_else(|| (cfg.generator_version >= super::ruins::TERRACE_VERSION)
                    .then(|| plant_frames(s.kind, q, v)).flatten())
                .or_else(|| (cfg.generator_version >= super::ruins::REBUILT_VERSION)
                    .then(|| ruin_plant(s.kind, q, v)).flatten())
                .or(Some(B::Air)),
            solid => solid,
        };
    }
    // Ruin grammars: workshop court, watchtower, and colonnaded reservoir.
    let variation = site_variation(s);
    let width = site_width(s, variation);
    let length = match s.kind {
        2 | 5 => 13.0,
        3 => 11.0,
        4 => 12.0,
        _ => 8.0 + variation,
    };
    let max_height = if s.kind == 1 { 22.0 } else { 14.0 };
    if q.x.abs() > width + 1.0 || q.z.abs() > length + 1.0 || q.y < -12.0 || q.y > max_height {
        return None;
    }
    let inside = q.x.abs() < width && q.z.abs() < length;
    if !inside {
        return None;
    }
    if q.y < 0.0 {
        return Some(B::WeatheredBricks);
    }
    if s.kind == 1 && q.y >= 10.0 {
        let r = q.x.hypot(q.z);
        if q.y < 15.5 {
            if r > 4.8 {
                return None;
            }
            if r >= 3.8 {
                let arch_window =
                    q.y > 11.5 && q.y < 14.0 && (q.x.abs() < 0.85 || q.z.abs() < 0.85);
                return Some(if arch_window { B::Air } else { B::StoneBricks });
            }
            if q.y < 10.0 + v && (q.x > -1.8 || q.z > -1.8) {
                return Some(B::Planks);
            }
            if q.x.abs() < 0.75 && q.z.abs() < 0.75 && (12.5..14.2).contains(&q.y) {
                return Some(B::Crystal);
            }
            return Some(B::Air);
        } else {
            let spire_t = ((q.y - 15.5) / 6.0).clamp(0.0, 1.0);
            let cone_r = 5.2 * (1.0 - spire_t * 0.92);
            if r <= cone_r {
                return Some(if spire_t > 0.82 {
                    B::Crystal
                } else {
                    B::Planks
                });
            }
            return None;
        }
    }
    let wall = q.x.abs() > width - 1.0 || q.z.abs() > length - 1.0;
    let doorway = q.x.abs() < 2.0 && q.z.abs() > length - 1.0;
    let corner_turret = s.kind == 3 && q.x.abs() > width - 2.8 && q.z.abs() > length - 2.8;
    let wall_height = if s.kind == 1 {
        10.0
    } else if corner_turret {
        let merlon = ((q.x.floor() as i32 + q.z.floor() as i32) & 1) == 0;
        if merlon { 8.6 } else { 7.6 }
    } else {
        3.0 + value_noise2(q.x * 0.19, q.z * 0.19, s.seed) * 3.0
    };
    if wall && !doorway && q.y < wall_height {
        let window = q.y > 3.0 && q.y < 5.0 && (q.x.abs() < 1.5 || q.z.abs() < 1.5);
        if !window {
            return Some(B::WeatheredBricks);
        }
    }
    if q.y < v {
        // Mine mouth is already carved above; other rooms have solid floor.
        if cfg.generator_version >= 12 && s.kind == 3 {
            // An inlaid process line leads from the operating plate to the three valves.
            return Some(
                if q.x.abs() < 0.6 || (q.z + 6.0).abs() < 0.4 && q.x.abs() < 6.0 {
                    B::Basalt
                } else {
                    B::StoneBricks
                },
            );
        }
        return Some(B::StoneBricks);
    }
    fixtures(s, cfg, q, v).or(Some(B::Air))
}

/// Overhead equipment leaves the ground routes, caches and valves clear.
fn plant_frames(kind: u32, q: Vec3, v: f32) -> Option<B> {
    let rail = v.max(0.85) * 0.65;
    let x = q.x.abs();
    let z = q.z.abs();
    match kind {
        // Metering frames stand above the pump pipes on either side of the sluice.
        2 if (z - 3.4).abs() < rail && x < 5.8 && (5.1..6.1).contains(&q.y) => {
            Some(if x < 1.3 { B::GlazedTiles } else { B::SteelPlate })
        }
        2 if (z - 3.4).abs() < rail && (x - 5.0).abs() < rail && (3.5..5.7).contains(&q.y) => {
            Some(B::RustedSteel)
        }
        // Flues turn from the furnace mouths toward the gantry over the court.
        3 if (x - 8.8).abs() < rail && (-5.4..0.5).contains(&q.z) && (7.0..8.0).contains(&q.y) => {
            Some(if (q.z + 2.0).abs() < rail { B::HazardPaint } else { B::RustedSteel })
        }
        3 if (x - 8.8).abs() < rail && z < rail && (5.0..7.5).contains(&q.y) => Some(B::SteelPlate),
        // Surviving greenhouse feeder ribs tie the old colonnade to the irrigation lines.
        4 if (z - 7.0).abs() < rail && (4.4..7.3).contains(&x) && (6.0..7.0).contains(&q.y) => {
            Some(if x < 5.4 { B::GlazedTiles } else { B::RustedSteel })
        }
        4 if (z - 7.0).abs() < rail && (x - 6.8).abs() < rail && (4.4..6.5).contains(&q.y) => Some(B::BronzeBlock),
        _ => None,
    }
}

fn ruin_plant(kind: u32, q: Vec3, v: f32) -> Option<B> {
    let pipe = v.max(0.85) * 0.65;
    match kind {
        2 if q.x.abs() < 7.7 && (q.z.abs() - 3.4).abs() < pipe && (q.y - 3.7).abs() < pipe => {
            Some(if (q.x.abs() - 5.0).abs() < v.max(0.55) { B::SteelPlate } else { B::RustedSteel })
        }
        2 if (q.x.abs() - 6.7).abs() < pipe && (q.z.abs() - 3.4).abs() < pipe
            && (1.0..3.7).contains(&q.y) => Some(B::RustedSteel),
        3 if q.x.abs() < 9.7 && q.z.abs() < pipe && (q.y - 5.3).abs() < pipe => {
            Some(if q.x.rem_euclid(4.0) < v.max(0.45) { B::HazardPaint } else { B::RustedSteel })
        }
        3 if (q.x.abs() - 8.8).abs() < pipe && (-5.0..0.4).contains(&q.z) && (q.y - 5.3).abs() < pipe => {
            Some(B::RustedSteel)
        }
        4 if (q.x.abs() - 6.8).abs() < pipe && q.z.abs() < 8.2 && (q.y - 4.5).abs() < pipe => {
            Some(if q.z.abs().rem_euclid(4.0) < v { B::GlazedTiles } else { B::RustedSteel })
        }
        4 if (q.x.abs() - 6.8).abs() < pipe && (q.z.abs() - 4.5).abs() < pipe
            && (3.0..4.5).contains(&q.y) => Some(B::BronzeBlock),
        _ => None,
    }
}

fn site_variation(s: Site) -> f32 {
    if s.starter {
        0.0
    } else {
        hash2(0, 0, s.seed.wrapping_add(67)) * 3.0
    }
}

fn site_width(s: Site, variation: f32) -> f32 {
    match s.kind {
        1 => 5.0,
        3 => 14.0 + variation,
        4 => 12.0,
        _ => 9.0 + variation,
    }
}

/// Puzzle fixtures and props inside open rooms; shared by every generator version.
fn fixtures(s: Site, cfg: &WorldSampleConfig, q: Vec3, v: f32) -> Option<B> {
    let width = site_width(s, site_variation(s));
    if cfg.generator_version >= 12 && s.kind == 3 {
        // An authored court: broad central dodge lanes, two waist-high cover islands,
        // and a screened maintenance alcove accessible around its southern end.
        if (q.x.abs() - 6.0).abs() < 0.65 && q.z.abs() < 2.0 && q.y < 1.5 {
            return Some(B::Basalt);
        }
        if (q.x + 10.7).abs() < 0.5 && q.z < -4.0 && q.z > -9.5 && q.y < 3.2 {
            return Some(B::WeatheredBricks);
        }
        if q.x < -10.7 && q.z < -4.0 && q.z > -9.5 && q.y > 3.0 && q.y < 3.5 {
            return Some(B::Planks);
        }
        // The entrance's surviving arch frames the heart rather than blocking it.
        if cfg.generator_version < 16
            && q.z > 8.5
            && q.z < 9.3
            && ((q.x.abs() - 3.0).abs() < 0.55 && q.y < 5.0
                || q.x.abs() < 3.5 && q.y > 4.4 && q.y < 5.0)
        {
            return Some(B::Basalt);
        }
    }
    if cfg.generator_version >= 4 {
        // Upper tower cache rewards climbing; all caches have exactly 1 m³ of loot.
        if s.kind == 1
            && (-1.0..0.0).contains(&q.x)
            && (3.0..4.0).contains(&q.z)
            && (6.0..7.0).contains(&q.y)
        {
            return Some(B::SurveyCache);
        }
        // Foundry: twin furnace stacks, overhead trusses, and ore bunkers.
        if s.kind == 3 {
            if (q.x.abs() - 9.0).abs() < 1.5 && (q.z + 5.0).abs() < 1.5 && q.y < 10.0 {
                let hollow = (q.x.abs() - 9.0).abs() < 0.65 && (q.z + 5.0).abs() < 0.65;
                return Some(if hollow { B::Air } else { B::Basalt });
            }
            if cfg.generator_version < 54 && q.y > 6.0
                && q.y < 7.0
                && q.z.abs() < 1.0
                && q.x < width - 2.0
                && (cfg.generator_version < 16 || q.x > 2.0 - width)
            {
                return Some(B::Log);
            }
            if q.x > 7.0 && q.x < 10.0 && q.z > 4.0 && q.z < 7.0 && q.y < 2.0 {
                return Some(B::IronOre);
            }
            if q.x < -7.0 && q.x > -10.0 && q.z > 4.0 && q.z < 7.0 && q.y < 2.0 {
                return Some(B::CoalOre);
            }
        }
        // Conservatory: broken colonnade and four walkable mushroom beds.
        if s.kind == 4 {
            if (q.x.abs() - 7.0).abs() < 0.6 && (q.z.abs() - 7.0).abs() < 0.6 && q.y < 7.0 {
                return Some(B::Limestone);
            }
            if q.x.abs() > 2.5 && q.x.abs() < 6.0 && q.z.abs() > 3.5 && q.z.abs() < 6.0 {
                if q.y < 1.0 {
                    return Some(B::FungalSoil);
                }
                if q.y < 2.0 {
                    return Some(if q.x * q.z > 0.0 {
                        B::RedCap
                    } else {
                        B::GlowCap
                    });
                }
            }
            if q.y > 7.0 && q.y < 8.0 && q.x > 0.0 && q.z.abs() > 6.5 && q.z.abs() < 7.5 {
                return Some(B::Planks);
            }
        }
        // Archive: branching aisles, eroded dividers and a crystal-lit rear alcove.
        if s.kind == 5 {
            if q.z.abs() < 9.0 && q.z.abs() > 1.5 && (q.x.abs() - 5.0).abs() < 0.6 && q.y < 3.5 {
                return Some(B::Planks);
            }
            if q.z < -10.0 && q.x.abs() > 5.5 && q.x.abs() < 6.5 && q.y < 3.0 {
                return Some(B::Crystal);
            }
        }
    }
    // Props retain a one-metre body at every voxel resolution. Cache contents
    // are volume-weighted by the expedition system.
    let point = |x: f32, z: f32| {
        q.x >= x && q.x < x + 1.0 && q.z >= z && q.z < z + 1.0 && q.y >= 1.0 && q.y < 2.0
    };
    if point(-4.0, 2.0) {
        return Some(B::Workbench);
    }
    if point(4.0, 2.0) {
        return Some(B::Smelter);
    }
    if point(-4.0, -3.0) {
        return Some(if cfg.generator_version < 4 {
            B::SalvageCrate
        } else {
            match s.kind {
                1 | 3 | 5 => B::SurveyCache,
                2 | 4 => B::CultivatorCache,
                _ => B::SalvageCrate,
            }
        });
    }
    // Raised pedestals make small-scale stations discoverable and reachable.
    if q.y < 1.0 && ((q.x + 4.0).abs() < 0.8 || (q.x - 4.0).abs() < 0.8) && (q.z - 2.0).abs() < 0.8
    {
        return Some(B::Planks);
    }
    if q.y < 1.0 && (q.x + 4.0).abs() < 0.8 && (q.z + 3.0).abs() < 0.8 {
        return Some(B::Planks);
    }
    if s.kind == 2 && q.y < 2.0 && q.x.abs() < 4.0 && q.z.abs() < 5.0 {
        return Some(B::Water);
    }
    if s.kind == 1 {
        let stair = ((q.z + 4.0) * 0.65).floor();
        if q.x > 2.0 && q.x < 4.0 && q.z > -4.0 && q.z < 4.0 && q.y > 0.0 && q.y < stair + v {
            return Some(B::StoneBricks);
        }
        if q.y > 5.0 && q.y < 5.0 + v && q.x < 2.0 && q.z > 2.0 {
            return Some(B::Planks);
        }
    }
    if s.starter && q.z > 4.0 && q.x.abs() < 1.0 && q.y < 3.0 {
        return Some(B::Air);
    }
    None
}

pub(super) fn fungus(
    cfg: &WorldSampleConfig,
    seed: u32,
    q: Vec3,
    ecosystem: Option<u32>,
) -> Option<B> {
    let gx = (q.x / 7.0).round() as i32;
    let gz = (q.z / 7.0).round() as i32;
    let n = if gx == 0 && gz == 1 {
        0.9
    } else {
        hash2(gx, gz, seed.wrapping_add(73))
    };
    let ax = gx as f32 * 7.0 + (n - 0.5) * 2.5;
    let az = gz as f32 * 7.0 + (hash2(gx, gz, seed.wrapping_add(79)) - 0.5) * 2.5;
    let radius = (q.x - ax).hypot(q.z - az);
    let variant = if cfg.generator_version >= 4 {
        ecosystem.unwrap_or((hash2(0, 0, seed.wrapping_add(167)) * 4.0) as u32 % 4)
    } else {
        0
    };
    let height = 2.0 + n * if variant == 2 { 5.0 } else { 3.5 };
    let cap = if variant == 1 && !(gx == 0 && gz == 1) {
        B::RedCap
    } else if n > 0.4 {
        B::GlowCap
    } else {
        B::RedCap
    };
    if radius < 0.5_f32.max(cfg.voxel_meters() * 0.7) && q.y < height && q.y >= cfg.voxel_meters() {
        return Some(B::MushroomStem);
    }
    if q.y >= height - 0.7
        && q.y < height + 1.1
        && radius < (2.4 + n) * (1.0 - ((q.y - height) / 1.6).powi(2)).max(0.0)
    {
        return Some(cap);
    }
    // Small luminous fruiting bodies make a continuous readable floor trail.
    if radius > 2.8 && radius < 3.5 && q.y < cfg.voxel_meters() * 2.0 && n > 0.45 {
        return Some(B::GlowCap);
    }
    None
}

// Holy-mountain mouth 18 m east of spawn, then four stacked landings & vaulted sanctums.
thread_local! {
    static SHAFT_GROUND: std::cell::Cell<Option<(WorldSampleConfig, f32)>> = const { std::cell::Cell::new(None) };
}

/// Surface height above the Holy Mountain shaft; sampled once per world configuration.
pub fn holy_ground(cfg: &WorldSampleConfig) -> f32 {
    shaft_ground(cfg)
}

fn shaft_ground(cfg: &WorldSampleConfig) -> f32 {
    if let Some((c, g)) = SHAFT_GROUND.get()
        && c == *cfg {
            return g;
        }
    let g = sample_column(cfg, cfg.blocks_for_height(18.0), 0).height as f32 * cfg.voxel_meters();
    SHAFT_GROUND.set(Some((*cfg, g)));
    g
}

/// Generator 16 Holy Mountain: the same path, spiral shaft and mouth, but each landing
/// opens into a roofed room complex instead of a single box.
fn noita_v16(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    let (dx, dz) = (m.x - 18.0, m.z);
    let core = dx.abs() <= 14.0 && dz.abs() <= 10.0;
    let wide = m.x >= -18.0 && m.x <= 94.0 && dz.abs() <= 36.0;
    if !core && !wide {
        return None;
    }
    let v = cfg.voxel_meters();
    let ground = shaft_ground(cfg);
    let below = ground - m.y;
    if core && dz.abs() < 1.15 && (8.0..16.5).contains(&m.x) {
        let path = sample_column(cfg, cfg.blocks_for_height(m.x), cfg.blocks_for_height(m.z)).height
            as f32
            * v;
        if (m.y - path).abs() < v {
            return Some(B::Cobblestone);
        }
    }
    if !(-5.0..=96.0).contains(&below) {
        return None;
    }
    if dx.abs() < 1.6 && dz.abs() < 1.6 {
        if (0.4..90.0).contains(&below) {
            let turn = below * 0.85;
            let (sx, sz) = (turn.cos() * 1.05, turn.sin() * 1.05);
            if (dx - sx).hypot(dz - sz) < 0.55 && below.fract() < v.max(0.4) {
                return Some(B::StoneBricks);
            }
            if below.rem_euclid(8.0) < v && (dx - sx).hypot(dz - sz) < 0.7 {
                return Some(B::GlowCap);
            }
        }
        if m.y < ground + v {
            return Some(B::Air);
        }
    }
    if (20.0..92.0).contains(&below)
        && let Some(b) = super::dungeon::vault(cfg, m, ground) {
            return Some(b);
        }
    let mouth = (-4.5..1.0).contains(&below) && dx.abs() < 2.4 && dz.abs() < 2.4;
    let opening = dx.abs() < 1.5 && dz.abs() < 1.5 && (-3.6..0.2).contains(&below);
    if mouth && !opening {
        return Some(B::StoneBricks);
    }
    None
}

fn noita_descent(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if cfg.generator_version < 13 {
        return None;
    }
    if cfg.generator_version >= 16 {
        return noita_v16(cfg, m);
    }
    let dx = m.x - 18.0;
    let dz = m.z;
    if dx.abs() > 14.0 || dz.abs() > 10.0 {
        return None;
    }
    let v = cfg.voxel_meters();
    let ground = sample_column(cfg, cfg.blocks_for_height(18.0), 0).height as f32 * v;
    let below = ground - m.y;
    if dz.abs() < 1.15 && (8.0..16.5).contains(&m.x) {
        let path = sample_column(cfg, cfg.blocks_for_height(m.x), cfg.blocks_for_height(m.z)).height
            as f32
            * v;
        if (m.y - path).abs() < v {
            return Some(B::Cobblestone);
        }
    }
    if !(-5.0..=96.0).contains(&below) {
        return None;
    }
    let shaft = dx.abs() < 1.6 && dz.abs() < 1.6;
    if shaft && (0.4..90.0).contains(&below) {
        let turn = below * 0.85;
        let sx = turn.cos() * 1.05;
        let sz = turn.sin() * 1.05;
        if (dx - sx).hypot(dz - sz) < 0.55 && below.fract() < v.max(0.4) {
            return Some(B::StoneBricks);
        }
        if below.rem_euclid(8.0) < v && (dx - sx).hypot(dz - sz) < 0.7 {
            return Some(B::GlowCap);
        }
    }
    let band = (below / 24.0).floor();
    let band_u = band as u32;
    let rel_depth = below.rem_euclid(24.0);
    // Main eastern vault chamber + western resource/sanctum annex at each 24m depth tier
    let east_vault = (1.0..=3.0).contains(&band)
        && (1.6..11.5).contains(&dx)
        && dz.abs() < 5.5
        && rel_depth < 6.5;
    let west_alcove = (1.0..=3.0).contains(&band)
        && (-7.5..-1.6).contains(&dx)
        && dz.abs() < 3.5
        && rel_depth < 5.2;
    let room = east_vault || west_alcove;
    let landing = below > 8.0 && rel_depth < 1.2 && dx.abs() < 11.5 && dz.abs() < 5.5;
    if shaft || room {
        if landing && m.y < ground - 1.0 {
            // Trim the Holy Mountain landing perimeter with StoneBricks and central floor by layer
            if dx.abs() > 9.8 || dz.abs() > 4.4 {
                return Some(B::StoneBricks);
            }
            return Some(match band_u {
                0 => B::Gravel,
                1 => B::FungalSoil,
                2 => B::Ice,
                _ => B::Basalt,
            });
        }
        // Furnishings inside each Holy Mountain landing (height above landing floor = 6.5 - rel_depth):
        if east_vault && (4.2..5.3).contains(&rel_depth) {
            // Vaulted corner pillars & ceiling lanterns
            if ((dx - 3.5).abs() < 0.6 || (dx - 9.5).abs() < 0.6) && (dz.abs() - 4.0).abs() < 0.6 {
                return Some(B::StoneBricks);
            }
            // Sanctum workstations & reward caches on the eastern dais
            if (7.5..8.5).contains(&dx) && (-0.6..0.6).contains(&dz) {
                return Some(match band_u {
                    1 => B::CultivatorCache,
                    2 => B::SurveyCache,
                    _ => B::Crystal,
                });
            }
            if (5.5..6.5).contains(&dx) && (2.2..3.2).contains(&dz) {
                return Some(match band_u {
                    1 => B::Workbench,
                    2 => B::Smelter,
                    _ => B::SalvageCrate,
                });
            }
        }
        if east_vault && (1.2..2.2).contains(&rel_depth) && (dx - 6.5).abs() < 0.8 && dz.abs() < 0.8
        {
            return Some(if band_u == 1 { B::GlowCap } else { B::Crystal });
        }
        // Western mineral seam & crystal geode alcove
        if west_alcove && (3.6..5.2).contains(&rel_depth) && dx < -5.8 && dz.abs() < 2.2 {
            return Some(match band_u {
                1 => B::CopperOre,
                2 => B::IronOre,
                _ => B::Crystal,
            });
        }
        if m.y < ground + v {
            return Some(B::Air);
        }
    }
    // StoneBrick shell around the Holy Mountain vaults so caverns/water don't breach them cleanly
    if (1.0..=3.0).contains(&band)
        && (1.5..12.3).contains(&dx)
        && dz.abs() < 6.3
        && rel_depth < 7.3
        && !room
        && !shaft
    {
        return Some(B::StoneBricks);
    }
    let mouth = (-4.5..1.0).contains(&below) && dx.abs() < 2.4 && dz.abs() < 2.4;
    let opening = dx.abs() < 1.5 && dz.abs() < 1.5 && (-3.6..0.2).contains(&below);
    if mouth && !opening {
        return Some(B::StoneBricks);
    }
    None
}

/// `height` is the natural terrain column height, so foundations only fill down to it.
pub fn block(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<B> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if let Some(block) = crate::habitats_region::sample(cfg, m) {
        return Some(block);
    }
    if crate::geo::integrated(cfg)
        && let Some(block) = crate::cistern::sample(cfg, m) {
        return Some(block);
    }
    let terrain_top = (height + 1) as f32 * cfg.voxel_meters();
    if let Some(built) = super::ruins::block(cfg, p, m, terrain_top) {
        return Some(built);
    }
    if let Some(carved) = noita_descent(cfg, m) {
        return Some(carved);
    }
    if cfg.generator_version >= 14
        && let Some(built) = super::valley::architecture(cfg, m) {
            return Some(built);
        }
    let rx = (m.x / region_span(cfg)).round() as i32;
    let rz = (m.z / region_span(cfg)).round() as i32;
    if cfg.generator_version >= 9 && rz != 0 && m.x.abs() < 32.0 && (-130.0..0.0).contains(&m.z)
        && let Some(s) = site(cfg, 0, 0)
            && let Some(b) = sample_site(s, cfg, m, terrain_top) {
                return Some(b);
            }
    // The valley hamlet and tower own their ground. Older box sites stay outside it.
    if cfg.generator_version >= 14 && super::valley::reserves(cfg, m.x, m.z) {
        return None;
    }
    // Fast reject before computing region site ground height when outside site bounds.
    let center_x = if rx == 0 && rz == 0 {
        0.0
    } else {
        rx as f32 * region_span(cfg)
    };
    let center_z = if rx == 0 && rz == 0 {
        -42.0
    } else {
        rz as f32 * region_span(cfg)
    };
    // Generator 32: the boss complex behind a boss ruin.
    if cfg.generator_version >= boss_access::VERSION
        && (m.x - center_x).abs() <= if cfg.generator_version >= 39 { 90.0 } else { boss_access::REACH }
        && (m.z - center_z).abs() <= if cfg.generator_version >= 39 { 90.0 } else { boss_access::REACH }
        && let Some(b) = boss_access::site_block(cfg, rx, rz, m)
    {
        return Some(b);
    }
    if cfg.generator_version >= 39 {
        // A rotated mine or a castle apron can cross the old fixed 48 m X
        // cutoff. Query candidate owners, then let their actual geometry decide.
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (ix, iz) = (rx + dx, rz + dz);
                let cx = ix as f32 * region_span(cfg);
                let cz = if ix == 0 && iz == 0 { -42.0 } else { iz as f32 * region_span(cfg) };
                if (m.x - cx).abs() > 90.0 || (m.z - cz).abs() > 90.0 { continue; }
                if let Some(s) = site(cfg, ix, iz)
                    && let Some(b) = sample_site(s, cfg, m, terrain_top) {
                    return Some(b);
                }
            }
        }
    } else if (m.x - center_x).abs() <= 48.0 && (m.z - center_z).abs() <= 84.0
        && let Some(s) = site(cfg, rx, rz)
            && let Some(b) = sample_site(s, cfg, m, terrain_top) {
                return Some(b);
            }
    if cfg.generator_version >= 16
        && let Some(b) = homestead_block(cfg, m, terrain_top) {
            return Some(b);
        }
    // Trails and roadside places are sampled after authored sites so they can
    // connect to a ruin without cutting through its rooms.
    super::surface::block(cfg, p, height, terrain_top)
}

/// Surface structures have placement priority over random dungeon collars.
/// Deliberately does not query dungeons: placement dependencies stay acyclic.
pub(crate) fn building_reserved(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    if super::basin::solved_face(cfg) { return false; }
    if super::valley::reserves(cfg, x, z) || super::amberroot::reserves(cfg, x, z) { return true; }
    if super::ruins::reserves(cfg, x, z, 10.0) { return true; }
    let span = region_span(cfg);
    let (rx, rz) = ((x / span).round() as i32, (z / span).round() as i32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (ix, iz) = (rx + dx, rz + dz);
            let cx = ix as f32 * span;
            let cz = if ix == 0 && iz == 0 { -42.0 } else { iz as f32 * span };
            if (x - cx).abs() > 90.0 || (z - cz).abs() > 90.0 { continue; }
            if let Some((s, extent)) = footprints::site(cfg, ix, iz) {
                let q = local(s, Vec3::new(x, s.ground, z));
                if let Some((x0, z0, x1, z1)) = extent {
                    if q.x >= x0 - 5.0 && q.x <= x1 + 5.0 && q.z >= z0 - 5.0 && q.z <= z1 + 5.0 { return true; }
                } else if q.x.abs() < 25.0 && q.z.abs() < 25.0 { return true; }
                // Reserve the workshop's mine as well as its roofed hall.
                if s.kind == 0 && q.x.abs() < 22.0 && (-72.0..0.0).contains(&q.z) { return true; }
                if matches!(s.kind, 1 | 3 | 5) && q.x.abs() < 28.0 && (12.0..67.0).contains(&q.z) { return true; }
            }
        }
    }
    let (ix, iz) = ((x / HOME_CELL).round() as i32, (z / HOME_CELL).round() as i32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            if (x - (ix + dx) as f32 * HOME_CELL).abs() > 64.0
                || (z - (iz + dz) as f32 * HOME_CELL).abs() > 64.0 { continue; }
            // The terrain-free bound decides most trees before any home is planned.
            if let Some((hx, hz, reach)) = footprints::home_bound(cfg, ix + dx, iz + dz)
                && (x - hx).hypot(z - hz) < reach + 5.0
                && footprints::home(cfg, ix + dx, iz + dz) { return true; }
        }
    }
    false
}

/// Keep complete tree roots/crowns off occupied campuses, without clearing
/// every riverbank just because the valley owns its terrain grading.
pub(crate) fn tree_reserved(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    if super::valley::reserves(cfg,x,z) && !super::valley::built_up(cfg,x,z) {
        return false;
    }
    building_reserved(cfg,x,z)
}

/// Generator 17 chamber dressing. Variant 2 (the broken road) is left alone.
/// Returns `Some` when this voxel is claimed by the room's own furnishing.
fn dress_chamber(variant: u32, dx: f32, dz: f32, dy: f32, roof: f32, v: f32) -> Option<B> {
    let r = dx.hypot(dz);
    match variant {
        0 => {
            // A lit clearing in a larger grove, with crystals in the canopy.
            if r < 5.5 && dy < v {
                return Some(B::FungalSoil);
            }
            // Terraria life crystal on a Noita pedestal, in the lit clearing.
            if r < 1.15 && dy >= v && dy < v * 2.0 {
                return Some(B::StoneBricks);
            }
            if r < 0.9 && dy >= v * 2.0 && dy < v * 3.0 {
                return Some(B::Crystal);
            }
            if r < 5.5 && dy >= v {
                return Some(B::Air);
            }
            if dy > roof - 2.2 && dy < roof - 0.3 && (dx * 0.37).sin() * (dz * 0.41).cos() > 0.72 {
                return Some(B::Crystal);
            }
            None
        }
        1 => {
            // Tall limestone nave. Hard surfaces, few growths, so the room reads as a hall.
            if dy < v {
                return Some(B::Limestone);
            }
            let pillar = |ox: f32, oz: f32| (dx - ox).hypot(dz - oz) < 0.95;
            let pillars = pillar(9.0, 10.0)
                || pillar(-9.0, 10.0)
                || pillar(9.0, -10.0)
                || pillar(-9.0, -10.0)
                || pillar(0.0, 16.0)
                || pillar(0.0, -16.0);
            if pillars && dy < roof - 2.2 {
                if dy > roof - 3.3 {
                    return Some(B::Crystal);
                }
                return Some(B::Limestone);
            }
            // One offering stand in the nave, with a short light shaft above it.
            if r < 1.15 && dy >= v && dy < v * 2.0 {
                return Some(B::StoneBricks);
            }
            if r < 0.85 && dy >= v * 2.0 && dy < v * 3.0 {
                return Some(B::Crystal);
            }
            if r < 0.75 && dy > roof - 2.8 && dy < roof - 0.4 {
                return Some(B::Crystal);
            }
            Some(B::Air)
        }
        3 => {
            // A dry binding circle: enchantment ring, four pieces of equipment,
            // braziers, and a font one voxel deep.
            if dy < v {
                let ring = (r - 6.4).abs() < 0.75;
                let cross = (dx.abs() < 0.35 || dz.abs() < 0.35) && r < 6.2;
                if ring || cross {
                    return Some(B::Crystal);
                }
                if r < 2.15 {
                    return Some(B::StoneBricks);
                }
                return Some(B::Limestone);
            }
            if r < 1.05 && dy >= v * 2.0 && dy < v * 3.0 {
                return Some(B::StoneBricks);
            }
            if r < 1.05 && dy >= v * 3.0 && dy < v * 4.0 {
                return Some(B::MithrilBlock);
            }
            if r < 1.15 && dy >= v && dy < v * 2.0 {
                return Some(B::Water);
            }
            if (1.15..2.15).contains(&r) && dy >= v && dy < v * 2.0 {
                return Some(B::StoneBricks);
            }
            let plinths = [
                (7.6, 0.0, B::CopperBlock),
                (-7.6, 0.0, B::SilverBlock),
                (0.0, 7.6, B::GoldBlock),
                (0.0, -7.6, B::PlatinumBlock),
            ];
            for (px, pz, metal) in plinths {
                let pr = (dx - px).hypot(dz - pz);
                if pr < 1.15 && dy >= v && dy < v * 2.0 {
                    return Some(B::StoneBricks);
                }
                if pr < 0.85 && dy >= v * 2.0 && dy < v * 3.0 {
                    return Some(metal);
                }
            }
            for (sx, sz) in [(5.2, 5.2), (5.2, -5.2), (-5.2, 5.2), (-5.2, -5.2)] {
                let pr = (dx - sx).hypot(dz - sz);
                if pr < 0.9 && dy >= v && dy < v * 2.0 {
                    return Some(B::Cobblestone);
                }
                if pr < 0.75 && dy >= v * 2.0 && dy < v * 3.0 {
                    return Some(B::Torch);
                }
            }
            if dx.abs() < 0.55 && (dz - 9.6).abs() < 0.55 && dy >= v && dy < v * 2.0 {
                return Some(B::SurveyCache);
            }
            if dy > roof - 1.8 && (dx * 0.51).sin() > 0.55 && (dz * 0.47).cos() > 0.15 {
                return Some(B::Crystal);
            }
            Some(B::Air)
        }
        _ => None,
    }
}

/// Additional dry underground ecosystems between sites, independent of sea level.
pub fn cavern(cfg: &WorldSampleConfig, p: IVec3, height: i32) -> Option<B> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if m.x.hypot(m.z) < 100.0 {
        return None;
    }
    // Caverns and dungeon bands belong below the natural surface. Skipping
    // above-ground voxels here avoids the per-voxel cave/dungeon solve for the
    // sky portion of every surface chunk.
    if p.y > height {
        // Generator 32 lets the gatehouse over a complex's shaft stand above
        // ground; older worlds never sampled it there.
        if cfg.generator_version >= super::dungeon::v2::VERSION
            && p.y <= height + cfg.blocks_for_span(14.0)
            && !super::valley::reserves(cfg, m.x, m.z)
        {
            return super::dungeon::block(cfg, m, height);
        }
        return None;
    }
    if super::undercrown::reserved(cfg, m) {
        return None;
    }
    if cfg.generator_version >= 14 && super::valley::reserves(cfg, m.x, m.z) {
        return None;
    }
    if cfg.generator_version >= 16 {
        if let Some(b) = super::dungeon::block(cfg, m, height) {
            return Some(b);
        }
        if super::dungeon::reserved(cfg, m) {
            return None;
        }
    }
    if cfg.generator_version >= 20
        && let Some(b) = super::cistern::sample(cfg, m) {
            return Some(b);
        }
    if cfg.generator_version >= boss_access::VERSION
        && let Some(b) = boss_access::cavern_block(cfg, m)
    {
        return Some(b);
    }
    if let Some(block) = super::karst::block(cfg, p, height) { return Some(block); }
    let depth = height as f32 * cfg.voxel_meters() - m.y;
    if !(9.0..=70.0).contains(&depth) {
        return None;
    }
    if let Some(floor) = gallery_floor(cfg, p, m) {
        let dy = m.y - floor;
        if dy >= 0.0
            && dy
                < if cfg.generator_version >= 17 {
                    6.4
                } else {
                    4.2
                }
        {
            return Some(if dy < cfg.voxel_meters() {
                B::FungalSoil
            } else {
                B::Air
            });
        }
    }
    let gx = (m.x / 72.0).round() as i32;
    let gz = (m.z / 72.0).round() as i32;
    let n = hash2(gx, gz, cfg.seed.wrapping_add(1101));
    if n < chamber_rarity(cfg.generator_version) {
        return None;
    }
    let ax = gx as f32 * 72.0;
    let az = gz as f32 * 72.0;
    let ground = sample_column(cfg, cfg.blocks_for_height(ax), cfg.blocks_for_height(az)).height
        as f32
        * cfg.voxel_meters();
    let floor = ((ground - 18.0 - n * 25.0) / cfg.voxel_meters()).floor() * cfg.voxel_meters();
    let dx = m.x - ax;
    let dz = m.z - az;
    let dy = m.y - floor;
    let variant = if cfg.generator_version >= 4 {
        (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4
    } else {
        0
    };
    let wide = cfg.generator_version >= 17 && variant != 2;
    let radius_x = if wide {
        26.0 + n * 14.0
    } else {
        18.0 + n * 9.0
    };
    let radius_z = if variant == 2 {
        29.0
    } else if wide && variant == 1 {
        36.0
    } else if wide {
        30.0
    } else {
        23.0
    };
    let ellipse = (dx / radius_x).powi(2) + (dz / radius_z).powi(2);
    let dy_limit = if cfg.generator_version >= 17 && variant == 1 {
        26.0
    } else if wide {
        19.0
    } else {
        14.0
    };
    if ellipse > 1.1 || dy < -1.5 || dy > dy_limit {
        return None;
    }
    let roof_base = if variant == 2 {
        14.0
    } else if cfg.generator_version >= 17 && variant == 1 {
        22.0
    } else if wide {
        16.0
    } else {
        11.0
    };
    let roof = roof_base - ellipse * 5.0 + (dx * 0.21).sin();
    if variant == 2 && cfg.generator_version >= super::prism_sanctum::FIRST_VERSION
        && let Some(block) = super::prism_sanctum::block(Vec3::new(dx, dy, dz), cfg.voxel_meters()) {
            return Some(block);
        }
    // The broken road is an optional construction puzzle, with a walkable floor detour.
    // Version-gated so existing worlds retain their terrain and discoveries.
    if cfg.generator_version >= 12
        && variant == 2
        && ellipse < 0.85
        && (-15.0..17.0).contains(&dx)
        && (6.0..12.0).contains(&dz)
        && (0.0..7.0).contains(&dy)
    {
        if dy < cfg.voxel_meters() {
            return Some(B::Limestone);
        }
        // Broad steps lead onto the western bridge; a three-metre gap invites building.
        let stair =
            (((dx + 15.0) * 0.5 / cfg.voxel_meters()).floor() * cfg.voxel_meters()).clamp(0.0, 3.0);
        if dx < -7.0 && (8.0..11.0).contains(&dz) && dy < stair + 1.0 {
            return Some(B::StoneBricks);
        }
        if (8.0..11.0).contains(&dz)
            && (-7.0..14.0).contains(&dx)
            && !(-1.5..=1.5).contains(&dx)
            && (3.0..4.0).contains(&dy)
        {
            return Some(B::StoneBricks);
        }
        // Survey cache on a luminous landing, reachable by bridge, climbing, or excavation.
        if (13.0..16.0).contains(&dx) && (7.0..12.0).contains(&dz) && dy < 4.0 {
            return Some(B::Limestone);
        }
        if (14.0..15.0).contains(&dx) && (9.0..10.0).contains(&dz) && (4.0..5.0).contains(&dy) {
            return Some(B::SurveyCache);
        }
        if (dx + 7.0).abs() < 0.5 && (6.0..7.0).contains(&dz) && dy < 2.0 {
            return Some(B::StoneBricks);
        }
        if (dx.abs() - 5.0).abs() < 0.5 && (11.0..12.0).contains(&dz) && (0.5..1.5).contains(&dy) {
            return Some(B::Crystal);
        }
        return Some(B::Air);
    }
    if ellipse < 1.0 && dy > 0.0 && dy < roof {
        if cfg.generator_version >= 17
            && let Some(dressed) = dress_chamber(variant, dx, dz, dy, roof, cfg.voxel_meters()) {
                return Some(dressed);
            }
        if dy < cfg.voxel_meters() {
            return Some(B::FungalSoil);
        }
        if cfg.generator_version >= 4 {
            // A perimeter niche leaves the main gallery clear and rewards detours.
            if (8.0..9.0).contains(&dx) && (5.0..6.0).contains(&dz) && (1.0..2.0).contains(&dy) {
                return Some(if variant == 2 {
                    B::SurveyCache
                } else {
                    B::CultivatorCache
                });
            }
            if (8.0..9.0).contains(&dx) && (5.0..6.0).contains(&dz) && dy < 1.0 {
                return Some(B::Limestone);
            }
            // Rich seams and geodes become visible along the chamber wall.
            if ellipse > 0.77 && ellipse < 0.95 && dy < 2.5 && dz.abs() < 3.0 {
                return Some(match variant {
                    1 => B::CoalOre,
                    2 => B::Crystal,
                    3 => B::TinOre,
                    _ => B::IronOre,
                });
            }
            if variant == 2 && (dx.abs() - 10.0).abs() < 1.0 && (dz.abs() - 11.0).abs() < 1.0 {
                return Some(if dy > roof - 2.0 || dy < 2.0 {
                    B::Crystal
                } else {
                    B::Limestone
                });
            }
            // Shallow pools split by a dry causeway, never a mandatory swim.
            if cfg.generator_version < 17
                && variant == 3
                && dx.abs() > 3.0
                && dx.abs() < 8.0
                && dz.abs() < 9.0
                && dy < 1.6
            {
                return Some(B::Water);
            }
            if variant == 1 && ellipse > 0.90 && dy < 4.0 {
                return Some(B::Basalt);
            }
        }
        return Some(
            fungus(
                cfg,
                if cfg.generator_version >= 4 {
                    cfg.seed
                        .wrapping_add((gx as u32).wrapping_mul(73856093))
                        .wrapping_add((gz as u32).wrapping_mul(19349663))
                } else {
                    cfg.seed.wrapping_add((gx ^ gz) as u32)
                },
                Vec3::new(dx, dy, dz),
                Some(variant),
            )
            .unwrap_or(B::Air),
        );
    }
    if dy < roof + 1.0 {
        return Some(B::Limestone);
    }
    None
}

pub fn location(cfg: &WorldSampleConfig, p: Vec3) -> Option<&'static str> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let m = p * cfg.voxel_meters();
    if let Some(name) = super::undercrown::location(cfg, m) {
        return Some(name);
    }
    if let Some(name) = super::ruins::location(cfg, m) {
        return Some(name);
    }
    if let Some(name) = super::surface::location(cfg, m) {
        return Some(name);
    }
    if let Some(name) = super::amberroot::location(cfg, m) {
        return Some(name);
    }
    if cfg.generator_version >= 16 {
        let ground = shaft_ground(cfg);
        if let Some(name) = super::dungeon::vault_name(cfg, m, ground) {
            return Some(name);
        }
        if (m.x - 18.0).abs() < 3.0 && m.z.abs() < 3.0 && (1.0..90.0).contains(&(ground - m.y)) {
            return Some("Holy Mountain Descent Shaft");
        }
        if let Some(name) = super::dungeon::name_at(cfg, m) {
            return Some(name);
        }
    }
    if cfg.generator_version >= 13 && (m.x - 18.0).abs() < 13.0 && m.z.abs() < 8.0 {
        let ground =
            sample_column(cfg, cfg.blocks_for_height(18.0), 0).height as f32 * cfg.voxel_meters();
        let below = ground - m.y;
        if (18.0..32.0).contains(&below) {
            return Some("Sanctum Landing I — Fungal Vault");
        } else if (42.0..56.0).contains(&below) {
            return Some("Sanctum Landing II — Rime Geode Vault");
        } else if (66.0..84.0).contains(&below) {
            return Some("Sanctum Landing III — Basalt Deep");
        } else if (1.0..90.0).contains(&below) && (m.x - 18.0).abs() < 3.0 && m.z.abs() < 3.0 {
            return Some("Holy Mountain Descent Shaft");
        }
    }
    if cfg.generator_version >= 4 && m.x.hypot(m.z) > 100.0 {
        let gx = (m.x / 72.0).round() as i32;
        let gz = (m.z / 72.0).round() as i32;
        if let Some(floor) = cavern_floor(cfg, gx, gz) {
            let (reach_y, reach_x, reach_z) = if cfg.generator_version >= 17 {
                (24.0, 42.0, 38.0)
            } else {
                (14.0, 28.0, 29.0)
            };
            if m.y > floor
                && m.y < floor + reach_y
                && (m.x - gx as f32 * 72.0).abs() < reach_x
                && (m.z - gz as f32 * 72.0).abs() < reach_z
            {
                return Some(
                    match (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4 {
                        0 => "Lanternroot grove",
                        1 if cfg.generator_version >= 17 => "Echoing limestone nave",
                        1 => "Embercap hollows",
                        2 if cfg.generator_version >= 12 => "Prismatic cathedral - the broken road",
                        2 => "Prismatic cathedral",
                        3 if cfg.generator_version >= 17 => "Circle of binding",
                        _ => "Drowned mycelium gardens",
                    },
                );
            }
        }
    }
    let s = site(
        cfg,
        (m.x / region_span(cfg)).round() as i32,
        (m.z / region_span(cfg)).round() as i32,
    )?;
    let q = local(s, m);
    if s.kind == 0 && q.x.abs() < 20.0 && (q.z + 48.0).abs() < 23.0 && q.y < -5.0 {
        Some("Lanternroot caverns")
    } else if cfg.generator_version >= 16
        && !s.starter
        && q.x.abs() < 8.0
        && (q.z + 6.0).abs() < 7.0
        && q.y < -2.0
    {
        Some(match ((-q.y) / 22.0).floor() as u32 {
            1 => "Ruin descent — fungal layer",
            2 => "Ruin descent — rime layer",
            _ => "Ruin descent — basalt layer",
        })
    } else if q.x.abs() < 18.0 && q.z.abs() < 16.0 {
        Some(match s.kind {
            0 => "Ruined artificer workshop",
            1 if cfg.generator_version >= 26 => "The ruined toll crossing",
            1 => "Overgrown watchtower",
            2 => "Sunken waterworks",
            3 => "Abandoned foundry",
            4 => "Mycologist conservatory",
            _ => "Resonant archive",
        })
    } else {
        None
    }
}

/// Encounter anchors share the terrain's deterministic site layout. Coordinates
/// are metres; the third key component separates ruins from fungal chambers.
fn encounter_anchor(cfg: &WorldSampleConfig, s: Site, x: i32, z: i32) -> Vec3 {
    if s.starter {
        Vec3::new(11.0, s.ground - 18.0, -90.0)
    } else if let Some(shrine) = boss_access::site_shrine(cfg, x, z) {
        shrine.relic_point()
    } else {
        Vec3::new(s.x, s.ground + 1.0, s.z + 3.0)
    }
}

/// Bounded survey invoked from the spell library. Save state decides which
/// encounters have already paid their one-time spell reward.
pub fn next_spell_site(cfg: &WorldSampleConfig, origin: Vec3, cleared: impl Fn(IVec3) -> bool) -> Option<Vec3> {
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat { return None; }
    let rx = (origin.x / region_span(cfg)).round() as i32;
    let rz = (origin.z / region_span(cfg)).round() as i32;
    let mut best: Option<Vec3> = None;
    for x in rx - 3..=rx + 3 {
        for z in rz - 3..=rz + 3 {
            if cleared(IVec3::new(x, z, 0)) { continue; }
            if let Some(s) = site(cfg, x, z) {
                let p = encounter_anchor(cfg, s, x, z);
                if best.is_none_or(|q| p.distance_squared(origin) < q.distance_squared(origin)) {
                    best = Some(p);
                }
            }
        }
    }
    best
}

pub fn encounters(cfg: &WorldSampleConfig, player: Vec3) -> Vec<(IVec3, Vec3, u32)> {
    if cfg.world_type == WorldType::Hall { return crate::hall::encounters(player); }
    // Pocket realms have no surface sites; their column top is a roof, not ground.
    if cfg.world_type.realm().is_some() { return Vec::new(); }
    if cfg.world_type == WorldType::Nether && cfg.generator_version >= crate::SEEDED_REALMS {
        return crate::nether::encounters(cfg, player);
    }
    if cfg.world_type == WorldType::Manyfold && cfg.generator_version >= crate::SEEDED_REALMS {
        return crate::manyfold::encounters(cfg, player);
    }
    let mut out = Vec::new();
    if cfg.generator_version < 3 || cfg.world_type == WorldType::Superflat {
        return out;
    }
    let rx = (player.x / region_span(cfg)).round() as i32;
    let rz = (player.z / region_span(cfg)).round() as i32;
    for x in rx - 1..=rx + 1 {
        for z in rz - 1..=rz + 1 {
            if let Some(s) = site(cfg, x, z) {
                let point = encounter_anchor(cfg, s, x, z);
                if (point - player).length() < 68.0 {
                    // Site identity picks the lead guardian so each ruin reads as a different fight.
                    let lead = if s.starter {
                        0
                    } else {
                        match s.kind % 6 {
                            0 => 0,
                            1 => 5,
                            2 => 2,
                            3 => 3,
                            4 => 1,
                            _ => 4,
                        }
                    };
                    out.push((IVec3::new(x, z, 0), point, lead));
                }
                if !s.starter && cfg.generator_version >= 16 && !(s.kind==1 && cfg.generator_version>=26) {
                    for layer in 1..=3 {
                        let point =
                            Vec3::new(s.x + 3.0, s.ground - layer as f32 * 22.0 + 1.2, s.z - 6.0);
                        if (point - player).length() < 36.0 {
                            let kind = (s.kind + layer) % 6;
                            out.push((
                                IVec3::new(x, z + layer as i32 * 1000, 0),
                                point,
                                kind | (layer << 8),
                            ));
                        }
                    }
                }
                // A cairn beside the ruin, and another on the walk toward the next region.
                // Both are real reliquaries: guardians, a claim, and the pickup fog.
                if !s.starter && cfg.generator_version >= 4 {
                    let near = hash2(x, z, s.seed.wrapping_add(4401));
                    if near > 0.28 {
                        let a = near * std::f32::consts::TAU;
                        let cairn =
                            Vec3::new(s.x + a.cos() * 16.0, s.ground + 1.0, s.z + a.sin() * 16.0);
                        if (cairn - player).length() < 68.0 {
                            out.push((IVec3::new(x, z, 2), cairn, 1));
                        }
                    }
                    let far = hash2(x, z, s.seed.wrapping_add(4407));
                    if far > 0.45 {
                        let a = far * std::f32::consts::TAU;
                        let cache =
                            Vec3::new(s.x + a.cos() * 42.0, s.ground + 1.0, s.z + a.sin() * 42.0);
                        if (cache - player).length() < 68.0 {
                            out.push((IVec3::new(x, z, 3), cache, 4));
                        }
                    }
                }
            }
        }
    }
    for (id, point, kind) in super::ruins::sentries(cfg, player, 68.0) {
        out.push((id, point, kind));
    }
    let gx = (player.x / 72.0).round() as i32;
    let gz = (player.z / 72.0).round() as i32;
    for x in gx - 1..=gx + 1 {
        for z in gz - 1..=gz + 1 {
            let (mx, mz) = (x as f32 * 72.0, z as f32 * 72.0);
            // Horizontal distance is a lower bound on distance to any floor.
            if mx.hypot(mz) < 100.0
                || glam::Vec2::new(mx - player.x, mz - player.z).length_squared() >= 46.0 * 46.0
            {
                continue;
            }
            if let Some(floor) = cavern_floor(cfg, x, z) {
                let point = Vec3::new(mx, floor + 1.0, mz);
                if (point - player).length() < 46.0 {
                    let lead = (hash2(x, z, cfg.seed.wrapping_add(1187)) * 6.0) as u32 % 6;
                    out.push((IVec3::new(x, z, 1), point, lead | (2 << 8)));
                }
            }
        }
    }
    if cfg.generator_version >= 13 {
        let ground =
            sample_column(cfg, cfg.blocks_for_height(18.0), 0).height as f32 * cfg.voxel_meters();
        let deep_vault = if cfg.generator_version >= 16 {
            super::dungeon::deep_vault_anchor(cfg, ground)
        } else {
            Vec3::new(24.0, ground - 71.0, 0.0)
        };
        if cfg.generator_version >= 16 {
            for (i, (point, kind, menace)) in super::dungeon::sanctum_fights(cfg, ground)
                .into_iter()
                .enumerate()
            {
                if (point - player).length() < 46.0 {
                    out.push((
                        IVec3::new(i as i32, 0, 4),
                        point,
                        kind | ((menace as u32) << 8),
                    ));
                }
            }
        } else if (deep_vault - player).length() < 46.0 {
            out.push((IVec3::new(0, 0, 1), deep_vault, 5 | (3 << 8)));
        }
    }
    if cfg.generator_version >= 16 {
        let mut seen = HashMap::<(i32, i32), i32>::new();
        for ((x, z), point) in super::dungeon::anchors(cfg, player, 46.0) {
            let n = seen.entry((x, z)).or_insert(0);
            let idz = if *n == 0 { 1 } else { 5 };
            *n += 1;
            let kind = (hash2(x, z, cfg.seed.wrapping_add(2310 + idz as u32)) * 6.0) as u32 % 6;
            let deep = if point.y < 8.0 {
                3u32
            } else if point.y < 40.0 {
                2
            } else {
                1
            };
            out.push((
                IVec3::new(x + 4096, z + 4096, idz),
                point,
                kind | (deep << 8),
            ));
        }
    }
    out
}

/// Reward identity follows the actual generated structure, not the enemy mix.
pub fn district_post(id: IVec3) -> bool { id.z == 5 && id.y >= super::ruins::POST_ID_Y - 1_000_000 }

pub fn reward_kind(cfg: &WorldSampleConfig, id: IVec3) -> u32 {
    if district_post(id) { return 11; }
    if id == crate::undercrown::v2::observatory::ENCOUNTER_ID {
        return 3;
    }
    if id.z == 0 {
        site(cfg, id.x, id.y).map_or(0, |s| if s.kind==1 && cfg.generator_version>=26 {10} else {s.kind})
    } else if id.z == 2 || id.z == 3 {
        // Treasure caches use the inscriptions that are not boss gates.
        const KINDS: [u32; 7] = [0, 1, 2, 4, 6, 7, 9];
        let n = hash2(
            id.x,
            id.y,
            cfg.seed.wrapping_add(if id.z == 2 { 4401 } else { 4407 }),
        );
        KINDS[(n * KINDS.len() as f32) as usize % KINDS.len()]
    } else {
        6 + if cfg.generator_version >= 4 {
            (hash2(id.x, id.y, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4
        } else {
            0
        }
    }
}

/// Opt-in native review spawn: uses the real generator, without modifying terrain.
pub fn review_causeway_spawn(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.generator_version < 12 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    for ring in 2i32..12 {
        for gx in -ring..=ring {
            for gz in -ring..=ring {
                if gx.abs().max(gz.abs()) != ring {
                    continue;
                }
                if (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4 != 2 {
                    continue;
                }
                if let Some(floor) = cavern_floor(cfg, gx, gz) {
                    return Some(
                        Vec3::new(gx as f32 * 72.0 - 5.0, floor + 4.1, gz as f32 * 72.0 + 9.5)
                            / cfg.voxel_meters(),
                    );
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod floating_tests;
#[cfg(test)]
mod holy_mountain {
    use super::*;
    #[test]
    fn spawn_has_a_brick_mouth_and_a_carved_shaft() {
        use engine_core::VoxelScalePreset;
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let cfg = world.sample_config();
        let ground = sample_column(&cfg, cfg.blocks_for_height(18.0), 0).height;
        let mouth = world.get(IVec3::new(19, ground + 2, 0));
        let shaft = world.get(IVec3::new(18, ground - 12, 0));
        assert_eq!(mouth, B::StoneBricks);
        assert_eq!(shaft, B::Air);
        assert_eq!(world.get(IVec3::new(12, ground, 0)), B::Cobblestone);
    }
}
#[cfg(test)]
mod causeway_review {
    use super::*;
    #[test]
    fn causeway_deck_gap_and_reward_exist_at_each_voxel_scale() {
        use engine_core::VoxelScalePreset;
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut world = crate::test_world::World::with_seed(WorldType::Hills, scale, 3040332434);
            // The causeway chamber is a variant-2 cavern. From generator version
            // 25 the Prism Sanctum replaces variant-2 caverns, so this layout only
            // generates for worlds saved before that version. Pin to the last one.
            world.generator_version = super::super::prism_sanctum::FIRST_VERSION - 1;
            let cfg = world.sample_config();
            let spawn = review_causeway_spawn(&cfg).unwrap() * scale.meters();
            let sample =
                |offset: Vec3| world.get(((spawn + offset) / scale.meters()).floor().as_ivec3());
            assert_eq!(
                sample(Vec3::new(0.0, -0.6, 0.0)),
                B::StoneBricks,
                "deck at {:?}",
                scale
            );
            assert_eq!(
                sample(Vec3::new(5.0, -0.6, 0.0)),
                B::Air,
                "gap at {:?}",
                scale
            );
            assert_eq!(
                sample(Vec3::new(19.5, 0.4, 0.0)),
                B::SurveyCache,
                "cache at {:?}",
                scale
            );
        }
    }
}

#[cfg(test)]
mod generator_16_structures {
    use super::*;
    use engine_core::VoxelScalePreset;

    fn column_has_roof(cfg: &WorldSampleConfig, x: f32, z: f32, ground: f32) -> Option<bool> {
        let v = cfg.voxel_meters();
        let at = |y: f32| {
            crate::sample_block(
                cfg,
                IVec3::new(
                    (x / v).floor() as i32,
                    (y / v).floor() as i32,
                    (z / v).floor() as i32,
                ),
            )
        };
        // An interior is open air about a metre-and-a-half over the floor.
        if at(ground + 1.6) != B::Air
            || !at(ground + 0.2 * v).is_solid() && at(ground + 0.2 * v) != B::Air
        {
            return None;
        }
        let mut y = ground + 2.4;
        while y < ground + 14.0 {
            if at(y).is_solid() {
                return Some(true);
            }
            y += v;
        }
        Some(false)
    }

    #[test]
    fn every_site_kind_is_roofed_in_the_real_world() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            let world = crate::test_world::World::with_seed(WorldType::Hills, scale, 3040332434);
            let cfg = world.sample_config();
            assert!(cfg.generator_version >= 16);
            let mut seen = [false; 6];
            for rx in -9..=9 {
                for rz in -9..=9 {
                    let Some(s) = site(&cfg, rx, rz) else {
                        continue;
                    };
                    if s.kind == 1 || seen[s.kind as usize] {
                        continue;
                    }
                    let (mut roofed, mut open) = (0, 0);
                    let mut dz = -12.0;
                    while dz <= 12.0 {
                        let mut dx = -12.0;
                        while dx <= 12.0 {
                            let w = site_point(s, Vec3::new(dx, 0.0, dz));
                            match column_has_roof(&cfg, w.x, w.z, s.ground) {
                                Some(true) => roofed += 1,
                                Some(false) => open += 1,
                                None => {}
                            }
                            dx += 1.5;
                        }
                        dz += 1.5;
                    }
                    // A site on a steep slope may be partly buried; only judge ones with interiors.
                    if roofed + open > 20 {
                        assert!(
                            roofed >= open / 2,
                            "kind {} at ({rx},{rz}) {scale:?}: {roofed} roofed vs {open} open",
                            s.kind
                        );
                        seen[s.kind as usize] = true;
                    }
                }
            }
            println!("roofed site kinds verified at {scale:?}: {seen:?}");
            assert!(
                seen[0] || seen[2] || seen[3] || seen[4] || seen[5],
                "no testable site found at {scale:?}"
            );
        }
    }

    #[test]
    fn homesteads_exist_and_are_roofed() {
        let world = crate::test_world::World::with_seed(
            WorldType::Hills,
            VoxelScalePreset::OneMeter,
            3040332434,
        );
        let cfg = world.sample_config();
        let mut found = 0;
        for ix in -8..=8 {
            for iz in -8..=8 {
                let Some(h) = homestead(&cfg, ix, iz) else {
                    continue;
                };
                found += 1;
                let mut roofed = 0;
                let mut dz = -h.reach;
                while dz <= h.reach {
                    let mut dx = -h.reach;
                    while dx <= h.reach {
                        let (x, z) = (h.x + dx, h.z + dz);
                        if column_has_roof(&cfg, x, z, h.ground) == Some(true) {
                            roofed += 1;
                        }
                        dx += 1.0;
                    }
                    dz += 1.0;
                }
                assert!(roofed > 0, "homestead at {ix},{iz} has no roofed interior");
            }
        }
        println!("homesteads in 17x17 cells: {found}");
        assert!(found >= 3, "only {found} homesteads in a 17x17 cell area");
    }

    /// `cargo test print_landmarks -- --ignored --nocapture` lists spots worth photographing.
    #[test]
    #[ignore]
    fn print_landmarks() {
        let seed: u32 = std::env::var("LANDMARK_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3040332434);
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, seed);
        let cfg = world.sample_config();
        let mut best: [Option<(f32, Site)>; 6] = [None; 6];
        for rx in -6..=6 {
            for rz in -6..=6 {
                if let Some(s) = site(&cfg, rx, rz) {
                    let d = s.x.hypot(s.z);
                    if best[s.kind as usize].is_none_or(|(bd, _)| d < bd) {
                        best[s.kind as usize] = Some((d, s));
                    }
                }
            }
        }
        for (k, b) in best.iter().enumerate() {
            if let Some((d, s)) = b {
                println!(
                    "site kind {k}: x={:.0} z={:.0} ground={:.0} turn={} dist={d:.0}",
                    s.x, s.z, s.ground, s.turn
                );
            }
        }
        let mut n = 0;
        for ix in -5..=5 {
            for iz in -5..=5 {
                if let Some(h) = homestead(&cfg, ix, iz) {
                    if n < 4 {
                        println!(
                            "homestead: x={:.0} z={:.0} ground={:.0} turn={}",
                            h.x, h.z, h.ground, h.turn
                        );
                    }
                    n += 1;
                }
            }
        }
        for (x, z) in [
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
            (2, 0),
            (0, 2),
            (-2, 0),
            (0, -2),
            (2, 2),
            (-2, -2),
            (2, -2),
            (-2, 2),
            (3, 0),
        ] {
            if let Some(d) = super::super::dungeon::anchors(
                &cfg,
                Vec3::new(x as f32 * 144.0, 0.0, z as f32 * 144.0),
                400.0,
            )
            .first()
            {
                println!("dungeon near cell ({x},{z}): {:?}", d);
                break;
            }
        }
        println!(
            "valley/water: spawn ground {:.0}",
            site(&cfg, 0, 0).map_or(0.0, |s| s.ground)
        );
    }

    #[test]
    #[ignore]
    fn print_water_spots() {
        let seed: u32 = std::env::var("LANDMARK_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3040332434);
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, seed);
        let cfg = world.sample_config();
        let mut found = 0;
        for r in (100..2000).step_by(60) {
            for a in 0..48 {
                let t = a as f32 / 48.0 * std::f32::consts::TAU;
                let (x, z) = ((t.cos() * r as f32) as i32, (t.sin() * r as f32) as i32);
                let h = crate::terrain_height(&cfg, x, z);
                if h <= 11 && found < 8 {
                    println!("water: x={x} z={z} bed={h}");
                    found += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod generator_17_caverns {
    use super::*;
    use engine_core::VoxelScalePreset;

    #[test]
    fn binding_circle_is_dry_and_holds_an_artifact() {
        let world =
            crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        let cfg = world.sample_config();
        assert!(cfg.generator_version >= 17);
        let mut found = false;
        for gz in -18..=18 {
            for gx in -18..=18 {
                let ax = gx as f32 * 72.0;
                let az = gz as f32 * 72.0;
                if ax.hypot(az) < 120.0 {
                    continue;
                }
                let n = hash2(gx, gz, cfg.seed.wrapping_add(1101));
                if n < chamber_rarity(cfg.generator_version) {
                    continue;
                }
                let variant = (hash2(gx, gz, cfg.seed.wrapping_add(1187)) * 4.0) as u32 % 4;
                if variant != 3 {
                    continue;
                }
                let ground = sample_column(
                    &cfg,
                    cfg.blocks_for_height(ax),
                    cfg.blocks_for_height(az),
                )
                .height as f32;
                let floor = (ground - 18.0 - n * 25.0).floor();
                let at = |dx: f32, dy: f32, dz: f32| {
                    crate::sample_block(
                        &cfg,
                        IVec3::new(
                            (ax + dx).floor() as i32,
                            (floor + dy).floor() as i32,
                            (az + dz).floor() as i32,
                        ),
                    )
                };
                if at(0.0, 3.2, 0.0) != B::MithrilBlock {
                    continue;
                }
                assert_eq!(at(0.0, 1.2, 0.0), B::Water, "the font is one block deep");
                assert_eq!(
                    at(5.0, 1.2, 0.0),
                    B::Air,
                    "the old drowned pool is open air"
                );
                assert_eq!(at(7.6, 2.2, 0.0), B::CopperBlock);
                assert_eq!(at(0.0, 8.0, 0.0), B::Air);
                assert_eq!(
                    location(&cfg, Vec3::new(ax, floor + 4.0, az)),
                    Some("Circle of binding")
                );
                found = true;
                break;
            }
            if found {
                break;
            }
        }
        assert!(found, "a circle of binding should generate away from spawn");
    }
}

#[cfg(test)]
mod stage_bench {
    use super::*;
    use engine_core::VoxelScalePreset;
    use crate::{test_world::World, WorldType};

    /// `cargo test --release sites::stage_bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn site_stage_costs() {
        for version in [15u32, 16] {
        let mut cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        cfg.generator_version = version;
        println!("--- generator v{version}");
        let origin = IVec3::new(64, 32, 64);
        let mut voxels = Vec::new();
        for z in 0..34 {
            for x in 0..34 {
                let h = crate::terrain_height(&cfg, origin.x + x, origin.z + z);
                for y in 0..34 {
                    voxels.push((origin + IVec3::new(x, y, z), h));
                }
            }
        }
        let time = |name: &str, f: &dyn Fn(IVec3, i32) -> bool| {
            let mut hits = 0;
            for &(p, h) in &voxels {
                hits += f(p, h) as u32;
            }
            let start = std::time::Instant::now();
            for &(p, h) in &voxels {
                hits += f(p, h) as u32;
            }
            println!(
                "{name:>12}: {:7.2} ms ({hits})",
                start.elapsed().as_secs_f64() * 1e3
            );
        };
        let m = |p: IVec3| (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
        let top = |h: i32| (h + 1) as f32 * cfg.voxel_meters();
        time("descent", &|p, _| noita_descent(&cfg, m(p)).is_some());
        time("architecture", &|p, _| {
            super::super::valley::architecture(&cfg, m(p)).is_some()
        });
        time("reserves", &|p, _| {
            super::super::valley::reserves(&cfg, m(p).x, m(p).z)
        });
        time("homestead", &|p, h| {
            homestead_block(&cfg, m(p), top(h)).is_some()
        });
        time("surface", &|p, h| {
            super::super::surface::block(&cfg, p, h, top(h)).is_some()
        });
        time("all", &|p, h| block(&cfg, p, h).is_some());
        }
    }
}
