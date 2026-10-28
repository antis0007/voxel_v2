//! Chip designer: a project's draft floorplan in the middle, the module
//! palette on the left, the compile report and released revisions on the
//! right. The draft is edited here and synced to the world's library after
//! each change; releases and prototypes are world actions.
use super::{UiAction, chip_canvas, chip_fab, chip_report, chip_tools, icons};
use crate::world::factory::View;
use crate::world::factory::chips::{Action, registry};
use egui::{Color32, Pos2, RichText};
use engine_chip::characterize::Report;
use engine_chip::compile::Compiled;
use engine_chip::design::{Design, Pin, Site};
use engine_chip::firmware::Task;
use engine_chip::library::Library;
use engine_chip::presets;
use engine_chip::registry::Registry;
use engine_chip::route::{route_all, route_dirty};

#[derive(Clone, PartialEq)]
pub(super) enum Tool {
    Select,
    Place(String),
    Wire,
    Draw,
}

#[derive(Clone)]
pub(super) struct Editor {
    open: bool,
    /// Library this state belongs to; another world's library resets it.
    session: u64,
    pub project: Option<u32>,
    pub generation: u64,
    projects_seen: usize,
    pub design: Design,
    pub undo: Vec<Design>,
    pub redo: Vec<Design>,
    pub unsynced: bool,
    /// Library generation expected once the last sent edit is applied.
    pub pending: Option<u64>,
    pub tool: Tool,
    pub rot: u8,
    pub selected: Option<u16>,
    pub net: Option<u16>,
    pub pin: Option<Pin>,
    pub layer: u8,
    pub hidden: [bool; 3],
    pub path: Vec<Site>,
    /// Dragged part and the grab offset inside it.
    pub drag: Option<(u16, u8, u8)>,
    pub zoom: f32,
    /// Canvas origin and site size last frame (input tests aim with it).
    pub canvas: Option<(Pos2, f32)>,
    /// Design text being pasted, and whether the last paste parsed.
    pub paste: Option<(String, bool)>,
    /// Fab column: chosen revision, lab, open lot and cross-section row.
    pub fab_rev: Option<u32>,
    pub fab_lab: String,
    pub lot: Option<u32>,
    pub section_row: u8,
    pub compiled: Option<Compiled>,
    pub reports: Vec<(u32, u64, Option<Report>)>,
}

impl Default for Editor {
    fn default() -> Self {
        Self { design: presets::sentinel(registry()), ..Self::blank() }
    }
}

impl Editor {
    /// A closed editor with an empty design; costs no preset build.
    fn blank() -> Self {
        Self {
            open: false,
            session: 0,
            project: None,
            generation: 0,
            projects_seen: 0,
            design: Design::new("", "", 0, 0, 0),
            undo: Vec::new(),
            redo: Vec::new(),
            unsynced: false,
            pending: None,
            tool: Tool::Select,
            rot: 0,
            selected: None,
            net: None,
            pin: None,
            layer: 0,
            hidden: [false; 3],
            path: Vec::new(),
            drag: None,
            zoom: 1.0,
            canvas: None,
            paste: None,
            fab_rev: None,
            fab_lab: "bench".into(),
            lot: None,
            section_row: 4,
            compiled: None,
            reports: Vec::new(),
        }
    }

    /// Apply an edit with undo; `f` returns whether it changed anything.
    /// Wires the edit left stale are routed again.
    pub fn commit(&mut self, f: impl FnOnce(&mut Design, &Registry) -> bool) {
        let before = self.design.clone();
        if f(&mut self.design, registry()) {
            route_dirty(&mut self.design, registry());
            self.undo.push(before);
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.changed();
        }
    }
    pub(super) fn changed(&mut self) {
        self.compiled = None;
        self.unsynced = self.project.is_some();
    }
    fn load(&mut self, project: u32, design: Design, generation: u64) {
        *self = Self { open: true, session: self.session, project: Some(project), generation, projects_seen: self.projects_seen, design, ..Self::default() };
    }
}

pub(super) fn id() -> egui::Id {
    egui::Id::new("chip_designer")
}

pub(super) fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Editor>(id()).open = true);
}

pub(super) fn open_lot(ctx: &egui::Context, lot: u32) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Editor>(id()).lot = Some(lot));
}

pub(super) fn window(ctx: &egui::Context, view: &View, action: &mut UiAction) {
    // Moved out and back rather than cloned each frame. Anything reading the
    // slot in between sees a blank editor that the end of this call replaces.
    let taken = ctx.data_mut(|d| {
        let stored = d.get_temp_mut_or_default::<Editor>(id());
        stored.open.then(|| std::mem::replace(stored, Editor::blank()))
    });
    let Some(mut ed) = taken else { return };
    let Some(library) = view.chips.clone() else {
        ctx.data_mut(|d| d.insert_temp(id(), ed));
        return;
    };
    if ed.session != library.session {
        ed = Editor { open: true, session: library.session, ..Editor::default() };
    }
    // A project created last frame (preset or paste) opens itself.
    if library.projects.len() > ed.projects_seen {
        if let Some(p) = library.projects.last() {
            ed.load(p.id, p.draft.clone(), p.generation);
        }
        ed.projects_seen = library.projects.len();
    }
    // Adopt the library's generation once the last sent edit landed; resend
    // an edit that another panel's action displaced.
    if let (Some(expected), Some(p)) = (ed.pending, ed.project.and_then(|p| library.project(p))) {
        if p.generation >= expected {
            ed.generation = p.generation;
        } else {
            ed.unsynced = true;
        }
        ed.pending = None;
    }
    let mut open = true;
    let screen = ctx.content_rect();
    let shown = egui::Window::new("Chip Designer")
        .open(&mut open)
        .default_size([(screen.width() - 40.0).min(1100.0), (screen.height() - 40.0).min(680.0)])
        .max_width(screen.width() - 16.0)
        .max_height(screen.height() - 16.0)
        .resizable(true)
        .show(ctx, |ui| {
            toolbar(ui, &mut ed, &library, action);
            ui.separator();
            ui.horizontal_top(|ui| {
                let total = ui.available_width();
                let side = (total * 0.24).clamp(110.0, 290.0);
                let middle = (total - side * 1.7 - 24.0).max(200.0);
                ui.vertical(|ui| {
                    ui.set_width(side * 0.7);
                    egui::ScrollArea::vertical().id_salt("chip_palette").max_height(screen.height() * 0.7).show(ui, |ui| chip_tools::palette(ui, &mut ed));
                });
                ui.vertical(|ui| {
                    ui.set_width(middle);
                    chip_canvas::canvas(ui, &mut ed, screen.height() * 0.62);
                });
                ui.vertical(|ui| {
                    ui.set_width(side);
                    egui::ScrollArea::vertical().id_salt("chip_report").max_height(screen.height() * 0.7).show(ui, |ui| {
                        chip_report::report(ui, &mut ed);
                        ui.separator();
                        super::semiconductor::section(ui, view, action);
                        let free = view.fabrication_site.is_some_and(|s| !view.fabrication.owns((s.x, s.y, s.z)));
                        chip_report::revisions(ui, &mut ed, &library, free, action);
                        chip_fab::section(ui, &mut ed, &library, free, &view.fabrication, action);
                    });
                });
            });
        });
    chip_fab::lot_window(ctx, &mut ed, &library);
    let hovered = shown.zip(ctx.pointer_hover_pos()).is_some_and(|(w, p)| w.response.rect.contains(p));
    if hovered {
        chip_tools::keys(ctx, &mut ed);
    }
    ed.open &= open;
    if ed.unsynced && matches!(action, UiAction::None)
        && let Some(project) = ed.project
    {
        *action = UiAction::Chip(Action::SaveDraft { project, design: ed.design.clone() });
        ed.pending = Some(ed.generation + 1);
        ed.unsynced = false;
    }
    ctx.data_mut(|d| d.insert_temp(id(), ed));
}

fn toolbar(ui: &mut egui::Ui, ed: &mut Editor, library: &Library, action: &mut UiAction) {
    ui.horizontal_wrapped(|ui| {
        let current = ed.project.and_then(|p| library.project(p)).map_or("Unsaved".to_string(), |p| p.name.clone());
        egui::ComboBox::from_id_salt("chip_project").selected_text(current).show_ui(ui, |ui| {
            for p in &library.projects {
                if ui.selectable_label(ed.project == Some(p.id), &p.name).clicked() {
                    ed.load(p.id, p.draft.clone(), p.generation);
                }
            }
        });
        egui::ComboBox::from_id_salt("chip_new").selected_text("New").show_ui(ui, |ui| {
            for (name, make) in [("Sentinel", presets::sentinel as fn(&Registry) -> Design), ("Twin Watch", presets::twin_watch)] {
                if ui.selectable_label(false, name).clicked() {
                    *action = UiAction::Chip(Action::NewProject(name.into(), make(registry())));
                }
            }
            if ui.selectable_label(false, "Blank 12×10").clicked() {
                *action = UiAction::Chip(Action::NewProject("Blank".into(), Design::new("Blank", presets::PROCESS, 12, 10, 2)));
            }
        });
        ui.separator();
        for (tool, label, key) in [(Tool::Select, "Select", "[Esc]"), (Tool::Wire, "Connect", "[RMB] Detach"), (Tool::Draw, "Draw Wire", "[Enter] Commit")] {
            if ui.selectable_label(ed.tool == tool, label).on_hover_text(key).clicked() {
                ed.tool = tool;
                ed.path.clear();
                ed.pin = None;
            }
        }
        ui.separator();
        for l in 0..ed.design.layers.min(3) {
            let on = !ed.hidden[usize::from(l)];
            let text = RichText::new(format!("M{}", l + 1)).color(if on { icons::PAPER } else { Color32::GRAY });
            let r = ui.selectable_label(ed.layer == l, text).on_hover_text(format!("[{}] · [RMB] Hide", l + 1));
            if r.clicked() {
                ed.layer = l;
            }
            if r.secondary_clicked() {
                ed.hidden[usize::from(l)] = on;
            }
        }
        ui.separator();
        if ui.button("Route").on_hover_text("[A]").clicked() {
            ed.commit(|d, r| {
                route_all(d, r);
                true
            });
        }
        if ui.add_enabled(!ed.undo.is_empty(), egui::Button::new("Undo")).on_hover_text("[Ctrl+Z]").clicked() {
            chip_tools::undo(ed);
        }
        if ui.add_enabled(!ed.redo.is_empty(), egui::Button::new("Redo")).on_hover_text("[Ctrl+Y]").clicked() {
            chip_tools::redo(ed);
        }
        ui.separator();
        if ui.button("Copy").on_hover_text("Design Text").clicked() {
            ui.ctx().copy_text(ron::to_string(&ed.design).unwrap_or_default());
        }
        if ui.selectable_label(ed.paste.is_some(), "Paste").clicked() {
            ed.paste = if ed.paste.is_some() { None } else { Some((String::new(), true)) };
        }
    });
    ui.horizontal_wrapped(|ui| {
        let mut fw = ed.design.firmware;
        let mut layers = ed.design.layers;
        let mut changed = false;
        egui::ComboBox::from_id_salt("chip_task").selected_text(fw.label()).show_ui(ui, |ui| {
            for task in Task::ALL {
                changed |= ui.selectable_value(&mut fw.task, task, task.label()).changed();
            }
        });
        changed |= ui.add(egui::DragValue::new(&mut fw.channels).range(1..=4).suffix(" Sensors")).changed();
        changed |= ui.add(egui::DragValue::new(&mut fw.sample_ms).range(1..=5000).suffix(" ms Wake")).changed();
        changed |= ui.add(egui::DragValue::new(&mut layers).range(1..=3).suffix(" Layers")).changed();
        ui.label(RichText::new(format!("{:.0}%", ed.zoom * 100.0)).small()).on_hover_text("[Ctrl]+Wheel");
        if changed {
            ed.commit(|d, _| {
                d.firmware = fw;
                d.layers = layers;
                d.nets.iter_mut().for_each(|n| n.paths.retain(|p| p.iter().all(|s| s.layer < layers)));
                true
            });
        }
    });
    chip_tools::paste(ui, ed, action);
}

#[cfg(test)]
#[path = "chip_ui_tests.rs"]
mod tests;
