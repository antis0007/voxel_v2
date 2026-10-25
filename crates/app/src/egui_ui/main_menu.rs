//! Main menu with a short list of actions.
use super::{grimoire as style, pixel_art};
use egui::{Color32, RichText, vec2};

#[derive(Default)]
pub(super) enum Intent {
    #[default]
    None,
    Continue,
    Load,
    New,
    Settings,
    Quit,
}

pub(super) fn draw(
    ctx: &egui::Context, recent: Option<&str>, busy: bool, status: Option<&str>,
) -> Intent {
    let mut result = Intent::None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(20, 24, 27)))
        .show(ctx, |ui| {
            let width = (ui.available_width() - 32.0).clamp(120.0, 320.0);
            ui.add_space((ui.available_height() * 0.22).min(160.0));
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                egui::ScrollArea::vertical().id_salt("title_menu").show(ui, |ui| {
                    ui.label(RichText::new("Play").size(32.0).color(pixel_art::PAPER));
                    ui.add_space(24.0);
                    for (label, enabled, intent) in [
                        ("Continue", recent.is_some() && !busy, Intent::Continue),
                        ("Load World", !busy, Intent::Load),
                        ("New World", !busy, Intent::New),
                        ("Options", true, Intent::Settings),
                        ("Quit", true, Intent::Quit),
                    ] {
                        let response = ui.add_enabled(enabled,
                            egui::Button::new(RichText::new(label).size(20.0))
                                .min_size(vec2(width, 44.0)));
                        if response.clicked() { result = intent; }
                        if label == "Continue"
                            && let Some(name) = recent { response.on_hover_text(name); }
                        ui.add_space(8.0);
                    }
                    if busy { ui.label(RichText::new("Loading...").color(style::SILVER)); }
                    if let Some(status) = status {
                        ui.label(RichText::new(status).color(pixel_art::GOLD));
                    }
                });
            });
        });
    result
}
