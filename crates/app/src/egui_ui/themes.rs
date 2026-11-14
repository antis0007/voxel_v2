//! Panel palettes only. Sprite colours, rainbow animation and shaders stay authored.
use egui::{Color32 as C, Context, Id};

const KEY: &str = "ui-panel-theme";
const APPLIED: &str = "ui-panel-theme-applied";
const BASE: &str = "ui-panel-theme-original-visuals";
pub(crate) const DEFAULT: u32 = 21; // Steel Amber; palette IDs remain stable.

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub name: &'static str,
    pub family: &'static str,
    pub panel: C,
    pub recess: C,
    pub edge: C,
    pub accent: C,
}

impl Palette {
    pub(crate) fn text(self) -> C { blend(rgb(0xeeeade), self.accent, 0.16) }
    pub(crate) fn muted(self) -> C { blend(self.edge, self.text(), 0.56) }
    /// Opaque state fills keep the selected palette on every painted control.
    pub(crate) fn selection(self) -> C { blend(self.recess, self.accent, 0.18) }
    pub(crate) fn hover(self) -> C { blend(self.panel, self.accent, 0.10) }
    pub(super) fn control(self, state: super::motion::Feedback) -> C {
        let base = blend(self.recess, self.selection(), state.selection);
        let hovered = blend(base, self.hover(), state.hover * (1.0 - state.press * 0.5));
        blend(hovered, self.recess, state.press * 0.55)
    }
    pub(super) fn control_edge(self, state: super::motion::Feedback) -> C {
        blend(self.edge, self.accent, state.hover.max(state.selection))
    }
}

pub(super) fn blend(a: C, b: C, amount: f32) -> C {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    C::from_rgb(channel(a.r(), b.r()), channel(a.g(), b.g()), channel(a.b(), b.b()))
}

const fn rgb(hex: u32) -> C {
    C::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
const fn palette(
    name: &'static str,
    family: &'static str,
    panel: u32,
    recess: u32,
    edge: u32,
    accent: u32,
) -> Palette {
    Palette {
        name,
        family,
        panel: rgb(panel),
        recess: rgb(recess),
        edge: rgb(edge),
        accent: rgb(accent),
    }
}

// Stable IDs: append new palettes so saved choices keep their meaning.
pub(crate) const PALETTES: &[Palette] = &[
    palette(
        "Original", "Original", 0x29221f, 0x14110e, 0x654728, 0xdaad60,
    ),
    palette("Stone", "Minecraft", 0x484848, 0x252525, 0xa7a7a7, 0xe2e2e2),
    palette("Slate", "Minecraft", 0x303338, 0x171a1e, 0x7e8790, 0xcbd4de),
    palette("Oak", "Minecraft", 0x443526, 0x231b14, 0x9e8055, 0xe1bc76),
    palette(
        "Deepslate",
        "Minecraft",
        0x292b31,
        0x121319,
        0x676979,
        0xb8a9dc,
    ),
    palette("Parchment", "Noita", 0x453c2d, 0x231d15, 0x9d8759, 0xe5ca84),
    palette("Coal", "Noita", 0x272421, 0x100f0e, 0x78674e, 0xdab477),
    palette("Moss", "Noita", 0x2c3528, 0x151c13, 0x72825a, 0xc0d18b),
    palette("Wand", "Noita", 0x332a3f, 0x1a1223, 0x8d6aa4, 0xd8aeea),
    palette("Ship", "FTL", 0x26333d, 0x101b23, 0x6e8c9d, 0x95d9df),
    palette("Federation", "FTL", 0x253632, 0x0f1d19, 0x58897b, 0x85d6ae),
    palette("Rebel", "FTL", 0x3e2b25, 0x21130f, 0xa46c4f, 0xf0ae73),
    palette("Ion", "FTL", 0x242d45, 0x101629, 0x6e83b0, 0xb4c5ff),
    palette("iDroid", "MGSV", 0x283630, 0x111d17, 0x5f8d73, 0xa0d7ac),
    palette("Olive", "MGSV", 0x32352a, 0x181c12, 0x7c8260, 0xc4cd93),
    palette("Amber", "MGSV", 0x383027, 0x1c160f, 0x998051, 0xe7bd6c),
    palette("Tactical", "MGSV", 0x262e31, 0x11181c, 0x687d80, 0xb1d5cf),
    palette(
        "Obsidian Prism",
        "Prism",
        0x20292b,
        0x101719,
        0x52645f,
        0xb9d8c0,
    ),
    palette(
        "Midnight Aurora",
        "Prism",
        0x192830,
        0x0d171e,
        0x527d88,
        0x82d8c4,
    ),
    palette(
        "Amethyst Glass",
        "Prism",
        0x2b2238,
        0x171120,
        0x7e6099,
        0xc5a3e8,
    ),
    palette(
        "Smoked Brass",
        "Prism",
        0x302925,
        0x191512,
        0x877052,
        0xc6a574,
    ),
    palette(
        "Steel Amber",
        "Prism",
        0x1b2430,
        0x080c12,
        0x4a6b8a,
        0xffb86b,
    ),
];

pub(crate) fn bounded(index: u32) -> u32 {
    if (index as usize) < PALETTES.len() {
        index
    } else {
        DEFAULT
    }
}
pub(crate) fn get(ctx: &Context) -> u32 {
    bounded(ctx.data(|d| d.get_temp(Id::new(KEY))).unwrap_or(DEFAULT))
}
pub(crate) fn current(ctx: &Context) -> Palette {
    PALETTES[get(ctx) as usize]
}
pub(crate) fn set(ctx: &Context, index: u32) {
    ctx.data_mut(|d| d.insert_temp(Id::new(KEY), bounded(index)));
    ctx.request_repaint();
}

pub(super) fn apply(ctx: &Context) {
    let index = get(ctx);
    let reduced = super::magic_fx::reduced(ctx);
    if ctx.data(|d| d.get_temp::<(u32, bool)>(Id::new(APPLIED))) == Some((index, reduced)) {
        return;
    }
    let mut style = (*ctx.global_style()).clone();
    style.animation_time = if reduced { 0.0 } else { 0.10 };
    style.visuals = ctx
        .data(|d| d.get_temp::<egui::Visuals>(Id::new(BASE)))
        .unwrap_or_else(|| {
            let original = style.visuals.clone();
            ctx.data_mut(|d| d.insert_temp(Id::new(BASE), original.clone()));
            original
        });
    if index != 0 {
        let p = current(ctx);
        let v = &mut style.visuals;
        v.window_fill = p.panel;
        v.panel_fill = p.panel;
        v.extreme_bg_color = p.recess;
        v.faint_bg_color = p.recess;
        v.window_stroke = egui::Stroke::new(1.0, p.edge);
        v.selection.bg_fill = p.selection();
        v.selection.stroke = egui::Stroke::new(1.0, p.accent);
        v.override_text_color = Some(p.text());
        v.widgets.noninteractive.bg_fill = p.panel;
        v.widgets.noninteractive.weak_bg_fill = p.panel;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.edge);
        for (widget, fill, edge) in [
            (&mut v.widgets.inactive, p.recess, p.edge),
            (
                &mut v.widgets.hovered,
                p.hover(),
                p.accent,
            ),
            (&mut v.widgets.active, p.selection(), p.accent),
            (&mut v.widgets.open, p.panel, p.accent),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = egui::Stroke::new(1.0, edge);
        }
        if super::prism::active(ctx) {
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, p.accent);
            v.widgets.active.bg_stroke = egui::Stroke::new(1.5, p.accent);
            v.window_shadow = egui::epaint::Shadow {
                offset: [3, 4], blur: 0, spread: 0,
                color: C::from_rgba_unmultiplied(p.recess.r(), p.recess.g(), p.recess.b(), 190),
            };
            v.popup_shadow = v.window_shadow;
        }
    }
    ctx.set_global_style(style);
    ctx.data_mut(|d| d.insert_temp(Id::new(APPLIED), (index, reduced)));
}

pub(super) fn quick_switch(ui: &mut egui::Ui) {
    let mut index = get(ui.ctx());
    let before = index;
    let chooser_width = (ui.available_width() - 150.0).clamp(80.0, 140.0);
    ui.push_id("panel_theme_quick", |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .small_button("‹")
                .on_hover_text("Previous theme")
                .clicked()
            {
                index = (index + PALETTES.len() as u32 - 1) % PALETTES.len() as u32;
            }
            if ui.small_button("›").on_hover_text("Next theme").clicked() {
                index = (index + 1) % PALETTES.len() as u32;
            }
            let p = PALETTES[index as usize];
            egui::ComboBox::from_id_salt("palette")
                .selected_text(label(p))
                .width(chooser_width)
                .truncate()
                .show_ui(ui, |ui| {
                    for (i, p) in PALETTES.iter().enumerate() {
                        ui.selectable_value(&mut index, i as u32, label(*p));
                    }
                });
        })
    });
    if index != before {
        set(ui.ctx(), index);
    }
}

pub(super) fn controls(ui: &mut egui::Ui) {
    quick_switch(ui);
    ui.collapsing("Theme Palettes", gallery);
}

fn label(p: Palette) -> String {
    if p.family == "Original" {
        p.name.into()
    } else {
        format!("{} · {}", p.family, p.name)
    }
}

pub(super) fn gallery(ui: &mut egui::Ui) {
    let selected = get(ui.ctx());
    for family in ["Original", "Minecraft", "Noita", "FTL", "MGSV", "Prism"] {
        ui.label(family);
        let columns = if ui.available_width() >= 340.0 { 2 } else { 1 };
        let width =
            ((ui.available_width() - 6.0 * (columns - 1) as f32) / columns as f32).min(250.0);
        egui::Grid::new(("theme_swatches", family))
            .spacing([6.0, 6.0])
            .show(ui, |ui| {
                for (column, (i, p)) in PALETTES
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.family == family)
                    .enumerate()
                {
                    let (r, response) =
                        ui.allocate_exact_size(egui::vec2(width, 46.0), egui::Sense::click());
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, p.name)
                    });
                    ui.painter().rect_filled(r, 0, p.panel);
                    ui.painter().rect_stroke(
                        r,
                        0,
                        egui::Stroke::new(
                            if selected == i as u32 { 2.0 } else { 1.0 },
                            if selected == i as u32 || response.hovered() {
                                p.accent
                            } else {
                                p.edge
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let slot = egui::Rect::from_min_size(
                        r.min + egui::vec2(6.0, 8.0),
                        egui::vec2(30.0, 30.0),
                    );
                    ui.painter().rect_filled(slot, 0, p.recess);
                    super::pixel_art::rainbow_selection(ui.painter(), slot, ui.input(|i| i.time));
                    super::inventory::stack_count(ui.painter(), slot, 1.0, 4096);
                    ui.painter().text(
                        r.left_center() + egui::vec2(43.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        p.name,
                        egui::FontId::proportional(12.0),
                        super::presentation::PAPER,
                    );
                    if response.clicked() {
                        set(ui.ctx(), i as u32);
                    }
                    if (column + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(4.0);
    }
    if ui.small_button("Default").clicked() {
        set(ui.ctx(), DEFAULT);
    }
}
