//! Controller socket on a machine panel: the installed part's live state,
//! its sensor channels against the thresholds, and stocked parts that fit.
use super::{UiAction, chip_editor, icons};
use crate::world::factory::View;
use crate::world::factory::chips::{Action, fits};
use egui::{Color32, RichText};
use engine_chip::firmware::{OPEN, Sensor, Task};
use engine_chip::runtime::Phase;
use glam::IVec3;

const RED: Color32 = Color32::from_rgb(235, 80, 60);

fn channel_name(sensor: Sensor) -> &'static str {
    match sensor {
        Sensor::TankLevel => "Tank",
        Sensor::CrateLevel => "Crate",
        Sensor::CoreHeat => "Core Heat",
    }
}

/// The socket section of any controllable machine's panel, plus the
/// designer window when it is open.
pub(super) fn section(ui: &mut egui::Ui, view: &View, pos: IVec3, action: &mut UiAction) {
    socket(ui, view, pos, action);
    chip_editor::window(ui.ctx(), view, action);
}

fn socket(ui: &mut egui::Ui, view: &View, pos: IVec3, action: &mut UiAction) {
    let Some(library) = view.chips.as_ref() else { return };
    // Review captures: VOXEL_REVIEW_CHIP_DESIGNER=1 opens the designer, =lot
    // also opens the first wafer lot.
    static REVIEW: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(mode) = REVIEW.get_or_init(|| std::env::var("VOXEL_REVIEW_CHIP_DESIGNER").ok()) {
        chip_editor::open(ui.ctx());
        if mode == "lot" {
            chip_editor::open_lot(ui.ctx(), 1);
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Controller").size(15.0).color(icons::PAPER));
        if ui.small_button("Designer").clicked() {
            chip_editor::open(ui.ctx());
        }
    });
    let node = view.focused.as_ref().map(|(_, n)| n);
    let Some(d) = view.controller.as_ref() else {
        // One row per (revision, bin), lowest serial first: lots make dozens.
        let mut groups: Vec<(u32, engine_chip::fab::Grade, u32, u32)> = Vec::new();
        for p in &library.stock {
            let Some(rev) = library.revision(p.revision) else { continue };
            if !node.is_some_and(|n| fits(rev.model.task, n)) {
                continue;
            }
            match groups.iter_mut().find(|g| (g.0, g.1) == (p.revision, p.grade)) {
                Some(g) => {
                    g.2 += 1;
                    g.3 = g.3.min(p.serial);
                }
                None => groups.push((p.revision, p.grade, 1, p.serial)),
            }
        }
        if groups.is_empty() {
            ui.label(RichText::new("Socket Empty").color(icons::GOLD));
        }
        for (revision, grade, count, serial) in groups.into_iter().take(8) {
            let Some(rev) = library.revision(revision) else { continue };
            let name = library.project(rev.project).map_or("?", |p| p.name.as_str());
            ui.horizontal(|ui| {
                let label = format!("{name} {} · Bin {grade:?} ×{count}", rev.letter());
                let r = ui.button(label).on_hover_ui(|ui| {
                    let m = rev.model;
                    ui.small(format!("{} · {} Ch · Wake {} ms · {:.1} mW", m.task.label(), m.channels, m.wake_ticks * 1000 / 120, m.idle_nw as f32 / 1e6));
                    ui.small(format!("Rated {:.0}% Clock", grade.clock_factor() * 100.0));
                });
                if r.clicked() {
                    *action = UiAction::Chip(Action::Install(pos, serial));
                }
                if ui.small_button("Scrap").clicked() {
                    *action = UiAction::Chip(Action::Scrap(serial));
                }
            });
        }
        return;
    };
    let rev = library.revision(d.package.revision);
    let name = rev.and_then(|r| library.project(r.project)).map_or("?", |p| p.name.as_str());
    let letter = rev.map(|r| r.letter()).unwrap_or_default();
    let (state, colour) = match (d.state.phase, d.state.output) {
        (Phase::Unpowered, _) => ("No Power", RED),
        (Phase::Hung, _) => ("Hung", RED),
        (Phase::Booting, _) => ("Booting", icons::GOLD),
        (Phase::Running, true) => ("On", icons::TEAL),
        (Phase::Running, false) => ("Off", icons::GOLD),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{name} {letter} #{}", d.package.serial)).color(icons::PAPER));
        ui.label(RichText::new(state).color(colour));
        if d.state.phase == Phase::Hung && ui.small_button("Reset").clicked() {
            *action = UiAction::Chip(Action::Reset(pos));
        }
        if ui.small_button("Remove").clicked() {
            *action = UiAction::Chip(Action::Remove(pos));
        }
    });
    let task = d.model.map_or(Task::FillBand, |m| m.task);
    let sensor = task.sensor();
    if d.sensors.is_empty() {
        ui.label(RichText::new(format!("No {}", channel_name(sensor))).color(icons::GOLD));
    }
    let lower = f32::from(d.state.lower) / 1000.0;
    let upper = f32::from(d.state.upper) / 1000.0;
    let watched = d.model.map_or(0, |m| usize::from(m.channels));
    for (i, &level) in d.state.readings.iter().enumerate().take(watched.max(d.sensors.len()).min(4)) {
        let label = match (i < watched, d.sensors.get(i), level) {
            (false, _, _) => format!("{} {} · Not Read", channel_name(sensor), i + 1),
            (true, None, _) | (true, _, OPEN) => format!("Ch {} · Open", i + 1),
            (true, Some(_), l) => format!("{} {} · {:.0}%", channel_name(sensor), i + 1, f32::from(l) / 10.0),
        };
        let v = if level == OPEN || i >= watched { 0.0 } else { f32::from(level) / 1000.0 };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(250.0), 10.0), egui::Sense::hover());
        icons::frame(ui.painter(), rect, false);
        let fill = egui::Rect::from_min_size(rect.min + egui::vec2(2.0, 2.0), egui::vec2((rect.width() - 4.0) * v, rect.height() - 4.0));
        let warm = if sensor == Sensor::CoreHeat { v > upper } else { v < lower };
        ui.painter().rect_filled(fill, 0, if warm { icons::GOLD } else { icons::TEAL });
        for t in [lower, upper] {
            let x = rect.left() + 2.0 + (rect.width() - 4.0) * t;
            ui.painter().line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.5, icons::PAPER));
        }
        ui.small(RichText::new(label).color(if i < watched { icons::PAPER } else { Color32::GRAY }));
    }
    let (on, off) = if sensor == Sensor::CoreHeat { ("% On Below", "% Off Above") } else { ("% On Below", "% Off At") };
    let (mut lo, mut hi) = (d.state.lower / 10, d.state.upper / 10);
    let changed = ui
        .horizontal(|ui| {
            let a = ui.add(egui::DragValue::new(&mut lo).range(0..=99).suffix(on)).changed();
            let b = ui.add(egui::DragValue::new(&mut hi).range(1..=100).suffix(off)).changed();
            a || b
        })
        .inner;
    if changed {
        *action = UiAction::Chip(Action::Thresholds(pos, lo * 10, hi.max(lo + 1) * 10));
    }
    if let Some(m) = d.model {
        ui.small(format!(
            "{} · Wake {} ms · ≤ {} ms · {:.1} mW · {} Wakes",
            m.task.label(),
            m.wake_ticks * 1000 / 120,
            (m.wake_ticks + m.response_ticks) * 1000 / 120,
            m.idle_nw as f32 / 1.0e6,
            d.state.wakes
        ));
    }
}
