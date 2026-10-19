//! Shared game styling and control reference, separate from renderer diagnostics.
use egui::{Color32, RichText};

pub(super) const INK: Color32 = Color32::from_rgb(29, 25, 29);
pub(super) const PAPER: Color32 = Color32::from_rgb(244, 230, 202);
pub(super) const MUTED: Color32 = Color32::from_rgb(173, 157, 139);
pub(super) const EDGE: Color32 = Color32::from_rgb(103, 86, 75);
pub(super) const JADE: Color32 = Color32::from_rgb(211, 183, 125);
pub(super) const AMBER: Color32 = Color32::from_rgb(218, 172, 105);
pub(super) const DANGER: Color32 = Color32::from_rgb(225, 105, 91);
pub(super) const MANA: Color32 = Color32::from_rgb(116, 174, 207);

pub(super) fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new().fill(INK).stroke(egui::Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(5, 1)).corner_radius(0)
        .show(ui, |ui| { ui.label(RichText::new(key).monospace().size(11.0).color(PAPER)); });
}

pub(super) fn apply_theme(ctx: &egui::Context) {
    // Original UI illustrations + an OFL bitmap-style typeface. Embed the font
    // so installed builds do not depend on the launch working directory.
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("pixel-code".into(), std::sync::Arc::new(egui::FontData::from_static(
        include_bytes!("../../assets/fonts/PixelCode.ttf"))));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "pixel-code".into());
    }
    ctx.set_fonts(fonts);
    // HUD panels are dark even on a light Windows desktop. Pin the preference
    // so the first native theme event cannot replace their readable text style.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style()).clone();
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
    for widget in [&mut style.visuals.widgets.noninteractive, &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered, &mut style.visuals.widgets.active, &mut style.visuals.widgets.open] {
        widget.corner_radius = egui::CornerRadius::ZERO;
    }
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
    for (kind, size) in [(egui::TextStyle::Body,16.0),(egui::TextStyle::Button,16.0),
        (egui::TextStyle::Monospace,16.0),(egui::TextStyle::Small,12.0)] {
        style.text_styles.insert(kind, egui::FontId::proportional(size));
    }
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
                ("Alt + left mouse", "Bore ore or rock (requires upgraded bore)"),
                ("R / T", "Hold to cast / change spell"),
                ("Space", "Jump / swim up / ascend in flight"),
                ("Double Space", "Toggle creative flight"),
                ("Shift", "Sneak / dive / descend in flight"),
                ("Ctrl", "Sprint"),
                ("Left / Right mouse", "Strike enemy or break block / place block"),
                ("Middle mouse", "Pick targeted block"),
                ("1–9 / Wheel", "Select hotbar slot"),
                ("E / Tab", "Open inventory · E / Esc closes · Tab moves focus"),
                ("B / Q", "Building tools / quick tools"),
                ("Z / X / C / V / G / H", "Hand / brush / build / erase / area / explosive wand"),
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

