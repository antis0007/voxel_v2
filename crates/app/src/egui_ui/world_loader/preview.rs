//! Review fixture for `--capture-menu`: the screen part-way through a load.
//!
//! The map is real generator output; stage states and clocks are a fixture
//! replayed through the same timeline so estimates come from its rates.
use std::sync::{Arc, OnceLock};

use engine_voxel::{CURRENT_GENERATOR_VERSION, WorldSampleConfig, WorldType};

use super::{GlobeMap, StageRow, StageState, WorldLoadReport, sphere, survey, timeline};

const SEED: u32 = 1337;
/// Review fixtures run the shortest history the slider allows, so captures do
/// not wait on the full 720 Myr solve. `VOXEL_HISTORY_MYR` still wins.
const REVIEW_HISTORY_MYR: u16 = *engine_voxel::HISTORY_MYR_RANGE.start();

fn review_planet() -> Option<engine_voxel::PlanetSettings> {
    Some(engine_voxel::PlanetSettings { history_myr: REVIEW_HISTORY_MYR, ..Default::default() })
}

fn row(key: &'static str, name: &'static str, state: StageState, count: Option<(u64, u64)>, unit: &'static str) -> StageRow {
    let fraction = count.map(|(d, n)| d as f32 / n.max(1) as f32);
    StageRow { key, name, group: super::stage_group(key), state, count, unit, fraction }
}

/// Mid-solve drainage preview of the review planet, waited for once.
fn solve_map() -> GlobeMap {
    static MAP: OnceLock<GlobeMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let cfg = WorldSampleConfig {
            planet: review_planet(),
            generator_version: CURRENT_GENERATOR_VERSION,
            seed: SEED,
            world_type: WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        };
        if !engine_worldgen::geo::active(&cfg) {
            return GlobeMap::Empty;
        }
        engine_worldgen::geo::prewarm_for(cfg);
        let entry = engine_worldgen::geo::entry_for(&cfg);
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 180 && !entry.ready() && entry.progress.steps().0 < 84 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        entry.progress.preview().map_or(GlobeMap::Empty, GlobeMap::Solve)
    })
    .clone()
}

fn hills() -> WorldSampleConfig {
    WorldSampleConfig {
        planet: review_planet(),
        generator_version: CURRENT_GENERATOR_VERSION,
        seed: SEED,
        world_type: WorldType::Hills,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    }
}

/// Finished solve of the review world, waited for once.
fn solved_map() -> GlobeMap {
    static MAP: OnceLock<GlobeMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let cfg = hills();
        if !engine_worldgen::geo::active(&cfg) {
            return GlobeMap::Empty;
        }
        GlobeMap::Solved(engine_worldgen::geo::planet_for(&cfg))
    })
    .clone()
}

fn survey_map() -> GlobeMap {
    static MAP: OnceLock<Arc<survey::Survey>> = OnceLock::new();
    let s = MAP.get_or_init(|| {
        let cfg = WorldSampleConfig {
            planet: review_planet(),
            generator_version: CURRENT_GENERATOR_VERSION,
            seed: SEED,
            world_type: WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        };
        survey::blocking(cfg, (0, 0))
    });
    GlobeMap::Survey(Arc::clone(s))
}

/// `world-load-history-<k>`: a real deep-time history for the fixture seed,
/// held at step `k` of 4 across one frame interval three fifths through.
/// Uses the short review history; `VOXEL_HISTORY_MYR` overrides it.
pub fn history_report(label: &str) -> Option<(WorldLoadReport, f64)> {
    let k: u32 = label.strip_prefix("world-load-history-")?.parse().ok()?;
    static FRAMES: OnceLock<Vec<Arc<engine_worldgen::planetary::DriftFrame>>> = OnceLock::new();
    let frames = FRAMES.get_or_init(|| {
        let cfg = WorldSampleConfig {
            planet: review_planet(),
            generator_version: engine_voxel::PLANET_GENERATOR_VERSION,
            seed: SEED,
            world_type: WorldType::Planet,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        };
        engine_worldgen::planetary::review::history::frames(&cfg)
    });
    if frames.len() < 2 { return None; }
    let i = (frames.len() * 3 / 5).min(frames.len() - 2);
    let (a, b) = (f64::from(frames[i].myr), f64::from(frames[i + 1].myr));
    let mut report = report("world-load-planet-stream");
    report.world_key = 0xF1C7_4000;
    report.drift = frames.clone();
    Some((report, a + (b - a) * f64::from(k.min(4)) / 4.0))
}

/// Starts every heavy fixture on its own thread, so the solves overlap each
/// other and the cheap captures that come first; each label then only waits.
pub fn prewarm() {
    for job in [solve_map as fn() -> GlobeMap, solved_map, survey_map, sphere_map] {
        std::thread::spawn(move || drop(job()));
    }
    std::thread::spawn(|| drop(history_report("world-load-history-0")));
}

/// Captures only: hold the history playback at `myr`.
pub fn pin_history(ctx: &egui::Context, report: &WorldLoadReport, myr: f64, now: f64) {
    super::geology::pin(ctx, report.world_key, myr, now);
}

fn sphere_map() -> GlobeMap {
    static MAP: OnceLock<Arc<sphere::SphereSurvey>> = OnceLock::new();
    let s = MAP.get_or_init(|| {
        let cfg = WorldSampleConfig {
            planet: review_planet(),
            generator_version: engine_voxel::PLANET_GENERATOR_VERSION,
            seed: SEED,
            world_type: WorldType::Planet,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        };
        sphere::blocking(cfg, (0, 0))
    });
    GlobeMap::Sphere(Arc::clone(s))
}

/// Report for the capture label: `world-load` mid-solve, `world-load-stream`
/// after the solve while spawn terrain and the far field build. `solved` shows
/// the finished solve, `survey` the column survey, `planet` the cube-sphere
/// survey; `pole` and `seam` hold the planet at a pole or a cube corner.
pub fn report(label: &str) -> WorldLoadReport {
    use StageState::{Done, Queued, Running};
    let streaming = label.contains("stream");
    let solve = if streaming { Done } else { Running };
    let carve = if streaming { (60, 60) } else { (24, 60) };
    let stages = vec![
        row("fixture-plates", "Tectonic plates", Done, None, ""),
        row("fixture-uplift", "Uplift", Done, Some((60, 60)), "steps"),
        row("fixture-climate", "Climate", Done, None, ""),
        row("fixture-carving", "Erosion", solve, Some(carve), "steps"),
        row("fixture-rivers", "River network", if streaming { Done } else { Queued }, None, ""),
        row("fixture-spawn", "Spawn terrain", if streaming { Running } else { Queued }, Some(if streaming { (17, 27) } else { (0, 27) }), "chunks"),
        row("fixture-base", "Far field base", if streaming { Done } else { Queued }, None, ""),
        row("fixture-pages", "Far field mesh", if streaming { Running } else { Queued }, None, ""),
        row("fixture-upload", "Far field upload", Queued, None, ""),
        row("fixture-globe", "Globe tiles", if streaming { Running } else { Queued }, Some(if streaming { (148, 412) } else { (0, 412) }), "tiles"),
    ];
    let planet = label.contains("planet");
    let map = if planet {
        sphere_map()
    } else if label.contains("survey") {
        survey_map()
    } else if streaming || label.contains("solved") {
        solved_map()
    } else {
        solve_map()
    };
    let pose = if label.contains("pole") {
        Some([0.4, 1.25])
    } else if label.contains("seam") {
        Some([std::f32::consts::FRAC_PI_4, 0.6])
    } else {
        // Captures hold one pose so before and after line up.
        Some([0.0, 0.3])
    };
    let face_km = (engine_worldgen::geo::face_edge_m(&hills()) / 1000.0) as f32;
    let spawn = (!planet).then_some(glam::Vec3::X);
    let report = WorldLoadReport { world_key: 0xF1C7_0000 | u64::from(streaming), seed: SEED, stages, map, drift: Vec::new(), planet: false, face_km, spawn, pose, regional: None, ready: false };
    report.prime();
    report
}

/// Replays the fixture over the 20 s before `now` so clocks and rates exist.
pub fn seed_clocks(ctx: &egui::Context, report: &WorldLoadReport, now: f64) {
    let mut tl = timeline::Timeline::default();
    for k in 0..=40 {
        let t = now - 20.0 + f64::from(k) * 0.5;
        let share = k as f32 / 40.0;
        let rows: Vec<StageRow> = report
            .stages
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut r = r.clone();
                // Finished stages complete one after another across the replay.
                if r.state == StageState::Done && share < (i as f32 + 1.0) * 0.1 {
                    r.state = StageState::Running;
                    r.fraction = r.fraction.map(|_| share / ((i as f32 + 1.0) * 0.1));
                } else if r.state == StageState::Running {
                    r.fraction = r.fraction.map(|f| f * share);
                }
                r
            })
            .collect();
        tl.update(report.world_key, t, &rows, false);
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("world_loader_timeline"), tl));
}

/// In-game planet map over the review planet: survey waited for once, the
/// player a little east of spawn, two waypoints, a fixed height.
pub fn planet_map(ctx: &egui::Context) {
    let cfg = WorldSampleConfig {
        planet: review_planet(),
        generator_version: engine_voxel::PLANET_GENERATOR_VERSION,
        seed: SEED,
        world_type: WorldType::Planet,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let start = std::time::Instant::now();
    while let Some(s) = sphere::request(cfg, (0, 0)) {
        if s.faces() == 6 || start.elapsed().as_secs() > 120 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let input = super::planet_map::Inputs {
        cfg,
        player_voxel: glam::Vec3::new(1800.0, 120.0, -900.0),
        forward: glam::Vec3::new(0.6, 0.0, -0.8),
        altitude_m: Some(3_420.0),
        body: None,
        waypoints: vec![
            (glam::IVec3::new(9000, 80, 4000), "Camp".to_owned(), super::super::map::waypoints::PALETTE[1]),
            (glam::IVec3::new(-6000, 80, -12000), "2".to_owned(), super::super::map::waypoints::PALETTE[2]),
        ],
    };
    let mut open = true;
    super::planet_map::draw(ctx, &mut open, &input);
}
