//! Collects the world-loading stage counters for the generation screen.
use crate::egui_ui::world_loader::{self, GlobeMap, StageRow, StageState, WorldLoadReport, sphere, survey};
use engine_voxel::WorldSampleConfig;
use engine_worldgen::geo::{self, Stage};
use engine_worldgen::planetary::{self, DriftFrame};
use std::sync::Arc;

const SOLVE: [(Stage, &str, &str); 5] = [
    (Stage::Plates, "solve-plates", "Tectonic plates"),
    (Stage::Uplift, "solve-uplift", "Uplift"),
    (Stage::Climate, "solve-climate", "Climate"),
    (Stage::Carving, "solve-carving", "Erosion"),
    (Stage::Rivers, "solve-rivers", "River network"),
];

fn row(key: &'static str, name: &'static str, state: StageState) -> StageRow {
    StageRow { key, name, group: world_loader::stage_group(key), state, count: None, unit: "", fraction: None }
}

fn counted(mut r: StageRow, done: u64, total: u64, unit: &'static str) -> StageRow {
    r.count = Some((done.min(total), total));
    r.unit = unit;
    r.fraction = (total > 0).then(|| done.min(total) as f32 / total as f32);
    r
}

/// Inputs gathered from the main loop for one frame.
pub struct Inputs<'a> {
    pub world: &'a crate::world::World,
    pub gpu: &'a crate::render::Gpu,
    /// Spawn chunks with GPU meshes, out of the total the streamer needs.
    pub spawn: (usize, usize),
    pub spawn_ready: bool,
    pub ready: bool,
    pub player_voxel: glam::Vec3,
}

fn solve_rows(cfg: &WorldSampleConfig, entry: &geo::Entry, drift: &[Arc<DriftFrame>]) -> Vec<StageRow> {
    let mut stages = Vec::new();
    let spherical = planetary::active(cfg);
    let evolves = planetary::drift_active(cfg);
    let stage = if entry.ready() { Stage::Done } else { entry.progress.stage() };
    let (step, steps) = entry.progress.steps();
    let (pass, passes) = entry.progress.phase();
    // The regional solve runs equal uplift and carving step counts.
    let half = u64::from(steps / 2);
    for (s, key, name) in SOLVE {
        if spherical && s == Stage::Uplift {
            continue;
        }
        let state = match (s as u8).cmp(&(stage as u8)) {
            std::cmp::Ordering::Less => StageState::Done,
            std::cmp::Ordering::Equal => StageState::Running,
            std::cmp::Ordering::Greater => StageState::Queued,
        };
        let name = if s == Stage::Carving && planetary::landscape_cycles(cfg.generator_version) {
            "Watershed Detail"
        } else { name };
        let r = row(key, name, state);
        stages.push(match s {
            Stage::Plates if evolves => counted(
                row("solve-drift", if planetary::landscape_cycles(cfg.generator_version) {
                    "Drift & Erosion"
                } else { "Continental Drift" }, state),
                drift.last().map_or(0, |f| u64::from(f.myr)),
                // The generator's own history length; deep-time planets run 720.
                u64::from(planetary::history_duration_myr(cfg)),
                "Myr",
            ),
            Stage::Uplift => counted(r, u64::from(step).min(half), half, "steps"),
            Stage::Carving if !spherical => counted(r, u64::from(step).saturating_sub(half), half, "steps"),
            // Spherical solves mark each finished pass of the running stage.
            Stage::Climate | Stage::Carving | Stage::Rivers
                if spherical && state == StageState::Running && passes > 0 =>
                counted(r, u64::from(pass), u64::from(passes), "passes"),
            _ => r,
        });
    }
    stages
}

pub(crate) fn world_key(cfg: &WorldSampleConfig) -> u64 {
    (u64::from(cfg.seed) << 32 | u64::from(cfg.generator_version) << 8 | u64::from(cfg.world_type.code()))
        ^ cfg.planet_fingerprint()
}

/// Geology before spawn selection can safely query the generated surface.
pub fn preparing(cfg: &WorldSampleConfig, entry: &geo::Entry) -> WorldLoadReport {
    let drift = entry.progress.drift_frames();
    let mut stages = solve_rows(cfg, entry, &drift);
    stages.push(row("spawn-chunks", "Spawn terrain", StageState::Queued));
    WorldLoadReport {
        world_key: world_key(cfg), seed: cfg.seed, stages,
        map: entry.progress.solve_atlas().map_or(GlobeMap::Empty, GlobeMap::PlanetSolve),
        drift, planet: true, face_km: 0.0, spawn: None, pose: None, regional: None, ready: false,
    }
}

pub fn build(i: &Inputs) -> WorldLoadReport {
    let cfg = i.world.sample_config();
    let solving = geo::active(&cfg).then(|| geo::entry_for(&cfg));
    let solved = solving.as_ref().is_none_or(|e| e.ready());
    let drift = solving.as_ref().filter(|_| planetary::drift_active(&cfg))
        .map_or_else(Vec::new, |e| e.progress.drift_frames());
    let mut stages = solving.as_ref().map_or_else(Vec::new, |e| solve_rows(&cfg, e, &drift));

    let terrain_state = if i.spawn_ready {
        StageState::Done
    } else if solved {
        StageState::Running
    } else {
        StageState::Queued
    };
    stages.push(counted(row("spawn-chunks", "Spawn terrain", terrain_state), i.spawn.0 as u64, i.spawn.1 as u64, "chunks"));

    let far_done = i.gpu.horizon_loaded_this_world() || i.gpu.horizon_ready();
    if cfg.is_planet() {
        let mut cover = row("planet-cover", "Planet Cover", if far_done { StageState::Done } else { StageState::Queued });
        if !far_done && let Some((name, done, total)) = i.gpu.planet_loading_progress(cfg) {
            cover.name = name;
            cover.state = StageState::Running;
            cover.count = (done > 0 || total > 0).then_some((done, total));
            cover.unit = match name { "Surface Samples" => "points", "Vegetation" => "sites", _ => "samples" };
            // Phase-local counts reset between grids; they are not whole-job completion.
            cover.fraction = None;
        }
        stages.push(cover);
    } else {
    let stage = i.gpu.horizon_work_stage();
    let base = if far_done || stage == "upload" || (stage == "build" && !i.gpu.horizon_building_base()) {
        StageState::Done
    } else if stage == "build" {
        StageState::Running
    } else {
        StageState::Queued
    };
    stages.push(row("far-base", "Far field base", base));
    let mesh = if far_done || stage == "upload" {
        StageState::Done
    } else if base == StageState::Done {
        StageState::Running
    } else {
        StageState::Queued
    };
    stages.push(row("far-pages", "Far field mesh", mesh));
    let upload = row("far-upload", "Far field upload", if far_done { StageState::Done } else if stage == "upload" { StageState::Running } else { StageState::Queued });
    stages.push(match i.gpu.horizon_upload_pages() {
        Some((d, n)) => counted(upload, d as u64, n as u64, "pages"),
        None => upload,
    });

    }

    if i.gpu.globe_enabled() {
        let g = i.gpu.globe_stats();
        let state = if g.wanted > 0 && g.resident >= g.wanted {
            StageState::Done
        } else if g.wanted > 0 {
            StageState::Running
        } else {
            StageState::Queued
        };
        stages.push(counted(row("globe-tiles", "Globe tiles", state), g.resident as u64, g.wanted as u64, "tiles"));
    }

    let center = (i.player_voxel.x.floor() as i32, i.player_voxel.z.floor() as i32);
    let planet = cfg.is_planet();
    let face_km = if solving.is_some() { (geo::face_edge_m(&cfg) / 1000.0) as f32 } else { 0.0 };
    let atlas = solving.as_ref().and_then(|e| e.progress.solve_atlas());
    let survey_ready = atlas.is_none() || (i.spawn_ready && far_done);
    let planet_survey = (planet && solved && survey_ready).then(|| sphere::request(cfg, center)).flatten();
    let map = match &solving {
        Some(e) if !e.ready() => atlas.map(GlobeMap::PlanetSolve)
            .or_else(|| e.progress.preview().map(GlobeMap::Solve)).unwrap_or(GlobeMap::Empty),
        _ if planet => {
            if planet_survey.as_ref().is_some_and(|s| s.faces() == 6) || atlas.is_none() {
                planet_survey.clone().map_or(GlobeMap::Empty, GlobeMap::Sphere)
            } else {
                atlas.map_or(GlobeMap::Empty, GlobeMap::PlanetSolve)
            }
        }
        // Sample final terrain, including changes applied after the drainage solve.

        _ => survey::request(cfg, center).map_or(GlobeMap::Empty, GlobeMap::Survey),
    };
    if let Some(s) = &planet_survey {
        let faces = s.faces();
        let state = if faces == 6 { StageState::Done } else { StageState::Running };
        stages.push(counted(row("planet-survey", "Planet survey", state), faces as u64, 6, "faces"));
    }
    let spawn = if planet {
        Some(sphere::spawn_dir(&cfg, center))
    } else {
        (face_km > 0.0).then(|| {
        let half = f64::from(face_km) * 500.0;
        let m = f64::from(cfg.voxel_meters());
        let s = (f64::from(center.0) * m / half).clamp(-1.0, 1.0);
        let t = (f64::from(center.1) * m / half).clamp(-1.0, 1.0);
        engine_core::cube_sphere::Projection::Tangent.to_sphere(engine_core::cube_sphere::Face::PosX, s, t).as_vec3()
        })
    };
    let world_key = world_key(&cfg);
    let origin = glam::Vec2::new(cfg.world_x(center.0), cfg.world_z(center.1));
    let regional = solved.then(|| {
        crate::egui_ui::world_loader::regional::request(
            cfg, glam::Vec3::new(origin.x, 0.0, origin.y), world_key,
        ).map(|map| {
            use crate::world::farming::jobs::communities;
            let populations = map.get().into_iter().flat_map(|map| map.settlement_ids())
                .filter_map(|id| communities::population(i.world, id).map(|count| (id, count)))
                .collect();
            crate::egui_ui::world_loader::regional::View { origin, map, populations }
        })
    }).flatten();
    WorldLoadReport { world_key, seed: cfg.seed, stages, map, drift, planet, face_km, spawn, pose: None, regional, ready: i.ready }
}
