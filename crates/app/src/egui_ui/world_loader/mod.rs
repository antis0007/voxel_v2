//! World generation screen: real stage list, clocks and a live globe.
//!
//! Every row comes from a work counter or a state flag of the generator,
//! streamer or renderer; clocks are measured here and nothing is simulated.
mod globe;
mod globe_gpu;
mod geology;
mod groups;
mod panel;
pub(crate) mod regional;
pub mod planet_map;
pub mod preview;
mod solve;
pub mod sphere;
mod stages;
pub mod surface;
pub mod survey;
mod timeline;

use std::sync::Arc;

use egui::{Id, LayerId, Order, Rect, Stroke, pos2, vec2};
use glam::Vec3;

use super::themes;

pub(super) use globe_gpu::install;

pub(crate) fn reset_drift(ctx: &egui::Context, world: u64) {
    geology::reset(ctx, world);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Queued,
    Running,
    Done,
}

#[derive(Clone, Debug)]
pub struct StageRow {
    /// Stable id for clocks and history; also the history key across loads.
    pub key: &'static str,
    pub name: &'static str,
    /// Summary row this stage folds into.
    pub group: &'static str,
    pub state: StageState,
    /// Work done and total, in `unit`; total zero means unbounded work.
    pub count: Option<(u64, u64)>,
    pub unit: &'static str,
    pub fraction: Option<f32>,
}

/// Summary group of a stage key; keys end in the stage's own name.
pub fn stage_group(key: &str) -> &'static str {
    let tail = key.rsplit('-').next().unwrap_or(key);
    match tail {
        "drift" | "plates" | "uplift" => "Planetary Geology",
        "climate" | "carving" | "rivers" => "Climate and Hydrology",
        "chunks" | "spawn" => "Starting Region",
        _ => "Distant Terrain",
    }
}

/// Map shown on the globe.
#[derive(Clone)]
pub enum GlobeMap {
    /// Drainage solve, newest snapshot.
    Solve(Arc<engine_worldgen::geo::Preview>),
    /// Finished drainage solve with its climate and lakes.
    Solved(Arc<engine_worldgen::geo::Planet>),
    /// Generator columns around spawn, filling row by row.
    Survey(Arc<survey::Survey>),
    /// Planet world sampled along cube-sphere directions, face by face.
    Sphere(Arc<sphere::SphereSurvey>),
    PlanetSolve(Arc<engine_worldgen::planetary::SolveAtlas>),
    Empty,
}

#[derive(Clone)]
pub struct WorldLoadReport {
    /// Changes whenever a different world starts loading; resets the clocks.
    pub world_key: u64,
    pub seed: u32,
    pub stages: Vec<StageRow>,
    pub map: GlobeMap,
    pub drift: Vec<Arc<engine_worldgen::planetary::DriftFrame>>,
    /// Planet world: solve snapshots cover one face and the rest wait for the survey.
    pub planet: bool,
    /// Edge of the solved face in km.
    pub face_km: f32,
    /// Spawn direction in the globe frame, once known.
    pub spawn: Option<Vec3>,
    /// Capture pose `(yaw offset, tilt)` in radians; live screens leave it unset.
    pub pose: Option<[f32; 2]>,
    pub regional: Option<regional::View>,
    pub ready: bool,
}

impl WorldLoadReport {
    /// Solve maps build off-thread; the key and builder for the current one.
    fn solve_build(&self) -> Option<(u64, Box<dyn FnOnce() -> surface::Surface + Send>)> {
        let (planet, face_km, salt) = (self.planet, self.face_km, self.world_key.rotate_left(17));
        match &self.map {
            GlobeMap::Solve(p) => {
                let key = Arc::as_ptr(p) as u64 ^ salt;
                let p = Arc::clone(p);
                Some((key, Box::new(move || solve::from_preview(&p, face_km, planet, key))))
            }
            GlobeMap::Solved(p) => {
                let key = Arc::as_ptr(p) as u64 ^ salt ^ 0x5017_ed00;
                let p = Arc::clone(p);
                Some((key, Box::new(move || solve::from_planet(&p, planet, key))))
            }
            _ => None,
        }
    }

    fn surface(&self) -> Option<Arc<surface::Surface>> {
        match &self.map {
            GlobeMap::Survey(s) => s.surface(),
            GlobeMap::Sphere(s) => s.surface(),
            GlobeMap::PlanetSolve(p) => {
                let scope = (self.world_key.rotate_left(17) ^ 0x4879_4463_032e_d700) & !0xff;
                let key = scope | u64::from(p.revision);
                let p = Arc::clone(p);
                surface::latest(key, move || sphere::from_solve(&p, key))
                    .filter(|s| s.key & !0xff == scope)
            }
            _ => self.solve_build().and_then(|(key, build)| surface::latest(key, build)),
        }
    }

    /// Builds the solve surface on this thread; captures draw it on their first frame.
    pub(super) fn prime(&self) {
        if let Some((key, build)) = self.solve_build() {
            surface::put(key, build());
        }
    }

    fn spawn_dir(&self) -> Option<Vec3> {
        match &self.map {
            GlobeMap::Sphere(s) => Some(s.spawn),
            // The survey square is centred on spawn.
            GlobeMap::Survey(_) => Some(Vec3::X),
            _ => self.spawn,
        }
    }

}

pub fn draw(ctx: &egui::Context, report: &WorldLoadReport) {
    let r = ctx.content_rect();
    if !r.is_positive() {
        return;
    }
    // Several call sites may ask for the screen in one pass; paint it once.
    let pass = ctx.cumulative_pass_nr();
    let guard = Id::new("world_loader_pass");
    if ctx.data(|d| d.get_temp::<u64>(guard)) == Some(pass) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(guard, pass));

    let now = ctx.input(|i| i.time);
    let seconds = now as f32;
    let still = super::magic_fx::reduced(ctx);
    // Timings feed the main bar every frame, not only the detail table.
    let (elapsed, times) = ctx.data_mut(|d| {
        let timeline = d.get_temp_mut_or_default::<timeline::Timeline>(Id::new("world_loader_timeline"));
        let times = timeline.update(report.world_key, now, &report.stages, true);
        (timeline.elapsed(now), times)
    });
    // Weighted by expected seconds, with concurrent stages sharing wall time,
    // so a minutes-long drift no longer counts the same as a one-second pass.
    let (target, left) = if report.ready {
        (1.0, None)
    } else {
        timeline::wall_progress(&report.stages, &times, elapsed)
    };
    let progress_id = Id::new("world_loader_progress");
    let previous = ctx.data(|d| d.get_temp::<(u64, f64, f32)>(progress_id))
        .filter(|(world, at, _)| *world == report.world_key && now - at < 1.0);
    let progress = if still || report.ready {
        target
    } else {
        previous.map_or(target, |(_, at, shown)| {
            let blend = 1.0 - (-(now - at).clamp(0.0, 0.1) as f32 * 6.0).exp();
            // A revised estimate slows the bar; it never moves it backwards.
            shown.max(shown + (target - shown) * blend)
        })
    };
    ctx.data_mut(|d| d.insert_temp(progress_id, (report.world_key, now, progress)));
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("world_loader")));
    let theme = themes::current(ctx);
    let wide = r.width() >= 900.0 && r.height() >= 240.0;
    let compact = r.height() < 260.0;
    let detail_height = panel::height(&report.stages, true);
    let can_show_detail = r.width() >= 560.0
        && detail_height + (if wide { 32.0 } else { 136.0 }) <= r.height();
    let detail = can_show_detail && ctx.input(|i| i.modifiers.shift);
    let panel_height = panel::height(&report.stages, detail);
    let radius = if wide {
        (r.height() * 0.34).min(r.width() * 0.205)
    } else {
        (r.width() * 0.28).min(r.height() * 0.24)
            .min(((r.height() - panel_height - 48.0) * 0.5).max(0.0))
    };
    let centre = if wide {
        pos2(r.left() + r.width() * 0.28, r.center().y)
    } else {
        let top = ((r.height() - 2.0 * radius - 16.0 - panel_height) * 0.5).max(16.0);
        pos2(r.center().x, r.top() + radius + top)
    };
    p.rect_filled(r, 0.0, theme.recess);
    let whole = matches!(report.map, GlobeMap::Sphere(_) | GlobeMap::PlanetSolve(_));
    let solved = if whole && !compact && radius > 8.0 { report.surface() } else { None };
    let history = geology::frame(ctx, report, now, still, solved.is_some());
    // Plates carried along their recorded rotations between frames, by the
    // globe shader; histories without motion crossfade the two endpoints.
    let carry = if !compact && radius > 8.0 {
        history.and_then(|frame| geology::motion(report.world_key, frame))
    } else { None };
    // Keep the retained endpoint before requesting its successor.
    let history_from = if !compact && radius > 8.0 {
        history.and_then(|frame| geology::surface(report.world_key, frame.from))
    } else { None };
    let spawn = report.spawn_dir();
    // The loading globe is a terrain map. Show the survey without synthetic cloud cover.
    // Use one geographic orientation for both epochs and their final map.
    // Capture poses still deliberately override it; the spawn remains a marker.
    let historical_world = !report.drift.is_empty();
    let aim = if historical_world { None } else { spawn };
    let pose = globe::pose(aim, whole && !historical_world, seconds, still || historical_world, 0.0, report.pose);
    let pose = globe::follow(ctx, report.world_key, pose, now, still || report.pose.is_some());
    // Sub-texel grain is fixed to the sphere; under drifting crust it shimmers.
    let mut pose = pose;
    if history.is_some() {
        pose.detail = 0.0;
    }
    // Captures hold their fixed pose; a live screen lets the viewer turn the globe.
    let (pose, orbiting) = if report.pose.is_none() && !compact && radius > 8.0 {
        globe::orbit(ctx, "loader", report.world_key, centre, radius, now, still, pose)
    } else {
        (pose, false)
    };
    // Never stretch a one-face diagnostic onto a whole planet.
    let surface = if compact || radius <= 8.0 {
        None
    } else if let Some(frame) = history {
        geology::surface(report.world_key, frame.to)
    } else if whole {
        solved
    } else if !report.planet {
        report.surface()
    } else {
        None
    };
    if !compact {
        globe_gpu::paint_backdrop(&p, r, centre, radius, pose.sun, seconds, still);
    }
    if let Some(s) = surface {
        // Do not crossfade a completed sphere with a one-face solve preview.
        let fade = if still || report.pose.is_some() {
            0.0
        } else if whole && !report.drift.is_empty() {
            geology::HANDOFF_SECONDS
        } else if whole {
            0.0
        } else {
            0.8
        };
        let scan = [progress, if still || report.ready || report.pose.is_some() { 0.0 } else { 1.0 }];
        if let (Some(frame), Some(from)) = (history, history_from)
        {
            globe_gpu::paint_history(&p, centre, radius, from, s, frame.blend, carry, pose, seconds, scan);
        } else {
            globe_gpu::paint(&p, centre, radius, s, pose, seconds, fade, scan);
        }
        if let Some(frame) = history {
            geology::events(&p, centre, radius, frame, &pose);
            geology::caption(&p, r, centre, radius, frame, theme, seconds, still);
        } else if let Some(at) = spawn {
            globe::marker(&p, centre, radius, &pose, at, theme.accent);
        }
    } else if !compact && radius > 8.0 {
        // Keep the same footprint until the first real survey image arrives.
        p.circle_stroke(centre, radius, Stroke::new(1.0, theme.edge));
        p.circle_stroke(centre, radius * 0.72, Stroke::new(1.0, theme.edge.gamma_multiply(0.55)));
        p.line_segment([centre - vec2(radius * 0.72, 0.0), centre + vec2(radius * 0.72, 0.0)],
            Stroke::new(1.0, theme.edge.gamma_multiply(0.55)));
    }

    if wide && radius >= 140.0 && let Some(view) = &report.regional
        && let Some(map) = view.map.get()
    {
        let inset = pos2(centre.x - radius * 0.72, centre.y + radius * 0.85);
        regional::draw(&p, inset, radius, view, map);
    }

    let panel = if compact {
        let w = (r.width() - 32.0).clamp(1.0, 520.0);
        Rect::from_center_size(r.center(), vec2(w, panel_height.min((r.height() - 32.0).max(1.0))))
    } else if wide {
        let w = (r.width() * 0.43).min(650.0);
        let h = panel_height.min(r.height() - 32.0);
        Rect::from_min_size(pos2(r.right() - w - r.width() * 0.05, r.center().y - h * 0.5), vec2(w, h))
    } else {
        let top = centre.y + radius + 16.0;
        Rect::from_min_max(pos2(r.left() + 16.0, top),
            pos2(r.right() - 16.0, (top + panel_height).min(r.bottom() - 16.0)))
    };
    let title = if report.planet || whole { "Loading Planet" } else { "Loading World" };
    panel::draw(&p, panel, &panel::Input {
        title,
        rows: &report.stages,
        times: &times,
        progress,
        elapsed,
        left,
        ready: report.ready,
        seed: report.seed,
        detail,
        can_show_detail,
        seconds,
        still,
    }, theme);
    if orbiting {
        // Follow the hand at display rate, not the 30 Hz idle cadence.
        ctx.request_repaint();
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(if still { 500 } else { 33 }));
    }
}
