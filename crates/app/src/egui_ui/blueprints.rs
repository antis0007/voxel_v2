//! Blueprint list: save the clipboard, load, rename and delete files.
use super::presentation as style;
use crate::creative::blueprint::{BlueprintTool, Shelf};
use crate::world::blueprint::library;
use egui::RichText;
use glam::IVec3;
use std::path::Path;

fn dims(size: IVec3) -> String {
    format!("{}×{}×{}", size.x, size.y, size.z)
}

fn refresh(shelf: &mut Shelf, dir: &Path) {
    match library::list(dir) {
        Ok(entries) => shelf.entries = entries,
        Err(e) => shelf.status = Some(format!("{e:#}")),
    }
    shelf.stale = false;
}

fn header(ui: &mut egui::Ui, tool: &mut BlueprintTool, open: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Clipboard").color(style::AMBER));
        let summary = tool.clipboard().map_or_else(|| "Empty".to_owned(), |bp| format!("{} · {} Blocks", dims(bp.size()), bp.solid_count()));
        ui.label(RichText::new(summary).color(style::MUTED).monospace());
    });
    if let Some((done, total)) = tool.progress() {
        ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(format!("{done} / {total} Cells")));
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut tool.paste_air, "Paste Air");
        ui.label(RichText::new(format!("Undo: {}", tool.undo_depth())).color(style::MUTED));
        if ui.button("Done").clicked() {
            *open = false;
        }
    });
}

fn save_row(ui: &mut egui::Ui, tool: &mut BlueprintTool, dir: &Path) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut tool.shelf.name).char_limit(library::MAX_NAME).desired_width(150.0).hint_text("Name"));
        let name = library::clean_name(&tool.shelf.name);
        let ready = name.is_some() && tool.clipboard().is_some();
        if ui.add_enabled(ready, egui::Button::new("Save")).clicked()
            && let (Some(name), Some(bp)) = (name, tool.clipboard())
        {
            tool.shelf.status = Some(match library::save(dir, &name, bp) {
                Ok(()) => format!("Saved: {name}"),
                Err(e) => format!("{e:#}"),
            });
            tool.shelf.stale = true;
        }
    });
}

enum RowAction {
    Load(String),
    StartRename(String),
    FinishRename(String, String),
    CancelRename,
    Delete(String),
}

fn rows(ui: &mut egui::Ui, shelf: &mut Shelf) -> Option<RowAction> {
    let mut action = None;
    egui::ScrollArea::vertical().max_height(280.0).auto_shrink([false, true]).show(ui, |ui| {
        for entry in &shelf.entries {
            ui.horizontal(|ui| {
                if let Some((from, to)) = shelf.renaming.as_mut().filter(|(from, _)| *from == entry.name) {
                    ui.add(egui::TextEdit::singleline(to).char_limit(library::MAX_NAME).desired_width(140.0));
                    if ui.small_button("Done").clicked() {
                        action = Some(RowAction::FinishRename(from.clone(), to.clone()));
                    }
                    if ui.small_button("Back").clicked() {
                        action = Some(RowAction::CancelRename);
                    }
                    return;
                }
                ui.label(RichText::new(&entry.name).color(style::PAPER));
                let size = entry.size.map_or_else(|| "Unreadable".to_owned(), dims);
                ui.label(RichText::new(size).color(style::MUTED).monospace());
                if ui.small_button("Load").clicked() {
                    action = Some(RowAction::Load(entry.name.clone()));
                }
                if ui.small_button("Rename").clicked() {
                    shelf.confirm_delete = None;
                    action = Some(RowAction::StartRename(entry.name.clone()));
                }
                let confirming = shelf.confirm_delete.as_deref() == Some(entry.name.as_str());
                if ui.small_button(if confirming { "Confirm" } else { "✕" }).clicked() {
                    action = Some(RowAction::Delete(entry.name.clone()));
                }
            });
        }
    });
    action
}

pub fn draw(ctx: &egui::Context, open: &mut bool, tool: &mut BlueprintTool) {
    if !*open {
        return;
    }
    let dir = library::directory();
    if tool.shelf.stale {
        refresh(&mut tool.shelf, &dir);
    }
    egui::Window::new("Blueprints")
        .collapsible(false)
        .resizable(false)
        .default_width(330.0)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
        .show(ctx, |ui| {
            header(ui, tool, open);
            save_row(ui, tool, &dir);
            ui.separator();
            let action = rows(ui, &mut tool.shelf);
            let shelf = &mut tool.shelf;
            match action {
                Some(RowAction::Load(name)) => match library::load(&dir, &name) {
                    Ok(decoded) => {
                        shelf.status = Some(if decoded.unknown > 0 {
                            format!("Loaded: {name} · Unknown Cells: {}", decoded.unknown)
                        } else {
                            format!("Loaded: {name}")
                        });
                        tool.set_clipboard(decoded.blueprint);
                    }
                    Err(e) => shelf.status = Some(format!("{e:#}")),
                },
                Some(RowAction::CancelRename) => shelf.renaming = None,
                Some(RowAction::StartRename(name)) => shelf.renaming = Some((name.clone(), name)),
                Some(RowAction::FinishRename(from, to)) => {
                    match library::rename(&dir, &from, &to) {
                        Ok(()) => shelf.status = Some(format!("Renamed: {}", to.trim())),
                        Err(e) => shelf.status = Some(format!("{e:#}")),
                    }
                    shelf.renaming = None;
                    shelf.stale = true;
                }
                Some(RowAction::Delete(name)) if shelf.confirm_delete.as_deref() == Some(name.as_str()) => {
                    shelf.status = Some(match library::delete(&dir, &name) {
                        Ok(()) => format!("Deleted: {name}"),
                        Err(e) => format!("{e:#}"),
                    });
                    shelf.confirm_delete = None;
                    shelf.stale = true;
                }
                Some(RowAction::Delete(name)) => shelf.confirm_delete = Some(name),
                None => {}
            }
            if tool.shelf.entries.is_empty() {
                ui.label(RichText::new("No Blueprints").color(style::MUTED));
            }
            if let Some(status) = &tool.shelf.status {
                ui.label(RichText::new(status).color(style::MUTED));
            }
        });
}
