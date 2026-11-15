//! Version 142: one spherical crust, climate and watershed network.
//!
//! Cube charts address the same physical fields. Edges share vertex IDs;
//! neither the coast nor the drainage solver knows about face boundaries.
mod alluvium;
mod geology_cache;
mod snapshot;
mod placer;
mod drift;
mod coastal_erosion;
mod coast_roughness;
mod shoreline;
mod epochs;
mod estuary;
mod delta_islands;
mod outlook;
mod slope_relief;
mod surface_query;
mod flank_erosion;
mod hotspots;
mod reefs;
mod island_clock;
mod erosion_clock;
mod hydrology;
mod morphodynamics;
mod stratovolcanoes;
mod tectonics;
mod strata;
mod solve_atlas;
mod topology;
mod volcanic_landforms;
mod volcanism;
pub mod review;
pub mod reference_relief;
mod glacial;
pub mod erratics;
pub mod ground_breaks;
mod fine_relief;
mod rift_valley;
mod dikes;
mod carve;
mod dunes;
mod rock_relief;

use crate::{BlockId, ColumnSample, WorldSampleConfig, WorldType, geo};
use engine_core::cube_sphere::{Face, Projection, atlas};
use engine_core::planet_frame::PlanetAxis;
use glam::DVec3;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};
use tectonics::smooth;
pub use drift::{DriftFrame, DriftTexel, HistoryBudget, HistoryEvent, DRIFT_MYR};
pub use solve_atlas::{SolveAtlas, SolveTexel};

/// Configure a dedicated, disposable geology cache before generation starts.
pub fn configure_geology_cache(directory: std::path::PathBuf) -> bool {
    geology_cache::configure(directory)
}
/// Opt-in disposable regional Stage storage. Never point this at player saves.
/// Configure before requesting a World; already published Worlds are unchanged.
pub fn configure_relief_cache(directory: std::path::PathBuf) -> bool {
    slope_relief::configure_cache(directory)
}
pub use slope_relief::ReliefCacheStats;
pub fn relief_cache_stats() -> ReliefCacheStats { slope_relief::cache_stats() }

/// Opt-in finer plate integration with bounded physical-history playback.
pub const FINE_HISTORY_GENERATOR: u32 = 285;

fn fine_history(generator: u32) -> bool { generator == FINE_HISTORY_GENERATOR || deep_time(generator) }

pub const VERSION: u32 = engine_voxel::SEAMLESS_PLANET_GENERATOR;
/// Opt-in landscape study, independent of the rolling Planet default.
pub const ESTUARY_OUTLOOK_GENERATOR: u32 = 195;
/// Explicit preview of erosion with sediment feedback into the next solve.
pub const COUPLED_SEDIMENT_GENERATOR: u32 = 206;
pub const DELTA_ISLANDS_GENERATOR: u32 = 221;
/// Refined mobile-bed solve spanning the last reach and the receiving shelf.
pub const MOBILE_DELTA_GENERATOR: u32 = 224;
/// Opt-in coastal solve with conveyance-weighted inflow and settling over the full bed.
pub const EVOLVING_DELTA_GENERATOR: u32 = 232;
/// Opt-in coastal solve that feeds downstream drainage back into the next flood.
pub const DELTA_FEEDBACK_GENERATOR: u32 = engine_voxel::FLOODPLAIN_MEANDER_GENERATOR;
/// Opt-in flood and bedload feedback across the lower river and tidal shelf.
pub const DISTRIBUTARY_GENERATOR: u32 = engine_voxel::FLOODPLAIN_SCROLL_GENERATOR;
pub const ERODED_RELIEF_GENERATOR: u32 = engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR;
pub const REGIONAL_EROSION_GENERATOR: u32 = 244;
/// Opt-in geological history with variable runoff, glacial sediment and tidal beaches.
pub const LANDSCAPE_HISTORY_GENERATOR: u32 = 245;
/// Playable canyon with shared erosional and mineable stratigraphy.
pub const MAPPED_CANYON_GENERATOR: u32 = 246;
/// Asymmetric subduction and tilted, segmented fault blocks feed erosion.
pub const FAULT_HISTORY_GENERATOR: u32 = 247;
/// Long regional uplift with episodic runoff and sediment-limited wind abrasion.
pub const WEATHERING_HISTORY_GENERATOR: u32 = 248;
/// Opt-in rainfall erosion on weak exposed upland beds as well as channels.
pub const UPLAND_RUNOFF_GENERATOR: u32 = 249;
/// Regional canyon incision in heterogeneous, jointed bedrock.
pub const JOINTED_CANYON_GENERATOR: u32 = 251;
/// Connected headwaters and surveyed exploration loops on the regional bed.
pub const CATCHMENT_EXPLORATION_GENERATOR: u32 = 255;
/// Wider valley reaches, resistant steps and bank-derived sediment storage.
pub const VALLEY_FORM_GENERATOR: u32 = 257;
/// Bedrock sills concentrate incision into short pools downstream.
pub const SILL_EROSION_GENERATOR: u32 = 265;
/// Moving bedload abrades rock and weathering replenishes upland sediment.
pub const SEDIMENT_WEATHERING_GENERATOR: u32 = 256;
/// Incision across asymmetric folds with retreating, sediment-producing scarps.
pub const FOLDED_SCARP_GENERATOR: u32 = 264;
/// Planet-wide headwater incision with local lithology and talus transport.
pub const HEADWATER_RELIEF_GENERATOR: u32 = 266;
/// Layered escarpment retreat and channel incision across a tilted rock package.
pub const BEDROCK_SCARP_GENERATOR: u32 = 269;
/// Continuous mountain belts, traversable passes, and bent hotspot chains.
pub const TECTONIC_TRAVERSE_GENERATOR: u32 = 270;
/// Eroded beds and composed gradients across the planet's tributaries.
pub const DRAINAGE_RELIEF_GENERATOR: u32 = 271;
pub use engine_voxel::WATERSHED_EROSION_GENERATOR;
/// Volcanoes dissected from sector failure through routed sediment transport.
pub const VOLCANIC_SECTOR_GENERATOR: u32 = 273;
/// Evolve bedrock, channel deposits and floodplains on the shared sphere.
pub const CONNECTED_WATERSHEDS_GENERATOR: u32 = 274;
/// Finer spherical drainage and sediment-driven lateral channel evolution.
pub const REFINED_DRAINAGE_GENERATOR: u32 = 275;
/// Rifted continental margins and fragments carried by plate motion.
pub const RIFTED_MARGIN_GENERATOR: u32 = 276;
/// Connect the planet's runoff and high-resolution erosional relief.
pub const CONNECTED_RELIEF_GENERATOR: u32 = 277;
/// Advected crust ancestry with erosion and coastal transport during plate motion.
pub const COUPLED_RIFT_GENERATOR: u32 = 278;
/// Faulted strata exposed by connected canyon erosion.
pub const LAYERED_CANYON_GENERATOR: u32 = 279;
/// Advected climate strata, episodic volcanism and stress-driven earthquakes.
/// Drainage evolves on a shared regional surface before local refinement.
pub use engine_voxel::{BRANCHING_WATERSHEDS_GENERATOR, CHANGING_SEAS_GENERATOR,
    EVOLVED_WATERSHEDS_GENERATOR, GEOLOGICAL_HISTORY_GENERATOR, RESOLVED_HISTORY_GENERATOR};
pub use engine_voxel::{LANDSCAPE_CYCLES_GENERATOR, DEEP_TIME_GENERATOR, FRACTAL_COAST_GENERATOR, PROCESS_COAST_GENERATOR, CLEAN_RIVERBED_GENERATOR, GROUND_BREAK_GENERATOR, RIFT_VALLEY_GENERATOR, DIKE_SWARM_GENERATOR, LIVING_SURFACE_GENERATOR, LOWLAND_RIVER_GENERATOR, CARVED_SURFACE_GENERATOR, ROCK_RELIEF_GENERATOR};

/// Generator 288 keeps the 287 history and adds the fractal shoreline.
pub fn deep_time(generator: u32) -> bool { generator == DEEP_TIME_GENERATOR || fractal_coasts(generator) }

pub fn fractal_coasts(generator: u32) -> bool {
    generator == FRACTAL_COAST_GENERATOR || process_coasts(generator) || clean_riverbeds(generator)
}
/// Generator 290 keeps the 288 coasts and cleans river, pond and shore cover.
pub fn clean_riverbeds(generator: u32) -> bool {
    generator >= engine_voxel::SHARED_SURFACE_GENERATOR
        || generator == CLEAN_RIVERBED_GENERATOR || ground_breaks(generator) || living_surface(generator)
        || lowland_rivers(generator)
}
/// Generator 295 keeps the 290 terrain and adds fine relief, shore forms and climate plants.
pub fn living_surface(generator: u32) -> bool { generator == LIVING_SURFACE_GENERATOR || rift_valleys(generator) }
/// Generator 297 keeps the 290 terrain, faults a graben into continental rifts and
/// turns on the 293 ground breaks and the 295 living surface.
pub fn rift_valleys(generator: u32) -> bool { generator == RIFT_VALLEY_GENERATOR || dike_swarms(generator) }
/// Generator 298 keeps the 297 terrain and intrudes basalt dikes through the rock.
pub fn dike_swarms(generator: u32) -> bool { generator == DIKE_SWARM_GENERATOR || carved_surface(generator) }
/// Generator 300 keeps the 298 terrain and the 296 lowland rivers and cuts
/// slope-aligned gullies, rills and terracettes below the erosion tiles.
pub fn carved_surface(generator: u32) -> bool {
    generator == CARVED_SURFACE_GENERATOR || generator == erratics::ERRATIC_RELIEF_GENERATOR || generator == erratics::ERRATIC_DEPOSIT_GENERATOR || generator == dunes::VERSION || generator == dunes::COHERENT_DUNES_GENERATOR || generator == dunes::DEPOSIT_GENERATOR || generator == dunes::ENVELOPE_GENERATOR || rock_relief(generator)
}
/// Generator 304 keeps the 300 terrain and lets the solved beds raise
/// hard-bed risers and soft benches (`rock_relief.rs`). It has no 303 dunes.
pub fn rock_relief(generator: u32) -> bool { generator == ROCK_RELIEF_GENERATOR }

/// True where a dike of generator 298 fills this voxel of solid ground.
pub fn dike_at(cfg: &WorldSampleConfig, p: glam::IVec3, column: &ColumnSample) -> bool {
    dike_swarms(cfg.generator_version) && active(cfg) && dikes::cuts(cfg, p, column.height)
}
/// Generator 293 keeps the 290 terrain and carves ravines, fissures and sinkholes.
/// Generator 296 keeps the 290 terrain and lets lowland rivers sit in their plain.
pub fn lowland_rivers(generator: u32) -> bool { generator == LOWLAND_RIVER_GENERATOR || carved_surface(generator) }
pub fn ground_breaks(generator: u32) -> bool { generator == GROUND_BREAK_GENERATOR || rift_valleys(generator) }
/// Generator 289 adds wave-driven sandy shorelines and rock-grain coasts.
pub fn process_coasts(generator: u32) -> bool { generator == PROCESS_COAST_GENERATOR }

/// Full length of the deep-time history, and the length the geology snapshot
/// format stores. Owned by `engine_voxel`, which also bounds the slider.
pub const DEEP_TIME_MYR: u16 = engine_voxel::FULL_HISTORY_MYR;

/// Simulated history length for a world, in Myr. This is the one source.
///
/// Deep-time planets take, in order: the `VOXEL_HISTORY_MYR` development
/// override, the world's own choice (`PlanetSettings::history_myr`, set by the
/// history slider and kept in the save), then the full `DEEP_TIME_MYR`. Earlier
/// generators always run their fixed 240 Myr drift.
pub fn history_duration_myr(cfg: &WorldSampleConfig) -> u16 {
    if !deep_time(cfg.generator_version) { return DRIFT_MYR; }
    history_override_myr()
        .or_else(|| cfg.planet.and_then(engine_voxel::PlanetSettings::history_myr))
        .unwrap_or(DEEP_TIME_MYR)
}

/// True when the world runs the full history the geology snapshot was built for.
pub fn full_history(cfg: &WorldSampleConfig) -> bool { history_duration_myr(cfg) == DEEP_TIME_MYR }

/// Development only: `VOXEL_HISTORY_MYR=20` shortens every deep-time history in
/// this process so a planet solves in seconds. It wins over the world's own
/// choice and is not stored, so such a world must not be kept as a save.
/// Snapped to the range and step the history slider allows; read once per process.
pub fn history_override_myr() -> Option<u16> {
    static OVERRIDE: std::sync::OnceLock<Option<u16>> = std::sync::OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        let myr = std::env::var("VOXEL_HISTORY_MYR").ok()?.trim().parse::<u16>().ok()?;
        let myr = engine_voxel::PlanetSettings::snap_history(myr);
        (myr != DEEP_TIME_MYR).then(|| {
            eprintln!("[history] VOXEL_HISTORY_MYR: {myr} Myr development history (not save compatible)");
            myr
        })
    })
}
pub mod ecoregion;

pub fn landscape_cycles(generator: u32) -> bool { generator == LANDSCAPE_CYCLES_GENERATOR || deep_time(generator) }

fn branching_watersheds(generator: u32) -> bool {
    generator == BRANCHING_WATERSHEDS_GENERATOR || landscape_cycles(generator)
}

fn changing_seas(generator: u32) -> bool {
    fine_history(generator) || branching_watersheds(generator) || generator == CHANGING_SEAS_GENERATOR
}

fn resolved_history(generator: u32) -> bool {
    generator == RESOLVED_HISTORY_GENERATOR || changing_seas(generator)
}

fn geological_history(generator: u32) -> bool {
    generator == GEOLOGICAL_HISTORY_GENERATOR || evolved_watersheds(generator)
}

fn evolved_watersheds(generator: u32) -> bool {
    generator == EVOLVED_WATERSHEDS_GENERATOR || resolved_history(generator)
}

fn coupled_history(generator: u32) -> bool {
    geological_history(generator) || matches!(generator, COUPLED_RIFT_GENERATOR | LAYERED_CANYON_GENERATOR)
}

fn rifted_margins(generator: u32) -> bool {
    generator == RIFTED_MARGIN_GENERATOR || coupled_history(generator)
}

fn connected_relief(generator: u32) -> bool {
    generator == CONNECTED_RELIEF_GENERATOR || coupled_history(generator)
}

fn refined_drainage(generator: u32) -> bool {
    connected_relief(generator) || matches!(generator, REFINED_DRAINAGE_GENERATOR | RIFTED_MARGIN_GENERATOR)
}

pub(crate) fn watershed_erosion(generator: u32) -> bool {
    refined_drainage(generator) || matches!(generator, WATERSHED_EROSION_GENERATOR | VOLCANIC_SECTOR_GENERATOR | CONNECTED_WATERSHEDS_GENERATOR)
}

fn evolved_volcanoes(generator: u32) -> bool {
    refined_drainage(generator) || matches!(generator, VOLCANIC_SECTOR_GENERATOR | CONNECTED_WATERSHEDS_GENERATOR)
}

fn uses_headwater_relief(generator: u32) -> bool {
    generator == HEADWATER_RELIEF_GENERATOR || composed_drainage(generator)
}

fn composed_drainage(generator: u32) -> bool {
    generator == DRAINAGE_RELIEF_GENERATOR || watershed_erosion(generator)
}

fn uses_fault_history(generator: u32) -> bool {
    uses_catchment_trails(generator) || matches!(generator, FAULT_HISTORY_GENERATOR | WEATHERING_HISTORY_GENERATOR | UPLAND_RUNOFF_GENERATOR | JOINTED_CANYON_GENERATOR | SEDIMENT_WEATHERING_GENERATOR | FOLDED_SCARP_GENERATOR | BEDROCK_SCARP_GENERATOR)
}

fn uses_catchment_trails(generator: u32) -> bool {
    matches!(generator, CATCHMENT_EXPLORATION_GENERATOR | VALLEY_FORM_GENERATOR | SILL_EROSION_GENERATOR)
}

fn uses_mapped_canyon(generator: u32) -> bool {
    generator == MAPPED_CANYON_GENERATOR || uses_fault_history(generator)
}

fn uses_landscape_history(generator: u32) -> bool {
    generator == LANDSCAPE_HISTORY_GENERATOR || uses_mapped_canyon(generator)
}
const SEA: f64 = crate::WATER_LEVEL_METERS as f64;
const SOURCE_AREA: f64 = 750_000.0;
const REACH: f64 = 110.0;
const BED_INTERVAL: f64 = 48.0;

/// Fit the upper land into the playable volume while preserving the order
/// and slope of high terrain. Match the lowland height and derivative at
/// the join so the fit cannot create a contour or a false drainage divide.
fn fit_mountain_height(height: f64) -> f64 {
    const SHOULDER_M: f64 = 312.0;
    const HEADROOM_M: f64 = 176.0;
    let rise = height - SHOULDER_M;
    if rise <= 0.0 {
        height.max(-58.0)
    } else {
        SHOULDER_M - HEADROOM_M * (-rise / HEADROOM_M).exp_m1()
    }
}

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && cfg.generator_version >= VERSION
}

pub fn tidal_coasts(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && cfg.generator_version >= engine_voxel::COASTAL_DELTA_GENERATOR
}

pub fn drift_active(cfg: &WorldSampleConfig) -> bool {
    cfg.world_type == WorldType::Planet && cfg.generator_version >= engine_voxel::CONTINENTAL_DRIFT_GENERATOR
}

struct World {
    mesh: Arc<topology::Mesh>,
    height: Vec<f64>,
    sediment_cover: Vec<f64>,
    history_top: Vec<f64>,
    flow: hydrology::Flow,
    rain: Vec<f64>,
    temperature: Vec<f64>,
    winter: Vec<f64>,
    carbonate: Vec<f64>,
    collision: Vec<f64>,
    rift: Vec<f64>,
    fault_damage: Vec<f64>,
    offshore_trench: Vec<f64>,
    fault_offset: Vec<f64>,
    aquifer: Vec<f64>,
    former_ice: Vec<f64>,
    /// Gradient of fold, fault and rift intensity per node (289 only); the
    /// rock grain runs across it.
    structure_gradient: Vec<DVec3>,
    river_nodes: Vec<DVec3>,
    alluvium: Option<alluvium::Network>,
    gradient: Vec<DVec3>,
    plates: tectonics::Plates,
    volcanoes: volcanism::Field,
    bare_height: Vec<f64>,
    edifice_lift: Vec<f64>,
    edifice_gradient: Vec<DVec3>,
    volcanic_delta: Vec<f64>,
    eroded_region: Option<review::catchment::PlayableRegion>,
    /// Rift-valley offset each node carried into the solve (297 only).
    rift_valley: Vec<f64>,
    /// How much of the full rift section each node kept after the sea-floor limit.
    rift_scale: Vec<f64>,
    slope_tiles: slope_relief::Tiles,
    checksum: u64,
}

struct Slot {
    world: OnceLock<Arc<World>>,
    entry: Arc<geo::Entry>,
}
type Key = (u32, u64, u32);
static WORLDS: Mutex<Vec<(Key, Arc<Slot>)>> = Mutex::new(Vec::new());
thread_local! {
    static HOT: RefCell<Option<(Key, Arc<Slot>)>> = const { RefCell::new(None) };
    static HISTORY_ROCK: RefCell<Option<((Key, [u64; 3]), drift::history::RockColumn)>> = const { RefCell::new(None) };
}

fn slot(cfg: &WorldSampleConfig) -> Arc<Slot> {
    let key = (cfg.seed, cfg.planet_fingerprint(), cfg.generator_version);
    if let Some(s) = HOT.with(|h| h.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, s)| Arc::clone(s))) {
        return s;
    }
    let mut cache = WORLDS.lock().unwrap_or_else(|e| e.into_inner());
    let s = if let Some((_, s)) = cache.iter().find(|(k, _)| *k == key) { Arc::clone(s) }
    else {
        // In-flight callers retain their slot; eviction never invalidates a reader.
        if cache.len() >= 3 { cache.remove(0); }
        let s = Arc::new(Slot { world: OnceLock::new(), entry: Arc::new(geo::Entry::default()) });
        cache.push((key, Arc::clone(&s)));
        s
    };
    HOT.with(|h| *h.borrow_mut() = Some((key, Arc::clone(&s))));
    s
}

pub fn entry(cfg: &WorldSampleConfig) -> Arc<geo::Entry> { Arc::clone(&slot(cfg).entry) }

fn get(cfg: &WorldSampleConfig) -> Arc<World> {
    solve_in(cfg, &slot(cfg))
}

pub(crate) fn trail_reserves(cfg: &WorldSampleConfig, x: f32, z: f32, radius: f32) -> bool {
    uses_catchment_trails(cfg.generator_version)
        && active(cfg)
        && get(cfg).eroded_region.as_ref().is_some_and(|region| {
            region.trail_overlaps(direction(cfg, x.into(), z.into()), radius.into())
        })
}

fn solve_in(cfg: &WorldSampleConfig, slot: &Slot) -> Arc<World> {
    Arc::clone(slot.world.get_or_init(|| {
        let mut world = build(cfg, &slot.entry.progress, true);
        let construction = world.plates.config();
        world.slope_tiles.attach_cache(&construction, world.plates.shape_shelf, world.checksum);
        let world = Arc::new(world);
        slot.entry.install(Arc::new(world.regional_view()));
        world
    }))
}

pub(crate) fn prewarm(cfg: WorldSampleConfig) {
    let slot = slot(&cfg);
    if !slot.entry.ready() && slot.entry.claim_prewarm() {
        // The loading screen watches this slot even if newer recipes evict it.
        let worker = Arc::clone(&slot);
        if std::thread::Builder::new().name("planet-solve".into())
            .spawn(move || drop(solve_in(&cfg, &worker))).is_err()
        {
            slot.entry.release_prewarm();
        }
    }
}

pub fn regional_view(cfg: &WorldSampleConfig) -> Arc<geo::Planet> {
    let _ = get(cfg);
    entry(cfg).installed().expect("spherical solve publishes its regional view")
}

pub fn direction(cfg: &WorldSampleConfig, x: f64, z: f64) -> DVec3 {
    let (face, p) = atlas::split(DVec3::new(x, 0.0, z));
    let _ = cfg;
    Projection::Tangent.to_sphere(face, p.x / atlas::HALF_M, p.z / atlas::HALF_M)
}

fn mesh(generator: u32) -> Arc<topology::Mesh> {
    if connected_relief(generator) {
        static CONNECTED: OnceLock<Arc<topology::Mesh>> = OnceLock::new();
        return Arc::clone(CONNECTED.get_or_init(|| Arc::new(topology::Mesh::irregular(224))));
    }
    if generator == RIFTED_MARGIN_GENERATOR {
        static MARGINS: OnceLock<Arc<topology::Mesh>> = OnceLock::new();
        return Arc::clone(MARGINS.get_or_init(|| Arc::new(topology::Mesh::with_side(224))));
    }
    if refined_drainage(generator) {
        static REFINED: OnceLock<Arc<topology::Mesh>> = OnceLock::new();
        return Arc::clone(REFINED.get_or_init(|| Arc::new(topology::Mesh::with_side(192))));
    }
    static MESH: OnceLock<Arc<topology::Mesh>> = OnceLock::new();
    Arc::clone(MESH.get_or_init(|| Arc::new(topology::Mesh::new())))
}

fn map_vertices<T: Send>(count: usize, workers: usize, sample: impl Fn(usize) -> T + Sync) -> Vec<T> {
    map_ordered(count, workers.min(count.div_ceil(16_384)), sample)
}

/// History meshes are small (about 42k nodes) and are mapped every plate
/// step, so they use finer ordered batches than the final planet mesh.
fn map_history<T: Send>(count: usize, workers: usize, sample: impl Fn(usize) -> T + Sync) -> Vec<T> {
    map_ordered(count, workers.min(count.div_ceil(2_048)), sample)
}

/// Ordered in-place variant of `map_history`: each element depends only on
/// its index, so results do not depend on the worker count.
fn fill_history<T: Send>(out: &mut [T], workers: usize, sample: impl Fn(usize, &mut T) + Sync) {
    let count = out.len();
    let workers = workers.min(count.div_ceil(2_048)).max(1);
    if workers <= 1 {
        for (i, value) in out.iter_mut().enumerate() { sample(i, value); }
        return;
    }
    let batch = count.div_ceil(workers);
    std::thread::scope(|scope| {
        let sample = &sample;
        let mut chunks = out.chunks_mut(batch).enumerate();
        let first = chunks.next();
        for (c, chunk) in chunks {
            scope.spawn(move || for (k, value) in chunk.iter_mut().enumerate() { sample(c * batch + k, value); });
        }
        if let Some((_, chunk)) = first {
            for (k, value) in chunk.iter_mut().enumerate() { sample(k, value); }
        }
    });
}

/// Only independent work uses this path. Joining contiguous batches in
/// index order preserves deterministic terrain and overlap tie breaks.
fn map_ordered<T: Send>(count: usize, workers: usize, sample: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let workers = workers.min(count);
    if workers <= 1 { return (0..count).map(sample).collect(); }
    let batch = count.div_ceil(workers);
    std::thread::scope(|scope| {
        let sample = &sample;
        let mut jobs = Vec::with_capacity(workers - 1);
        for start in (batch..count).step_by(batch) {
            jobs.push(scope.spawn(move || (start..(start + batch).min(count)).map(sample).collect::<Vec<_>>()));
        }
        let mut result = Vec::with_capacity(count);
        result.extend((0..batch).map(sample));
        for job in jobs { result.append(&mut job.join().expect("planet vertex worker panicked")); }
        result
    })
}

fn build(cfg: &WorldSampleConfig, progress: &geo::Progress, shape_shelf: bool) -> World {
    let started = std::time::Instant::now();
    progress.set_stage(geo::Stage::Plates);
    // Reserve a core for streaming and presentation on small machines.
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1).clamp(1, 4));
    let mut plates = tectonics::Plates::new(cfg);
    plates.shape_shelf = shape_shelf;
    if drift_active(cfg) && !geology_cache::restore(&mut plates, progress) {
        plates.evolve(progress);
        geology_cache::store(&plates, progress);
    }
    let drift_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mesh = mesh(cfg.generator_version);
    let provinces = map_vertices(mesh.directions.len(), workers, |i| plates.at(mesh.directions[i]));
    let offshore_trench = if changing_seas(cfg.generator_version) {
        provinces.iter().map(|p| p.trench).collect()
    } else { Vec::new() };
    let mut height: Vec<_> = provinces.iter().map(|p| p.height).collect();
    let (rift_valley, rift_scale) = if rift_valleys(cfg.generator_version) {
        let mut offsets = map_vertices(height.len(), workers,
            |i| plates.rift_frame(mesh.directions[i]).map_or(0.0, |frame| frame.solve()));
        // A floor stops a little under the sea: a flooded rift is a shallow
        // gulf or lake, not a trench at the bottom of the world.
        let scale: Vec<_> = height.iter().zip(&offsets).map(|(&h, &offset)| {
            if offset < 0.0 { ((h - (SEA - rift_valley::SEA_FLOOR_M)) / -offset).clamp(0.0, 1.0) } else { 1.0 }
        }).collect();
        for ((h, offset), k) in height.iter_mut().zip(&mut offsets).zip(&scale) {
            *offset *= k;
            *h += *offset;
        }
        (offsets, scale)
    } else { (Vec::new(), Vec::new()) };
    let history_top = if geological_history(cfg.generator_version) { height.clone() } else { Vec::new() };
    let coupled = coupled_history(cfg.generator_version);
    let layered_canyons = cfg.generator_version == LAYERED_CANYON_GENERATOR;
    let mut sediment_cover: Vec<_> = if coupled {
        mesh.directions.iter().map(|&dir| plates.crust_at(dir).map_or(0.0, |c| c.sediment_m)).collect()
    } else { Vec::new() };
    let carbonate: Vec<_> = provinces.iter().map(|p| p.carbonate).collect();
    let collision: Vec<_> = provinces.iter().map(|p| p.collision).collect();
    let rift: Vec<_> = provinces.iter().map(|p| p.rift).collect();
    let fault_damage: Vec<_> = if plates.faults() {
        let (mut low, mut high, mut damaged) = (0.0_f64, 0.0_f64, 0);
        for p in &provinces {
            low = low.min(p.fault_displacement_m);
            high = high.max(p.fault_displacement_m);
            damaged += usize::from(p.fault_damage > 0.35);
        }
        if let Some((i, p)) = provinces.iter().enumerate().filter(|(_, p)| p.height > SEA + 12.0)
            .max_by(|(_, a), (_, b)| a.fault_displacement_m.total_cmp(&b.fault_displacement_m))
        {
            let at = atlas::from_body(mesh.directions[i] * atlas::RADIUS_M);
            eprintln!("[worldgen] faults nodes={damaged} displacement_m={low:.2}..{high:.2} candidate_x={:.2} candidate_z={:.2} candidate_uplift_m={:.2}",
                at.x, at.z, p.fault_displacement_m);
        }
        provinces.iter().map(|p| p.fault_damage).collect()
    } else { Vec::new() };
    let fault_offset = if plates.faults() {
        provinces.iter().map(|p| p.fault_displacement_m).collect()
    } else { Vec::new() };
    let mut volcanoes = volcanism::Field::new(cfg, &plates);
    let edifices = if cfg.generator_version >= engine_voxel::HOTSPOT_PLANET_GENERATOR {
        map_vertices(height.len(), workers, |i| volcanoes.edifice(mesh.directions[i]))
    } else {
        Vec::new()
    };
    for (h, edifice) in height.iter_mut().zip(&edifices) { *h += edifice.lift; }
    progress.set_stage(geo::Stage::Climate);
    progress.set_phase(0, 2);
    let settings = cfg.planet_settings();
    let profile = crate::planet_profile::profile(cfg.seed);
    let mut rain = Vec::with_capacity(height.len());
    let mut temperature = Vec::with_capacity(height.len());
    let mut winter = Vec::with_capacity(height.len());
    let climate = map_vertices(height.len(), workers, |i| {
        let d = mesh.directions[i];
        let land = ((height[i] - SEA) / 200.0).clamp(0.0, 1.0);
        let mut c = crate::planet_climate::at(cfg.seed, &PlanetAxis::EARTHLIKE, d, land);
        if let Some(s) = settings {
            let swing = c.winter_c - c.annual_c;
            c.annual_c += (s.unit(4) - 0.5) * 40.0 - f64::from(profile.temperature - 0.5) * 10.0;
            c.winter_c = c.annual_c + swing * (s.unit(6) * 47.0).to_radians().sin() / 0.3978;
            c.wet = ((c.wet - f64::from(profile.rainfall - 0.5) * 0.3) * s.unit(5) * 2.0).clamp(0.0, 1.0);
        }
        if deep_time(cfg.generator_version) {
            let anomaly = plates.climate_anomaly(d);
            c.annual_c += anomaly;
            c.winter_c += anomaly;
        }
        // Zonal wind reverses in the temperate belt. Moisture crosses charts in body space.
        let east = DVec3::Y.cross(d).normalize_or_zero();
        let wind = if d.y.abs() > 0.42 && d.y.abs() < 0.82 { east } else { -east };
        let upstream = (d - wind * (2200.0 / atlas::RADIUS_M)).normalize();
        let rise = height[i] - mesh.sample(&height, upstream);
        c.wet = (c.wet + (rise / 320.0).clamp(-0.3, 0.3)).clamp(0.0, 1.0);
        c
    });
    for c in climate {
        rain.push(c.wet);
        temperature.push(c.annual_c);
        winter.push(c.winter_c);
    }
    progress.set_phase(1, 2);
    if resolved_history(cfg.generator_version) {
        volcanoes.bake_reefs(Arc::clone(&mesh), &mut height, &rain, &temperature, fine_history(cfg.generator_version));
        for (i, sediment) in sediment_cover.iter_mut().enumerate() {
            *sediment += volcanoes.reef_sediment(mesh.directions[i]);
        }
    }
    let publish = |revision, height: &[f64]| {
        if !layered_canyons { return; }
        progress.publish_atlas(SolveAtlas::sample(revision, |dir| {
            let elevation = mesh.sample(height, dir);
            let sample = crate::planet_climate::Climate {
                annual_c: mesh.sample(&temperature, dir),
                winter_c: mesh.sample(&winter, dir),
                wet: mesh.sample(&rain, dir),
            }.lifted((elevation - SEA).max(0.0));
            SolveTexel { height_m: (elevation - SEA) as f32, wet: sample.wet as f32,
                annual_c: sample.annual_c as f32, winter_c: sample.winter_c as f32,
                slope: 0.0, inland_depth_m: 0.0 }
        }));
    };
    publish(0, &height);
    progress.set_stage(geo::Stage::Carving);
    // Display-only pass counter; each mark follows one finished pass.
    const CARVING_PASSES: u32 = 5;
    progress.set_phase(0, CARVING_PASSES);
    let erosion_start = std::time::Instant::now();
    let mut erodibility: Vec<_> = carbonate.iter().zip(&collision).map(|(c, m)| 0.65 + c * 0.7 - m.min(1.0) * 0.25).collect();
    for (rock, damage) in erodibility.iter_mut().zip(&fault_damage) {
        *rock *= 1.0 + 0.55 * damage;
    }
    for (rock, edifice) in erodibility.iter_mut().zip(&edifices) {
        // Resurfacing limits incision on young cones. Runoff has longer to
        // cut through the older flows and export their sediment.
        let cover = smooth(10.0, 58.0, edifice.lift);
        *rock += (0.32 + 4.6 * edifice.exposure - *rock) * cover;
    }
    let strength = settings.map_or(1.0, |s| 0.25 + s.unit(3) * 1.5);
    let alluvial = cfg.generator_version >= engine_voxel::ALLUVIAL_PLANET_GENERATOR;
    let mut sediment = if alluvial { vec![0.0; height.len()] } else { Vec::new() };
    let rifted = rifted_margins(cfg.generator_version);
    let history_height = coupled.then(|| height.clone());
    let mut coast_export = hydrology::erode(&mesh, &mut height, &rain, &erodibility, SEA, strength,
        cfg.generator_version >= engine_voxel::TALUS_PLANET_GENERATOR, &mut sediment,
        tidal_coasts(cfg) || matches!(cfg.generator_version, COUPLED_SEDIMENT_GENERATOR | DELTA_ISLANDS_GENERATOR
            | MOBILE_DELTA_GENERATOR),
        (rifted || uses_landscape_history(cfg.generator_version)).then(|| epochs::History::new(&temperature, &rain)));
    let erosion_ms = erosion_start.elapsed().as_secs_f64() * 1000.0;
    progress.set_phase(1, CARVING_PASSES);
    let bare_height = height.clone();
    if let Some(before) = history_height {
        for ((cover, after), before) in sediment_cover.iter_mut().zip(&height).zip(before) {
            *cover = (*cover + after - before).max(0.0);
        }
    }
    let edifice_lift: Vec<_> = edifices.into_iter().map(|s| s.lift).collect();
    let flank_start = std::time::Instant::now();
    if cfg.generator_version >= engine_voxel::VOLCANIC_LANDFORM_PLANET_GENERATOR {
        volcanoes.bake_landforms(
            cfg,
            workers,
            |dir| mesh.sample(&bare_height, dir) - mesh.sample(&edifice_lift, dir),
            |dir| mesh.sample(&rain, dir),
        );
    }
    let flank_ms = flank_start.elapsed().as_secs_f64() * 1000.0;
    progress.set_phase(2, CARVING_PASSES);
    let volcanic_delta = map_vertices(height.len(), workers, |i| {
        let d = mesh.directions[i];
        volcanoes.shape(d, bare_height[i], |dir| mesh.sample(&bare_height, dir)).metres - bare_height[i]
    });
    let fit_highlands = uses_headwater_relief(cfg.generator_version) || cfg.generator_version == TECTONIC_TRAVERSE_GENERATOR;
    for (h, delta) in height.iter_mut().zip(&volcanic_delta) {
        let top = *h + delta;
        *h = if fit_highlands { fit_mountain_height(top) } else { top.clamp(-58.0, 495.0) };
    }
    for (cover, delta) in sediment_cover.iter_mut().zip(&volcanic_delta) {
        if *delta > 0.5 { *cover = 0.0; }
    }
    progress.set_phase(3, CARVING_PASSES);
    let history_height = coupled.then(|| height.clone());
    let refined = refined_drainage(cfg.generator_version);
    let connected = connected_relief(cfg.generator_version);
    let former_ice = if landscape_cycles(cfg.generator_version) {
        mesh.directions.iter().map(|&dir| plates.former_ice(dir)).collect()
    } else if refined {
        glacial::carve(&mesh, &mut height, &rain, &temperature)
    } else { Vec::new() };
    if rifted && !coupled {
        coastal_erosion::evolve(&mesh, &mut height, &erodibility, strength);
    }
    if refined || cfg.generator_version == CONNECTED_WATERSHEDS_GENERATOR {
        let beds = layered_canyons.then(|| map_vertices(height.len(), workers, |i| {
            strata::Column::new(cfg.seed, mesh.directions[i] * atlas::RADIUS_M, collision[i],
                carbonate[i], rift[i], fault_offset.get(i).copied().unwrap_or(0.0))
        }));
        let export = hydrology::evolve_watersheds(&mesh, &mut height, &rain, &erodibility, SEA,
            strength, &mut sediment, connected, beds.as_deref());
        if let Some(total) = &mut coast_export {
            for (old, new) in total.iter_mut().zip(export) { *old += new; }
        } else {
            coast_export = Some(export);
        }
    }
    progress.set_phase(4, CARVING_PASSES);
    if let Some(before) = history_height {
        for ((cover, after), before) in sediment_cover.iter_mut().zip(&height).zip(before) {
            *cover = (*cover + after - before).max(0.0);
        }
        coastal_erosion::transport(&mesh, &mut height, &mut sediment_cover, &erodibility, strength, 6);
    }
    if process_coasts(cfg.generator_version) {
        // Before routing, so rivers find the mouths that spits and capes leave.
        shoreline::evolve(&mesh, &mut height, &mut sediment_cover, strength, workers);
    }
    publish(1, &height);
    progress.set_stage(geo::Stage::Rivers);
    const RIVER_PASSES: u32 = 6;
    progress.set_phase(0, RIVER_PASSES);
    let routing_start = std::time::Instant::now();
    let history_height = coupled.then(|| height.clone());
    let mut flow = hydrology::route_landscape(&mesh, &height, &rain, SEA, connected);
    if cfg.generator_version >= engine_voxel::RIVER_MORPHOLOGY_GENERATOR {
        let mut breached = 0;
        for _ in 0..2 {
            let count = hydrology::breach_shallow_basins(&mesh, &mut height, &flow, &erodibility, &mut sediment, SEA);
            if count == 0 { break; }
            breached += count;
            flow = hydrology::route_landscape(&mesh, &height, &rain, SEA, connected);
        }
        eprintln!("[worldgen] shallow_spillways breached={breached}");
    }
    let lake_depth = settings.map_or(30.0, |s| 2.0 + s.unit(11) * 56.0);
    let mut lake_bed_changed = false;
    for i in 0..height.len() {
        if height[i] > SEA && flow.spill[i] > height[i] + lake_depth {
            height[i] = flow.spill[i] - lake_depth;
            lake_bed_changed = true;
        }
    }
    // The final receiver and discharge fields describe the final lake beds.
    if lake_bed_changed { flow = hydrology::route_landscape(&mesh, &height, &rain, SEA, connected); }
    if let Some(before) = history_height {
        for ((cover, after), before) in sediment_cover.iter_mut().zip(&height).zip(before) {
            *cover = (*cover + after - before).max(0.0);
        }
    }
    let routing_ms = routing_start.elapsed().as_secs_f64() * 1000.0;
    progress.set_phase(1, RIVER_PASSES);
    let gradient = map_vertices(height.len(), workers, |i| terrain_gradient(&mesh, &height, i));
    let edifice_gradient = map_vertices(edifice_lift.len(), workers, |i| {
        terrain_gradient(&mesh, &edifice_lift, i)
    });
    let river_nodes = if refined {
        alluvium::channel_anchors(&mesh, &height, &flow, &erodibility, cfg.seed)
    } else {
        map_vertices(height.len(), workers, |i| {
            let d = mesh.directions[i];
            let jitter = DVec3::new(noise(cfg.seed, d * 110.0, 901), noise(cfg.seed, d * 110.0, 907),
                noise(cfg.seed, d * 110.0, 911)) - 0.5;
            (d + jitter * (35.0 / atlas::RADIUS_M)).normalize() * atlas::RADIUS_M
        })
    };
    progress.set_phase(2, RIVER_PASSES);
    let alluvium = alluvial.then(|| {
        let gold = (cfg.generator_version >= engine_voxel::PLACER_PLANET_GENERATOR)
            .then(|| placer::route(cfg.seed, &mesh, &height, &flow, &sediment, &collision, &rift));
        let mobile = cfg.generator_version >= engine_voxel::MOBILE_RIVERBED_GENERATOR;
        // The coastal export array only contains deliveries at the sea.
        // Inlet gravel is supplied by the contributing inland catchment.
        let inland_load = (mobile && coast_export.is_some()).then(|| {
            let mut load = sediment.clone();
            hydrology::deliver_sediment(&flow, &height, &mut load);
            load
        });
        if coast_export.is_none() {
            hydrology::deliver_sediment(&flow, &height, &mut sediment);
        }
        let supply = coast_export.as_deref().unwrap_or(&sediment);
        let gravel = if mobile { inland_load.as_deref().unwrap_or(&sediment) } else { &[] };
        alluvium::Network::build(&mesh, &river_nodes, &height, &flow, supply, gravel, &erodibility,
            gold.as_deref().unwrap_or(&[]), cfg.generator_version, cfg.seed)
    });
    progress.set_phase(3, RIVER_PASSES);
    let mut checksum = 0xcbf29ce484222325u64;
    for field in [&height, &rain, &temperature, &winter, &flow.spill, &flow.discharge] {
        for &v in field { checksum = (checksum ^ v.to_bits()).wrapping_mul(0x100000001b3); }
    }
    for &r in &flow.receiver { checksum = (checksum ^ r as u64).wrapping_mul(0x100000001b3); }
    for volume in sediment { checksum = (checksum ^ volume.to_bits()).wrapping_mul(0x100000001b3); }
    if let Some(export) = coast_export {
        for volume in export { checksum = (checksum ^ volume.to_bits()).wrapping_mul(0x100000001b3); }
    }
    if let Some(network) = &alluvium { checksum = network.morphology_checksum(checksum); }
    if cfg.generator_version >= engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR {
        checksum = (checksum ^ u64::from(engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR)).wrapping_mul(0x100000001b3);
    }
    if cfg.generator_version == DRAINAGE_RELIEF_GENERATOR {
        checksum = (checksum ^ u64::from(DRAINAGE_RELIEF_GENERATOR)).wrapping_mul(0x100000001b3);
    }
    let (reaches, deltas) = alluvium.as_ref().map_or((0, 0), alluvium::Network::counts);
    let aquifer = if watershed_erosion(cfg.generator_version) {
        let channel_head = alluvium::channel_levels(&river_nodes, &height, &flow, cfg.generator_version);
        aquifer_head_field(&mesh, &height, &flow, &rain, &carbonate, &channel_head)
    } else {
        Vec::new()
    };
    for &head in &aquifer {
        checksum = (checksum ^ head.to_bits()).wrapping_mul(0x100000001b3);
    }
    for &ice in &former_ice {
        checksum = (checksum ^ ice.to_bits()).wrapping_mul(0x100000001b3);
    }
    for cover in &sediment_cover { checksum = (checksum ^ cover.to_bits()).wrapping_mul(0x100000001b3); }
    checksum = plates.history_checksum(checksum);
    let structure_gradient = if process_coasts(cfg.generator_version) {
        let structure: Vec<f64> = (0..collision.len()).map(|i| collision[i].min(1.5) + 0.6 * rift[i].min(1.0)
            + fault_damage.get(i).copied().unwrap_or(0.0)).collect();
        map_vertices(structure.len(), workers, |i| terrain_gradient(&mesh, &structure, i))
    } else { Vec::new() };
    let mut world = World { mesh, height, sediment_cover, history_top, flow, rain, temperature, winter, carbonate, collision, rift, river_nodes, alluvium, gradient, offshore_trench,
        plates, volcanoes, bare_height, edifice_lift, edifice_gradient, volcanic_delta, fault_damage, fault_offset, aquifer, former_ice, structure_gradient,
        eroded_region: None, rift_valley, rift_scale, slope_tiles: slope_relief::Tiles::default(), checksum };
    progress.set_phase(4, RIVER_PASSES);
    let riverbed_start = std::time::Instant::now();
    if cfg.generator_version >= engine_voxel::MOBILE_RIVERBED_GENERATOR
        && let Some(network) = &world.alluvium
    {
        let rock_strength = (cfg.generator_version >= engine_voxel::BEDROCK_RIVER_GENERATOR)
            .then_some(|dir, height| match rock_in(&world, cfg, dir, height) {
                BlockId::CoalOre => 0.75,
                BlockId::Sandstone => 1.25,
                BlockId::Limestone => 1.65,
                BlockId::Basalt => 2.60,
                _ => 1.90,
            });
        let valley_ground = |dir| {
            if watershed_erosion(cfg.generator_version) {
                // The river solve owns its floodplain. Local slope tiles
                // sample the finished valley when chunk generation needs them.
                world.ground_base(cfg, dir, 1.0, world.volcano_shape(dir))
            } else {
                world.ground(cfg, dir, 1.0)
            }
        };
        let tiles = network.bake_riverbeds(workers, valley_ground, rock_strength);
        let scrolls = cfg.generator_version >= engine_voxel::FLOODPLAIN_SCROLL_GENERATOR;
        let bend = (cfg.generator_version >= engine_voxel::FLOODPLAIN_MEANDER_GENERATOR)
            .then(|| network.bake_meander(scrolls, valley_ground)).flatten();
        for tile in &tiles {
            let p = atlas::from_body(tile.origin);
            eprintln!("[worldgen] inland_river x={:.1} z={:.1} split_sections={} bank_m3={:.1} headcut_m3={:.1} incised_m3={:.1} water_error_m3={:.6} sediment_error_m3={:.6}",
                p.x, p.z, tile.split_sections, tile.bank_m3, tile.headcut_m3, tile.incision_m3, tile.water_error_m3, tile.sediment_error_m3);
        }
        if let Some(network) = &mut world.alluvium {
            world.checksum = network.install_riverbeds(tiles, world.checksum);
            if let Some(tile) = bend {
                let neck = atlas::from_body(tile.contact);
                let loop_ = atlas::from_body(tile.oxbow);
                eprintln!("[worldgen] meander_cutoff neck_x={:.1} neck_z={:.1} loop_x={:.1} loop_z={:.1} migration_m={:.1} retired_m={:.1} shortcut_m={:.1} episodes={} isolated_m2={:.1}",
                    neck.x, neck.z, loop_.x, loop_.z, tile.migration, tile.retired, tile.shortcut, tile.episodes, tile.isolated_area);
                world.checksum = network.install_meander(tile, world.checksum);
            }
        }
    }
    let riverbed_ms = riverbed_start.elapsed().as_secs_f64() * 1000.0;
    progress.set_phase(5, RIVER_PASSES);
    if cfg.generator_version == ESTUARY_OUTLOOK_GENERATOR && let Some(network) = &world.alluvium {
        let surveys = network.survey_outlooks(|dir| world.ground(cfg, dir, 1.0));
        if let Some(network) = &mut world.alluvium {
            world.checksum = network.install_outlooks(surveys, world.checksum);
        }
    }
    if cfg.generator_version >= REGIONAL_EROSION_GENERATOR
        && !uses_headwater_relief(cfg.generator_version)
        && cfg.generator_version != TECTONIC_TRAVERSE_GENERATOR
        && !reference_relief::active(cfg)
    {
        world.eroded_region = review::catchment::PlayableRegion::bake(cfg, &world);
        if let Some(region) = &world.eroded_region {
            world.checksum = region.checksum(world.checksum);
        }
    }
    if reference_relief::active(cfg) {
        world.checksum = reference_relief::checksum(world.checksum);
    }
    if layered_canyons { progress.publish_atlas(SolveAtlas::finished(&world, cfg)); }
    eprintln!("[worldgen] spherical seed={} nodes={} alluvial_reaches={reaches} deltas={deltas} solve_ms={:.1} drift_ms={drift_ms:.1} flank_ms={flank_ms:.1} riverbed_ms={riverbed_ms:.1} erosion_ms={erosion_ms:.1} routing_ms={routing_ms:.1} checksum={:016x}",
        cfg.seed, world.height.len(), started.elapsed().as_secs_f64() * 1000.0, world.checksum);
    world
}

fn aquifer_head_field(mesh: &topology::Mesh, height: &[f64], flow: &hydrology::Flow,
    rain: &[f64], carbonate: &[f64], channel_head: &[f64]) -> Vec<f64> {
    let mut head: Vec<_> = height.iter().zip(rain).map(|(&top, &wet)| {
        if top <= SEA { SEA } else { top - 31.0 + 19.0 * wet }
    }).collect();
    let mut next = head.clone();
    for _ in 0..24 {
        for (i, &top) in height.iter().enumerate() {
            next[i] = if top <= SEA {
                SEA
            } else if rain[i] > 0.12 && flow.spill[i] > top + 0.75 {
                top - 4.0
            } else if flow.discharge[i] >= SOURCE_AREA {
                // The incised channel drains the aquifer along its valley.
                channel_head[i] - 4.0
            } else {
                let (mut total, mut conductance) = (0.0, 0.0);
                for &(j, distance) in &mesh.neighbours[i] {
                    let k = (0.6 + 0.5 * (carbonate[i] + carbonate[j])) / distance;
                    total += head[j] * k;
                    conductance += k;
                }
                let recharge = (0.5 + 1.5 * carbonate[i]) * rain[i];
                (total / conductance + recharge).min(top - 7.0)
            };
        }
        std::mem::swap(&mut head, &mut next);
    }
    head
}

pub(crate) fn aquifer_head(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
    let world = get(cfg);
    let dir = direction(cfg, f64::from(cfg.world_x(x)), f64::from(cfg.world_z(z)));
    cfg.blocks_for_height(world.mesh.sample(&world.aquifer, dir) as f32)
}

fn terrain_gradient(mesh: &topology::Mesh, height: &[f64], i: usize) -> DVec3 {
    let d = mesh.directions[i];
    if mesh.is_irregular() {
        let east = d.cross(if d.y.abs() < 0.9 { DVec3::Y } else { DVec3::X }).normalize();
        let north = d.cross(east);
        let mut xx = 0.0;
        let mut zz = 0.0;
        let mut xz = 0.0;
        let mut xh = 0.0;
        let mut zh = 0.0;
        for &(j, _) in &mesh.neighbours[i] {
            let edge = (mesh.directions[j] - d) * atlas::RADIUS_M;
            let x = edge.dot(east);
            let z = edge.dot(north);
            let w = (x * x + z * z).recip();
            let rise = height[j] - height[i];
            xx += w * x * x;
            zz += w * z * z;
            xz += w * x * z;
            xh += w * x * rise;
            zh += w * z * rise;
        }
        return (east * (zz * xh - xz * zh) + north * (xx * zh - xz * xh)) / (xx * zz - xz * xz);
    }
    let mut slope = DVec3::ZERO;
    for &(j, _) in &mesh.neighbours[i] {
        let edge = (mesh.directions[j] - d) * atlas::RADIUS_M;
        let tangent = edge - d * edge.dot(d);
        slope += tangent * ((height[j] - height[i]) / tangent.length_squared());
    }
    slope * (2.0 / mesh.neighbours[i].len() as f64)
}

fn noise(seed: u32, p: DVec3, salt: u32) -> f64 { crate::planet::noise_at(p, seed.wrapping_add(salt)) }

fn rainforest_floor(cfg: &WorldSampleConfig, body: DVec3) -> (f32, f32) {
    let drainage = noise(cfg.seed, body / 26.0, 169_021) as f32;
    let mound = noise(cfg.seed, body / 8.0, 169_027) as f32;
    (crate::trees::rainforest::swale(drainage), mound)
}

fn bedding_height(cfg: &WorldSampleConfig, body: DVec3, collision: f64, elevation: f64) -> f64 {
    let fold = (noise(cfg.seed, body / 700.0, 1201) - 0.5) * (8.0 + collision * 24.0);
    let datum = elevation + fold;
    if cfg.generator_version >= ERODED_RELIEF_GENERATOR {
        let dip = (noise(cfg.seed, body / 230.0, 1207) - 0.5) * 26.0;
        // Vary bed thickness with depth while preserving the order of contacts.
        let thickness = 18.0 * ((datum + dip) * std::f64::consts::TAU / 176.0).sin();
        return (datum + dip + thickness).rem_euclid(BED_INTERVAL);
    }
    datum.rem_euclid(BED_INTERVAL)
}

/// Height change and talus thickness along a weathered sedimentary bed.
fn weathered_bed(seed: u32, body: DVec3, bed: f64, footprint: f64) -> (f64, f64) {
    // The same warped seam widens into a gully and feeds the fan below it.
    let tilt = DVec3::new(0.53, -0.67, 0.52) * (bed / 115.0);
    let seam = (noise(seed, body / 72.0 + tilt, 1229) - 0.51).abs();
    let face = smooth(9.0, 17.0, bed) * (1.0 - smooth(31.0, 44.0, bed));
    let gully = 1.0 - smooth(0.025, 0.13 + (1.0 - bed / BED_INTERVAL) * 0.10, seam);
    let broad = 1.0 - smooth(20.0, 125.0, footprint);
    let detail = 1.0 - smooth(3.0, 23.0, footprint);
    let joint = (noise(seed, body / 19.0 - tilt, 1237) - 0.48).abs();
    let fracture = (1.0 - smooth(0.018, 0.105, joint)) * (1.0 - gully * 0.55);
    let cut = face * (11.0 * gully * broad + 4.0 * fracture * detail);

    let toe = smooth(2.0, 10.0, bed) * (1.0 - smooth(12.0, 22.0, bed));
    let fan = 1.0 - smooth(0.055, 0.27, seam);
    let debris = 0.82 + (noise(seed, body / 7.0, 1249) - 0.5) * 0.55 * detail;
    let scree = toe * fan * broad * (2.4 + 4.0 * gully) * debris;
    (scree - cut, scree)
}

#[derive(Clone, Copy)]
struct Channel { distance: f64, width: f64, level: f64, depth: f64 }

impl World {
    fn surface_query(&self, dir: DVec3) -> surface_query::SurfaceQuery<'_> {
        surface_query::SurfaceQuery::new(self, dir)
    }

    fn history_strata(&self, cfg: &WorldSampleConfig, dir: DVec3) -> Option<drift::history::RockColumn> {
        self.surface_query(dir).history_strata(cfg)
    }

    fn history_strata_prepared(&self, cfg: &WorldSampleConfig,
        point: &surface_query::SurfaceQuery<'_>) -> Option<drift::history::RockColumn>
    {
        let dir = point.direction();
        if self.history_top.is_empty() { return None; }
        let key = ((cfg.seed, cfg.planet_fingerprint(), cfg.generator_version), dir.to_array().map(f64::to_bits));
        if let Some(column) = HISTORY_ROCK.with(|cache| cache.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, c)| *c)) {
            return Some(column);
        }
        let (mut beds, weathered_myr) = self.plates.history_column(dir)?;
        let (local, weights) = point.stencil();
        let sample = |field: &[f64]| (0..4).map(|i| field[local[i]] * weights[i]).sum::<f64>();
        let raw_top = sample(&self.history_top);
        beds.fit(raw_top, fit_mountain_height);
        let historical_top = fit_mountain_height(raw_top);
        let top = sample(&self.height);
        let edifice = point.edifice_lift();
        let lava_m = if edifice > 12.0 {
            (top - fit_mountain_height(point.volcanic_base())).max(0.0)
        } else { 0.0 };
        let sediment_top = top - lava_m;
        let carbonate = sample(&self.carbonate);
        if sediment_top < historical_top {
            beds.trim(historical_top - sediment_top);
        } else {
            beds.deposit(history_duration_myr(cfg), sediment_top - historical_top, sample(&self.rain),
                sample(&self.temperature), carbonate, false, sediment_top < SEA + 2.0);
        }
        let column = drift::history::RockColumn { beds, top, lava_m, carbonate, weathered_myr };
        HISTORY_ROCK.with(|cache| *cache.borrow_mut() = Some((key, column)));
        Some(column)
    }

    fn strata(&self, cfg: &WorldSampleConfig, dir: DVec3) -> strata::Column {
        strata::Column::new(cfg.seed, dir * atlas::RADIUS_M, self.mesh.sample(&self.collision, dir),
            self.mesh.sample(&self.carbonate, dir), self.mesh.sample(&self.rift, dir),
            if self.fault_offset.is_empty() { 0.0 } else { self.mesh.sample(&self.fault_offset, dir) })
    }

    fn source_area(&self) -> f64 {
        if self.alluvium.is_some() { alluvium::SOURCE_AREA } else { SOURCE_AREA }
    }

    fn edifice_lift(&self, dir: DVec3) -> f64 {
        if self.edifice_lift.is_empty() { 0.0 } else { self.mesh.sample(&self.edifice_lift, dir) }
    }

    fn volcanic_base(&self, dir: DVec3) -> f64 {
        let bare = self.mesh.sample(&self.bare_height, dir);
        if self.edifice_lift.is_empty() { return bare; }
        // Restore precise rifts and headwalls between mesh vertices. Incision
        // and transported sediment stay in the solved relief.
        bare + self.volcanoes.edifice(dir).lift - self.edifice_lift(dir)
    }

    fn volcano_shape(&self, dir: DVec3) -> crate::volcano::Shaped {
        self.volcanoes.shape(dir, self.volcanic_base(dir), |d| self.volcanic_base(d))
    }
    fn channel(&self, dir: DVec3) -> Option<Channel> {
        let (ids, _) = self.mesh.stencil(dir);
        let p = dir * atlas::RADIUS_M;
        let mut best: Option<Channel> = None;
        // A channel's entire influence is narrower than one solve cell. One
        // neighbour ring includes every segment able to affect this stencil.
        for i in ids.into_iter().flat_map(|i| std::iter::once(i).chain(self.mesh.neighbours[i].iter().map(|e| e.0))) {
            let r = self.flow.receiver[i];
            if r == i || self.height[i] <= SEA || self.flow.discharge[i] < SOURCE_AREA { continue; }
            let a = self.river_nodes[i];
            let b = self.river_nodes[r];
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let distance = (p - (a + ab * t).normalize() * atlas::RADIUS_M).length();
            if distance >= REACH || best.is_some_and(|c| c.distance <= distance) { continue; }
            let size = (self.flow.discharge[i] / 1e6).sqrt();
            let level = self.flow.spill[i] * (1.0 - t) + self.flow.spill[r] * t;
            best = Some(Channel { distance, width: (1.3 + size * 1.4).min(22.0),
                depth: (0.8 + size * 0.45).min(6.0), level: level.max(SEA) });
        }
        best
    }

    fn climate(&self, dir: DVec3) -> crate::planet_climate::Climate {
        crate::planet_climate::Climate { annual_c: self.mesh.sample(&self.temperature, dir),
            winter_c: self.mesh.sample(&self.winter, dir), wet: self.mesh.sample(&self.rain, dir) }
    }

    fn ground(&self, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32) -> geo::Ground {
        self.ground_shaped(cfg, dir, footprint, self.volcano_shape(dir))
    }

    fn ground_shaped(&self, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32, volcanic: crate::volcano::Shaped) -> geo::Ground {
        let mut ground = self.ground_shaped_height(cfg, dir, footprint, volcanic);
        if ground.deposit_m > 0.0 {
            // Metadata must describe the final surface (spawn checks use it),
            // including mask transitions. Inner height sampling cannot recurse.
            let tangent = if dir.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let east = tangent.cross(dir).normalize_or_zero();
            let north = dir.cross(east);
            let step = f64::from(footprint).max(0.5);
            let height = |axis: DVec3, sign: f64| {
                let at = (dir + axis * (sign * step / atlas::RADIUS_M)).normalize();
                self.ground_shaped_height(cfg, at, footprint, self.volcano_shape(at)).top_m
            };
            let dx = (height(east, 1.0) - height(east, -1.0)) / (2.0 * step as f32);
            let dz = (height(north, 1.0) - height(north, -1.0)) / (2.0 * step as f32);
            ground.slope = dx.hypot(dz);
        }
        ground
    }

    fn ground_shaped_height(&self, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32, volcanic: crate::volcano::Shaped) -> geo::Ground {
        if reference_relief::active(cfg) && let Some(ground) = reference_relief::ground(dir) {
            return ground;
        }
        let carved = carved_surface(cfg.generator_version);
        let point = self.surface_query(dir);
        let mut local_envelope = dunes::Envelope::default();
        let mut ground = if cfg.generator_version == dunes::ENVELOPE_GENERATOR {
            self.ground_base_prepared(cfg, &point, footprint, volcanic, Some(&mut local_envelope))
        } else { point.ground_base(cfg, footprint, volcanic) };
        // Read before the tiles can run a nested ground query on this thread.
        let open = if carved { carve::take_open() } else { 0.0 };
        if uses_headwater_relief(cfg.generator_version) {
            let drainage = self.slope_tiles.apply(self, cfg, dir, footprint, &mut ground);
            if carved && let Some((weight, gradient)) = drainage {
                self.carve_ground(cfg, &point, footprint, &mut ground, open * weight, gradient);
            }
        }
        if cfg.generator_version == dunes::ENVELOPE_GENERATOR {
            // Sample eligibility on a fixed body-space lattice: local slope
            // transitions must not collapse a tall dune within one voxel.
            let key = self.checksum ^ (u64::from(cfg.generator_version) << 40) ^ u64::from(cfg.seed);
            let envelope = dunes::envelope(key, dir * atlas::RADIUS_M, |at| {
                let mut out = dunes::Envelope::default();
                let point = self.surface_query(at);
                self.ground_base_prepared(cfg, &point, 1.0, self.volcano_shape(at), Some(&mut out));
                out
            });
            // Keep the local continuous river/floodplain clearance, not the
            // local slope eligibility that caused metre-wide sand walls.
            let clearance = if volcanic.active_caldera || volcanic.lake.is_some() { 0.0 } else { local_envelope.calm };
            ground.deposit_m = (dunes::coherent_height(cfg.seed, dir * atlas::RADIUS_M, f64::from(footprint), dir, envelope.erg) * envelope.sand * clearance) as f32;
            ground.deposit_material = BlockId::Sand;
        }
        if cfg.generator_version == erratics::ERRATIC_DEPOSIT_GENERATOR {
            // Deposit after all erosion, without passing a boulder through
            // talus/scree rules. Stone is a coherent provisional lithology,
            // not a claim to have simulated transport from an up-ice source.
            let ice = if self.former_ice.is_empty() { 0.0 } else { self.mesh.sample(&self.former_ice, dir) };
            let mask = smooth(SEA + 5.0, SEA + 12.0, f64::from(ground.top_m))
                * (1.0 - smooth(0.20, 0.38, f64::from(ground.slope)))
                * (1.0 - smooth(0.12, 0.25, f64::from(ground.riparian)));
            ground.deposit_m = if volcanic.active_caldera || volcanic.lake.is_some() { 0.0 }
                else { (erratics::height(cfg.seed, dir, ice, footprint) * mask) as f32 };
            ground.deposit_material = BlockId::Stone;
        }
        // Both surface deposits retain the exact eroded substrate. Keep the
        // historical306 water exclusion and arithmetic unchanged.
        if matches!(cfg.generator_version, dunes::DEPOSIT_GENERATOR | dunes::ENVELOPE_GENERATOR)
            || cfg.generator_version == erratics::ERRATIC_DEPOSIT_GENERATOR {
            ground.deposit_bed_slope = ground.slope;
            ground.deposit_bed_top = ground.top_m;
            ground.deposit_m *= if ground.water_m.is_none() && !ground.river_bedrock { 1.0 } else { 0.0 };
            ground.top_m += ground.deposit_m;
        }
        if reference_relief::active(cfg) {
            reference_relief::apply(dir, &mut ground);
        }
        ground
    }

    /// Generator 300: gullies, rills and terracettes on the surface the
    /// erosion tiles left, following the gradient they solved.
    fn carve_ground(&self, cfg: &WorldSampleConfig, point: &surface_query::SurfaceQuery<'_>, footprint: f32,
        ground: &mut geo::Ground, open: f64, gradient: DVec3)
    {
        if open <= 0.0 || ground.water_m.is_some() || ground.river_gravel { return; }
        let dir = point.direction();
        let p = dir * atlas::RADIUS_M;
        let top = f64::from(ground.top_m);
        // Keep clear of the shore, the summit clamp and deposits still settling.
        let open = open * smooth(SEA + 3.0, SEA + 9.0, top) * (1.0 - smooth(455.0, 490.0, top))
            * (1.0 - smooth(0.05, 0.3, f64::from(ground.delta)));
        if open <= 0.0 { return; }
        let hard = (point.collision().clamp(0.0, 1.0) * 0.8 + point.fault_damage().clamp(0.0, 1.0) * 0.3).min(1.0);
        let site = carve::Site { up: dir, hard, wet: f64::from(ground.moisture), open };
        let cut = carve::carve(cfg.seed, p, f64::from(footprint), &site, |x, y, z| self.carve_lattice(cfg, x, y, z));
        let mut top = top + cut.delta_m;
        let mut uphill = gradient - dir * gradient.dot(dir) + cut.steer;
        // Loose debris collects in the creases of steep ground; ribs shed it.
        let crease = smooth(0.15, 0.7, -cut.rib) * cut.strength;
        // Generator 304: the solved beds stand as risers and benches.
        let mut riser = 0.0;
        if rock_relief(cfg.generator_version) {
            let site = rock_relief::Site { up: dir, gradient: uphill, crease,
                moisture: f64::from(ground.moisture), open };
            if let Some(relief) = rock_relief::apply(cfg.seed, p, top, f64::from(footprint),
                point.collision().clamp(0.0, 1.0), &site, |x, y, z| self.rock_profile(cfg, x, y, z))
            {
                (top, uphill, riser) = (relief.top, relief.gradient, relief.riser);
            }
        }
        let slope = uphill.length();
        let scree = 2.6 * crease * smooth(0.30, 0.55, slope) * (1.0 - smooth(0.85, 1.2, slope));
        // Terracettes need a turf mantle: no scree, no bare steep rock.
        let soil = (1.0 - smooth(0.45, 0.9, hard)) * smooth(0.25, 0.5, f64::from(ground.moisture))
            * (1.0 - smooth(0.2, 0.6, f64::from(ground.scree_m))) * (1.0 - riser);
        top += carve::terracette(cfg.seed, p, f64::from(footprint), top, slope, soil * open);
        ground.top_m = top.max(SEA + 1.5) as f32;
        ground.slope = slope as f32;
        ground.scree_m = ground.scree_m.max(scree as f32);
        // Creases hold water longer than the ribs beside them.
        let drier = smooth(0.2, 0.8, cut.rib) * cut.strength;
        ground.moisture = (ground.moisture + (0.10 * crease - 0.05 * drier) as f32).clamp(0.0, 1.0);
    }

    /// Generator 304: bed resistance per metre of height at a rock relief
    /// lattice corner, read from the history column there.
    fn rock_profile(&self, cfg: &WorldSampleConfig, x: i64, y: i64, z: i64) -> rock_relief::Profile {
        let world = self.checksum ^ (u64::from(cfg.generator_version) << 40) ^ u64::from(cfg.seed);
        rock_relief::cached_profile((world, x, y, z), || {
            let dir = (DVec3::new(x as f64, y as f64, z as f64) * rock_relief::LATTICE_M).normalize_or_zero();
            if dir == DVec3::ZERO { return rock_relief::Profile::NONE; }
            let point = self.surface_query(dir);
            match self.history_strata_prepared(cfg, &point) {
                // Above the column top there is no bed: hold the top one.
                Some(column) => rock_relief::Profile::sample(column.top, |h| {
                    let h = h.min(column.top - 0.25);
                    let crystalline = !(column.lava_m > 0.0 && h > column.top - column.lava_m)
                        && column.beds.block(column.top - column.lava_m - h).is_none();
                    (column.resistance(h), crystalline)
                }),
                None => rock_relief::Profile::NONE,
            }
        })
    }

    /// Uphill gradient of the refined surface at a carve lattice corner,
    /// tangent, scaled by how much of the erosion tiles applies there (zero
    /// on water, shores and deltas). The gullies of every column in the
    /// surrounding cells follow it.
    fn carve_lattice(&self, cfg: &WorldSampleConfig, x: i64, y: i64, z: i64) -> carve::Corner {
        let world = self.checksum ^ (u64::from(cfg.generator_version) << 40) ^ u64::from(cfg.seed);
        carve::cached_corner((world, x, y, z), || {
            let dir = (DVec3::new(x as f64, y as f64, z as f64) * carve::LATTICE_M).normalize_or_zero();
            if dir == DVec3::ZERO { return carve::Corner::NONE; }
            // Tangent gradient of the refined surface at an 8 m footprint.
            let gradient_at = |dir: DVec3| -> DVec3 {
                let point = self.surface_query(dir);
                let mut ground = point.ground_base(cfg, 8.0, self.volcano_shape(dir));
                carve::take_open();
                if ground.water_m.is_some() { return DVec3::ZERO; }
                match self.slope_tiles.apply(self, cfg, dir, 8.0, &mut ground) {
                    Some((weight, gradient)) => (gradient - dir * gradient.dot(dir)) * weight,
                    None => DVec3::ZERO,
                }
            };
            let gradient = gradient_at(dir);
            if gradient == DVec3::ZERO { return carve::Corner::NONE; }
            // Forward differences along two tangent axes give the divergence.
            let east = DVec3::Y.cross(dir).try_normalize().unwrap_or_else(|| DVec3::X.cross(dir).normalize());
            let north = dir.cross(east);
            let step = carve::CONVERGENCE_STEP_M;
            let ahead = |axis: DVec3| {
                let g = gradient_at((dir * atlas::RADIUS_M + axis * step).normalize());
                if g == DVec3::ZERO { 0.0 } else { (g - gradient).dot(axis) / step }
            };
            carve::Corner { gradient, convergence: ahead(east) + ahead(north) }
        })
    }

    fn ground_base(&self, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32, volcanic: crate::volcano::Shaped) -> geo::Ground {
        self.surface_query(dir).ground_base(cfg, footprint, volcanic)
    }

    fn ground_base_prepared(&self, cfg: &WorldSampleConfig, point: &surface_query::SurfaceQuery<'_>,
        footprint: f32, volcanic: crate::volcano::Shaped, dune_envelope: Option<&mut dunes::Envelope>) -> geo::Ground
    {
        let dir = point.direction();
        let layered = cfg.generator_version >= engine_voxel::BEDDING_PLANET_GENERATOR;
        let valley_guided = cfg.generator_version >= engine_voxel::RIVER_MORPHOLOGY_GENERATOR;
        let (local, weights) = point.stencil();
        let sample = |field: &[f64]| (0..4).map(|i| field[local[i]] * weights[i]).sum::<f64>();
        let bare = sample(&self.bare_height);
        let coarse = sample(&self.height);
        let spill = sample(&self.flow.spill);
        let inland = if valley_guided || spill <= coarse + 0.75 {
            smooth(SEA + 2.0, SEA + 24.0, coarse)
        } else { 0.0 };
        // At sea level the same tangent reconstruction removes bends tied
        // to the edges of solve cells. Keep lake basins and active craters
        // on their existing elevation and let the river model cut its bed.
        let coastal = if cfg.generator_version >= engine_voxel::CONTINUOUS_COAST_PLANET_GENERATOR
            && !volcanic.active_caldera && volcanic.lake.is_none() {
            (1.0 - smooth(4.0, 24.0, (coarse - SEA).abs()))
                * (1.0 - smooth(0.25, 0.75, spill - coarse))
        } else { 0.0 };
        let relief_delta = if cfg.generator_version >= engine_voxel::CONTINUOUS_RELIEF_PLANET_GENERATOR
            && (inland > 0.0 || coastal > 0.0) {
            let mut blended = self.mesh.reconstruct_height_from_stencil(
                &self.height, &self.gradient, dir, (local, weights));
            let edifice = point.edifice_lift();
            if edifice > 0.0 {
                // Use the same reconstruction for volcanic relief and terrain.
                // The analytic edifice correction below replaces its linear part.
                blended -= self.mesh.reconstruct_height_from_stencil(
                    &self.edifice_lift, &self.edifice_gradient, dir, (local, weights)) - edifice;
            }
            blended - coarse
        } else { 0.0 };
        let relief = coarse + relief_delta * inland;
        let volcanic_base = bare + sample(&self.volcanic_delta);
        let h = if uses_headwater_relief(cfg.generator_version) || cfg.generator_version == TECTONIC_TRAVERSE_GENERATOR {
            // Apply the analytic volcanic correction in the same height
            // space as the routed mesh. The continuous fit is applied once.
            (relief + fit_mountain_height(volcanic.metres) - fit_mountain_height(volcanic_base))
                .clamp(-58.0, 503.0)
        } else {
            // Retain the historical operation order for saved generators.
            (relief + volcanic.metres - bare - sample(&self.volcanic_delta))
                .clamp(-58.0, 495.0)
        };
        let c = crate::planet_climate::Climate { annual_c: sample(&self.temperature),
            winter_c: sample(&self.winter), wet: sample(&self.rain) };
        let lake = volcanic.lake.map(|(head, _)| head).or_else(|| {
            if coarse <= SEA || c.wet <= 0.12 {
                return None;
            }
            if !valley_guided {
                return (spill > coarse + 0.75 && h < spill).then_some(spill);
            }
            // Terrain intersects a lake's level. Interpolating the routing
            // elevations instead tilts the lake toward the dry corner of a cell.
            local.into_iter().zip(weights).filter_map(|(i, weight)| {
                let level = self.flow.spill[i];
                (weight > 0.0 && self.height[i] > SEA && level > self.height[i] + 0.75
                    && level > h).then_some((weight, level))
            }).max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, level)| level)
        });
        let p = dir * atlas::RADIUS_M;
        let coastal_relief = cfg.generator_version >= engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR;
        let alluvial = self.alluvium.as_ref().filter(|_| !volcanic.active_caldera)
            .map(|network| network.sample(p, h, footprint));
        let riverbed = alluvial.as_ref().and_then(|a| a.riverbed);
        let channel = if alluvial.is_none() && !volcanic.active_caldera { self.channel(dir) } else { None };
        let calm = alluvial.as_ref().map_or_else(
            || channel.map_or(1.0, |c| smooth(c.width + 5.0, c.width + 40.0, c.distance)),
            |a| a.calm * (1.0 - a.delta) * (1.0 - a.morph.map_or(0.0, |m| {
                if coastal_relief { m.blend * smooth(0.3, 3.0, m.deposited.abs()) * 0.45 }
                else { m.blend }
            })),
        ) * if lake.is_some() || volcanic.active_caldera { 0.0 } else { 1.0 };
        let gradient: DVec3 = (0..4).map(|i| self.gradient[local[i]] * weights[i]).sum();
        // Establish the river bed and floodplain against the solved grade.
        // Limit the displacement on shallow shelves as well as on cliffs.
        // Generator 296: a river corridor sinks with its surrounding plain.
        // Suppressing only the lowering left the routed channel perched on a
        // causeway above the lowered coast; raising stays suppressed by `calm`.
        let lowland = lowland_rivers(cfg.generator_version);
        let mut corridor_drop = 0.0;
        let h = if coastal > 0.0 {
            let limit = (gradient.length() * 64.0).clamp(0.55, 6.0);
            if lowland {
                let shift = limit * ((relief_delta * coastal * (1.0 - inland)) / limit).tanh();
                let lower = shift.min(0.0);
                corridor_drop += lower;
                (h + lower + shift.max(0.0) * calm.powi(4)).clamp(-58.0, 495.0)
            } else {
                let shore_shift = relief_delta * coastal * (1.0 - inland) * calm.powi(4);
                (h + limit * (shore_shift / limit).tanh()).clamp(-58.0, 495.0)
            }
        } else { h };
        let h = if fractal_coasts(cfg.generator_version) && lake.is_none() && !volcanic.active_caldera {
            let shore = coast_roughness::Shore {
                collision: sample(&self.collision), rift: sample(&self.rift),
                former_ice: sample(&self.former_ice), sediment_m: sample(&self.sediment_cover),
                grain: if self.structure_gradient.is_empty() { DVec3::ZERO } else {
                    coast_roughness::grain(dir, (0..4).map(|i| self.structure_gradient[local[i]] * weights[i]).sum())
                },
            };
            let weight = coast_roughness::band(h, gradient.length(), &shore);
            if weight > 0.0 {
                let offset = coast_roughness::warp(cfg.seed, p, f64::from(footprint), &shore);
                let moved = self.mesh.reconstruct_height(&self.height, &self.gradient, (p + offset).normalize());
                let here = self.mesh.reconstruct_height_from_stencil(&self.height, &self.gradient, dir, (local, weights));
                // River mouths keep the routed channel; the warp fades with `calm`.
                let shift = coast_roughness::shift(moved, here, weight);
                if lowland {
                    corridor_drop += shift.min(0.0);
                    (h + shift.min(0.0) + shift.max(0.0) * calm).clamp(-58.0, 495.0)
                } else {
                    (h + shift * calm).clamp(-58.0, 495.0)
                }
            } else { h }
        } else { h };
        let land = smooth(SEA, SEA + 12.0, h);
        let carbonate = sample(&self.carbonate);
        let collision = sample(&self.collision).min(1.0);
        let rift = sample(&self.rift).min(1.0);
        let filtered = |w: f64| 1.0 - smooth(w * 0.25, w * 2.0, f64::from(footprint));
        // Province-conditioned relief: rounded uplands, resistant crags,
        // limestone terraces and sinkholes, volcanic ridges. All wavelengths are metres.
        let round = (noise(cfg.seed, p / 150.0, 1001) - 0.5) * (10.0 + collision * 22.0) * filtered(150.0);
        let crag = (1.0 - (noise(cfg.seed, p / 48.0, 1009) * 2.0 - 1.0).abs()).powi(3) * collision * 12.0 * filtered(48.0);
        let dissolution = if valley_guided {
            // Develop hollows in wet carbonate depressions, where water can
            // collect and enter the rock. Carbonate by itself is insufficient.
            carbonate * smooth(0.2, 0.65, c.wet) * smooth(0.3, 3.0, spill - coarse)
        } else {
            carbonate
        };
        let sink = smooth(0.65, 0.88, noise(cfg.seed, p / 85.0, 1013)) * dissolution * 10.0 * filtered(85.0);
        let basalt = (noise(cfg.seed, p / 110.0, 1019) - 0.5) * rift * 16.0 * filtered(110.0);
        let micro = (noise(cfg.seed, p / 7.0, 1021) - 0.5) * 0.9 * filtered(7.0);
        // Leave the highlands' sharp relief to the routed erosion and beds.
        let crag_weight = if cfg.generator_version == LAYERED_CANYON_GENERATOR { 0.18 }
            else if connected_relief(cfg.generator_version) { 0.36 } else { 1.0 };
        let detail = if geological_history(cfg.generator_version) || cfg.generator_version == COUPLED_RIFT_GENERATOR {
            micro
        } else { round + crag * crag_weight - sink + basalt + micro };
        let detail = detail.min(0.0) + detail.max(0.0) * (1.0 - smooth(470.0, 500.0, h));
        let mut top = h + detail * calm * land;
        if living_surface(cfg.generator_version) && lake.is_none() && !volcanic.active_caldera {
            let ground = fine_relief::Ground { collision, rift, former_ice: sample(&self.former_ice),
                sediment_m: sample(&self.sediment_cover), slope: gradient.length(), wet: c.wet };
            // Generator 300 reads the knolls and hollows at a warped point.
            // Thresholded lattice noise drew each hollow as a rounded square
            // the size of a noise cell; a 35 m warp gives them free outlines.
            let relief_at = if carved_surface(cfg.generator_version) {
                p + DVec3::new(noise(cfg.seed, p / 61.0, 300_701) - 0.5, noise(cfg.seed, p / 61.0, 300_703) - 0.5,
                    noise(cfg.seed, p / 61.0, 300_707) - 0.5) * 70.0
            } else { p };
            let relief = fine_relief::upland(cfg.seed, relief_at, f64::from(footprint), &ground);
            let relief = relief.max(0.0) * (1.0 - smooth(470.0, 500.0, h)) + relief.min(0.0) * calm;
            // Generator 300: coastal and alluvial flats keep a gentle roll; the
            // knolls and hollows read as rows of dimples on level sand.
            let relief = if carved_surface(cfg.generator_version) {
                relief * (0.2 + 0.8 * smooth(0.02, 0.12, gradient.length()))
            } else { relief };
            // Hollows fade faster than rises beside channels, so banks stay above the head.
            top += relief * calm * land
                + if carved_surface(cfg.generator_version) {
                    // Generator 300 lays wind-aligned dunes in place of ridge noise.
                    // The dune field is laid below, where the sand cover is known.
                    let (rock, _, _) = fine_relief::shore_forms(cfg.seed, p, f64::from(footprint), h, &ground);
                    // Stacks stand in groups off a headland, not as one net of ridges.
                    if rock != 0.0 { rock * smooth(0.48, 0.74, noise(cfg.seed, p / 120.0, 300_401)) } else { 0.0 }
                } else {
                    fine_relief::shore(cfg.seed, p, f64::from(footprint), h, &ground)
                } * calm;
            top = top.clamp(-58.0, 495.0);
        }
        let mut rift_scarp = 0.0;
        if !self.rift_valley.is_empty() && rift > 0.0 && !volcanic.active_caldera
            && let Some(frame) = self.plates.rift_frame(dir)
        {
            // The mesh carried the smooth valley; restore what it could not hold.
            let (full, scarp) = frame.full(f64::from(footprint));
            let weight = if lake.is_some() { 1.0 } else { calm };
            let kept = sample(&self.rift_scale);
            top = (top + (full * kept - sample(&self.rift_valley)) * weight).clamp(-58.0, 495.0);
            rift_scarp = scarp * kept * weight;
        }
        let lava_lake = cfg.generator_version >= engine_voxel::VOLCANIC_LANDFORM_PLANET_GENERATOR
            && volcanic.lake.is_some_and(|(_, fluid)| fluid == BlockId::Lava);
        // The coarse watershed may see the active caldera as a freshwater
        // catchment. Its visible basin is instead ash, exposed rock and lava.
        let mut water = if lava_lake || volcanic.active_caldera { None } else { lake };
        let mut riparian = 0.0;
        let mut valley_wall = 0.0;
        let mut delta = 0.0;
        let mut delta_mud = 1.0;
        let mut suspended = 0.0;
        let mut intertidal_exposure = 0.0;
        let mut outlook_grade = None;
        let mut placer = 0.0;
        let mut fresh_sand = 0.0;
        // Generator 296 moves the routed stage down with the corridor, never below sea.
        let alluvial = alluvial.map(|mut a| {
            if lowland && corridor_drop < 0.0 {
                let d = corridor_drop;
                let stage = |s: f64| (s + d).max(SEA);
                if let Some(plain) = a.floodplain.as_mut() { plain.ceiling += d; }
                a.bed = a.bed.map(|(bed, head, w)| (bed + d, stage(head), w));
                a.channel_head = a.channel_head.map(stage);
                a.overbank = a.overbank.map(stage);
                a.placer = a.placer.map(|(c, g)| (c + d, g));
                a.swamp = a.swamp.map(|(w, s)| (w, stage(s)));
            }
            a
        });
        if let Some(a) = alluvial {
            delta = a.delta;
            if lowland { fresh_sand = a.sand; }
            if delta > 0.0 && h < SEA + 6.0 {
                top += (a.deposit - top).clamp(0.0, 34.0) * delta;
            }
            if let Some(plain) = a.floodplain {
                top -= (top - plain.ceiling).max(0.0) * plain.weight;
                if a.bed.is_none() && lake.is_none()
                    && !(landscape_cycles(cfg.generator_version) && a.channel_head.is_some()) {
                    top += plain.fill.min((plain.ceiling - top).max(0.0));
                }
                valley_wall = plain.wall;
                if lowland && a.bed.is_none() && lake.is_none() {
                    // Low natural levee beside the bank, then a backswamp held
                    // near the water table: shallow marsh water where it reaches it.
                    // Crevasse splays then spread a thin sand lobe over both.
                    top += a.levee;
                    let marsh = a.swamp.map(|(swamp, stage)| {
                        let table = stage.max(SEA) + 0.15;
                        top -= (top - (table - 0.25)).clamp(0.0, 0.9) * swamp;
                        (swamp, table)
                    });
                    top += a.splay;
                    if let Some((swamp, table)) = marsh
                        && swamp > 0.45 && top < table + 0.1 && water.is_none() {
                        water = Some(top.max(table - 0.4) + 0.3);
                    }
                }
            }
            if let Some((bed, head, weight)) = a.bed {
                // Generator 296 bars stand just clear of the (shifted) stage.
                let bed = if lowland { bed + (head.max(SEA) + 0.6 - bed).max(0.0) * a.bar } else { bed };
                top -= (top - bed).max(0.0) * weight;
                if lowland { top += (bed - top).max(0.0) * weight * a.bar; }
                if top < head {
                    water = Some(water.map_or(head, |l| l.max(head)));
                }
            }
            if let Some(head) = a.overbank.filter(|head| *head > top) {
                water = Some(water.map_or(head, |l| l.max(head)));
            }
            if landscape_cycles(cfg.generator_version)
                && let Some(head) = a.channel_head.filter(|head| *head > top) {
                // Filtering narrow river geometry cannot make the physical
                // water disappear and abruptly enable dry-slope erosion.
                water = Some(water.map_or(head, |old| old.max(head)));
            }
            placer = a.placer.filter(|&(ceiling, _)| top <= ceiling).map_or(0.0, |(_, grade)| grade);
            riparian = a.riparian.max(delta * 0.7);
            if let Some(m) = a.morph {
                outlook_grade = m.outlook;
                let deposited = m.deposited - m.deposited.max(0.0) * m.island_share;
                delta = m.blend * smooth(0.08, 0.7, m.deposited);
                if m.mobile {
                    let wet = m.blend * smooth(0.08, 0.70, m.depth);
                    top += m.deposited * m.blend;
                    let intertidal = if tidal_coasts(cfg) {
                        m.blend * (1.0 - smooth(1.2, 5.0, (top - SEA).abs()))
                    } else { 0.0 };
                    top += (m.bed - top) * wet.max(intertidal);
                    if wet > 0.0 {
                        let old = water.unwrap_or(SEA);
                        let stage = old + (m.bed + m.depth - old) * wet;
                        water = (stage > top && stage > SEA + 0.02).then_some(stage);
                    }
                    delta_mud = m.mud;
                    suspended = m.suspended * m.blend * (1.0 - smooth(48.0, 240.0, f64::from(footprint)));
                    intertidal_exposure = m.blend * smooth(0.04, 0.22, m.inundation)
                        * (1.0 - smooth(0.76, 0.98, m.inundation))
                        * smooth(0.008, 0.065, m.fine_accretion);
                } else if let Some(form) = m.estuary {
                    let active = m.blend * form.weight;
                    let bar_weight = m.blend * form.sediment;
                    let relief = if bar_weight > 0.02 {
                        (noise(cfg.seed, p / 16.0, 1491) - 0.5) * 1.05 * filtered(16.0)
                    } else { 0.0 };
                    // The solved deposit supplies the shelf. The smaller bars
                    // and channel cross-sections reshape only the active reach.
                    top += deposited * m.blend * (1.0 - 0.78 * active.max(bar_weight));
                    top += (form.bar + relief - top).clamp(0.0, 9.0) * bar_weight;
                    let bank = if form.bed >= form.water { 1.0 - bar_weight * 0.87 } else { 1.0 };
                    let bed = if form.water <= SEA + 0.01 && top < SEA - 3.0 && form.bed < SEA {
                        top.min(form.bed)
                    } else { form.bed };
                    top += (bed - top) * active * bank;
                    if active > 0.0 {
                        let previous = water.unwrap_or(SEA);
                        let head = previous + (form.water - previous) * active;
                        water = (head > top && head > SEA + 0.01).then_some(head);
                    }
                    if let Some((bed, head, resolved)) = a.bed.filter(|_| form.inlet_tie > 0.0) {
                        // Both routes share a wet cross-section before they
                        // diverge. A flood deposit cannot plug the handoff.
                        let inlet = head - (head - bed).clamp(0.0, 2.0) * 0.8;
                        top -= (top - inlet).max(0.0) * resolved * form.inlet_tie;
                        if top < head { water = Some(water.map_or(head, |level| level.max(head))); }
                    }
                    delta = delta.max(bar_weight * 0.96);
                    delta_mud = m.mud + (form.fine - m.mud) * bar_weight;
                } else if coastal_relief {
                    // Transfer erosion and deposition onto the walked terrain.
                    // The coarse grid's baseline would flatten the coast in a rectangle.
                    let detail = delta * 0.75 * filtered(11.0);
                    let texture = if detail > 0.0 {
                        (noise(cfg.seed, p / 11.0, 1469) - 0.5) * detail
                    } else { 0.0 };
                    let shore = m.blend * a.calm * (1.0 - smooth(1.0, 9.0, (top - SEA).abs()))
                        * filtered(72.0);
                    let beach_relief = if shore > 0.0 {
                        let banks = noise(cfg.seed, p / 71.0, 1481) - 0.5;
                        let shoals = noise(cfg.seed, p / 33.0, 1483) - 0.5;
                        (3.0 * banks + 1.4 * shoals) * shore
                    } else { 0.0 };
                    top += deposited * m.blend + texture + beach_relief;
                    if let Some((bed, river_head, resolved)) = a.bed {
                        // The short depositional event can strand its inflow.
                        // Retain a low-flow outlet through the active thalweg.
                        let relief = (river_head - bed).clamp(0.0, 1.8) * 0.68;
                        let outlet = river_head - relief;
                        top -= (top - outlet).max(0.0) * resolved * smooth(0.0, 0.15, m.blend);
                        if top < river_head {
                            water = Some(water.map_or(river_head, |head| head.max(river_head)));
                        }
                    }
                    if let Some(bed) = m.outlet_bed {
                        top -= (top - bed).max(0.0) * m.blend;
                    }
                    let head = m.bed + m.depth;
                    let wet = m.blend * smooth(0.12, 0.6, m.depth);
                    let old = water.unwrap_or(SEA);
                    let stage = old + (head - old).max(0.0) * wet;
                    if stage > (SEA + 0.08).max(top) {
                        water = Some(stage);
                    }
                } else {
                    let old = (water.unwrap_or(SEA) - top).max(0.0);
                    top += (m.bed - top) * m.blend;
                    let wet = if m.depth > 0.04 { m.depth } else { 0.0 };
                    let depth = old + (wet - old) * m.blend;
                    water = (depth > 0.0).then_some(top + depth);
                }
                if m.estuary.is_none() { delta_mud = m.mud; }
                if let Some(island) = m.island {
                    let weight = island.weight * m.blend * (1.0 - smooth(SEA + 3.2, SEA + 7.5, h));
                    let shift = island.level - top;
                    top += (shift.min(0.0) + shift.max(0.0) * (1.0 - island.channel)) * weight;
                    let open = smooth(0.40, 0.94, island.channel) * m.blend
                        * (1.0 - smooth(SEA + 4.0, SEA + 9.0, h));
                    top -= (top - (SEA - 1.6)).max(0.0) * open;
                    delta = delta.max(weight);
                    delta_mud += (island.mud - delta_mud) * weight;
                }
                riparian = riparian.max(delta * 0.65);
            }
        }
        if let Some(c) = channel {
            let bank = c.width + 12.0;
            let valley = 1.0 - smooth(bank, REACH, c.distance);
            // Valleys belong to the drainage solve at every viewing distance.
            // Only the narrow bed below is subpixel detail. Keep historical
            // distant surfaces unchanged for existing saves.
            let weight = if cfg.generator_version >= engine_voxel::WATERSHED_LOD_GENERATOR {
                1.0
            } else {
                1.0 - smooth(45.0, 400.0, f64::from(footprint))
            };
            let floor = c.level + 0.7 + (c.distance - c.width).max(0.0) * 0.055;
            if layered && top > floor {
                // Rise across the carved bank from the derivative of its blend.
                let t = ((c.distance - bank) / (REACH - bank)).clamp(0.0, 1.0);
                valley_wall = (top - floor) * 6.0 * t * (1.0 - t) / (REACH - bank);
            }
            top += (top.min(floor) - top) * valley * weight;
            let bed_weight = 1.0 - smooth((c.width * 2.0).max(8.0), (c.width * 8.0).max(32.0), f64::from(footprint));
            if c.distance < c.width {
                let bed = c.level - c.depth * (1.0 - (c.distance / c.width).powi(2));
                top += (top.min(bed) - top) * bed_weight;
                if (landscape_cycles(cfg.generator_version) || bed_weight > 0.0) && top < c.level {
                    water = Some(water.map_or(c.level, |l| l.max(c.level)));
                }
            }
            if cfg.generator_version >= engine_voxel::COASTAL_PLANET_GENERATOR {
                // Backwater widens toward the coast. The same filtered head
                // drives the ground column and the distant water surface.
                let tidal = (1.0 - smooth(SEA + 0.5, SEA + 4.0, c.level))
                    * (1.0 - smooth(SEA + 1.0, SEA + 8.0, h));
                let mouth = 1.0 - smooth(c.width, c.width * 2.8 + 7.0, c.distance);
                let resolve = filtered((c.width * 5.0).max(30.0));
                top -= (top - (SEA - 1.4)).max(0.0) * tidal * mouth * resolve;
            }
            riparian = valley;
        }
        let area = sample(&self.flow.discharge);
        let mut slope = (gradient.length() * (1.0 - delta * 0.85)).max(valley_wall).max(rift_scarp);
        let mut scree_m = 0.0;
        // The tributary solve uses these beds as erodibility and repose
        // limits, without imposing contour ripples on its parent surface.
        if layered && !composed_drainage(cfg.generator_version) {
            // Folded beds follow the same datum as the exposed and buried rock.
            // Stretch the profile through resistant rock and flatten it between
            // beds. The monotone remap preserves the order of the underlying slope.
            let province = (1.0 - smooth(0.12, 0.55, collision)).max(carbonate)
                * (1.0 - smooth(0.12, 0.25, rift))
                * (1.0 - smooth(8.0, 35.0, point.edifice_lift()));
            let strength = 0.78 * province * calm * smooth(0.09, 0.30, slope)
                * smooth(SEA + 6.0, SEA + 28.0, top) * (1.0 - smooth(460.0, 495.0, top))
                * (1.0 - smooth(0.0, 0.3, volcanic.slope)) * filtered(95.0);
            if strength > 0.0 {
                let offset = if self.fault_offset.is_empty() { 0.0 } else { sample(&self.fault_offset) };
                let bed = bedding_height(cfg, p, collision, top - offset);
                if cfg.generator_version >= ERODED_RELIEF_GENERATOR {
                    // The resistant band projects from the slope, with its softer
                    // contact recessed. A continuous profile leaves no flat steps.
                    let phase = (bed - 10.0) * std::f64::consts::TAU / BED_INTERVAL;
                    let ledge = 2.8 * strength;
                    top += ledge * phase.sin();
                    slope *= 1.0 + ledge * (std::f64::consts::TAU / BED_INTERVAL) * phase.cos();
                    if water.is_none() && footprint < 125.0 {
                        let (shift, debris) = weathered_bed(cfg.seed, p, bed, f64::from(footprint));
                        let talus = 1.0 - smooth(0.35, 0.80, slope);
                        scree_m = strength * debris * talus;
                        top += scree_m + strength * (shift - debris) * 0.65;
                        // Sub-metre joint relief is limited to exposed, steep beds.
                        let resolved = 1.0 - smooth(0.25, 0.9, f64::from(footprint));
                        let exposed = smooth(0.36, 0.85, slope) * smooth(10.0, 18.0, bed)
                            * (1.0 - smooth(31.0, 38.0, bed));
                        if resolved * exposed > 0.0 {
                            let joint = (noise(cfg.seed, p / 2.8, 1259) - 0.5).abs();
                            top -= 0.48 * strength * resolved * exposed * (1.0 - smooth(0.02, 0.19, joint));
                        }
                    }
                } else {
                    let riser = ((bed - 12.0) / 24.0).clamp(0.0, 1.0);
                    top += strength * (BED_INTERVAL * smooth(12.0, 36.0, bed) - bed);
                    slope *= 1.0 + strength * (12.0 * riser * (1.0 - riser) - 1.0);
                    if cfg.generator_version >= engine_voxel::SCREE_PLANET_GENERATOR
                        && water.is_none() && footprint < 125.0
                    {
                        let (shift, debris) = weathered_bed(cfg.seed, p, bed, f64::from(footprint));
                        top += strength * shift;
                        scree_m = strength * debris;
                    }
                }
            }
        }
        if crate::trees::rainforest::forest_floor(cfg) && cfg.world_type == WorldType::Planet
            && footprint < 9.0 && water.is_none() && volcanic.lake.is_none() && scree_m < 0.5
        {
            let t = c.lifted((top - SEA).max(0.0));
            let tropical = smooth(17.0, 22.0, t.annual_c) * smooth(0.66, 0.79, c.wet + riparian * 0.2);
            if tropical > 0.0 {
                if carved_surface(cfg.generator_version) {
                    // The same patches the cover reads: wet hollows, drained rises.
                    let (wet, rise) = carve::floor_patches(cfg.seed, p);
                    let gentle = (1.0 - smooth(0.24, 0.52, slope)) * smooth(2.2, 6.5, top - SEA)
                        * (1.0 - smooth(3.0, 9.0, f64::from(footprint)));
                    top += tropical * (0.6 * rise - 0.9 * wet) * gentle;
                } else {
                    let (swale, mound) = rainforest_floor(cfg, p);
                    let shift = crate::trees::rainforest::floor_relief(swale, mound, top as f32, slope as f32, footprint);
                    top += tropical * f64::from(shift);
                }
            }
        }
        if !self.fault_damage.is_empty() && water.is_none() && top > SEA + 8.0 {
            let damage = sample(&self.fault_damage);
            let apron = smooth(0.12, 0.30, slope) * (1.0 - smooth(0.43, 0.72, slope));
            // Loose blocks accumulate below damaged rock; cliff geometry was
            // displaced and eroded before this surface-cover classification.
            scree_m = scree_m.max(3.4 * damage * apron * calm);
        }
        let mut outlook_clearing = 0.0;
        if let Some(grade) = outlook_grade
            && water.is_none() && volcanic.lake.is_none() && top > SEA + 4.0
        {
            let weight = grade.weight * smooth(SEA + 4.0, SEA + 6.0, top);
            top += (grade.level - top).clamp(-3.0, 3.0) * weight;
            outlook_clearing = grade.clearing as f32;
        }
        let mut river_gravel = false;
        let mut river_bedrock = false;
        if let Some(river) = riverbed.filter(|_| lake.is_none() && delta < 0.1) {
            let old_depth = water.map_or(0.0, |head| (head - top).max(0.0));
            if old_depth > 0.0 || river.depth > 0.08 || river.shift.abs() > 0.035 {
                top += (river.bed - top) * river.blend;
                let depth = old_depth + (river.depth - old_depth) * river.blend;
                water = (depth > 0.11).then_some(top + depth);
                river_gravel = if clean_riverbeds(cfg.generator_version) {
                    // Pilot fill and flood spread raised `shift` across whole
                    // straight tiles; keep gravel on the wet bed and real bars.
                    let deposit = river.deposited * river.blend;
                    (depth > 0.11 && deposit > 0.16) || deposit > 0.5
                } else {
                    river.shift.max(river.deposited) * river.blend > 0.16
                };
                river_bedrock = river.rock_exposure * river.blend > 0.62
                    && river.shift < -0.25 && depth > 0.11;
            }
        }
        let mut sediment = ((area.sqrt() / 5000.0).min(1.0) * (1.0 - collision)
            + riparian * 0.4).max(delta).min(1.0);
        if !self.sediment_cover.is_empty() {
            sediment = sediment.max(smooth(0.1, 2.0, sample(&self.sediment_cover)));
        }
        if let Some(region) = self.eroded_region.as_ref().and_then(|region| region.sample(dir)) {
            let mix = |parent: f64, local: f64| parent + region.weight * (local - parent);
            // Keep parent water at its solved grade where it crosses the apron.
            // The regional runoff is annual volume and supplies no water stage.
            let wet = water.is_some_and(|head| head > top);
            let weight = if wet { region.weight * region.weight } else { region.weight };
            let old_top = top;
            top += weight * (region.height - top);
            slope = mix(slope, region.slope);
            riparian = mix(riparian, region.drainage);
            sediment = mix(sediment, smooth(0.15, 2.0, region.soil));
            scree_m = mix(scree_m, 0.0);
            if wet {
                // A legacy stage may otherwise inundate an independently cut gorge.
                water = water.filter(|head| old_top - top < 1.0 && *head > top);
            }
        }
        if cfg.generator_version != erratics::ERRATIC_DEPOSIT_GENERATOR
            && water.is_none() && top > SEA + 5.0 && slope < 0.38 && riparian < 0.25 {
            let erratic = if cfg.generator_version == erratics::ERRATIC_RELIEF_GENERATOR {
                // Fade before the enclosing safety cutoffs so a rock cannot
                // end in a wall at an ice, slope, shore or drainage boundary.
                erratics::height(cfg.seed, dir, if self.former_ice.is_empty() { 0.0 } else { sample(&self.former_ice) }, footprint)
                    * smooth(SEA + 5.0, SEA + 12.0, top)
                    * (1.0 - smooth(0.20, 0.38, slope))
                    * (1.0 - smooth(0.12, 0.25, riparian))
            } else {
                glacial::erratic(&self.mesh, &self.former_ice, dir, cfg.seed, footprint)
            };
            top += erratic;
            scree_m = scree_m.max(erratic * 1.45);
        }
        if changing_seas(cfg.generator_version) && coarse < SEA - 8.0
            && water.is_none() && !volcanic.active_caldera && delta < 0.1
        {
            // Shape only submerged open-ocean beds, after the shoreline,
            // rivers, volcanic relief and local sediment have been solved.
            let weight = smooth(8.0, 20.0, SEA - coarse) * (1.0 - smooth(0.0, 0.1, delta));
            let trench = sample(&self.offshore_trench);
            top += weight * (self.plates.offshore_bed(dir, top, SEA, trench) - top);
        }
        // Generator 296 splays and bars read as fresh sand deposits.
        if fresh_sand > 0.0 && lake.is_none() {
            delta = delta.max(fresh_sand * 0.9);
            delta_mud += (0.05 - delta_mud) * fresh_sand;
        }
        let mut deposit_m = 0.0_f32;
        if carved_surface(cfg.generator_version) && water.is_none() && lake.is_none() && !volcanic.active_caldera {
            // Wind-laid dunes on the same ground the cover rule turns to blown
            // sand: loose, dry, gentle, between the swash and the first rise.
            let gentle = (1.0 - smooth(0.10, 0.22, slope)) * calm;
            let backshore = smooth(0.40, 0.50, sediment.max(delta)) * smooth(SEA + 2.0, SEA + 4.0, top)
                * (1.0 - smooth(SEA + 10.0, SEA + 15.0, top)) * (1.0 - 0.6 * c.wet.clamp(0.0, 1.0));
            // Hot dry lowland carries sand seas in patches between stony
            // plains, where the climate cover is already sand.
            let arid = c.lifted((top - SEA).max(0.0));
            let erg = smooth(18.0, 20.5, arid.annual_c) * (1.0 - smooth(0.17, 0.23, arid.wet))
                * smooth(SEA + 3.0, SEA + 8.0, top) * (1.0 - smooth(150.0, 230.0, top))
                * smooth(0.36, 0.52, noise(cfg.seed, p / 1400.0, 300_601) * 0.7 + noise(cfg.seed, p / 380.0, 300_603) * 0.3);
            // Blown sand climbs a gentle hillside; a backshore dune belt does not.
            let climbing = (1.0 - smooth(0.16, 0.32, slope)) * calm;
            let sand = (backshore * gentle).max(erg * climbing);
            if let Some(out) = dune_envelope { *out = dunes::Envelope { sand, erg, calm }; }
            if sand > 0.0 && cfg.generator_version != dunes::ENVELOPE_GENERATOR {
                let relief = if matches!(cfg.generator_version, dunes::COHERENT_DUNES_GENERATOR | dunes::DEPOSIT_GENERATOR) {
                    dunes::coherent_height(cfg.seed, p, f64::from(footprint), dir, erg)
                } else if cfg.generator_version == dunes::VERSION {
                    dunes::height(cfg.seed, p, f64::from(footprint), dir, erg)
                } else {
                    carve::dunes(cfg.seed, p, f64::from(footprint), dir, erg)
                };
                if cfg.generator_version == dunes::DEPOSIT_GENERATOR {
                    deposit_m = (relief * sand) as f32;
                } else { top += relief * sand; }
            }
        }
        if carved_surface(cfg.generator_version) {
            carve::publish_open(if lake.is_some() || volcanic.active_caldera || volcanic.lake.is_some() { 0.0 }
                else { calm * land * (1.0 - smooth(0.0, 0.5, riparian)) });
        }
        let t = c.lifted((top - SEA).max(0.0));
        geo::Ground { deposit_material: if cfg.generator_version == dunes::DEPOSIT_GENERATOR { BlockId::Sand } else { BlockId::Stone }, deposit_bed_top: 0.0, deposit_bed_slope: 0.0, deposit_m, top_m: top as f32,
            slope: slope as f32,
            temperature: t.heat01() as f32,
            moisture: (c.wet + riparian * 0.2).min(1.0) as f32,
            sediment: sediment as f32,
            maat_c: t.annual_c as f32, permafrost: t.annual_c < geo::PERMAFROST_C,
            water_m: water.filter(|w| !lava_lake && *w > top).map(|w| w as f32), riparian: riparian as f32,
            delta: delta as f32, delta_mud: delta_mud as f32, suspended: suspended as f32,
            intertidal: intertidal_exposure as f32,
            scree_m: scree_m as f32, outlook_clearing,
            placer: placer as f32, river_gravel, river_bedrock }
    }

    fn regional_view(&self) -> geo::Planet {
        let n = topology::SIDE;
        let grid = geo::grid::Grid { n, cell_m: 2.0 * atlas::HALF_M / n as f64, min_m: [-atlas::HALF_M; 2] };
        let mut height = Vec::with_capacity(n * n);
        let mut lake = Vec::with_capacity(n * n);
        let mut discharge = Vec::with_capacity(n * n);
        let mut maat = Vec::with_capacity(n * n);
        let mut wet = Vec::with_capacity(n * n);
        let mut season = Vec::with_capacity(n * n);
        let mut node = Vec::with_capacity(n * n);
        for i in 0..n * n {
            let [x, z] = grid.at(i);
            let d = Projection::Tangent.to_sphere(Face::PosX, x / atlas::HALF_M, z / atlas::HALF_M);
            let h = self.mesh.sample(&self.height, d);
            let s = self.mesh.sample(&self.flow.spill, d);
            let c = self.climate(d);
            height.push(h as f32);
            lake.push(if s > h + 0.75 { s as f32 } else { f32::MIN });
            discharge.push(self.mesh.sample(&self.flow.discharge, d) as f32);
            maat.push(c.annual_c as f32);
            wet.push(c.wet as f32);
            season.push((c.annual_c - c.winter_c).max(0.0) as f32);
            node.push([x as f32, z as f32]);
        }
        geo::Planet { grid, history: None, land: height.iter().map(|h| *h > SEA as f32).collect(),
            height, lake, discharge, maat, wet, season, node,
            // Regional views have no independent flow network. Consumers of
            // drainage use this module's full spherical receiver graph.
            receiver: (0..n * n).map(|i| i as u32).collect(), ice: vec![0.0; n * n], checksum: self.checksum }
    }
}

pub fn ground(cfg: &WorldSampleConfig, x: f32, z: f32, footprint: f32) -> geo::Ground {
    get(cfg).ground(cfg, direction(cfg, f64::from(x), f64::from(z)), footprint)
}

/// Build-local probe for whether a patch can overlap the baked meander.
pub fn meander_coverage(cfg: WorldSampleConfig) -> impl Fn(f32, f32, f32) -> bool {
    let world = (cfg.world_type == WorldType::Planet
        && cfg.generator_version >= engine_voxel::FLOODPLAIN_MEANDER_GENERATOR).then(|| get(&cfg));
    move |x, z, radius| {
        world.as_ref().and_then(|w| w.alluvium.as_ref()).is_some_and(|network| {
            let p = direction(&cfg, f64::from(x), f64::from(z)) * atlas::RADIUS_M;
            network.meander_overlaps(p, f64::from(radius))
        })
    }
}

/// Reject a whole surface plot if its footprint reaches the active summit.
pub(crate) fn reserves_volcanic_summit(cfg: &WorldSampleConfig, x: f32, z: f32, margin: f32) -> bool {
    cfg.world_type == WorldType::Planet
        && cfg.generator_version >= engine_voxel::VOLCANIC_LANDFORM_PLANET_GENERATOR
        && get(cfg).volcanoes.reserves_summit(direction(cfg, f64::from(x), f64::from(z)), f64::from(margin))
}

pub fn climate_at(cfg: &WorldSampleConfig, dir: DVec3) -> crate::planet_climate::Climate { get(cfg).climate(dir) }

/// Regional surface forcing. Sample the watershed mesh directly, without
/// evaluating voxel columns, caves, materials or vegetation.
pub struct WeatherSite {
    pub land: f32,
    pub relief: f32,
    pub moisture: f32,
    pub slope: DVec3,
}

impl World {
    fn weather_site(&self, dir: DVec3) -> WeatherSite {
        let (ids, weights) = self.mesh.stencil(dir);
        let mut height = 0.0;
        let mut moisture = 0.0;
        let mut slope = DVec3::ZERO;
        for (i, w) in ids.into_iter().zip(weights) {
            height += self.height[i] * w;
            moisture += self.rain[i] * w;
            slope += self.gradient[i] * w;
        }
        WeatherSite { land: smooth(SEA - 12.0, SEA + 12.0, height) as f32,
            relief: ((height - SEA) / 450.0).clamp(0.0, 1.0) as f32,
            moisture: moisture.clamp(0.0, 1.0) as f32, slope }
    }
}

/// Render-thread callers never start or wait on a planet build.
pub fn ready_weather_site(cfg: &WorldSampleConfig, dir: DVec3) -> Option<WeatherSite> {
    if !active(cfg) { return None; }
    let key = (cfg.seed, cfg.planet_fingerprint(), cfg.generator_version);
    if let Some(site) = HOT.with(|hot| {
        hot.borrow().as_ref().filter(|(k, _)| *k == key)
            .map(|(_, s)| s.world.get().map(|world| world.weather_site(dir)))
    }) {
        return site;
    }
    Some(slot(cfg).world.get()?.weather_site(dir))
}

/// Worker-only initialization: fill the solver's static surface once per world.
pub fn weather_surface(cfg: &WorldSampleConfig, grid: &crate::sphere::SphereGrid) -> crate::sphere::Surface {
    let mut surface = crate::sphere::Surface::ocean(grid.len());
    if !active(cfg) { return surface; }
    let world = get(cfg);
    for i in 0..grid.len() {
        let s = world.weather_site(grid.centre(i));
        surface.land[i] = s.land;
        surface.relief[i] = s.relief;
        surface.land_moisture[i] = s.moisture;
    }
    surface
}

/// Find dry ground on the actual sphere, including land beyond the origin face.
pub fn spawn_point(cfg: &WorldSampleConfig) -> DVec3 {
    let w = get(cfg);
    if reference_relief::active(cfg) { return reference_relief::spawn(); }
    if let Some(at) = w.eroded_region.as_ref().and_then(review::catchment::PlayableRegion::entry) {
        let dir = direction(cfg, at.x, at.y);
        let g = w.ground(cfg, dir, cfg.voxel_meters());
        if g.water_m.is_none() && g.top_m > SEA as f32 + 3.0 && g.slope <= 0.35 {
            return DVec3::new(at.x, f64::from(g.top_m) + 2.0, at.y);
        }
    }
    let mut candidates: Vec<_> = w.mesh.directions.iter().enumerate()
        .filter(|(i, _)| w.height[*i] > SEA + 6.0 && w.flow.spill[*i] <= w.height[*i] + 0.5)
        .map(|(i, d)| ((d - DVec3::X).length_squared(), i)).collect();
    candidates.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, i) in candidates.into_iter().take(128) {
        let d = w.mesh.directions[i];
        let p = atlas::from_body(d * atlas::RADIUS_M);
        let g = w.ground(cfg, d, cfg.voxel_meters());
        if g.water_m.is_some() || g.top_m < SEA as f32 + 3.0 || g.slope > 0.35 { continue; }
        let v = w.volcano_shape(d);
        if v.heat > 0.2 || v.lake.is_some() { continue; }
        return DVec3::new(p.x, f64::from(g.top_m) + 2.0, p.z);
    }
    let g = w.ground(cfg, DVec3::X, cfg.voxel_meters());
    DVec3::new(0.0, f64::from(g.top_m).max(SEA) + 2.0, 0.0)
}

pub fn tectonics_at(cfg: &WorldSampleConfig, dir: DVec3) -> crate::volcano::Tectonics {
    let p = get(cfg).plates.at(dir);
    crate::volcano::Tectonics { boundary_m: p.boundary_m, convergence: p.convergence }
}

pub fn vents_near(cfg: &WorldSampleConfig, dir: DVec3, reach_m: f64) -> Vec<crate::volcano::Vent> {
    get(cfg).volcanoes.vents.iter().filter(|v| (v.dir - dir).length() * atlas::RADIUS_M < reach_m + v.radius_m * 1.6).copied().collect()
}

/// The rock supporting a province also controls its erosion and exposed cliffs.
pub fn rock_at(cfg: &WorldSampleConfig, p: glam::IVec3) -> BlockId {
    let w = get(cfg);
    let dir = direction(cfg, f64::from(cfg.world_x(p.x)), f64::from(cfg.world_z(p.z)));
    rock_in(&w, cfg, dir, f64::from(cfg.world_y(p.y)))
}

fn rock_in(w: &World, cfg: &WorldSampleConfig, dir: DVec3, elevation: f64) -> BlockId {
    if let Some(rock) = w.volcanoes.reef_material(dir, elevation) { return rock; }
    if let Some(rock) = w.eroded_region.as_ref().and_then(|region| region.rock_at(dir, elevation)) {
        return rock;
    }
    if let Some(column) = w.history_strata(cfg, dir) { return column.block(elevation); }
    if w.edifice_lift(dir) > 12.0 { return BlockId::Basalt; }
    let carbonate = w.mesh.sample(&w.carbonate, dir);
    let collision = w.mesh.sample(&w.collision, dir);
    let rift = w.mesh.sample(&w.rift, dir);
    let body = dir * atlas::RADIUS_M;
    if rift > 0.25 { return BlockId::Basalt; }
    let offset = if w.fault_offset.is_empty() { 0.0 } else { w.mesh.sample(&w.fault_offset, dir) };
    if cfg.generator_version == LAYERED_CANYON_GENERATOR {
        return strata::Column::new(cfg.seed, body, collision, carbonate, rift, offset).block(elevation);
    }
    let bed = bedding_height(cfg, body, collision, elevation - offset);
    if cfg.generator_version >= engine_voxel::BEDROCK_RIVER_GENERATOR
        && carbonate < 0.58 && collision < 0.48
    {
        // A broad, pinching peat lens inside the same folded package as
        // the sandstone. Its datum is independent of the modern river.
        let basin = noise(cfg.seed, body / 1800.0, 238_013);
        let centre = 34.0 + (noise(cfg.seed, body / 440.0, 238_021) - 0.5) * 5.0;
        let thickness = 2.0 * smooth(0.37, 0.64, basin);
        if (bed - centre).abs() < thickness * 0.5 {
            return BlockId::CoalOre;
        }
    }
    if cfg.generator_version >= engine_voxel::BEDDING_PLANET_GENERATOR {
        if (10.0..38.0).contains(&bed) {
            if carbonate > 0.5 { return BlockId::Limestone; }
            if collision < 0.35 { return BlockId::Sandstone; }
        }
        return BlockId::Stone;
    }
    if carbonate > 0.5 && bed > 8.0 { BlockId::Limestone }
    else if collision < 0.18 && bed > 32.0 { BlockId::Sandstone }
    else { BlockId::Stone }
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> ColumnSample {
    column_with_water(cfg, x, z, footprint).0
}

/// First-pass ground ice in wet, cold soils. Snow and bedrock are independent:
/// freezing alone does not create massive ice. The shallow summer thaw layer
/// remains ordinary soil and physical wavelengths are shared across faces.
pub(crate) fn ground_ice(cfg: &WorldSampleConfig, p: glam::IVec3, column: &ColumnSample) -> bool {
    if cfg.generator_version < engine_voxel::PERMAFROST_PLANET_GENERATOR
        || column.filler != BlockId::Dirt
        || cfg.world_y(column.height) <= crate::WATER_LEVEL_METERS + 2.0
    {
        return false;
    }
    let depth = f64::from(cfg.world_y(column.height - p.y));
    if !(0.5..=3.0).contains(&depth) { return false; }
    let dir = direction(cfg, f64::from(cfg.world_x(p.x)), f64::from(cfg.world_z(p.z)));
    let c = get(cfg).climate(dir).lifted(f64::from(cfg.world_y(column.height)) - SEA);
    if c.annual_c >= geo::PERMAFROST_C || c.wet < 0.45 { return false; }
    let summer_c = 2.0 * c.annual_c - c.winter_c;
    // A bounded thaw-depth proxy, not a transient soil temperature solver.
    let active_layer_m = (0.45 + 0.32 * summer_c.max(0.0).sqrt()).min(2.0);
    if depth < active_layer_m { return false; }
    let body = dir * atlas::RADIUS_M;
    let lens = noise(cfg.seed, body / 24.0 + dir * (depth / 1.6), 1361);
    lens > 0.68 - (c.wet - 0.45) * 0.14
}

/// Terrain and inland water elevation from the same filtered ground sample.
pub fn column_with_water(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> (ColumnSample, Option<f32>) {
    let (column, head, _) = column_with_coastal_water(cfg, x, z, footprint);
    (column, head)
}

/// The same terrain and head, plus suspended sediment for the distant water pass.
pub fn column_with_coastal_water(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> (ColumnSample, Option<f32>, f32) {
    let (column, head, ground) = column_ground(cfg, x, z, footprint, true);
    (column, head, ground.suspended)
}

/// One-query appearance metadata. Deliberately has no physical slope:
/// placement callers use ground(), while meshes derive normals from heights.
#[derive(Clone, Copy)]
pub struct SurfaceMetadata {
    pub top_m: f32,
    pub water_m: Option<f32>,
    pub temperature: f32,
    pub moisture: f32,
    pub suspended: f32,
}

/// Surface appearance and relief from the same query as the playable column.
/// No cave plans or neighboring slope queries are needed for orbital baking.
pub fn orbital_surface(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> (ColumnSample, Option<f32>, SurfaceMetadata) {
    let (column, head, g) = column_ground(cfg, x, z, footprint, false);
    (column, head, SurfaceMetadata { top_m: g.top_m, water_m: g.water_m,
        temperature: g.temperature, moisture: g.moisture, suspended: g.suspended })
}

/// Exact cover inputs from one terrain query, including the unrounded bed.
pub fn cover_column(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32) -> (ColumnSample, Option<f32>, f32) {
    let (column, _, ground) = column_ground(cfg, x, z, footprint, false);
    if column.surface == BlockId::Lava {
        return (column, None, 0.0);
    }
    let bed = (ground.top_m - cfg.world_y(column.height)).clamp(0.0, cfg.voxel_meters());
    (column, ground.water_m, bed)
}

fn column_ground(cfg: &WorldSampleConfig, x: i32, z: i32, footprint: f32, with_caves: bool) -> (ColumnSample, Option<f32>, geo::Ground) {
    let world = get(cfg);
    let dir = direction(cfg, f64::from(cfg.world_x(x)), f64::from(cfg.world_z(z)));
    if reference_relief::active(cfg) && let Some(ground) = reference_relief::ground(dir) {
        let height = cfg.clamp_height((ground.top_m / cfg.voxel_meters()).floor() as i32);
        return (ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height, surface: BlockId::Sandstone, filler: BlockId::Sandstone,
            vegetation_density: 0.0, cave: Default::default() }, None, ground);
    }
    let volcanic = world.volcano_shape(dir);
    let final_ground = world.ground_shaped_height(cfg, dir, footprint, volcanic);
    // Material identity is evaluated at the buried bed, not at the new crest.
    let g = if final_ground.deposit_m > 0.0 {
        geo::Ground { top_m: final_ground.deposit_bed_top,
            slope: final_ground.deposit_bed_slope, ..final_ground }
    } else { final_ground };
    let c = world.climate(dir);
    let vm = f64::from(cfg.voxel_meters());
    let elevation = (f64::from(g.top_m) / vm).floor() * vm;
    let rock = || rock_in(&world, cfg, dir, elevation);
    let mut cover = crate::planet_climate::cover(c, f64::from(g.top_m), SEA + 2.0);
    if matches!(cover.0, BlockId::Stone) {
        cover.0 = rock();
        cover.1 = cover.0;
    }
    if let Some(floor) = crate::planet_ocean::floor_cover(SEA - f64::from(g.top_m)) { cover = floor; }
    if cfg.generator_version >= engine_voxel::COASTAL_PLANET_GENERATOR {
        let shore = f64::from(g.top_m) - SEA;
        let clean = clean_riverbeds(cfg.generator_version);
        if (-3.0..=6.0).contains(&shore) && g.slope > if clean { 0.6 } else { 0.33 } {
            // Exposed headlands inherit their province's rock, rather than
            // drawing a sand ribbon through steep coastal cliffs.
            let bed = rock();
            cover = (bed, bed, 0.0);
        } else if (-1.5..=1.5).contains(&shore) && g.slope < 0.06 && g.sediment > 0.45 {
            cover = (BlockId::Mud, BlockId::Clay, 0.0);
        } else if clean && ((-2.0..=4.0).contains(&shore)
                // Generator 295 dune ridges carry the beach sand inland over loose cover.
                || (living_surface(cfg.generator_version) && (4.0..=11.0).contains(&shore) && g.sediment > 0.45))
            && g.slope < 0.25 && c.winter_c > -12.0 && cover.0 != BlockId::Ice
        {
            // Swash and wind keep a sand beach on gentle shores.
            cover = (BlockId::Sand, BlockId::Sand, 0.0);
        }
    }
    let coastal_relief = cfg.generator_version >= engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR;
    let (deposit_limit, mud_limit, shore_limit) = if coastal_relief && g.delta > 0.1 {
        // Mix the transition over a few metres of bank rather than outlining
        // a single isobath across the entire sediment field.
        let grain = (noise(cfg.seed, dir * atlas::RADIUS_M / 23.0, 1471) - 0.5) as f32;
        (0.35 + grain * 0.32, 0.34 + grain * 0.16, 2.5 + grain * 2.0)
    } else { (0.35, 0.34, 2.5) };
    let mudflat = g.intertidal > 0.25
        && ((SEA as f32 - 1.4)..=(SEA as f32 + 1.2)).contains(&g.top_m);
    if mudflat {
        cover = (BlockId::Mud, BlockId::Clay, 0.0);
    } else if g.delta > deposit_limit && c.winter_c > -12.0 && g.top_m <= SEA as f32 + shore_limit {
        cover = if g.delta_mud >= mud_limit { (BlockId::Mud, BlockId::Clay, 0.0) }
            else { (BlockId::Sand, BlockId::Sand, 0.0) };
    }
    else if g.water_m.is_some_and(|head| !clean_riverbeds(cfg.generator_version) || head - g.top_m > 0.5)
        && g.top_m > SEA as f32 { cover = (BlockId::Gravel, BlockId::Stone, 0.0); }
    // Generator 296 crevasse splays and emergent bars: fresh sand.
    else if lowland_rivers(cfg.generator_version) && g.delta > 0.45 && g.delta_mud < 0.2
        && g.water_m.is_none_or(|head| head - g.top_m <= 0.3) && g.slope < 0.15
        && c.winter_c > -12.0 { cover = (BlockId::Sand, BlockId::Sand, 0.0); }
    // Generator 296 backswamp marsh: shallow standing water on a muddy floor.
    else if lowland_rivers(cfg.generator_version) && g.slope < 0.08 && g.riparian > 0.3
        && g.water_m.is_some_and(|head| head > g.top_m && head - g.top_m <= 0.5)
        && c.winter_c > -12.0 { cover = (BlockId::Mud, BlockId::Clay, cover.2); }
    else if g.scree_m > 0.5 && cover.0 != BlockId::Ice {
        let body = dir * atlas::RADIUS_M;
        let fragments = noise(cfg.seed, body / 7.0, 1249) as f32;
        if g.scree_m > 0.9 + fragments * 1.6 {
            // Angular fragments inherit rock from the bed above the apron.
            let surface = if fragments > 0.57 && g.scree_m > 2.0 {
                rock_in(&world, cfg, dir, f64::from(g.top_m) + 12.0)
            } else {
                BlockId::Gravel
            };
            let filler = if g.scree_m > 2.5 { BlockId::Gravel } else { rock() };
            cover = (surface, filler, 0.0);
        } else {
            cover.2 *= 0.35;
        }
    }
    else if cfg.generator_version >= engine_voxel::BEDDING_PLANET_GENERATOR
        && g.top_m > SEA as f32 + 6.0 && cover.0 != BlockId::Ice
        && g.slope > if crate::trees::rainforest::active(cfg) && c.annual_c > 18.0 && c.wet > 0.68 {
            0.78
        } else if clean_riverbeds(cfg.generator_version) {
            // Short terrace risers left one-voxel rock lines at 0.52.
            0.75
        } else {
            0.52
        }
    {
        let bed = rock();
        cover = (bed, bed, 0.0);
    }
    else if g.riparian > 0.3 && g.top_m > SEA as f32 + 2.0 && c.annual_c > 2.0 {
        cover.0 = BlockId::Grass;
        cover.1 = BlockId::Dirt;
        cover.2 = (cover.2 + 0.15).min(0.85);
    }
    if g.water_m.is_none() && !matches!(cover.0, BlockId::Ice | BlockId::SnowGrass)
        && let Some(bed) = world.eroded_region.as_ref().and_then(|region| region.outcrop(dir, elevation))
    {
        cover = (bed, bed, 0.0);
    }
    if crate::trees::rainforest::forest_floor(cfg) && cfg.world_type == WorldType::Planet
        && g.water_m.is_none() && g.top_m > SEA as f32 + 2.2 && g.maat_c > 18.0 && g.moisture > 0.69
        && matches!(cover.0, BlockId::Grass | BlockId::Dirt | BlockId::Mud)
    {
        if carved_surface(cfg.generator_version) {
            // Generator 300: mud where water stands (flat, low on the local
            // relief), moss on drained rises, in patches of mixed size.
            let (wet, rise) = carve::floor_patches(cfg.seed, dir * atlas::RADIUS_M);
            let flat = 1.0 - smooth(0.10, 0.24, f64::from(g.slope));
            if wet * flat > 0.5 { (cover.0, cover.1) = (BlockId::Mud, BlockId::Clay); }
            else if rise > 0.5 && g.slope < 0.49 { (cover.0, cover.1) = (BlockId::GroveMoss, BlockId::Dirt); }
        } else {
            let (swale, mound) = rainforest_floor(cfg, dir * atlas::RADIUS_M);
            if let Some((surface, filler)) = crate::trees::rainforest::floor_cover(swale, mound, g.slope) {
                (cover.0, cover.1) = (surface, filler);
            }
        }
    }
    if let Some(volcanic_cover) = crate::volcano::cover(&volcanic, noise(cfg.seed, dir * 900.0, 619)) {
        if volcanic.lake.is_some() || g.top_m > SEA as f32 { cover = volcanic_cover; }
    }
    // Expose the solved carbonate framework and transported sand beneath
    // shallow water too; generic ocean-floor cover must not hide every reef.
    if volcanic.lake.is_none() && g.top_m <= SEA as f32 + 2.0
        && !matches!(cover.0, BlockId::Ice | BlockId::SnowGrass)
        && let Some(surface) = world.volcanoes.reef_material(dir, elevation)
    {
        cover = (surface, rock_in(&world, cfg, dir, elevation - vm), 0.0);
    }
    if g.river_gravel {
        cover = (BlockId::Gravel, BlockId::Gravel, 0.0);
    }
    if g.placer >= crate::placer::PAYDIRT_GRADE && g.delta < 0.1 && volcanic.lake.is_none()
        && g.top_m > SEA as f32 && matches!(cover.0, BlockId::Gravel | BlockId::Grass | BlockId::Dirt)
    {
        // Wet riffles expose the pay streak. Low banks bury it beneath clean gravel.
        cover = if g.water_m.is_some() {
            (BlockId::Paydirt, BlockId::Paydirt, 0.0)
        } else if clean_riverbeds(cfg.generator_version) && cover.0 != BlockId::Gravel {
            // Dry pay streaks lie under the turf; a 100 m grey band read as a road.
            (cover.0, BlockId::Paydirt, cover.2)
        } else {
            (BlockId::Gravel, BlockId::Paydirt, 0.0)
        };
    }
    if g.outlook_clearing > 0.0 {
        cover.2 *= 1.0 - g.outlook_clearing;
    }
    if landscape_cycles(cfg.generator_version) && cover.2 > 0.0 && g.water_m.is_none() {
        let stand = ecoregion::stand(cfg.seed, dir, g.maat_c, g.moisture, g.slope, g.riparian);
        // Keep all existing shoreline, scree and rock exclusions. Regional
        // canopy and real forest soil replace uniform tree occupancy.
        cover.2 *= 0.08 + 1.42 * stand.canopy + if living_surface(cfg.generator_version) {
            // Open woodland and lone trees between the stands, where climate allows trees at all.
            0.34 * ecoregion::potential(g.maat_c, g.moisture, g.slope) * (1.0 - stand.regeneration)
        } else { 0.0 };
        // Generator 300 frays the stand floor's edge; the bare canopy
        // threshold drew smooth ovals of moss on grass.
        let fray = if carved_surface(cfg.generator_version) {
            let body = dir * atlas::RADIUS_M;
            ((noise(cfg.seed, body / 19.0, 300_501) - 0.5) * 0.26 + (noise(cfg.seed, body / 6.0, 300_503) - 0.5) * 0.12) as f32
        } else { 0.0 };
        if cover.0 == BlockId::Grass && stand.canopy + fray > 0.50 && stand.regeneration < 0.15 {
            cover.0 = if g.moisture > 0.72 { BlockId::GroveMoss } else { BlockId::Dirt };
        }
    }
    let top = volcanic.lake.map_or(f64::from(final_ground.top_m), |(level, _)| level);
    let mut height = if volcanic.lake.is_some() { (top / vm).ceil() as i32 - 1 }
        else { (top / vm).floor() as i32 };
    if volcanic.lake.is_none() && let Some(head) = g.water_m {
        // A solid voxel occupies [height, height + 1]. Leave a water cell
        // below the hydraulic head even when a shallow bed rounds upward.
        height = height.min((f64::from(head) / vm).ceil() as i32 - 2);
    }
    let height = cfg.clamp_height(height);
    if g.river_bedrock {
        let bed = rock_in(&world, cfg, dir, f64::from(cfg.world_y(height)));
        let below = rock_in(&world, cfg, dir, f64::from(cfg.world_y(height - 1)));
        cover = (bed, below, 0.0);
    }
    if matches!(cfg.generator_version, VALLEY_FORM_GENERATOR | SILL_EROSION_GENERATOR)
        && g.water_m.is_none() && volcanic.lake.is_none()
        && let Some(deposit) = world.eroded_region.as_ref().and_then(|region| region.valley_cover(dir))
    {
        cover = (deposit, cover.1, cover.2 * 0.15);
    }
    if uses_catchment_trails(cfg.generator_version)
        && g.water_m.is_none() && volcanic.lake.is_none()
        && world.eroded_region.as_ref().is_some_and(|region| region.on_trail(dir))
    {
        cover = (BlockId::Gravel, cover.1, 0.0);
    }
    // The same deposit that raised the bed owns both visible cover and
    // excavatable material; bedrock/outcrop rules cannot repaint the deposit.
    let bed_height = cfg.clamp_height((f64::from(g.top_m) / vm).floor() as i32);
    let deposit_m = if volcanic.lake.is_none() && !g.river_bedrock && g.deposit_m > 0.0 {
        (height - bed_height).max(0) as f32 * cfg.voxel_meters()
    } else { 0.0 };
    let deposit_bed_surface = cover.0;
    if deposit_m > 0.0 { cover = (g.deposit_material, cover.1, 0.0); }
    (ColumnSample { deposit_m, deposit_bed_surface, height, surface: cover.0, filler: cover.1,
        vegetation_density: (cover.2 * cfg.planet_settings().map_or(1.0, |s| (s.unit(7) * 2.0) as f32)).min(1.0),
        cave: if with_caves && footprint <= 32.0 {
            let cave_height = if cfg.generator_version == erratics::ERRATIC_DEPOSIT_GENERATOR && deposit_m > 0.0 { bed_height } else { height };
            crate::cave_column(cfg, x, z, cave_height)
        } else { Default::default() } },
        match volcanic.lake {
            Some((head, BlockId::Water)) => Some(head as f32),
            Some(_) => None,
            None => g.water_m,
        }, final_ground)
}

/// Spherical caves use the same body-space wavelengths on every chart.
pub fn cave_carves(cfg: &WorldSampleConfig, p: glam::IVec3, column: &ColumnSample) -> bool {
    let y = f64::from(cfg.world_y(p.y));
    let depth = f64::from(cfg.world_y(column.height)) - y;
    if ground_breaks(cfg.generator_version)
        && depth >= 0.0
        && depth < f64::from(ground_breaks::MAX_DEPTH)
        && y > SEA + f64::from(ground_breaks::FREEBOARD)
        && ground_breaks::carves(cfg, p, column.height, y as f32, depth as f32)
    {
        return true;
    }
    if depth < 4.0 || y < f64::from(cfg.world_y(cfg.min_world_y())) + 4.0 { return false; }
    let dir = direction(cfg, f64::from(cfg.world_x(p.x)), f64::from(cfg.world_z(p.z)));
    let body = dir * (atlas::RADIUS_M + y);
    let a = (noise(cfg.seed, body / 46.0, 1103) * 2.0 - 1.0).abs();
    let b = (noise(cfg.seed, body / 46.0, 1109) * 2.0 - 1.0).abs();
    a < 0.16 && b < 0.16
}

pub fn river_sources(cfg: &WorldSampleConfig, center: glam::Vec2, radius: f32) -> Vec<crate::drainage::Spring> {
    let w = get(cfg);
    let source_area = w.source_area();
    let mut parent = vec![false; w.height.len()];
    for i in 0..parent.len() {
        if w.flow.discharge[i] >= source_area && w.height[i] > SEA && w.flow.receiver[i] != i { parent[w.flow.receiver[i]] = true; }
    }
    let mut sources = Vec::new();
    for (i, &has_parent) in parent.iter().enumerate() {
        if has_parent || w.height[i] <= SEA || w.flow.discharge[i] < source_area
            || w.flow.receiver[i] == i || w.flow.spill[i] > w.height[i] + 0.75 { continue; }
        let p = atlas::from_body(w.river_nodes[i]);
        if (glam::Vec2::new(p.x as f32, p.z as f32) - center).length_squared() > radius * radius { continue; }
        let g = w.ground(cfg, w.river_nodes[i].normalize(), cfg.voxel_meters());
        if let Some(level) = g.water_m {
            sources.push(crate::drainage::Spring { position_m: glam::Vec3::new(p.x as f32, level, p.z as f32),
                microlitres_per_second: (w.flow.discharge[i] * 20.0).clamp(8e6, 240e6) as u64 });
        }
    }
    sources
}

/// Transfer a coarse upstream catchment to the active voxel domain.
/// Multiple wet columns share the same discharge across the inlet section.
pub fn river_inlets(cfg: &WorldSampleConfig, centre: glam::Vec2, radius: f32) -> Vec<crate::drainage::Inlet> {
    let world = get(cfg);
    let Some(network) = &world.alluvium else { return Vec::new() };
    let scale = cfg.voxel_meters();
    let mut inlets: Vec<crate::drainage::Inlet> = Vec::new();
    for (p, direction, area, width) in network.incoming_reaches(centre.as_dvec2(), f64::from(radius)) {
        let across = direction.perp();
        let width = width.clamp(1.2, 12.0);
        let weather_point = (p - direction * 400.0).as_vec2();
        let mut section = Vec::new();
        for u in [0.0, -0.35, 0.35, -0.65, 0.65] {
            let point = (p + across * (width * u)).as_vec2();
            let x = (point.x / scale).floor() * scale;
            let z = (point.y / scale).floor() * scale;
            if section.iter().any(|q: &glam::Vec3| q.x == x && q.z == z) { continue; }
            let dir = self::direction(cfg, f64::from(x), f64::from(z));
            let ground = world.ground(cfg, dir, scale);
            if let Some(head) = ground.water_m.filter(|h| *h > SEA as f32 + 0.5 && *h - ground.top_m > 0.20) {
                section.push(glam::Vec3::new(x, head, z));
            }
        }
        if section.is_empty() { continue; }
        let rate = (area * 20.0).clamp(8e6, 2.4e9) as u64 / section.len() as u64;
        for position_m in section {
            if inlets.iter().any(|inlet| inlet.source.position_m.x == position_m.x
                && inlet.source.position_m.z == position_m.z) { continue; }
            inlets.push(crate::drainage::Inlet {
                source: crate::drainage::Spring { position_m, microlitres_per_second: rate },
                weather_point,
            });
        }
    }
    inlets
}

#[cfg(test)]
mod ground_stencil_tests;

#[cfg(test)]
mod history_stencil_tests;

#[cfg(test)]
mod ground_reconstruction_tests;
