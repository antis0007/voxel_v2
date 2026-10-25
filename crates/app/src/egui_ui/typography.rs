//! Live, saved font roles. Font atlases rebuild only after a font choice changes.
use egui::{Context, FontData, FontDefinitions, FontFamily, Id, RichText};
use std::sync::Arc;

pub const WORDS: usize = 6;
const KEY: &str = "ui-typography";
const APPLIED: &str = "ui-typography-applied";
const REFRESH: &str = "ui-font-texture-refresh";
const NAMES: [&str; 4] = ["Monocraft", "Tiny5", "PixelCode", "Smooth"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    body: u32,
    heading: u32,
    numbers: u32,
    pixel: bool,
    weight: u32,
    snap: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { body: 0, heading: 0, numbers: 0, pixel: true, weight: 50, snap: true }
    }
}

impl Settings {
    pub fn words(self) -> [u32; WORDS] {
        [self.body, self.heading, self.numbers, self.pixel as u32, self.weight, self.snap as u32]
    }

    pub fn decode(words: &[u32]) -> Self {
        if words.len() != WORDS { return Self::default(); }
        Self { body: words[0].min(3), heading: words[1].min(3), numbers: words[2].min(3),
            pixel: words[3] != 0, weight: words[4].clamp(35, 65), snap: words[5] != 0 }
    }
}

pub fn get(ctx: &Context) -> Settings {
    ctx.data(|d| d.get_temp(Id::new(KEY))).unwrap_or_default()
}

pub fn set(ctx: &Context, settings: Settings) {
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY), settings));
    ctx.request_repaint();
}

fn definitions(settings: Settings) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("Monocraft", include_bytes!("../../../../assets/fonts/Monocraft.ttf").as_slice()),
        ("Tiny5", include_bytes!("../../../../assets/fonts/Tiny5.ttf").as_slice()),
        ("PixelCode", include_bytes!("../../../../assets/fonts/PixelCode.ttf").as_slice()),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let proportional = fonts.families[&FontFamily::Proportional].clone();
    let monospace = fonts.families[&FontFamily::Monospace].clone();
    // Routing the standard families also covers explicitly painted FontIds.
    for (family, choice, fallback) in [
        (FontFamily::Proportional, settings.body, proportional.clone()),
        (FontFamily::Monospace, settings.numbers, monospace.clone()),
        (FontFamily::Name("PixelAccent".into()), settings.heading, proportional),
        (FontFamily::Name("PixelFigures".into()), settings.numbers, monospace),
    ] {
        let mut chain = fallback;
        if choice < 3 { chain.insert(0, NAMES[choice as usize].into()); }
        fonts.families.insert(family, chain);
    }
    fonts
}

/// Called before begin_pass so the next frame uses a coherent font atlas.
pub(super) fn apply(ctx: &Context) {
    let settings = get(ctx);
    let previous = ctx.data(|d| d.get_temp::<Settings>(Id::new(APPLIED)));
    if previous == Some(settings) { return; }
    if previous.is_none_or(|old| (old.body, old.heading, old.numbers)
        != (settings.body, settings.heading, settings.numbers)) {
        ctx.set_fonts(definitions(settings));
    }
    if previous.is_none_or(|old| (old.pixel, old.weight) != (settings.pixel, settings.weight)) {
        ctx.data_mut(|d| d.insert_temp(Id::new(REFRESH), true));
    }
    ctx.tessellation_options_mut(|options| {
        options.round_text_to_pixels = settings.snap;
        // The atlas also contains prepared discs. Keep non-text circles on the
        // vector path while glyph coverage is thresholded.
        options.prerasterized_discs = !settings.pixel;
    });
    ctx.data_mut(|d| d.insert_temp(Id::new(APPLIED), settings));
}

pub(super) fn defer_texture(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new(REFRESH), true));
}

/// Copy only updated font pixels; sprite/image textures retain their own filters.
/// A raster-mode change republishes the unmodified source atlas once, so smooth
/// mode can recover its coverage values after a pixel-mode upload.
pub(super) fn prepare_textures(ctx: &Context, delta: &mut egui::TexturesDelta) {
    let settings = ctx.data(|d| d.get_temp::<Settings>(Id::new(APPLIED))).unwrap_or_default();
    let refresh = ctx.data_mut(|d| d.remove_temp::<bool>(Id::new(REFRESH))).unwrap_or(false);
    let font_id = egui::TextureId::default();
    if refresh {
        delta.set.retain(|(id, _)| *id != font_id);
        let image = ctx.fonts(|fonts| fonts.image());
        delta.set.push((font_id, egui::epaint::ImageDelta::full(image, egui::TextureOptions::LINEAR)));
    }
    for (_, image) in delta.set.iter_mut().filter(|(id, _)| *id == font_id) {
        image.options = if settings.pixel { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
        if settings.pixel {
            let egui::ImageData::Color(pixels) = &mut image.image;
            let threshold = ((100 - settings.weight) * 255 / 100) as u8;
            for pixel in &mut Arc::make_mut(pixels).pixels {
                *pixel = if pixel.a() >= threshold { egui::Color32::WHITE } else { egui::Color32::TRANSPARENT };
            }
        }
    }
}

pub(super) fn controls(ui: &mut egui::Ui, scale: &mut f32) {
    ui.collapsing("Font", |ui| {
        let mut settings = get(ui.ctx());
        let before = settings;
        egui::Grid::new("font_roles").num_columns(2).show(ui, |ui| {
            for (label, choice) in [("Body", &mut settings.body),
                ("Headings", &mut settings.heading), ("Numbers", &mut settings.numbers)] {
                ui.label(label);
                egui::ComboBox::from_id_salt(("font_role", label))
                    .selected_text(NAMES[*choice as usize]).show_ui(ui, |ui| {
                        for (index, name) in NAMES.iter().enumerate() {
                            ui.selectable_value(choice, index as u32, *name);
                        }
                    });
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Edges");
            ui.selectable_value(&mut settings.pixel, true, "Pixel");
            ui.selectable_value(&mut settings.pixel, false, "Smooth");
        });
        ui.add_enabled(settings.pixel, egui::Slider::new(&mut settings.weight, 35..=65).suffix("%").text("Weight"));
        ui.checkbox(&mut settings.snap, "Pixel Alignment");
        ui.add(egui::Slider::new(scale, crate::preferences::UI_SCALE_MIN..=crate::preferences::UI_SCALE_MAX)
            .step_by(0.05).text("UI Scale"));
        ui.horizontal(|ui| {
            for (label, value) in [("100%", 1.0), ("150%", 1.5), ("200%", 2.0)] {
                if ui.small_button(label).clicked() { *scale = value; }
            }
            if ui.small_button("Reset").clicked() {
                settings = Settings::default();
                *scale = crate::preferences::UI_SCALE_DEFAULT;
            }
        });
        ui.separator();
        ui.label(RichText::new("Stone · Iron Ingot · Burning").size(14.0));
        ui.label(RichText::new("Inventory").family(FontFamily::Name("PixelAccent".into())).size(20.0));
        ui.monospace("64x · 1,240 / 8,000 FE · 840°C");
        if settings != before { set(ui.ctx(), settings); }
    });
}
