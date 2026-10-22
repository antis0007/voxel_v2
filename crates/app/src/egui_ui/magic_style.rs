//! Saved presentation profiles. These never grant enchantments or alter item stats.
use egui::{Context, Id};
use std::sync::{OnceLock, RwLock};

pub const WORDS: usize = 31;
const KEY: &str = "magic-material-profiles";

#[derive(Clone, Copy, PartialEq)]
pub struct Style {
    pub colors: [[u8; 3]; 3],
    pub strength: f32,
    pub speed: f32,
    pub width: f32,
    pub chromatic: f32,
    pub density: f32,
    pub angle: f32,
    pub sparkle: f32,
}

impl Style {
    /// Typed magic keeps its identifying hue; swatches provide a chromatic accent.
    pub fn with_source(mut self, source: egui::Color32) -> Self {
        let source = [source.r(), source.g(), source.b()];
        let accent = 0.18 + self.chromatic * 0.40;
        for color in &mut self.colors {
            for i in 0..3 {
                color[i] =
                    (source[i] as f32 * (1.0 - accent) + color[i] as f32 * accent).round() as u8;
            }
        }
        self
    }
    fn default_for(index: usize) -> Self {
        let colors = match index {
            1 => [[245, 124, 111], [205, 82, 157], [244, 193, 109]],
            2 => [[171, 118, 215], [217, 147, 191], [139, 186, 119]],
            _ => [[237, 167, 219], [171, 130, 239], [148, 226, 176]],
        };
        Self {
            colors,
            strength: 0.72,
            speed: 0.8,
            width: 1.2,
            chromatic: 0.32,
            density: 1.0,
            angle: 35.0,
            sparkle: 0.25,
        }
    }
    fn normalized(mut self, fallback: Self) -> Self {
        for (value, default, min, max) in [
            (&mut self.strength, fallback.strength, 0.0, 1.2),
            (&mut self.speed, fallback.speed, 0.15, 2.0),
            (&mut self.width, fallback.width, 0.5, 3.0),
            (&mut self.chromatic, fallback.chromatic, 0.0, 1.0),
            (&mut self.density, fallback.density, 0.5, 2.0),
            (&mut self.angle, fallback.angle, 0.0, 180.0),
            (&mut self.sparkle, fallback.sparkle, 0.0, 1.0),
        ] {
            *value = if value.is_finite() {
                value.clamp(min, max)
            } else {
                default
            };
        }
        self
    }
    pub fn shader_words(self) -> [u32; 20] {
        let mut result = [0; 20];
        for (i, color) in self.colors.iter().enumerate() {
            for j in 0..3 {
                result[i * 4 + j] = (color[j] as f32 / 255.0).to_bits();
            }
        }
        for (i, value) in [
            self.strength,
            self.speed,
            self.width,
            self.chromatic,
            self.density,
            self.angle.to_radians(),
            self.sparkle,
            0.0,
        ]
        .into_iter()
        .enumerate()
        {
            result[12 + i] = value.to_bits();
        }
        result
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Settings {
    pub profiles: [Style; 3],
    pub reduced: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            profiles: std::array::from_fn(Style::default_for),
            reduced: false,
        }
    }
}
impl Settings {
    pub fn words(self) -> [u32; WORDS] {
        let mut result = [0; WORDS];
        for (index, style) in self.profiles.iter().enumerate() {
            let start = index * 10;
            for (i, c) in style.colors.iter().enumerate() {
                result[start + i] = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
            }
            for (i, value) in [
                style.strength,
                style.speed,
                style.width,
                style.chromatic,
                style.density,
                style.angle,
                style.sparkle,
            ]
            .into_iter()
            .enumerate()
            {
                result[start + 3 + i] = value.to_bits();
            }
        }
        result[30] = self.reduced as u32;
        result
    }
    pub fn decode(words: &[u32]) -> Self {
        if words.len() != WORDS {
            return Self::default();
        }
        let mut result = Self::default();
        for (index, style) in result.profiles.iter_mut().enumerate() {
            let w = &words[index * 10..index * 10 + 10];
            let fallback = *style;
            style.colors =
                std::array::from_fn(|i| [(w[i] >> 16) as u8, (w[i] >> 8) as u8, w[i] as u8]);
            [
                style.strength,
                style.speed,
                style.width,
                style.chromatic,
                style.density,
                style.angle,
                style.sparkle,
            ] = std::array::from_fn(|i| f32::from_bits(w[i + 3]));
            *style = style.normalized(fallback);
        }
        result.reduced = words[30] != 0;
        result
    }
}

pub fn get(ctx: &Context) -> Settings {
    ctx.data(|d| d.get_temp::<Settings>(Id::new(KEY)).unwrap_or_default())
}
pub fn set(ctx: &Context, value: Settings) {
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY), value));
    if let Ok(mut world) = world_style().write() {
        *world = (value.profiles[0], value.reduced);
    }
}

fn world_style() -> &'static RwLock<(Style, bool)> {
    static WORLD: OnceLock<RwLock<(Style, bool)>> = OnceLock::new();
    WORLD.get_or_init(|| RwLock::new((Style::default_for(0), false)))
}

/// Same controls in the world lighting uniform; sRGB swatches become linear light.
pub fn world_uniform() -> [[f32; 4]; 5] {
    let (style, reduced) = world_style()
        .read()
        .map(|v| *v)
        .unwrap_or((Style::default_for(0), false));
    let bits = style.shader_words();
    let mut data =
        std::array::from_fn(|i| std::array::from_fn(|j| f32::from_bits(bits[i * 4 + j])));
    for color in &mut data[..3] {
        for channel in &mut color[..3] {
            *channel = if *channel <= 0.04045 {
                *channel / 12.92
            } else {
                ((*channel + 0.055) / 1.055).powf(2.4)
            };
        }
    }
    if reduced {
        data[3][1] = 0.0;
    }
    data
}

pub fn panel(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("Magic materials")
        .default_open(std::env::var("VOXEL_GRIMOIRE_REVIEW").is_ok_and(|v| v == "magic"))
        .show(ui, |ui| {
            let mut settings = get(ui.ctx());
            let id = ui.make_persistent_id("magic-profile-tab");
            let mut active = ui
                .ctx()
                .data(|d| d.get_temp::<usize>(id).unwrap_or(0))
                .min(2);
            ui.horizontal(|ui| {
                for (index, name) in ["Enchantment", "Instability", "Curse"]
                    .into_iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut active, index, name);
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(id, active));
            let style = &mut settings.profiles[active];
            ui.horizontal(|ui| {
                if ui.small_button("Roseglass").clicked() {
                    *style = Style::default_for(0);
                }
                if ui.small_button("Witchlight").clicked() {
                    *style = Style {
                        colors: [[154, 230, 165], [199, 142, 230], [240, 178, 211]],
                        density: 1.35,
                        sparkle: 0.45,
                        ..Style::default_for(active)
                    };
                }
                if ui.small_button("Astral").clicked() {
                    *style = Style {
                        colors: [[133, 202, 236], [188, 156, 244], [239, 197, 233]],
                        angle: 65.0,
                        chromatic: 0.55,
                        ..Style::default_for(active)
                    };
                }
                if ui.small_button("Reset").clicked() {
                    *style = Style::default_for(active);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Palette");
                for c in &mut style.colors {
                    ui.color_edit_button_srgb(c);
                }
            });
            for (label, value, range) in [
                ("Strength", &mut style.strength, 0.0..=1.2),
                ("Flow speed", &mut style.speed, 0.15..=2.0),
                ("Ribbon width", &mut style.width, 0.5..=3.0),
                ("Chromatic edge", &mut style.chromatic, 0.0..=1.0),
                ("Ribbon density", &mut style.density, 0.5..=2.0),
                ("Direction", &mut style.angle, 0.0..=180.0),
                ("Sparkle", &mut style.sparkle, 0.0..=1.0),
            ] {
                ui.add(egui::Slider::new(value, range).text(label));
            }
            set(ui.ctx(), settings);
            let effect = [
                super::magic_fx::Effect::Enchanted,
                super::magic_fx::Effect::Unstable,
                super::magic_fx::Effect::Cursed,
            ][active];
            ui.horizontal(|ui| {
                for icon in [
                    super::icons::Icon::Wand(4),
                    super::icons::Icon::Sword(3),
                    super::icons::Icon::Armor(0, 3),
                ] {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    super::grimoire::slot(ui.painter(), r, false);
                    super::icons::paint_with_effect(ui.painter(), r.shrink(3.0), icon, effect);
                }
                ui.label("Live preview");
            });
            ui.small("Enchantment flow also styles the wand; its hue follows its spell.");
            ui.small("Palette swatches style icons. Curse preview does not add an affix.");
        });
}
