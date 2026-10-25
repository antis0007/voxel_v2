//! The designer's right column: the compile report with one-click fixes, the
//! derived hardware figures, and released revisions side by side.
use super::chip_editor::{Editor, Tool};
use super::{UiAction, icons};
use crate::world::factory::chips::{Action, BLANK, COPPER_PER_BILL, registry};
use egui::{Color32, RichText};
use engine_chip::characterize::Report;
use engine_chip::compile::{Compiled, Fix, Severity, compile};
use engine_chip::design::{Design, Pin};
use engine_chip::extract::extract;
use engine_chip::presets;
use engine_chip::registry::Registry;
use engine_chip::route::{repair, route_all};

pub(super) fn report(ui: &mut egui::Ui, ed: &mut Editor) {
    let compiled = ed.compiled.get_or_insert_with(|| compile(&ed.design, registry())).clone();
    let (errors, warnings) = compiled.diagnostics.iter().fold((0, 0), |(e, w), d| if d.severity == Severity::Error { (e + 1, w) } else { (e, w + 1) });
    ui.label(RichText::new(if compiled.ok() { "Design Valid".to_string() } else { format!("{errors} Errors") }).size(15.0).color(if compiled.ok() { icons::TEAL } else { Color32::from_rgb(235, 80, 60) }));
    if warnings > 0 {
        ui.small(format!("{warnings} Warnings"));
    }
    for d in compiled.diagnostics.iter().take(8) {
        ui.horizontal_wrapped(|ui| {
            let c = if d.severity == Severity::Error { Color32::from_rgb(235, 120, 90) } else { icons::GOLD };
            ui.label(RichText::new(&d.message).small().color(c));
            if let Some(fix) = &d.fix
                && ui.small_button(fix_label(fix)).clicked()
            {
                apply_fix(ed, fix);
            }
        });
    }
    if let Some(r) = &compiled.report {
        stats(ui, r);
    }
}

fn fix_label(fix: &Fix) -> String {
    match fix {
        Fix::UsePreset => "Load Preset".into(),
        Fix::RepairRoutes => "Repair Routes".into(),
        Fix::Add(role) => format!("Add {}", registry().modules.iter().find(|m| m.role == *role).map_or("Module", |m| m.name.as_str())),
        Fix::Connect { port, .. } => format!("Connect {}", port.to_uppercase()),
        Fix::MoveToEdge(_) => "Move To Edge".into(),
        Fix::AddLayer => "Add Layer".into(),
        Fix::SampleAtLeast(ms) => format!("Wake {ms} ms"),
    }
}

fn apply_fix(ed: &mut Editor, fix: &Fix) {
    match fix.clone() {
        Fix::UsePreset => ed.commit(|d, r| {
            *d = presets::sentinel(r);
            true
        }),
        Fix::RepairRoutes => ed.commit(|d, r| {
            let broken = extract(d, r).broken();
            repair(d, r, &broken);
            true
        }),
        Fix::AddLayer => ed.commit(|d, r| {
            let max = r.process(&d.process).map_or(1, |p| p.max_layers);
            d.layers = (d.layers + 1).min(max);
            route_all(d, r);
            true
        }),
        Fix::SampleAtLeast(ms) => ed.commit(|d, _| {
            d.firmware.sample_ms = ms;
            true
        }),
        Fix::Add(role) => {
            if let Some(m) = registry().modules.iter().find(|m| m.role == role) {
                ed.tool = Tool::Place(m.id.clone());
            }
        }
        Fix::Connect { part, port } => {
            ed.tool = Tool::Wire;
            ed.pin = Some(Pin { part, port });
        }
        Fix::MoveToEdge(id) => ed.commit(|d, r| move_to_edge(d, r, id)),
    }
}

/// Nearest free position (by Manhattan distance) where the part touches an edge.
fn move_to_edge(d: &mut Design, r: &Registry, id: u16) -> bool {
    let Some(p) = d.part(id).cloned() else { return false };
    let Some((_, _, w, h)) = d.rect(r, &p) else { return false };
    let mut best = None;
    for y in 0..=d.height.saturating_sub(h) {
        for x in 0..=d.width.saturating_sub(w) {
            let edge = x == 0 || y == 0 || x + w == d.width || y + h == d.height;
            let cost = x.abs_diff(p.x) as u32 + y.abs_diff(p.y) as u32;
            if edge && best.is_none_or(|(c, _, _)| cost < c) && d.clone().relocate(r, id, x, y, p.rot).is_ok() {
                best = Some((cost, x, y));
            }
        }
    }
    best.is_some_and(|(_, x, y)| d.relocate(r, id, x, y, p.rot).is_ok())
}

fn stats(ui: &mut egui::Ui, r: &Report) {
    let mut row = |k: &str, v: String| {
        ui.horizontal_wrapped(|ui| {
            ui.small(k);
            ui.label(RichText::new(v).small().color(icons::PAPER));
        });
    };
    row("Die", format!("{:.2} mm² · {}/{} sites", r.die_mm2, r.used_sites, r.die_sites));
    row("Wire", format!("{} sites · {} vias", r.wire_sites, r.vias));
    row("Critical Path", format!("{:.0} ns", r.critical_ns));
    row("Clock", format!("{:.2} of {:.2} MHz", r.clock_hz / 1e6, r.max_clock_hz / 1e6));
    row("Wake Work", format!("{} cycles · {:.0} µs", r.cycles_per_wake, r.busy_us));
    row("Throughput", format!("{:.0} kinstr/s", r.instructions_per_s / 1e3));
    row("Sensors · GPIO", format!("{} · {}", r.channels, r.outputs));
    row("ROM · RAM", format!("{} B · {} B", r.rom_bytes, r.ram_bytes));
    row("Wake", if r.sleeps { format!("{:.0} ms", r.wake_ms) } else { "Polling".into() });
    row("Response", format!("≤ {:.1} ms", r.worst_response_ms));
    row("Power", format!("{:.2} mW idle · {:.2} mW avg", r.idle_w * 1e3, r.average_w * 1e3));
    row("Wake Energy", format!("{:.1} µJ", r.wake_uj));
    ui.small(&r.critical_path);
}

pub(super) fn revisions(ui: &mut egui::Ui, ed: &mut Editor, library: &engine_chip::library::Library, bench: bool, action: &mut UiAction) {
    let Some(project) = ed.project else {
        ui.small("Unsaved Draft");
        return;
    };
    let valid = ed.compiled.as_ref().is_some_and(Compiled::ok) && ed.pending.is_none() && !ed.unsynced;
    ui.add_enabled_ui(valid, |ui| {
        if ui.button("Release Revision").clicked() {
            *action = UiAction::Chip(Action::Release { project, design: ed.design.clone(), generation: ed.generation });
            ed.pending = Some(ed.generation + 1);
        }
    });
    let revs: Vec<_> = library.revisions_of(project).collect();
    if revs.is_empty() {
        return;
    }
    egui::Grid::new("chip_revisions").num_columns(8).spacing([8.0, 2.0]).striped(true).show(ui, |ui| {
        for h in ["Rev", "Task", "Ch", "Resp", "mW", "mm²", "WDT", ""] {
            ui.small(h);
        }
        ui.end_row();
        for rev in revs {
            let key = (rev.id, rev.fingerprint);
            if !ed.reports.iter().any(|(id, fp, _)| (*id, *fp) == key) {
                ed.reports.push((rev.id, rev.fingerprint, compile(&rev.design, registry()).report));
            }
            let report = ed.reports.iter().find(|(id, fp, _)| (*id, *fp) == key).and_then(|(_, _, r)| r.as_ref());
            let m = rev.model;
            ui.label(RichText::new(rev.letter()).color(icons::GOLD));
            ui.small(m.task.label());
            ui.small(m.channels.to_string());
            ui.small(format!("{} ms", (m.wake_ticks + m.response_ticks) * 1000 / 120));
            ui.small(format!("{:.1}", report.map_or(m.idle_nw as f32 / 1e6, |r| r.average_w * 1e3)));
            ui.small(report.map_or_else(|| "?".into(), |r| format!("{:.2}", r.die_mm2)));
            ui.small(if m.recovers { "Yes" } else { "No" });
            let copper = library.bill(rev.id).unwrap_or(0) * COPPER_PER_BILL;
            let tip = format!("{} Ancient circuits + {:.1} Copper ingots", BLANK.1 / 64, copper as f32 / 64.0);
            let r = ui.add_enabled(bench, egui::Button::new("Prototype").small()).on_hover_text(tip);
            if r.clicked() {
                *action = UiAction::Chip(Action::Prototype(rev.id));
            }
            ui.end_row();
        }
    });
    if !bench {
        ui.small("Prototype: Workbench Required");
    }
    ui.small(format!("Stock {}", library.stock.len()));
}
