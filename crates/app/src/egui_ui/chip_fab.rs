//! Fab column and lot window: start a wafer lot of a revision, follow it
//! step by step, and read a cross-section of the die along one site row,
//! intended (the full route) against what the lot has built so far.
use super::chip_editor::Editor;
use super::{UiAction, chip_art, icons};
use crate::world::factory::chips::{Action, registry, resources};
use crate::world::expedition::NAMES;
use egui::{Color32, Rect, RichText, Stroke, pos2, vec2};
use engine_chip::design::Design;
use engine_chip::fab::{MaskLayer, Step, StepKind, route};
use engine_chip::library::Library;
use engine_chip::lot::WaferLot;

pub(super) fn section(ui: &mut egui::Ui, ed: &mut Editor, library: &Library, bench: bool, now: u64, action: &mut UiAction) {
    let Some(project) = ed.project else { return };
    let revs: Vec<_> = library.revisions_of(project).collect();
    let Some(last) = revs.last() else { return };
    ui.separator();
    ui.label(RichText::new("Fab").color(icons::GOLD));
    let rev_id = ed.fab_rev.filter(|r| revs.iter().any(|v| v.id == *r)).unwrap_or(last.id);
    ui.horizontal_wrapped(|ui| {
        let letter = |id: u32| library.revision(id).map(|r| r.letter()).unwrap_or_default();
        egui::ComboBox::from_id_salt("fab_rev").selected_text(format!("Rev {}", letter(rev_id))).show_ui(ui, |ui| {
            for r in &revs {
                if ui.selectable_label(r.id == rev_id, format!("Rev {}", r.letter())).clicked() {
                    ed.fab_rev = Some(r.id);
                }
            }
        });
        let lab_name = registry().lab(&ed.fab_lab).map_or("?", |l| l.name.as_str());
        egui::ComboBox::from_id_salt("fab_lab").selected_text(lab_name).show_ui(ui, |ui| {
            for lab in &registry().labs {
                let tip = format!("{:.0} Defects/cm² · σ {:.0}% · Overlay {:.1} µm", lab.defect_per_cm2, lab.sigma * 100.0, lab.overlay_um);
                if ui.selectable_label(lab.id == ed.fab_lab, &lab.name).on_hover_text(tip).clicked() {
                    ed.fab_lab = lab.id.clone();
                }
            }
        });
        let bill = library.lot_bill(registry(), rev_id, &ed.fab_lab).map(|b| resources(&b)).unwrap_or_default();
        let tip = bill.iter().map(|(r, n)| format!("{:.1} {}", *n as f32 / 64.0, NAMES[*r])).collect::<Vec<_>>().join("\n");
        let masks = if library.mask_of(rev_id).is_some() { "Masks Made" } else { "Masks Needed" };
        let start = ui.add_enabled(bench, egui::Button::new("Start Lot")).on_hover_text(format!("{masks}\n{tip}"));
        if start.clicked() {
            *action = UiAction::Chip(Action::StartLot { revision: rev_id, lab: ed.fab_lab.clone() });
        }
    });
    for lot in library.lots.iter().rev().filter(|l| revs.iter().any(|r| r.id == l.revision)).take(6) {
        let Some(rev) = library.revision(lot.revision) else { continue };
        let steps = route(rev.design.layers);
        let lab = registry().lab(&lot.lab).map_or("?", |l| l.name.as_str());
        let line = match (&lot.outcome, steps.get(usize::from(lot.step))) {
            (Some(o), None) => format!("{} Dies · {} Good · A {} · B {} · C {}", o.dies, o.good, o.bins[0], o.bins[1], o.bins[2]),
            (_, Some(s)) => format!("{}/{} · {}", lot.step, steps.len(), step_name(*s)),
            (None, None) => String::new(),
        };
        ui.horizontal(|ui| {
            let (thumb, _) = ui.allocate_exact_size(vec2(44.0, 44.0), egui::Sense::hover());
            if let Some((side, dies)) = wafer_dies(library, lot.id) {
                chip_art::wafer(ui.painter(), thumb, side, dies.into_iter());
            }
            ui.vertical(|ui| {
                let r = ui.selectable_label(ed.lot == Some(lot.id), format!("Lot {} · Rev {} · {lab}", lot.id, rev.letter()));
                if r.clicked() {
                    ed.lot = if ed.lot == Some(lot.id) { None } else { Some(lot.id) };
                }
                let done = lot.step as f32 / steps.len().max(1) as f32;
                let step_left = lot.busy_until.saturating_sub(now) as f32 / 120.0;
                icons::gauge(ui, done, icons::TEAL, &format!("{:.0}% · Next {:.0} s", done * 100.0, step_left));
                ui.small(line);
            });
        });
    }
}

/// Dies of a lot at their wafer sites: probe results once probed, the bare
/// site grid before.
fn wafer_dies(library: &Library, lot: u32) -> Option<(f32, Vec<((f32, f32), Color32)>)> {
    if let Some((side, map)) = library.wafer_map(registry(), lot) {
        return Some((side, map.into_iter().map(|(at, d)| (at, chip_art::die_colour(d))).collect()));
    }
    let l = library.lots.iter().find(|l| l.id == lot)?;
    let rev = library.revision(l.revision)?;
    let process = registry().process(&rev.design.process)?;
    let area = engine_chip::foundry::die_mm2(registry(), &rev.design);
    let n = engine_chip::lot::dies_per_wafer(process.wafer_mm, area);
    let (side, sites) = engine_chip::lot::wafer_sites(process.wafer_mm, area, n);
    Some((side, chip_art::unprobed(&sites).collect()))
}

fn wafer_panel(ui: &mut egui::Ui, library: &Library, lot: &WaferLot) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(150.0, 150.0), egui::Sense::hover());
        if let Some((side, dies)) = wafer_dies(library, lot.id) {
            chip_art::wafer(ui.painter(), rect, side, dies.into_iter());
        }
        ui.vertical(|ui| {
            let Some(o) = lot.outcome else {
                ui.small(RichText::new("Unprobed").color(icons::GOLD));
                return;
            };
            ui.label(RichText::new(format!("{} Dies · {} Good · {:.0}%", o.dies, o.good, 100.0 * f32::from(o.good) / f32::from(o.dies.max(1)))).color(icons::PAPER));
            let dead = o.dies.saturating_sub(o.good + o.slow);
            for (c, name, n) in [
                (chip_art::BIN_A, "Bin A", o.bins[0]),
                (chip_art::BIN_B, "Bin B", o.bins[1]),
                (chip_art::BIN_C, "Bin C", o.bins[2]),
                (chip_art::SLOW, "Slow", o.slow),
                (chip_art::DEAD, "Dead", dead),
            ] {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0, c);
                    ui.small(format!("{name} {n}"));
                });
            }
        });
    });
}

fn step_name(s: Step) -> String {
    match s.mask {
        Some(m) => format!("{} {}", s.kind.label(), m.label()),
        None => s.kind.label().to_string(),
    }
}

/// Built state of one layer after the steps done so far.
#[derive(Clone, Copy, PartialEq)]
enum Film {
    None,
    Blanket,
    Patterned,
}

struct Stack {
    oxide: Film,
    doped: bool,
    gate: Film,
    dielectric: u8,
    contact: bool,
    metal: [Film; 3],
    via: [bool; 3],
    resist: Option<(MaskLayer, bool)>,
    passivated: bool,
}

fn build(steps: &[Step], done: usize) -> Stack {
    let mut s = Stack { oxide: Film::None, doped: false, gate: Film::None, dielectric: 0, contact: false, metal: [Film::None; 3], via: [false; 3], resist: None, passivated: false };
    let mut deposited_metal = 0usize;
    let mut films = 0;
    for (i, step) in steps.iter().take(done).enumerate() {
        let next_mask = steps[i..].iter().find_map(|t| t.mask);
        match step.kind {
            StepKind::Oxidize => s.oxide = Film::Blanket,
            StepKind::Coat => s.resist = next_mask.map(|m| (m, false)),
            StepKind::Develop => s.resist = s.resist.map(|(m, _)| (m, true)),
            StepKind::Etch => match s.resist.map(|r| r.0) {
                Some(MaskLayer::Diffusion) => s.oxide = Film::Patterned,
                Some(MaskLayer::Gate) => s.gate = Film::Patterned,
                Some(MaskLayer::Contact) => s.contact = true,
                Some(MaskLayer::Metal(l)) => s.metal[usize::from(l).min(2)] = Film::Patterned,
                Some(MaskLayer::Via(l)) => s.via[usize::from(l).min(2)] = true,
                None => {}
            },
            StepKind::Strip => s.resist = None,
            StepKind::Implant => s.doped = true,
            StepKind::DepositFilm => {
                films += 1;
                if films == 1 {
                    s.gate = Film::Blanket;
                } else {
                    s.dielectric += 1;
                }
            }
            StepKind::DepositMetal => {
                s.metal[deposited_metal.min(2)] = Film::Blanket;
                deposited_metal += 1;
            }
            StepKind::Passivate => s.passivated = true,
            _ => {}
        }
    }
    s
}

fn feature(d: &Design, layer: MaskLayer, x: u8, y: u8) -> bool {
    let r = registry();
    match layer {
        MaskLayer::Diffusion | MaskLayer::Gate => d.part_at(r, x, y).is_some(),
        MaskLayer::Contact => d.nets.iter().flat_map(|n| n.pins.iter()).any(|p| d.pin_site(r, p).is_some_and(|s| (s.x, s.y) == (x, y))),
        MaskLayer::Metal(l) => d.nets.iter().flat_map(|n| n.paths.iter().flatten()).any(|s| (s.x, s.y, s.layer) == (x, y, l)),
        MaskLayer::Via(l) => d.nets.iter().flat_map(|n| n.paths.iter()).flat_map(|p| p.windows(2))
            .any(|w| w[0].layer != w[1].layer && w[0].layer.max(w[1].layer) == l && (w[0].x, w[0].y) == (x, y)),
    }
}

const SUBSTRATE: Color32 = Color32::from_rgb(70, 78, 96);
const DOPED: Color32 = Color32::from_rgb(140, 90, 170);
const OXIDE: Color32 = Color32::from_rgb(170, 200, 220);
const GATE: Color32 = Color32::from_rgb(170, 70, 60);
const DIELECTRIC: Color32 = Color32::from_rgb(110, 130, 120);
const CONTACT: Color32 = Color32::from_rgb(220, 190, 80);
const METAL: [Color32; 3] = [Color32::from_rgb(214, 128, 64), Color32::from_rgb(70, 190, 180), Color32::from_rgb(170, 120, 230)];
const RESIST: Color32 = Color32::from_rgb(230, 120, 170);
const PASSIVATION: Color32 = Color32::from_rgb(90, 170, 110);

fn draw(ui: &mut egui::Ui, d: &Design, s: &Stack, y: u8, label: &str) {
    ui.small(label);
    let w = ui.available_width().min(520.0);
    let cell = w / f32::from(d.width);
    let (rect, _) = ui.allocate_exact_size(vec2(w, 70.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0, Color32::from_rgb(16, 20, 19));
    let band = |x: u8, top: f32, h: f32, c: Color32| {
        let x0 = rect.left() + f32::from(x) * cell;
        p.rect_filled(Rect::from_min_size(pos2(x0, rect.bottom() - top - h), vec2(cell, h)), 0, c);
    };
    for x in 0..d.width {
        let has = |m| feature(d, m, x, y);
        band(x, 0.0, 22.0, SUBSTRATE);
        if s.doped && has(MaskLayer::Diffusion) {
            band(x, 14.0, 8.0, DOPED);
        }
        if s.oxide == Film::Blanket || (s.oxide == Film::Patterned && !has(MaskLayer::Diffusion)) {
            band(x, 22.0, 4.0, OXIDE);
        }
        if s.gate == Film::Blanket || (s.gate == Film::Patterned && has(MaskLayer::Gate) && x % 2 == 0) {
            band(x, 26.0, 4.0, GATE);
        }
        if s.dielectric > 0 && !(s.contact && has(MaskLayer::Contact)) {
            band(x, 30.0, 6.0, DIELECTRIC);
        } else if s.contact && has(MaskLayer::Contact) {
            band(x, 22.0, 14.0, CONTACT);
        }
        let mut top = 36.0;
        for l in 0..3u8 {
            let m = s.metal[usize::from(l)];
            if m == Film::Blanket || (m == Film::Patterned && has(MaskLayer::Metal(l))) {
                band(x, top, 5.0, METAL[usize::from(l)]);
            }
            if l > 0 && s.via[usize::from(l)] && has(MaskLayer::Via(l)) {
                band(x, top - 6.0, 6.0, Color32::WHITE);
            }
            top += 11.0;
        }
        if let Some((m, developed)) = s.resist
            && !(developed && has(m))
        {
            band(x, 64.0, 4.0, RESIST);
        }
        if s.passivated {
            band(x, 66.0, 3.0, PASSIVATION);
        }
    }
    p.rect_stroke(rect, 0, Stroke::new(1.0, icons::GOLD), egui::StrokeKind::Inside);
}

pub(super) fn lot_window(ctx: &egui::Context, ed: &mut Editor, library: &Library) {
    let Some(lot) = ed.lot.and_then(|id| library.lots.iter().find(|l| l.id == id)).cloned() else { return };
    let Some(rev) = library.revision(lot.revision) else { return };
    let steps = route(rev.design.layers);
    let mut open = true;
    egui::Window::new(format!("Lot {}", lot.id)).open(&mut open).default_width(560.0).show(ctx, |ui| {
        let mut row = ed.section_row.min(rev.design.height.saturating_sub(1));
        ui.add(egui::Slider::new(&mut row, 0..=rev.design.height.saturating_sub(1)).text("Row"));
        ed.section_row = row;
        wafer_panel(ui, library, &lot);
        draw(ui, &rev.design, &build(&steps, steps.len()), row, "Intended");
        draw(ui, &rev.design, &build(&steps, usize::from(lot.step)), row, &current(&lot, &steps));
        legend(ui);
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for r in lot.records.iter().rev() {
                let c = if r.fault { Color32::from_rgb(235, 120, 90) } else { icons::PAPER };
                ui.small(RichText::new(format!("{:>2} · {}", r.step + 1, r.text())).color(c));
            }
        });
    });
    if !open {
        ed.lot = None;
    }
}

fn current(lot: &WaferLot, steps: &[Step]) -> String {
    match steps.get(usize::from(lot.step)) {
        Some(s) => format!("Built · Next {}", step_name(*s)),
        None => "Built · Complete".into(),
    }
}

fn legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (c, name) in [(SUBSTRATE, "Substrate"), (DOPED, "Doped"), (OXIDE, "Oxide"), (GATE, "Gate"), (DIELECTRIC, "Dielectric"), (CONTACT, "Contact"), (METAL[0], "M1"), (METAL[1], "M2"), (Color32::WHITE, "Via"), (RESIST, "Resist"), (PASSIVATION, "Passivation")] {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 0, c);
            ui.small(name);
        }
    });
}
