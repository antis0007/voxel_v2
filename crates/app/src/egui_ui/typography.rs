//! Live, saved font roles. Font atlases rebuild only after a font choice changes.
use egui::{Context, FontData, FontDefinitions, FontFamily, Id, RichText};
use flate2::read::GzDecoder;
use std::io::Read;
use std::sync::{Arc, LazyLock};

pub const WORDS: usize = 6;
const KEY: &str = "ui-typography";
const APPLIED: &str = "ui-typography-applied";
const REFRESH: &str = "ui-font-texture-refresh";
const PENDING: &str = "ui-texture-uploads-pending";
const NAMES: [&str; 4] = ["Monocraft", "Tiny5", "PixelCode", "Smooth"];

static EMBEDDED_FONTS: LazyLock<[Arc<FontData>; 3]> = LazyLock::new(|| {
    [
        include_bytes!("../../../../assets/fonts/Monocraft.ttf.gz").as_slice(),
        include_bytes!("../../../../assets/fonts/Tiny5.ttf.gz").as_slice(),
        include_bytes!("../../../../assets/fonts/PixelCode.ttf.gz").as_slice(),
    ]
    .map(|packed| {
        let mut bytes = Vec::new();
        GzDecoder::new(packed)
            .read_to_end(&mut bytes)
            .expect("embedded UI font");
        Arc::new(FontData::from_owned(bytes))
    })
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    body: u32,
    heading: u32,
    numbers: u32,
    pixel: bool,
    weight: u32,
    snap: bool,
    inspector: u32,
    inspector_scale: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { body: 0, heading: 0, numbers: 0, pixel: true, weight: 50, snap: true,
            inspector: 0, inspector_scale: 100 }
    }
}

impl Settings {
    pub fn words(self) -> [u32; WORDS] {
        // Reuse unused high bits so existing preference offsets remain stable.
        [self.body | (self.inspector << 8), self.heading, self.numbers,
            self.pixel as u32, self.weight | (self.inspector_scale << 8), self.snap as u32]
    }

    pub fn decode(words: &[u32]) -> Self {
        if words.len() != WORDS { return Self::default(); }
        let scale = (words[4] >> 8) & 255;
        Self { body: (words[0] & 255).min(3), heading: words[1].min(3), numbers: words[2].min(3),
            pixel: words[3] != 0, weight: (words[4] & 255).clamp(35, 65), snap: words[5] != 0,
            inspector: ((words[0] >> 8) & 255).min(3),
            inspector_scale: if scale == 0 { 100 } else { scale.clamp(75, 200) } }
    }
}

pub(super) fn inspector_scale(ctx: &Context) -> f32 {
    get(ctx).inspector_scale as f32 / 100.0
}

pub(super) fn inspector_family() -> FontFamily {
    FontFamily::Name("Inspector".into())
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
    for (&name, data) in NAMES.iter().zip(EMBEDDED_FONTS.iter()) {
        fonts.font_data.insert(name.into(), Arc::clone(data));
    }
    let proportional = fonts.families[&FontFamily::Proportional].clone();
    let monospace = fonts.families[&FontFamily::Monospace].clone();
    // Routing the standard families also covers explicitly painted FontIds.
    for (family, choice, fallback) in [
        (FontFamily::Proportional, settings.body, proportional.clone()),
        (FontFamily::Monospace, settings.numbers, monospace.clone()),
        (FontFamily::Name("PixelAccent".into()), settings.heading, proportional.clone()),
        (inspector_family(), settings.inspector, proportional),
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
    if previous.is_none_or(|old| (old.body, old.heading, old.numbers, old.inspector)
        != (settings.body, settings.heading, settings.numbers, settings.inspector)) {
        ctx.set_fonts(definitions(settings));
    }
    if previous.is_none_or(|old| (old.pixel, old.weight) != (settings.pixel, settings.weight)) {
        ctx.data_mut(|d| d.insert_temp(Id::new(REFRESH), true));
    }
    ctx.tessellation_options_mut(|options| {
        options.round_text_to_pixels = settings.snap;
        // The atlas also contains prepared discs. Keep non-text circles on the
        // vector path while glyph coverage is adjusted.
        options.prerasterized_discs = !settings.pixel;
    });
    ctx.data_mut(|d| d.insert_temp(Id::new(APPLIED), settings));
}

type Uploads = Vec<(egui::TextureId, egui::epaint::ImageDelta)>;

/// Keeps a skipped frame's texture uploads for the next painted frame.
/// Handles created on that frame stay valid; their pixels arrive late, in order.
pub(super) fn defer_textures(ctx: &Context, delta: &mut egui::TexturesDelta) {
    ctx.data_mut(|d| {
        let pending = d.get_temp_mut_or_default::<Uploads>(Id::new(PENDING));
        pending.retain(|(id, _)| !delta.free.contains(id));
        for (id, images) in delta.set.drain() {
            if delta.free.contains(&id) {
                continue;
            }
            for image in images {
                // A whole image supersedes every earlier upload to the same texture.
                if image.pos.is_none() {
                    pending.retain(|(old, _)| *old != id);
                }
                pending.push((id, image));
            }
        }
    });
}

/// Copy only updated font pixels; sprite/image textures retain their own filters.
/// A raster-mode change republishes the unmodified source atlas once, so smooth
/// mode can recover its coverage values after a pixel-mode upload.
pub(super) fn prepare_textures(ctx: &Context, delta: &mut egui::TexturesDelta) {
    if let Some(pending) = ctx.data_mut(|d| d.remove_temp::<Uploads>(Id::new(PENDING))) {
        let mut combined = egui::TexturesDelta::default();
        for (id, image) in pending {
            combined.push(id, image);
        }
        combined.append(std::mem::take(delta));
        *delta = combined;
    }
    let settings = ctx.data(|d| d.get_temp::<Settings>(Id::new(APPLIED))).unwrap_or_default();
    let refresh = ctx.data_mut(|d| d.remove_temp::<bool>(Id::new(REFRESH))).unwrap_or(false);
    let font_id = egui::TextureId::default();
    if refresh {
        let image = ctx.fonts(|fonts| fonts.image());
        delta.push(font_id, egui::epaint::ImageDelta::full(image, egui::TextureOptions::LINEAR));
    }
    for image in delta.set.get_mut(&font_id).into_iter().flatten() {
        image.options = if settings.pixel { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
        if settings.pixel {
            let egui::ImageData::Color(pixels) = &mut image.image;
            // Preserve glyph coverage at fractional DPI. A binary cutoff erased
            // thin strokes and counters, especially in small inspection text.
            let gamma = 50.0 / settings.weight as f32;
            for pixel in &mut Arc::make_mut(pixels).pixels {
                let coverage = (pixel.a() as f32 / 255.0).powf(gamma);
                *pixel = egui::Color32::from_white_alpha((coverage * 255.0).round() as u8);
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
                ("Headings", &mut settings.heading), ("Numbers", &mut settings.numbers),
                ("Block Tooltip", &mut settings.inspector)] {
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
        ui.add(egui::Slider::new(&mut settings.inspector_scale, 75..=200)
            .step_by(5.0).suffix("%").text("Block Tooltip Scale"));
        ui.add(egui::Slider::new(scale, crate::preferences::UI_SCALE_MIN..=crate::preferences::UI_SCALE_MAX)
            .step_by(0.05).text("UI Scale"));
        ui.horizontal(|ui| {
            for (label, value) in [("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("175%", 1.75), ("200%", 2.0)] {
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
        ui.label(RichText::new("Stone · 24°C").family(inspector_family())
            .size(16.0 * settings.inspector_scale as f32 / 100.0));
        if settings != before { set(ui.ctx(), settings); }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::TextureId;

    fn upload(pos: Option<[usize; 2]>) -> egui::epaint::ImageDelta {
        let pixels = egui::ColorImage::filled([2, 2], egui::Color32::RED);
        let mut delta = egui::epaint::ImageDelta::full(pixels, egui::TextureOptions::NEAREST);
        delta.pos = pos;
        delta
    }

    fn order(delta: &egui::TexturesDelta, id: TextureId) -> Vec<Option<[usize; 2]>> {
        delta.set.get(&id).into_iter().flatten().map(|image| image.pos).collect()
    }

    fn defer(ctx: &Context, uploads: Uploads, free: &[TextureId]) {
        let mut delta = egui::TexturesDelta::default();
        for (id, image) in uploads { delta.push(id, image); }
        delta.free.extend(free.iter().copied());
        defer_textures(ctx, &mut delta);
        assert!(delta.set.is_empty(), "deferred uploads leave the skipped frame");
    }

    #[test]
    fn egui_skipped_frame_texture_uploads_reach_next_painted_frame() {
        let ctx = Context::default();
        let (a, b, gone) = (TextureId::Managed(7), TextureId::Managed(8), TextureId::Managed(9));
        defer(&ctx, vec![(a, upload(None)), (gone, upload(None)), (a, upload(Some([0, 0])))], &[]);
        defer(&ctx, vec![(b, upload(None))], &[gone]);
        let mut delta = egui::TexturesDelta::default();
        delta.push(a, upload(Some([1, 1])));
        prepare_textures(&ctx, &mut delta);
        assert_eq!(order(&delta, a), vec![None, Some([0, 0]), Some([1, 1])]);
        assert_eq!(order(&delta, b), vec![None]);
        assert_eq!(delta.set.len(), 2);
        assert!(!delta.set.contains_key(&gone));
        let mut next = egui::TexturesDelta::default();
        prepare_textures(&ctx, &mut next);
        assert!(next.set.is_empty(), "pending uploads apply once");
    }

    #[test]
    fn egui_deferred_whole_image_replaces_earlier_uploads() {
        let ctx = Context::default();
        let a = TextureId::Managed(7);
        defer(&ctx, vec![(a, upload(None)), (a, upload(Some([0, 1])))], &[]);
        defer(&ctx, vec![(a, upload(None))], &[]);
        let mut delta = egui::TexturesDelta::default();
        prepare_textures(&ctx, &mut delta);
        assert_eq!(order(&delta, a), vec![None]);
        assert_eq!(delta.set.len(), 1);
    }
}
