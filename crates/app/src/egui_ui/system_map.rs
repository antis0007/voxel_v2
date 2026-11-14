//! System map: an orrery of the star system with body data and transfer figures.
//!
//! Left: bodies as a tree. Centre: orbits, bodies and the transfer arc at the
//! chosen time. Right: the selected body's numbers and the target control.
use super::flight_hud::body_color;
use super::presentation::{AMBER, MUTED};
use crate::creative::CreativeState;
use crate::space_nav::{self, Fix, TICKS_PER_SECOND, Transfer, text};
use egui::{Align2, RichText, Sense, vec2};
use engine_space::{BodyId, BodyKind, Snapshot, System};
use std::sync::Mutex;

mod canvas;
mod info;

const LIST_WIDTH: f32 = 150.0;
const INFO_WIDTH: f32 = 230.0;
/// Ticks between transfer recomputations while the map is open.
const PLAN_REFRESH_TICKS: u64 = 120;

struct View {
    focus: BodyId,
    selected: BodyId,
    /// Points per metre.
    zoom: f64,
    yaw: f32,
    /// 0 looks straight down on the system plane.
    pitch: f32,
    /// Seconds ahead of now the bodies are shown at.
    offset_s: f64,
    /// Freeze a preview at an absolute instant while the live world keeps moving.
    preview_tick: Option<u64>,
    fitted: bool,
    pan: egui::Vec2,
    search: String,
    plan: Option<(BodyId, BodyId, u64, Option<Transfer>)>,
}

static VIEW: Mutex<Option<View>> = Mutex::new(None);

/// Native, opt-in review: exercises the same target and panel state as gameplay.
/// Renderer captures remain ordinary in-game frames, including world geometry.
pub fn review_frame(ctx: &egui::Context, creative: &mut CreativeState, world: &crate::world::World, player: &crate::player::Player) {
    if std::env::var("VOXEL_REVIEW_NAVIGATION").as_deref() == Ok("manual") { return; }
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(u64::MAX);
    let Some(start) = super::nav_console::REVIEW_STARTED.get() else { return };
    let stage = (start.elapsed().as_secs() / 12).min(5);
    if LAST.swap(stage, Ordering::Relaxed) == stage { return; }
    let system = space_nav::system().unwrap();
    let fix = space_nav::fix(world, player).unwrap();
    match stage {
        0 => {
            creative.close_panels();
            assert!(super::nav_console::focused(world, player));
            eprintln!("[navigation-review] stage=console focused=true");
        }
        1 => {
            toggle(creative, Some(world));
            assert!(creative.show_system_map);
            eprintln!("[navigation-review] stage=local-map target=moon");
        }
        2 | 3 => {
            let mut guard = VIEW.lock().unwrap_or_else(|e| e.into_inner());
            let view = guard.get_or_insert_with(|| View::new(&fix));
            let giant = system.id("giant").unwrap();
            view.selected = giant;
            view.focus_on(system.parent(giant).unwrap());
            space_nav::set_target(Some(giant));
            let plan = view.plan(system, fix.reference, fix.tick).expect("giant transfer");
            if stage == 3 {
                view.offset_s = plan.window_s.expect("giant window");
                view.preview_tick = Some(fix.tick.saturating_add((view.offset_s * TICKS_PER_SECOND).round() as u64));
                let preview = view.plan(system, fix.reference, view.preview_tick.unwrap()).unwrap();
                assert_eq!(preview.window_s, Some(0.0), "preview must hold the selected departure window");
            }
            eprintln!("[navigation-review] stage={} target=giant coast_s={} preview_s={}",
                if stage == 2 { "system-map" } else { "window-preview" }, plan.coast_s, view.offset_s);
        }
        4 => {
            toggle(creative, Some(world));
            assert!(!creative.show_system_map && space_nav::target() == system.id("giant"));
            eprintln!("[navigation-review] stage=flight target=giant preserved=true");
        }
        _ => {
            creative.close_panels();
            creative.show_palette = true;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), 8u32));
            eprintln!("[navigation-review] stage=orbital-menu target=giant complete=true");
        }
    }
}

impl View {
    fn new(fix: &Fix) -> Self {
        Self { focus: fix.reference, selected: fix.reference, zoom: 1.0e-6, yaw: 0.0, pitch: 0.7, offset_s: 0.0, preview_tick: None, fitted: false, pan: egui::Vec2::ZERO, search: String::new(), plan: None }
    }

    fn focus_on(&mut self, body: BodyId) {
        self.focus = body;
        self.pan = egui::Vec2::ZERO;
        self.fitted = false;
    }

    /// Transfer from the player's body to the selection, recomputed once a second.
    fn plan(&mut self, system: &System, near: BodyId, tick: u64) -> Option<Transfer> {
        let key = (near, self.selected, tick / PLAN_REFRESH_TICKS);
        match self.plan {
            Some((a, b, t, plan)) if (a, b, t) == key => plan,
            _ => {
                let plan = space_nav::transfer(system, near, self.selected, tick);
                self.plan = Some((key.0, key.1, key.2, plan));
                plan
            }
        }
    }
}

/// Opens or closes the system map; opening closes other panels. No map off a planet world.
pub fn toggle(creative: &mut CreativeState, world: Option<&crate::world::World>) {
    let open = !creative.show_system_map && world.is_some_and(|w| w.world_type == crate::world::WorldType::Planet);
    creative.close_panels();
    creative.show_system_map = open;
    if open {
        let mut guard = VIEW.lock().unwrap_or_else(|e| e.into_inner());
        if let (Some(view), Some(body), Some(system)) = (guard.as_mut(), space_nav::target(), space_nav::system()) {
            view.selected = body;
            view.focus_on(system.parent(body).unwrap_or(body));
        }
    }
}

/// Longest orbit in the system, seconds: the span of the time slider.
fn longest_period_s(system: &System) -> f64 {
    system.bodies().map(|b| system.period_ticks(b)).max().unwrap_or(0) as f64 / TICKS_PER_SECOND
}

fn depth(system: &System, body: BodyId) -> usize {
    std::iter::successors(system.parent(body), |b| system.parent(*b)).count()
}

fn body_list(ui: &mut egui::Ui, system: &System, view: &mut View, target: Option<BodyId>) {
    ui.add(egui::TextEdit::singleline(&mut view.search).hint_text("Search").desired_width(LIST_WIDTH - 12.0));
    let query = view.search.trim().to_lowercase();
    let mut bodies: Vec<_> = system.bodies().filter(|b| !matches!(system.kind(*b), BodyKind::Barycenter)).collect();
    bodies.sort_by_key(|body| {
        let mut path: Vec<_> = std::iter::successors(Some(*body), |b| system.parent(*b)).map(|b| b.0).collect();
        path.reverse();
        path
    });
    for body in bodies {
        if !text::name(system, body).to_lowercase().contains(&query) { continue; }
        ui.horizontal(|ui| {
            ui.add_space(depth(system, body) as f32 * 12.0);
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, body_color(system.kind(body)));
            let name = RichText::new(text::name(system, body));
            let name = if Some(body) == target { name.color(AMBER) } else { name };
            let row = ui.selectable_label(view.selected == body, name);
            if row.clicked() {
                view.selected = body;
                if !std::iter::successors(Some(body), |b| system.parent(*b)).any(|b| b == view.focus) {
                    view.focus_on(system.parent(body).unwrap_or(body));
                }
            }
            if row.double_clicked() {
                view.focus_on(body);
            }
        });
    }
}

fn time_bar(ui: &mut egui::Ui, system: &System, view: &mut View) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(if view.offset_s > 0.0 { "Preview" } else { "Live" }).color(MUTED));
        if ui.add(egui::Slider::new(&mut view.offset_s, 0.0..=longest_period_s(system)).show_value(false)).changed() {
            view.preview_tick = None;
        }
        ui.label(RichText::new(format!("+{}", text::duration(view.offset_s))).monospace());
        if view.offset_s > 0.0 && ui.button("Now").clicked() {
            view.offset_s = 0.0;
            view.preview_tick = None;
        }
        ui.separator();
        let mut lines = space_nav::orbit_lines();
        if ui.checkbox(&mut lines, "Orbit Lines").changed() {
            space_nav::set_orbit_lines(lines);
        }
    });
}

pub fn draw(ctx: &egui::Context, open: &mut bool, world: &crate::world::World, player: &crate::player::Player) {
    if !*open {
        return;
    }
    let (Some(system), Some(fix)) = (space_nav::system(), space_nav::fix(world, player)) else {
        *open = false;
        return;
    };
    let mut guard = VIEW.lock().unwrap_or_else(|e| e.into_inner());
    let view = guard.get_or_insert_with(|| {
        let mut view = View::new(&fix);
        if let Some(body) = space_nav::target() {
            view.selected = body;
            view.focus_on(system.parent(body).unwrap_or(body));
        }
        view
    });
    let shown_tick = if view.offset_s > 0.0 {
        *view.preview_tick.get_or_insert_with(|| fix.tick.saturating_add((view.offset_s * TICKS_PER_SECOND).round() as u64))
    } else {
        view.preview_tick = None;
        fix.tick
    };
    let mut shown = Snapshot::default();
    system.evaluate(shown_tick, &mut shown);
    let screen = ctx.content_rect();
    let size = vec2((screen.width() - 80.0).clamp(640.0, 1500.0), (screen.height() - 120.0).clamp(360.0, 900.0));
    egui::Window::new("System Map")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let target = space_nav::target();
            ui.horizontal(|ui| {
                if ui.button("System").clicked() {
                    let root = std::iter::successors(Some(fix.reference), |b| system.parent(*b)).last().unwrap_or(fix.reference);
                    view.focus_on(root);
                }
                if ui.button("Local").clicked() {
                    view.selected = fix.reference;
                    view.focus_on(fix.reference);
                }
                if ui.add_enabled(target.is_some(), egui::Button::new("Target")).clicked() {
                    if let Some(body) = target {
                        view.selected = body;
                        view.focus_on(system.parent(body).unwrap_or(body));
                    }
                }
                if ui.button("Fit").clicked() {
                    view.focus_on(view.focus);
                }
                if ui.button("Top View").clicked() {
                    view.yaw = 0.0;
                    view.pitch = 0.0;
                }
                if let Some(parent) = system.parent(view.focus) {
                    if ui.button("Parent").clicked() { view.focus_on(parent); }
                }
                ui.label(RichText::new(text::name(system, view.focus)).color(MUTED));
            });
            let canvas_size = vec2(size.x - LIST_WIDTH - INFO_WIDTH, size.y - 92.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LIST_WIDTH);
                    body_list(ui, system, view, target);
                });
                let plan = view.plan(system, fix.reference, shown_tick);
                let (rect, response) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());
                let scene = canvas::Scene { system, fix: &fix, shown: &shown, shown_tick, plan, target };
                canvas::draw(ui, rect, &response, view, &scene);
                let plan = view.plan(system, fix.reference, shown_tick);
                ui.vertical(|ui| {
                    ui.set_width(INFO_WIDTH);
                    info::draw(ui, system, &fix, view, plan, target);
                });
            });
            ui.separator();
            time_bar(ui, system, view);
            ui.horizontal(|ui| {
                ui.colored_label(super::presentation::PAPER, "◇ Position");
                ui.colored_label(AMBER, "Target");
                ui.colored_label(super::presentation::JADE, "Transfer");
                ui.label(RichText::new("○ Destination at Arrival").color(MUTED));
            });
            ui.label(RichText::new("[Drag] Rotate · [Shift + Drag] Pan · [Scroll] Zoom · [Double Click] Focus").small().color(MUTED));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_indents_moons_under_their_planet() {
        let system = space_nav::system().unwrap();
        let id = |name| system.id(name).unwrap();
        assert_eq!(depth(system, id("star")), 0);
        assert_eq!(depth(system, id("home")), 1);
        assert_eq!(depth(system, id("moon")), 2);
        assert!(longest_period_s(system) >= 11_520.0);
    }

    #[test]
    fn the_plan_is_kept_until_the_selection_or_second_changes() {
        let system = space_nav::system().unwrap();
        let (home, giant, moon) = (system.id("home").unwrap(), system.id("giant").unwrap(), system.id("moon").unwrap());
        let fix = Fix::at(system, 0, glam::DVec3::new(0.0, 3.0, 0.0)).unwrap();
        let mut view = View::new(&fix);
        view.selected = giant;
        let first = view.plan(system, home, 0).unwrap();
        // Same second: the cached figures, not a fresh scan.
        assert_eq!(view.plan(system, home, 60), Some(first));
        assert!(view.plan(system, home, 12_000).unwrap().window_s.is_some());
        view.selected = moon;
        assert!(view.plan(system, home, 12_000).unwrap().child);
    }
}
