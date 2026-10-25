//! Designer keys, undo history and the module palette.
use super::chip_editor::{Editor, Tool};
use super::{chip_canvas, icons};
use crate::world::factory::chips::registry;
use egui::RichText;
use engine_chip::route::route_all;

pub(super) fn undo(ed: &mut Editor) {
    if let Some(d) = ed.undo.pop() {
        ed.redo.push(std::mem::replace(&mut ed.design, d));
        ed.changed();
    }
}
pub(super) fn redo(ed: &mut Editor) {
    if let Some(d) = ed.redo.pop() {
        ed.undo.push(std::mem::replace(&mut ed.design, d));
        ed.changed();
    }
}

pub(super) fn keys(ctx: &egui::Context, ed: &mut Editor) {
    if ctx.wants_keyboard_input() {
        return;
    }
    ctx.input(|i| {
        if i.modifiers.command && i.key_pressed(egui::Key::Z) {
            undo(ed);
        } else if i.modifiers.command && i.key_pressed(egui::Key::Y) {
            redo(ed);
        } else if i.key_pressed(egui::Key::R) {
            ed.rot = (ed.rot + 1) % 4;
            if let (Tool::Select, Some(id)) = (&ed.tool, ed.selected) {
                ed.commit(|d, r| d.part(id).cloned().is_some_and(|p| d.relocate(r, id, p.x, p.y, p.rot + 1).is_ok()));
            }
        } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
            if let Some(id) = ed.selected.take() {
                ed.commit(|d, r| d.remove(r, id).is_ok());
            }
        } else if i.key_pressed(egui::Key::A) {
            ed.commit(|d, r| {
                route_all(d, r);
                true
            });
        } else if i.key_pressed(egui::Key::Escape) {
            ed.tool = Tool::Select;
            ed.path.clear();
            ed.pin = None;
        }
        for (k, l) in [(egui::Key::Num1, 0), (egui::Key::Num2, 1), (egui::Key::Num3, 2)] {
            if i.key_pressed(k) && l < ed.design.layers {
                ed.layer = l;
            }
        }
    });
}

pub(super) fn palette(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.label(RichText::new("Modules").color(icons::GOLD));
    for m in &registry().modules {
        let on = ed.tool == Tool::Place(m.id.clone());
        let r = ui.selectable_label(on, format!("{} {}×{}", chip_canvas::short(m.role), m.size.0, m.size.1));
        if r.clicked() {
            ed.tool = Tool::Place(m.id.clone());
        }
        r.on_hover_text(format!("{} · {:.0} µW idle · {:.0} ns{}", m.name, m.idle_uw, m.delay_ns, if m.edge { " · Edge Pads" } else { "" }));
    }
    ui.add_space(6.0);
    ui.small(format!("Rotation {}°", u32::from(ed.rot) * 90));
}

