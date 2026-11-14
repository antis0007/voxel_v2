//! The registered dish projects live orbital rails above its physical workbench.
//! The portable service page, console and flight instruments share one target.
use super::{UiAction, flight_hud::{body_color, orbit_paths}, presentation::{AMBER, JADE, MUTED, PAPER}};
use crate::{player::Player, space_nav::{self, text}, world::{self, World, WorldType, orbital::radio}};
use egui::{Align2, FontId, Stroke};
use glam::{DVec3, Mat4, Vec3};
pub(super) static REVIEW_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub fn manual_review() -> bool {
    static MANUAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MANUAL.get_or_init(|| std::env::var("VOXEL_REVIEW_NAVIGATION").as_deref() == Ok("manual"))
}

pub fn manual_ready() -> bool { manual_review() && REVIEW_STARTED.get().is_some() }

fn available(world: &World, player: &Player) -> bool {
    world.world_type == WorldType::Planet && world.orbital.radio.level >= 4
        && radio::near(world, player) && radio::intact(world)
}

/// Both the physical bench and its projection can be inspected; walls block access.
pub fn focused(world: &World, player: &Player) -> bool {
    if !available(world, player) { return false; }
    let Some(dish) = world.orbital.radio.dish else { return false };
    let eye = player.eye(world.voxel_scale);
    let scale = world.voxel_scale.meters();
    let centre = dish.as_vec3() + Vec3::splat(0.5) + Vec3::Y * (1.5 / scale);
    let delta = centre - eye;
    let direct = world::raycast_block(world, eye, player.forward_dir(), 8.0 / scale);
    if direct.is_some_and(|(cell, _)| cell == dish) { return true; }
    delta.normalize_or_zero().dot(player.forward_dir()) > 0.94
        && world::raycast_block(world, eye, delta.normalize_or_zero(), delta.length()).is_none()
}

pub fn menu(ui: &mut egui::Ui, action: &mut UiAction) {
    let Some(system) = space_nav::system() else { return };
    ui.horizontal(|ui| {
        ui.strong("Navigation");
        if ui.button("System Map").clicked() { *action = UiAction::SystemMap; }
        if space_nav::target().is_some() && ui.button("Clear Target").clicked() {
            space_nav::set_target(None);
        }
    });
    ui.horizontal_wrapped(|ui| {
        for body in system.bodies().filter(|b| !matches!(system.kind(*b), engine_space::BodyKind::Barycenter)) {
            if ui.selectable_label(space_nav::target() == Some(body), text::name(system, body)).clicked() {
                space_nav::set_target(Some(body));
            }
        }
    });
    ui.separator();
}

/// Opt-in disposable benchmark fixture, using the same registered dish as play.
pub fn prepare_review(world: &mut World, player: &Player) -> Vec<glam::IVec3> {
    if REVIEW_STARTED.get().is_some() { return Vec::new(); }
    let scale = world.voxel_scale.meters();
    let centre = player.eye(world.voxel_scale) + player.forward_dir() * (4.0 / scale);
    let cell = (centre - Vec3::Y * (1.5 / scale)).floor().as_ivec3();
    let mut changed = Vec::new();
    for x in -2..=2 {
        for z in -2..=2 {
            let support = cell + glam::IVec3::new(x, -1, z);
            world.set(support, world::BlockId::Stone);
            changed.push(support);
        }
    }
    world.set(cell, world::BlockId::Workbench);
    changed.push(cell);
    world.orbital.radio.dish = Some(cell);
    world.orbital.radio.level = 4;
    world.orbital.radio.knowledge = 7;
    space_nav::set_target(space_nav::system().and_then(|s| s.id("moon")));
    eprintln!("[navigation-review] dish={cell:?} focused={}", focused(world, player));
    assert!(focused(world, player), "navigation fixture must be inspectable");
    let _ = REVIEW_STARTED.set(std::time::Instant::now());
    changed
}

pub fn draw(ctx: &egui::Context, world: &World, player: &Player, view_proj: Mat4) {
    if !available(world, player) { return; }
    let Some(dish) = world.orbital.radio.dish else { return };
    let (Some(system), Some(fix)) = (space_nav::system(), space_nav::fix(world, player)) else { return };
    let scale = world.voxel_scale.meters();
    let base = dish.as_vec3() + Vec3::splat(0.5);
    let centre = base + Vec3::Y * (1.5 / scale);
    let eye = player.eye(world.voxel_scale);
    let delta = centre - eye;
    // This is a nearby physical display, never an overlay visible through walls.
    if world::raycast_block(world, eye, delta.normalize_or_zero(), delta.length()).is_some() { return; }
    let screen = ctx.content_rect();
    let project = |point: Vec3| {
        let clip = view_proj * point.extend(1.0);
        if clip.w <= 0.05 { return None; }
        let ndc = clip.truncate() / clip.w;
        let at = egui::pos2(screen.center().x + ndc.x * screen.width() * 0.5,
            screen.center().y - ndc.y * screen.height() * 0.5);
        screen.contains(at).then_some(at)
    };
    let Some(at) = project(centre) else { return };
    let target = space_nav::target();
    let focus = target.and_then(|b| system.parent(b)).unwrap_or(fix.reference);
    let extent = system.bodies().filter(|b| system.parent(*b) == Some(focus))
        .filter_map(|b| system.orbit(b)).map(|o| o.a * (1.0 + o.e)).fold(1.0, f64::max);
    let origin = fix.snapshot.position(focus);
    let tilt = glam::Quat::from_rotation_x(0.65);
    let to_world = |q: DVec3| centre + tilt * (q / extent).as_vec3() * (1.2 / scale);
    let painter = ctx.layer_painter(egui::LayerId::background());
    if let Some(foot) = project(base + Vec3::Y * (0.5 / scale)) {
        painter.line_segment([foot, at], Stroke::new(1.0, JADE.gamma_multiply(0.4)));
    }
    painter.circle_filled(at, 5.0, body_color(system.kind(focus)));
    for body in system.bodies().filter(|b| system.parent(*b) == Some(focus)) {
        let color = if Some(body) == target { AMBER } else { body_color(system.kind(body)) };
        for pair in orbit_paths(system)[body.0 as usize].windows(2) {
            if let (Some(a), Some(b)) = (project(to_world(pair[0])), project(to_world(pair[1]))) {
                painter.line_segment([a, b], Stroke::new(1.0, color.gamma_multiply(0.6)));
            }
        }
        if let Some(point) = project(to_world(fix.snapshot.position(body) - origin)) {
            painter.circle_filled(point, 4.0, color);
            if Some(body) == target { painter.circle_stroke(point, 7.0, Stroke::new(1.5, AMBER)); }
            painter.text(point + egui::vec2(8.0, -8.0), Align2::LEFT_BOTTOM, text::name(system, body), FontId::proportional(12.0), color);
        }
    }
    if focused(world, player) {
        painter.text(at + egui::vec2(0.0, -65.0), Align2::CENTER_BOTTOM,
            format!("{} · Live Orbits", text::name(system, focus)), FontId::proportional(14.0), PAPER);
        painter.text(at + egui::vec2(0.0, 65.0), Align2::CENTER_TOP,
            format!("[{}] Navigation", crate::keybinds::hint(crate::keybinds::Action::Interact)), FontId::proportional(13.0), JADE);
        if let Some(body) = target {
            painter.text(at + egui::vec2(0.0, 84.0), Align2::CENTER_TOP,
                format!("{} · {}", text::name(system, body), text::range(fix.contact(system, body).range_m)), FontId::proportional(12.0), MUTED);
        }
    }
}
