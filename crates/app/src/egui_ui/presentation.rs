//! Shared game styling and control reference, separate from renderer diagnostics.
use egui::{Color32, RichText};

pub(super) const INK: Color32 = super::grimoire::RECESS;
pub(super) const PAPER: Color32 = Color32::from_rgb(0xe6, 0xd8, 0xb8);
pub(super) const MUTED: Color32 = Color32::from_rgb(173, 157, 139);
pub(super) const EDGE: Color32 = super::grimoire::SILVER_DARK;
pub(super) const JADE: Color32 = Color32::from_rgb(211, 183, 125);
pub(super) const AMBER: Color32 = super::grimoire::GOLD;
pub(super) const DANGER: Color32 = Color32::from_rgb(225, 105, 91);
pub(super) const MANA: Color32 = Color32::from_rgb(0x70, 0xc9, 0xcf);

pub(super) fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new()
        .fill(INK)
        .stroke(egui::Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .corner_radius(0)
        .show(ui, |ui| {
            ui.label(RichText::new(key).monospace().size(11.0).color(PAPER));
        });
}

pub(super) fn apply_theme(ctx: &egui::Context) {
    ctx.tessellation_options_mut(|o| {
        o.feathering = false;
        o.round_text_to_pixels = true;
        o.round_line_segments_to_pixels = true;
        o.round_rects_to_pixels = true;
    });
    // Tiny5 for interface text, PixelCode for figures and keys. Both are embedded
    // so a build does not depend on the launch directory. end_frame sets the
    // font atlas to nearest sampling; begin_frame resolves integer scaling.
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "tiny5".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../../assets/fonts/Tiny5.ttf"
        ))),
    );
    fonts.font_data.insert(
        "pixel-code".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../../assets/fonts/PixelCode.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "tiny5".into());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "pixel-code".into());
    ctx.set_fonts(fonts);
    // HUD panels are dark even on a light Windows desktop. Pin the preference
    // so the first native theme event cannot replace their readable text style.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style()).clone();
    // State changes are crisp; slot movement uses the explicit integer-pixel recoil.
    style.animation_time = 0.0;
    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = INK;
    style.visuals.panel_fill = INK;
    style.visuals.extreme_bg_color = Color32::from_rgb(17, 15, 20);
    style.visuals.override_text_color = Some(PAPER);
    style.visuals.selection.bg_fill = Color32::from_rgb(93, 73, 48);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, JADE);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(55, 46, 43);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(83, 67, 51);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(100, 78, 52);
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
    // Solid scrollbars and a bordered tooltip/popup frame, as in Minecraft's
    // GUI, so lists read as lists and hover text reads as a box.
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.visuals.window_stroke = egui::Stroke::new(2.0, EDGE);
    // Tiny5 is drawn on a 10px grid; body text sits at 13 so it stays legible
    // at the default UI scale. PixelCode stays on the monospace family for
    // counts and keycaps, at its own 12px grid.
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
    for (kind, size) in [
        (egui::TextStyle::Body, 13.0),
        (egui::TextStyle::Button, 13.0),
        (egui::TextStyle::Small, 11.5),
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
                ui.label(RichText::new(key).color(Color32::from_rgb(186, 222, 171)));
                ui.label(action);
                ui.end_row();
            }
        });
}

pub(super) fn hud_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(INK)
        .stroke(egui::Stroke::new(2.0, EDGE))
        .corner_radius(0)
        .inner_margin(egui::Margin::symmetric(10, 7))
}
