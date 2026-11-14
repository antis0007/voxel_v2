//! Bounded regional history for the solved planet. The terrain is an input:
//! simulation runs once per region and its surviving plans are sampled on demand.
mod fabric;
mod routes;
pub mod chronicle;

use crate::{BlockId as B, WorldSampleConfig, planetary};
use engine_core::cube_sphere::atlas;
use glam::{DVec3, IVec2, IVec3, Vec2, Vec3, Vec3Swizzles};
use std::{cell::RefCell, collections::HashMap, sync::{Arc, Mutex, OnceLock}};

pub const VERSION: u32 = engine_voxel::CIVILIZATION_GENERATOR;
pub const CHRONICLE_GENERATOR: u32 = 254;
pub use fabric::{archive, archive_position, archive_text, location, visit_skylines};
pub(crate) use fabric::{block, clears_vegetation};
const CELL: f32 = 512.0;
const FOOTPRINT: Vec2 = Vec2::new(23.0, 18.0);
const LOTS: u32 = 6;
const BASE: u32 = 1 << 28;
const MAX_RESIDENT_ID: u32 = BASE + (1 << 21);
const STEP_YEARS: u32 = 20;
const GENERATIONS: u32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Livelihood {
    Floodplain,
    Limeworks,
    Quarry,
    Pasture,
}

impl Livelihood {
    pub fn name(self) -> &'static str {
        match self {
            Self::Floodplain => "River Commons",
            Self::Limeworks => "Limeworkers' Hold",
            Self::Quarry => "Stonecutters' Hold",
            Self::Pasture => "Hearth Commons",
        }
    }

    fn harvest(self) -> i32 {
        match self {
            Self::Floodplain => 14,
            Self::Limeworks => 6,
            Self::Quarry => 5,
            Self::Pasture => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Burial {
    Alluvium,
    Scree,
    Bedrock,
}

impl Burial {
    pub fn description(self) -> &'static str {
        match self {
            Self::Alluvium => "River deposits sealed the old storehouse.",
            Self::Scree => "Rockfall and slope debris sealed the old storehouse.",
            Self::Bedrock => "The old storehouse was cut directly into bedrock.",
        }
    }

    fn depth(self) -> f32 {
        match self {
            Self::Alluvium => 9.0,
            Self::Scree => 8.0,
            Self::Bedrock => 7.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct History {
    pub livelihood: Livelihood,
    pub founded: u32,
    pub peak_year: u32,
    pub peak_households: u32,
    pub households: u32,
    pub abandoned: Option<u32>,
    pub flood_year: Option<u32>,
    pub rockfall_year: Option<u32>,
    pub refugees_received: u32,
    pub burial: Burial,
    pub rock: B,
    pub cap_rock: B,
    pub neighbor: Option<Livelihood>,
}

#[derive(Clone, Copy)]
struct Site {
    center: Vec2,
    datum: f32,
    history: History,
    flood_risk: f32,
    shelter: i32,
    landscape: chronicle::Landscape,
    burial_candidate: Burial,
}

type Region = [Option<Site>; 2];
struct Plan {
    sites: Region,
    route: Vec<Vec2>,
    chronicle: Option<std::sync::Arc<chronicle::Chronicle>>,
}
impl std::ops::Deref for Plan {
    type Target = Region;
    fn deref(&self) -> &Region { &self.sites }
}
type Plans = (Option<WorldSampleConfig>, HashMap<IVec2, Arc<Plan>>);
type SharedPlan = (WorldSampleConfig, IVec2, Arc<OnceLock<Arc<Plan>>>);
static SHARED_PLANS: Mutex<Vec<SharedPlan>> = Mutex::new(Vec::new());
thread_local! {
    static PLANS: RefCell<Plans> = RefCell::new((None, HashMap::new()));
}

fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.civilization_enabled() && cfg.generator_version >= VERSION && planetary::active(cfg)
}

fn key_at(p: Vec2) -> IVec2 {
    (p / CELL).floor().as_ivec2()
}

fn cell_center(key: IVec2) -> Vec2 {
    (key.as_vec2() + Vec2::splat(0.5)) * CELL
}

fn anchor(key: IVec2, slot: usize) -> Vec2 {
    cell_center(key) + Vec2::new(if slot == 0 { -132.0 } else { 132.0 }, 0.0)
}

fn hash(cfg: &WorldSampleConfig, key: IVec2, salt: u32) -> u32 {
    crate::hash_u32(
        cfg.seed
            ^ (key.x as u32).wrapping_mul(0x9e37_79b9)
            ^ (key.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ salt,
    )
}

fn shared_plan(cfg: &WorldSampleConfig, key: IVec2) -> Arc<OnceLock<Arc<Plan>>> {
    let mut shared = SHARED_PLANS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, slot)) = shared.iter().find(|(c, k, _)| c == cfg && *k == key) {
        return Arc::clone(slot);
    }
    if shared.len() >= 256 { shared.remove(0); }
    let slot = Arc::new(OnceLock::new());
    shared.push((*cfg, key, Arc::clone(&slot)));
    slot
}

/// Queue cold settlement surveys without blocking the simulation. The same
/// immutable plans are consumed by terrain generation and resident discovery.
/// One worker and one pending request bound both CPU pressure and queue growth.
pub fn prepare_settlements_near(cfg: &WorldSampleConfig, point: Vec3, radius: f32) -> bool {
    if !enabled(cfg) || !planetary::landscape_cycles(cfg.generator_version) { return true; }
    let cfg = WorldSampleConfig { voxel_scale: engine_core::VoxelScalePreset::OneMeter, ..*cfg };
    let keys: Vec<_> = keys_near(point.xz(), radius).collect();
    if keys.iter().all(|&key| shared_plan(&cfg, key).get().is_some()) { return true; }
    type Request = (WorldSampleConfig, Vec<IVec2>);
    static SURVEY: OnceLock<std::sync::mpsc::SyncSender<Request>> = OnceLock::new();
    let worker = SURVEY.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Request>(1);
        std::thread::Builder::new().name("settlement-survey".into()).spawn(move || {
            while let Ok((cfg, keys)) = rx.recv() {
                for key in keys {
                    let ready = plan(&cfg, key);
                    // The worker's hot cache may outlive a shared-cache entry.
                    // Republish it so readiness cannot stay false after eviction.
                    let _ = shared_plan(&cfg, key).set(ready);
                }
            }
        }).expect("settlement survey worker");
        tx
    });
    let _ = worker.try_send((cfg, keys));
    false
}

fn plan(cfg: &WorldSampleConfig, key: IVec2) -> Arc<Plan> {
    let physical = WorldSampleConfig { voxel_scale: engine_core::VoxelScalePreset::OneMeter, ..*cfg };
    let cfg = if cfg.generator_version >= CHRONICLE_GENERATOR { &physical } else { cfg };
    if let Some(hit) = PLANS.with(|plans| {
        let plans = plans.borrow();
        (plans.0 == Some(*cfg))
            .then(|| plans.1.get(&key).cloned())
            .flatten()
    }) {
        return hit;
    }
    let sites = if planetary::landscape_cycles(cfg.generator_version) {
        let slot = shared_plan(cfg, key);
        // No cache lock is held while terrain is sampled. One deterministic
        // plan is shared by chunk workers, foliage and resident simulation.
        Arc::clone(slot.get_or_init(|| Arc::new(build_plan(cfg, key))))
    } else { Arc::new(build_plan(cfg, key)) };
    PLANS.with(|plans| {
        let mut plans = plans.borrow_mut();
        if plans.0 != Some(*cfg) || plans.1.len() >= 256 {
            *plans = (Some(*cfg), HashMap::new());
        }
        plans.1.insert(key, Arc::clone(&sites));
    });
    sites
}

fn build_plan(cfg: &WorldSampleConfig, key: IVec2) -> Plan {
    let survey_cfg = cfg;
    let mut sites = std::array::from_fn(|slot| survey(survey_cfg, key, slot));
    let route = if planetary::landscape_cycles(cfg.generator_version) {
        match sites { [Some(left), Some(right)] => routes::find(cfg, left.center, right.center), _ => Vec::new() }
    } else { Vec::new() };
    let linked = match sites {
        [Some(left), Some(right)] => if planetary::landscape_cycles(cfg.generator_version) { !route.is_empty() }
            else { route_passable(cfg, left.center, right.center) },
        _ => false,
    };
    let mut chronicle = if cfg.generator_version >= CHRONICLE_GENERATOR {
        Some(chronicle::Chronicle::simulate(survey_cfg, key, &mut sites, linked))
    } else {
        simulate(cfg, key, &mut sites, linked);
        None
    };
    for site in sites.iter_mut().flatten() {
        let cap_y = site.datum - site.history.burial.depth() - 0.5;
        site.history.cap_rock = rock_at(survey_cfg, site.center, cap_y);
    }
    if let Some(record) = &mut chronicle { record.sites = chronicle::Chronicle::records(key, &sites); }
    Plan { sites, route, chronicle: chronicle.map(Arc::new) }
}

fn survey(cfg: &WorldSampleConfig, key: IVec2, slot: usize) -> Option<Site> {
    let origin = anchor(key, slot);
    let (_, local) = atlas::split(DVec3::new(origin.x.into(), 0.0, origin.y.into()));
    if local.x.abs().max(local.z.abs()) > atlas::HALF_M - 100.0 {
        return None;
    }
    let freight = crate::planet_freight::site(cfg);
    let mut best: Option<(f32, Site)> = None;
    for dz in [-48.0, 0.0, 48.0] {
        for dx in [-48.0, 0.0, 48.0] {
            let p = origin + Vec2::new(dx, dz);
            if freight.is_some_and(|site| (site.origin.xz() - p).length() < 110.0) {
                continue;
            }
            let g = planetary::ground(cfg, p.x, p.y, 1.0);
            if g.top_m < crate::WATER_LEVEL_METERS + 2.0
                || g.slope > 0.17
                || g.water_m.is_some()
                || g.permafrost
                || planetary::trail_reserves(cfg, p.x, p.y, FOOTPRINT.length() + 2.0)
            {
                continue;
            }

            let mut low = g.top_m;
            let mut high = g.top_m;
            let mut safe = true;
            for z in [-FOOTPRINT.y, 0.0, FOOTPRINT.y] {
                for x in [-FOOTPRINT.x, 0.0, FOOTPRINT.x] {
                    let q = p + Vec2::new(x, z);
                    let edge = planetary::ground(cfg, q.x, q.y, 1.0);
                    low = low.min(edge.top_m);
                    high = high.max(edge.top_m);
                    safe &= edge.water_m.is_none()
                        && edge.top_m > crate::WATER_LEVEL_METERS + 1.0
                        && !crate::ruins::reserves(cfg, q.x, q.y, 14.0)
                        && !crate::surveyor::reserves(cfg, q.x, q.y);
                }
            }
            if !safe || high - low > 3.5 {
                continue;
            }

            let rock = rock_at(cfg, p, g.top_m - 6.0);
            let flood_risk = (g.delta.max(g.riparian) * (1.0 - g.slope)).clamp(0.0, 1.0);
            let livelihood = if flood_risk > 0.25 {
                Livelihood::Floodplain
            } else if rock == B::Limestone {
                Livelihood::Limeworks
            } else if matches!(rock, B::Sandstone | B::Basalt) {
                Livelihood::Quarry
            } else {
                Livelihood::Pasture
            };
            let burial = if flood_risk > 0.25 && g.sediment > 0.3 {
                Burial::Alluvium
            } else if g.scree_m > 0.8 {
                Burial::Scree
            } else {
                Burial::Bedrock
            };
            // Physical street access helps a hold; unbuilt rail bearings do not.
            let score = g.moisture + flood_risk * 0.45 - (high - low) * 0.18
                + if rock == B::Limestone { 0.18 } else { 0.0 }
                + crate::ruins::line::route_proximity(cfg, p.x, p.y) * 0.35;
            let founded = 1 + hash(cfg, key, 7123 + slot as u32) % 5;
            let site = Site {
                center: p,
                datum: (high + 0.5).ceil(),
                flood_risk,
                shelter: (g.temperature * 4.0 + g.moisture * 3.0).round() as i32,
                landscape: chronicle::Landscape::surveyed(g, flood_risk, rock),
                burial_candidate: burial,
                history: History {
                    livelihood,
                    founded: founded * STEP_YEARS,
                    peak_year: founded * STEP_YEARS,
                    peak_households: 0,
                    households: 0,
                    abandoned: None,
                    flood_year: None,
                    rockfall_year: None,
                    refugees_received: 0,
                    burial,
                    rock,
                    cap_rock: rock,
                    neighbor: None,
                },
            };
            if best.is_none_or(|(rating, _)| score > rating) {
                best = Some((score, site));
            }
        }
    }
    best.map(|(_, site)| site)
}

fn rock_at(cfg: &WorldSampleConfig, p: Vec2, y: f32) -> B {
    let voxel = (Vec3::new(p.x, y, p.y) / cfg.voxel_meters())
        .floor()
        .as_ivec3();
    planetary::rock_at(cfg, voxel)
}

fn route_passable(cfg: &WorldSampleConfig, start: Vec2, end: Vec2) -> bool {
    let steps = (start.distance(end) / 16.0).ceil() as u32;
    let mut water_run = 0;
    let mut previous = None;
    for i in 0..=steps {
        let p = start.lerp(end, i as f32 / steps as f32);
        let ground = planetary::ground(cfg, p.x, p.y, 1.0);
        if ground.top_m < crate::WATER_LEVEL_METERS {
            return false;
        }
        water_run = if ground.water_m.is_some() {
            water_run + 1
        } else {
            0
        };
        if water_run > 1 || previous.is_some_and(|height: f32| (ground.top_m - height).abs() > 6.0)
        {
            return false;
        }
        previous = Some(ground.top_m);
    }
    true
}

fn simulate(cfg: &WorldSampleConfig, key: IVec2, sites: &mut Region, linked: bool) {
    let mut stores = [0; 2];
    let mut route = [0; 2];
    for step in 1..=GENERATIONS {
        let year = step * STEP_YEARS;
        let weather = hash(cfg, key, 9200 + step) % 100;
        for (i, site) in sites.iter_mut().enumerate() {
            let Some(site) = site else { continue };
            let h = &mut site.history;
            if year < h.founded || h.abandoned.is_some() {
                continue;
            }
            if year == h.founded {
                h.households = 2;
                stores[i] = 8;
            }
            let flood = weather < (site.flood_risk * 28.0) as u32;
            let slide = matches!(h.burial, Burial::Scree) && weather < 5;
            let drought = weather >= 76;
            let harvest = h.livelihood.harvest() + site.shelter
                - if flood || slide {
                    14
                } else if drought {
                    9
                } else {
                    0
                };
            stores[i] = (stores[i] + harvest.max(0) - h.households as i32 * 3).min(30);
            if flood {
                h.flood_year = Some(year);
            }
            if slide {
                h.rockfall_year = Some(year);
            }
        }

        // Move food first. Any remaining shortage can displace one household
        // to a viable neighbour with room, otherwise it leaves the region.
        let [left, right] = sites;
        if linked && let (Some(left), Some(right)) = (left, right) {
            let pair = [&mut left.history, &mut right.history];
            for (from, to) in [(0, 1), (1, 0)] {
                if pair[from].households == 0
                    || pair[to].households == 0
                    || stores[from] >= 0
                    || stores[to] < 9
                {
                    continue;
                }
                let aid = (-stores[from]).min(4);
                stores[from] += aid;
                stores[to] -= aid;
                route[from] += 1;
                if stores[from] < 0 && pair[to].households < LOTS {
                    pair[from].households -= 1;
                    pair[to].households += 1;
                    pair[to].refugees_received += 1;
                    stores[from] += 3;
                    stores[to] -= 3;
                }
            }
        }

        for (i, site) in sites.iter_mut().enumerate() {
            let Some(site) = site else { continue };
            let h = &mut site.history;
            if year < h.founded || h.abandoned.is_some() {
                continue;
            }
            if stores[i] < 0 {
                h.households = h.households.saturating_sub(1 + u32::from(stores[i] < -6));
                stores[i] = 0;
            } else if stores[i] >= 14 && h.households < LOTS {
                h.households += 1;
                stores[i] -= 8;
            }
            if h.households > h.peak_households {
                h.peak_households = h.households;
                h.peak_year = year;
            }
            if h.households == 0 {
                h.abandoned = Some(year);
            }
        }
    }
    for i in 0..2 {
        let neighbor = sites[1 - i].map(|s| s.history.livelihood);
        if let Some(site) = &mut sites[i] {
            site.history.neighbor = if route[i] > 0 { neighbor } else { None };
            if (matches!(site.history.burial, Burial::Alluvium)
                && site.history.flood_year.is_none())
                || (matches!(site.history.burial, Burial::Scree)
                    && site.history.rockfall_year.is_none())
            {
                site.history.burial = Burial::Bedrock;
            }
        }
    }
}

fn encode(key: IVec2, slot: usize, lot: u32) -> u32 {
    BASE + (((key.x + 64) as u32) << 11) + (((key.y + 64) as u32) << 4) + ((slot as u32) << 3) + lot
}

pub(crate) fn role(id: u32) -> Option<u32> {
    resident_id(id).then_some(id & 3)
}

pub fn original_settlement(id: u32) -> Option<u32> {
    ((BASE..MAX_RESIDENT_ID).contains(&id) && id & 7 < LOTS).then_some(id & !7)
}

pub fn resident_id(id: u32) -> bool {
    original_settlement(id).is_some() || (EXTENSION_ID..EXTENSION_ID + (1 << 25)).contains(&id)
}

fn by_id(cfg: &WorldSampleConfig, id: u32) -> Option<(Site, u32)> {
    if !enabled(cfg) {
        return None;
    }
    original_settlement(id)?;
    let code = id - BASE;
    let key = IVec2::new((code >> 11) as i32 - 64, ((code >> 4) & 127) as i32 - 64);
    let site = plan(cfg, key)[((code >> 3) & 1) as usize]?;
    let lot = code & 7;
    (lot < site.history.households).then_some((site, lot))
}

pub fn history(cfg: &WorldSampleConfig, id: u32) -> Option<History> {
    by_id(cfg, id).map(|(site, _)| site.history)
}

pub(crate) fn home(cfg: &WorldSampleConfig, id: u32) -> Option<Vec3> {
    let (site, lot) = by_id(cfg, id)?;
    Some(site.home(cfg, lot))
}

pub fn daily_destination(
    cfg: &WorldSampleConfig,
    id: u32,
    position: Vec3,
    at_home: bool,
) -> Option<Vec3> {
    let (site, lot) = by_id(cfg, id)?;
    Some(site.daily_destination(cfg, lot, position, at_home))
}

pub(crate) fn nearby(
    cfg: &WorldSampleConfig,
    player: Vec3,
    out: &mut Vec<crate::households::Household>,
) {
    if !enabled(cfg) || !prepare_settlements_near(cfg, player, 100.0) {
        return;
    }
    let point = player.xz();
    for key in keys_near(point, 100.0) {
        for (slot, site) in plan(cfg, key).iter().enumerate() {
            let Some(site) = site else { continue };
            for lot in 0..site.history.households {
                let home = site.home(cfg, lot);
                if home.distance_squared(player) < 100.0 * 100.0 {
                    out.push(crate::households::Household {
                        id: encode(key, slot, lot),
                        home,
                    });
                }
            }
        }
    }
}

fn keys_near(point: Vec2, radius: f32) -> impl Iterator<Item = IVec2> {
    let low = key_at(point - Vec2::splat(radius));
    let high = key_at(point + Vec2::splat(radius));
    (low.y.max(-64)..=high.y.min(63))
        .flat_map(move |z| (low.x.max(-64)..=high.x.min(959)).map(move |x| IVec2::new(x, z)))
}

/// The generated inputs shared by the prehistory and live settlement simulation.
#[derive(Clone, Copy)]
pub struct Settlement {
    pub id: u32,
    pub center: Vec3,
    pub history: History,
    pub harvest: i32,
    pub flood_risk: f32,
}

fn settlement(cfg: &WorldSampleConfig, key: IVec2, slot: usize, site: Site) -> Settlement {
    Settlement {
        id: encode(key, slot, 0),
        center: site.standing(cfg, Vec2::ZERO),
        history: site.history,
        harvest: site.history.livelihood.harvest() + site.shelter,
        flood_risk: site.flood_risk,
    }
}

pub fn settlements_near(cfg: &WorldSampleConfig, point: Vec3, radius: f32) -> Vec<Settlement> {
    if !enabled(cfg) {
        return Vec::new();
    }
    keys_near(point.xz(), radius)
        .flat_map(|key| {
            let region = plan(cfg, key);
            region.sites
                .into_iter()
                .enumerate()
                .filter_map(move |(slot, site)| {
                    let site = site?;
                    (site.center.distance(point.xz()) <= radius)
                        .then(|| settlement(cfg, key, slot, site))
                })
        })
        .collect()
}

pub fn settlement_by_id(cfg: &WorldSampleConfig, id: u32) -> Option<Settlement> {
    if !enabled(cfg) || !(BASE..MAX_RESIDENT_ID).contains(&id) || id & 7 != 0 {
        return None;
    }
    let code = id - BASE;
    let key = IVec2::new((code >> 11) as i32 - 64, ((code >> 4) & 127) as i32 - 64);
    let slot = ((code >> 3) & 1) as usize;
    Some(settlement(cfg, key, slot, plan(cfg, key)[slot]?))
}

/// Shared immutable history; runtime edits and settlement ledgers remain authoritative.
pub fn generation_record(cfg: &WorldSampleConfig, id: u32) -> Option<std::sync::Arc<chronicle::Chronicle>> {
    if !enabled(cfg) || cfg.generator_version < CHRONICLE_GENERATOR
        || original_settlement(id) != Some(id) { return None; }
    let code = id - BASE;
    let key = IVec2::new((code >> 11) as i32 - 64, ((code >> 4) & 127) as i32 - 64);
    let region = plan(cfg, key);
    region[((code >> 3) & 1) as usize]?;
    region.chronicle.clone()
}

pub fn linked(cfg: &WorldSampleConfig, a: Settlement, b: Settlement) -> bool {
    let distance = a.center.distance(b.center);
    if !distance.is_finite() || distance > CELL { return false; }
    if planetary::landscape_cycles(cfg.generator_version)
        && original_settlement(a.id) == Some(a.id)
        && original_settlement(b.id) == Some(b.id)
        && (a.id ^ b.id) == 8
    {
        let code = a.id - BASE;
        let key = IVec2::new((code >> 11) as i32 - 64, ((code >> 4) & 127) as i32 - 64);
        let slot = ((code >> 3) & 1) as usize;
        let region = plan(cfg, key);
        if region[slot].is_some_and(|site| site.center == a.center.xz())
            && region[1 - slot].is_some_and(|site| site.center == b.center.xz())
        {
            // History and the visible road use this terrain-validated route.
            // A straight chord can reject its detour or miss an unsafe crossing,
            // while repeating expensive terrain probes on the simulation thread.
            return !region.route.is_empty();
        }
    }
    route_passable(cfg, a.center.xz(), b.center.xz())
}

pub const EXTENSION_LOTS: u8 = 16;
const EXTENSION_ID: u32 = 1 << 27;

pub fn extension_resident(settlement: u32, serial: u8) -> u32 {
    EXTENSION_ID | ((settlement - BASE) << 4) | u32::from(serial)
}

pub fn extension_plot(cfg: &WorldSampleConfig, site: Settlement, lot: u8) -> Option<IVec3> {
    if lot >= EXTENSION_LOTS {
        return None;
    }
    let p = site.center.xz()
        + Vec2::new(
            (30.0 + f32::from(lot / 4) * 12.0) * if lot & 2 == 0 { 1.0 } else { -1.0 },
            if lot & 1 == 0 { 7.0 } else { -7.0 },
        );
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for z in [-3.0, 0.0, 3.0] {
        for x in [-3.0, 0.0, 3.0] {
            let q = p + Vec2::new(x, z);
            let g = planetary::ground(cfg, q.x, q.y, 1.0);
            if g.water_m.is_some()
                || g.top_m < crate::WATER_LEVEL_METERS + 1.0
                || crate::ruins::reserves(cfg, q.x, q.y, 8.0)
                || crate::surveyor::reserves(cfg, q.x, q.y)
                || crate::planet_freight::site(cfg)
                    .is_some_and(|freight| freight.origin.xz().distance(q) < 75.0)
                || site_at_extension_collision(cfg, q, site.id)
            {
                return None;
            }
            low = low.min(g.top_m);
            high = high.max(g.top_m);
        }
    }
    if high - low > 1.8 {
        return None;
    }
    Some(
        (Vec3::new(p.x, high + 0.5, p.y) / cfg.voxel_meters())
            .ceil()
            .as_ivec3(),
    )
}

fn site_at_extension_collision(cfg: &WorldSampleConfig, point: Vec2, id: u32) -> bool {
    settlements_near(cfg, Vec3::new(point.x, 0.0, point.y), 75.0)
        .iter()
        .any(|other| other.id != id)
}
