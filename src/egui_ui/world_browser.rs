//! Searchable saved-world cards, with a separate selection/details surface.
use super::front_end::{HomePage, action, caption, heading, landscape, surface, tab};
use super::*;
use crate::world::storage::WorldEntry;
use egui::{Color32, FontId, Rect, Sense, Stroke, StrokeKind, vec2};
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Sort {
    #[default]
    Recent,
    Name,
    Favorites,
}
#[derive(Default)]
pub(super) struct BrowserState {
    query: String,
    selected: Option<String>,
    sort: Sort,
    only_favorites: bool,
    editing_name: String,
    original_name: String,
    revision: u64,
}
fn age(saved: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let seconds = now.saturating_sub(saved / 1_000_000);
    match seconds {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86399 => format!("{} h ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86400),
    }
}
fn kind(e: &WorldEntry) -> &'static str {
    match e.kind {
        WorldType::Hills => "Wildlands",
        WorldType::Ridged => "Highlands",
        WorldType::Superflat => "Open canvas",
    }
}
fn card(ui: &mut egui::Ui, e: &WorldEntry, selected: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 80.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!(
                "{}, {}",
                e.name,
                if selected { "selected" } else { "saved world" }
            ),
        )
    });
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(
        r,
        0,
        if selected {
            Color32::from_rgb(76, 60, 44)
        } else {
            Color32::from_rgb(42, 35, 36)
        },
    );
    p.rect_stroke(
        r,
        0,
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected || response.has_focus() {
                Color32::from_rgb(244, 230, 202)
            } else {
                Color32::from_rgb(103, 86, 75)
            },
        ),
        StrokeKind::Inside,
    );
    let art = Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2(64.0, 64.0));
    landscape(&p.with_clip_rect(art), art, e.seed, e.kind);
    let x = r.left() + 82.0;
    let width = (r.width() - 94.0).max(80.0);
    let name = p.layout(
        format!("{}{}", if e.favorite { "★  " } else { "" }, e.name),
        FontId::proportional(18.0),
        presentation::PAPER,
        width,
    );
    p.galley(
        egui::pos2(x, r.top() + 10.0),
        name,
        presentation::PAPER,
    );
    p.text(
        egui::pos2(x, r.bottom() - 20.0),
        Align2::LEFT_CENTER,
        format!(
            "{} · {}",
            kind(e),
            if e.saved > 0 {
                age(e.saved)
            } else {
                "Needs recovery".into()
            }
        ),
        FontId::proportional(14.0),
        presentation::MUTED,
    );
    response.on_hover_text(&e.name)
}
fn details(ui: &mut egui::Ui, owner: &mut EguiUi, result: &mut UiAction) {
    let entries = std::sync::Arc::clone(&owner.library_entries);
    let Some(e) = entries
        .iter()
        .find(|e| Some(&e.id) == owner.browser.selected.as_ref())
    else {
        heading(
            ui,
            "Select a world",
            "Choose a saved world to view its details.",
        );
        return;
    };
    caption(ui, "SELECTED WORLD");
    ui.heading(RichText::new(&e.name).size(25.0));
    ui.label(RichText::new(format!("{} · Seed {}", kind(e), e.seed)).weak());
    ui.label(format!(
        "{} edited chunks · {} recovery checkpoints",
        e.chunks, e.checkpoints
    ));
    ui.label(format!(
        "{} · Last saved {}",
        e.scale.label(),
        if e.saved > 0 {
            age(e.saved)
        } else {
            "unknown".into()
        }
    ));
    if let Some(problem) = &e.problem {
        ui.label(RichText::new(problem).color(Color32::from_rgb(229, 183, 110)));
    }
    ui.add_space(12.0);
    ui.add_enabled_ui(!owner.library_loading, |ui| {
        if action(
            ui,
            if owner.library_loading {
                "Opening world…"
            } else {
                "Continue this world"
            },
            true,
        )
        .clicked()
        {
            *result = UiAction::OpenWorld(e.id.clone());
        }
    });
    ui.small("Opens the newest complete checkpoint for this world. Earlier checkpoints are tried if recovery is needed.");
    if !e.id.starts_with("legacy-") {
        ui.add_space(14.0);
        caption(ui, "WORLD DETAILS");
        ui.add(
            egui::TextEdit::singleline(&mut owner.browser.editing_name)
                .hint_text("World name")
                .char_limit(64)
                .desired_width(f32::INFINITY),
        );
        let name = owner.browser.editing_name.trim();
        let valid = !name.is_empty() && !name.chars().any(char::is_control) && name.len() <= 256;
        ui.add_enabled_ui(valid && name != e.name && !owner.library_loading, |ui| {
            if action(ui, "Save name", false).clicked() {
                *result = UiAction::RenameWorld {
                    id: e.id.clone(),
                    name: name.to_owned(),
                };
            }
        });
        if tab(
            ui,
            if e.favorite {
                "★ Favorite"
            } else {
                "☆ Add to favorites"
            },
            e.favorite,
        )
        .clicked()
        {
            *result = UiAction::FavoriteWorld {
                id: e.id.clone(),
                favorite: !e.favorite,
            };
        }
    } else {
        ui.small("This is a legacy save. Open and save it once to enable named-world details.");
    }
}
pub(super) fn draw(owner: &mut EguiUi) -> UiAction {
    let ctx = owner.ctx.clone();
    let entries = std::sync::Arc::clone(&owner.library_entries);
    if owner.browser.revision != owner.library_revision {
        owner.browser.revision = owner.library_revision;
        if !entries
            .iter()
            .any(|e| Some(&e.id) == owner.browser.selected.as_ref())
        {
            owner.browser.selected = entries.first().map(|e| e.id.clone());
        }
        if let Some(e) = entries
            .iter()
            .find(|e| Some(&e.id) == owner.browser.selected.as_ref())
        {
            if owner.browser.editing_name == owner.browser.original_name
                || owner.browser.original_name.is_empty()
            {
                owner.browser.editing_name = e.name.clone();
            }
            owner.browser.original_name = e.name.clone();
        }
    }
    let mut result = UiAction::None;
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(&ctx,|ui| {
        egui::ScrollArea::vertical().id_salt("library_screen").show(ui,|ui| {
            ui.add_space(22.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width((ui.available_width()-32.0).clamp(220.0,1080.0));
                surface().show(ui,|ui| {
                    caption(ui,"VOXEL WORKSHOP");
                    heading(ui,"Worlds","Open a saved world or create a new one.");
                    if !entries.is_empty() { ui.horizontal_wrapped(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut owner.browser.query).hint_text("Search name, landscape or seed").desired_width(270.0));
                        for (sort,label) in [(Sort::Recent,"Recent"),(Sort::Name,"A–Z"),(Sort::Favorites,"Favorites first")] {
                            if tab(ui,label,owner.browser.sort==sort).clicked(){owner.browser.sort=sort;}
                        }
                        if tab(ui,"★ Only favorites",owner.browser.only_favorites).clicked(){owner.browser.only_favorites=!owner.browser.only_favorites;}
                        if ui.small_button("Refresh").clicked(){result=UiAction::RefreshWorlds;}
                    }); }
                    if owner.library_loading {ui.label(RichText::new("Opening your world… Terrain will be prepared before movement begins.").color(Color32::LIGHT_GREEN));}
                    else if owner.library_busy {ui.small("Updating your world library…");}
                    if let Some(status)=&owner.library_status {ui.label(status);}
                    if let Some(status)=&owner.persistence_status {ui.label(RichText::new(status).small().weak());}
                    ui.add_space(10.0);
                    let query=owner.browser.query.trim().to_lowercase();
                    let mut matching:Vec<_>=entries.iter().filter(|e|(!owner.browser.only_favorites || e.favorite)
                        && (query.is_empty() || e.name.to_lowercase().contains(&query) || kind(e).to_lowercase().contains(&query) || e.seed.to_string().contains(&query))).collect();
                    match owner.browser.sort {
                        Sort::Recent=>matching.sort_by_key(|e|std::cmp::Reverse(e.saved)),
                        Sort::Name=>matching.sort_by_cached_key(|e|e.name.to_lowercase()),
                        Sort::Favorites=>matching.sort_by_key(|e|(!e.favorite,std::cmp::Reverse(e.saved))),
                    }
                    if matching.is_empty() {
                        ui.add_space(20.0);
                        ui.label(if entries.is_empty(){"No saved worlds. Create one to start playing."}else{"No worlds match your search."});
                        if action(ui,"Create a new world",true).clicked(){owner.home_page=HomePage::Create;}
                    } else {
                        let list=|ui:&mut egui::Ui,owner:&mut EguiUi| {
                            egui::ScrollArea::vertical().id_salt("world_cards").max_height(470.0)
                                .show_rows(ui,104.0,matching.len(),|ui,range| {
                                    for i in range {
                                        let e=matching[i];
                                        if card(ui,e,Some(&e.id)==owner.browser.selected.as_ref()).clicked(){
                                            owner.browser.selected=Some(e.id.clone());
                                            owner.browser.editing_name=e.name.clone();owner.browser.original_name=e.name.clone();
                                        }
                                    }
                                });
                        };
                        if ui.available_width()>820.0 {
                            ui.columns(2,|cols| {list(&mut cols[0],owner);details(&mut cols[1],owner,&mut result);});
                        } else {list(ui,owner);ui.add_space(16.0);details(ui,owner,&mut result);}
                    }
                });
            });
            ui.add_space(24.0);
        });
    });
    result
}
