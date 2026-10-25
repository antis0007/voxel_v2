//! Shared game styling and control reference, separate from renderer diagnostics.
use egui::{Color32, RichText};

pub(super) const INK: Color32 = super::pixel_art::OUTLINE;
pub(super) const PAPER: Color32 = super::pixel_art::PAPER;
pub(super) const MUTED: Color32 = Color32::from_rgb(174, 176, 164);
pub(super) const EDGE: Color32 = super::grimoire::SILVER_DARK;
pub(super) const JADE: Color32 = super::pixel_art::TEAL;
pub(super) const AMBER: Color32 = super::pixel_art::GOLD;
pub(super) const DANGER: Color32 = Color32::from_rgb(225, 105, 91);
pub(super) const MANA: Color32 = super::pixel_art::TEAL;


pub(super) fn apply_theme(ctx: &egui::Context) {
    ctx.tessellation_options_mut(|o| {
        o.feathering = true;
        o.round_text_to_pixels = true;
        o.round_line_segments_to_pixels = true;
        o.round_rects_to_pixels = true;
    });
    super::typography::apply(ctx);
    // HUD panels are dark even on a light Windows desktop. Pin the preference
    // so the first native theme event cannot replace their readable text style.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style()).clone();
    // State changes are crisp; slot movement uses the explicit integer-pixel recoil.
    style.animation_time = 0.0;
    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = INK;
    style.visuals.panel_fill = INK;
    style.visuals.extreme_bg_color = super::grimoire::RECESS;
    style.visuals.override_text_color = Some(PAPER);
    style.visuals.selection.bg_fill = Color32::from_rgb(65, 59, 45);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, AMBER);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 46, 47);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, EDGE);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 62, 61);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, JADE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(65, 59, 45);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, AMBER);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.fg_stroke = egui::Stroke::new(1.0, PAPER);
    }
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    style.visuals.window_corner_radius = egui::CornerRadius::same(0);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
    }
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    // Solid scrollbars and bordered tooltip frames keep small controls clear.
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.visuals.window_stroke = egui::Stroke::new(1.0, EDGE);
    // Typography selects font families and atlas sampling independently of
    // physical voxel size and world render resolution.
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::new(20.0, egui::FontFamily::Name("PixelAccent".into())));
    for (kind, size) in [
        (egui::TextStyle::Body, 14.0),
        (egui::TextStyle::Button, 14.0),
        (egui::TextStyle::Small, 12.0),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));
    ctx.set_style(style);
}

pub(super) fn controls(ui: &mut egui::Ui) {
    egui::Grid::new("control_reference")
        .num_columns(2)
        .spacing([24.0, 8.0])
        .show(ui, |ui| {
            for (key, action) in [
                ("W A S D", "Move"),
                ("F", "Use station / open cache / harvest plant"),
                ("L", "Toggle Lumen Lantern"),
                (
                    "Alt + left mouse",
                    "Bore ore or rock (requires upgraded bore)",
                ),
                ("R / T", "Hold to cast / change spell"),
                ("Space", "Jump / swim up / ascend in flight"),
                ("Double Space", "Toggle creative flight"),
                ("Shift", "Sneak / dive / descend in flight"),
                ("Ctrl", "Sprint"),
                (
                    "Left / Right mouse",
                    "Strike enemy or break block / place block",
                ),
                ("Middle mouse", "Pick targeted block"),
                ("1–9 / Wheel", "Select hotbar slot"),
                (
                    "E / Tab",
                    "Open inventory · E / Esc closes · Tab moves focus",
                ),
                (
                    "E → Forge",
                    "Fit tool heads, bindings and grips; install modifiers",
                ),
                ("B / Q", "Building tools / quick tools"),
                (
                    "Z / X / C / V / G / H",
                    "Hand / brush / build / erase / area / explosive wand",
                ),
                ("Alt", "Placement preview / build in air"),
                ("Alt + Wheel", "Tool size"),
                ("Ctrl + Wheel / [ ]", "Building reach"),
                ("Esc", "Close panel / pause / resume"),
                ("F1 / F3 / F11", "HUD / diagnostics / fullscreen"),
            ] {
                ui.label(RichText::new(key).color(JADE));
                ui.label(action);
                ui.end_row();
            }
        });
}

pub(super) fn hud_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(INK)
        .stroke(egui::Stroke::new(1.0, EDGE))
        .corner_radius(0)
        .inner_margin(egui::Margin::symmetric(10, 7))
}
