//! Integer logical pixels; ornament stays outside the quiet icon cavities.
use egui::{Color32 as C, Painter, Rect, pos2, vec2};

pub const RECESS: C = C::from_rgb(0x12, 0x15, 0x19);
pub const LEATHER: C = C::from_rgb(0x29, 0x22, 0x1f);
pub const GOLD_DARK: C = C::from_rgb(0x65, 0x47, 0x28);
pub const GOLD: C = C::from_rgb(0xb5, 0x8a, 0x42);
pub const GOLD_LIGHT: C = C::from_rgb(0xf1, 0xd6, 0x8a);
pub const SILVER_DARK: C = C::from_rgb(0x4b, 0x56, 0x63);
pub const SILVER: C = C::from_rgb(0xaa, 0xb8, 0xc5);
pub const SILVER_LIGHT: C = C::from_rgb(0xea, 0xf0, 0xeb);
pub const SLOT: f32 = 18.0;
pub const GAP: f32 = 2.0;
pub const ICON: f32 = 14.0;

/// Move the paint only. Hit targets, drag coordinates and layout stay fixed.
pub fn react(response: &egui::Response, r: Rect, selected: bool) -> Rect {
    let ctx = &response.ctx;
    if super::magic_fx::reduced(ctx) {
        return snap(r);
    }
    let down = response.is_pointer_button_down_on();
    let now = ctx.input(|i| i.time);
    let state = (selected as u8) | ((down as u8) << 1);
    let id = response.id.with("pixel-press");
    let since = ctx.data_mut(|d| {
        let (previous, time) = d.get_temp::<(u8, f64)>(id).unwrap_or((state, -10.0));
        let changed = (selected && previous & 1 == 0) || (!down && previous & 2 != 0);
        let since = if changed { now } else { time };
        d.insert_temp(id, (state, since));
        since
    });
    let recoil = now - since < 0.12;
    if recoil {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    let y = if down {
        1.0
    } else if response.hovered() || response.has_focus() || recoil {
        -1.0
    } else {
        0.0
    };
    snap(r).translate(vec2(0.0, y))
}

pub fn review_page() -> Option<u32> {
    static PAGE: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *PAGE.get_or_init(
        || match std::env::var("VOXEL_GRIMOIRE_REVIEW").ok().as_deref() {
            Some("inventory") => Some(2),
            Some("wands") => Some(1),
            _ => None,
        },
    )
}

/// Opt-in native review input, confined to the component sheet's sample tile.
pub fn review_input(ctx: &egui::Context, input: &mut egui::RawInput) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        std::env::var("VOXEL_GRIMOIRE_MOTION_REVIEW").is_ok_and(|v| v == "1")
            && std::env::var("VOXEL_GRIMOIRE_REVIEW").is_ok_and(|v| v == "components")
    }) {
        return;
    }
    let Some(rect) = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("grimoire-review-tile"))) else {
        return;
    };
    let t = input.time.unwrap_or_else(|| ctx.input(|i| i.time)) % 4.0;
    let phase = if t < 1.0 {
        0_u8
    } else if t < 2.0 {
        1
    } else if t < 2.5 {
        2
    } else if t < 3.0 {
        3
    } else {
        0
    };
    let id = egui::Id::new("grimoire-review-phase");
    let previous = ctx.data_mut(|d| {
        let old = d.get_temp::<u8>(id).unwrap_or(0);
        d.insert_temp(id, phase);
        old
    });
    let pos = if phase == 0 {
        rect.right_top() + vec2(12.0, -10.0)
    } else {
        rect.center()
    };
    input.events.push(egui::Event::PointerMoved(pos));
    if (phase == 2) != (previous == 2) {
        input.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: phase == 2,
            modifiers: Default::default(),
        });
    }
    if phase != previous {
        eprintln!("[ui-motion-review] phase={phase}");
    }
}

pub fn state_sheet(ctx: &egui::Context) {
    static MAGIC_REVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *MAGIC_REVIEW
        .get_or_init(|| std::env::var("VOXEL_GRIMOIRE_REVIEW").is_ok_and(|v| v == "magic"))
    {
        egui::Window::new("Arcane materials")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(340.0)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.spacing_mut().interact_size.y = 16.0;
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 60.0).max(150.0))
                    .show(ui, |ui| super::magic_fx::motion_setting(ui));
            });
        return;
    }
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED
        .get_or_init(|| std::env::var("VOXEL_GRIMOIRE_REVIEW").is_ok_and(|v| v == "components"))
    {
        return;
    }
    egui::Window::new("Grimoire component rules")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label("18px cells / 14px icons / 2px gaps");
            ui.label("Gold: selected / silver: hover or keyboard focus");
            super::magic_fx::motion_setting(ui);
            ui.horizontal(|ui| {
                for (index, (label, effect)) in [
                    ("Enchanted", super::magic_fx::Effect::Enchanted),
                    ("Unstable", super::magic_fx::Effect::Unstable),
                    ("Curse sample", super::magic_fx::Effect::Cursed),
                ]
                .into_iter()
                .enumerate()
                {
                    ui.vertical(|ui| {
                        let (r, response) =
                            ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::click());
                        if index == 0 {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(egui::Id::new("grimoire-review-tile"), r)
                            });
                        }
                        let selected_id = response.id.with("review-selected");
                        let mut selected = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(selected_id).unwrap_or(false));
                        if response.clicked() {
                            selected = !selected;
                            ui.ctx().data_mut(|d| d.insert_temp(selected_id, selected));
                        }
                        let r = react(&response, r, selected);
                        slot(ui.painter(), r, selected);
                        let icon = match index {
                            0 => super::icons::Icon::Wand(4),
                            1 => super::icons::Icon::Spell(46),
                            _ => super::icons::Icon::Sword(3),
                        };
                        super::icons::paint_with_effect(ui.painter(), r.shrink(4.0), icon, effect);
                        if response.hovered() || response.has_focus() {
                            focus(ui.painter(), r);
                        }
                        ui.label(label);
                    });
                }
            });
            ui.horizontal(|ui| {
                for (i, label) in [
                    "Empty", "Item", "Selected", "Focus", "Disabled", "Recharge", "Low MP",
                ]
                .iter()
                .enumerate()
                {
                    ui.vertical(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(SLOT, SLOT), egui::Sense::hover());
                        slot(ui.painter(), r, i == 2);
                        if i > 0 {
                            super::icons::paint(
                                ui.painter(),
                                r.shrink(2.0),
                                super::icons::Icon::Spell(0),
                            );
                        }
                        if i == 3 {
                            focus(ui.painter(), r);
                        }
                        if i == 4 {
                            ui.painter()
                                .rect_filled(r.shrink(2.0), 0, C::from_black_alpha(170));
                        }
                        if i == 5 {
                            ui.painter().rect_filled(
                                Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(14.0, 8.0)),
                                0,
                                C::from_black_alpha(170),
                            );
                        }
                        if i == 6 {
                            ui.painter().rect_filled(
                                Rect::from_min_size(r.min + vec2(2.0, 13.0), vec2(14.0, 3.0)),
                                0,
                                super::presentation::DANGER,
                            );
                        }
                        ui.label(*label);
                    });
                }
            });
            ui.label("Hover: lift 1px / press: sink 1px / release: 120ms recoil");
            ui.label("Curse sample only: no cursed-item state exists yet.");
        });
}

fn pixel(p: &Painter, r: Rect, x: f32, y: f32, w: f32, h: f32, c: C) {
    p.rect_filled(Rect::from_min_size(r.min + vec2(x, y), vec2(w, h)), 0, c);
}

pub fn snap(r: Rect) -> Rect {
    Rect::from_min_max(
        pos2(r.left().round(), r.top().round()),
        pos2(r.right().round(), r.bottom().round()),
    )
}

pub fn slot(p: &Painter, r: Rect, selected: bool) {
    let r = snap(r);
    let (w, h) = (r.width(), r.height());
    p.rect_filled(r, 0, RECESS);
    let metal = if selected { GOLD } else { SILVER_DARK };
    pixel(p, r, 1.0, 0.0, w - 2.0, 1.0, metal);
    pixel(p, r, 0.0, 1.0, 1.0, h - 2.0, metal);
    pixel(p, r, w - 1.0, 1.0, 1.0, h - 2.0, GOLD_DARK);
    pixel(p, r, 1.0, h - 1.0, w - 2.0, 1.0, GOLD_DARK);
    if selected {
        // Two corner teeth and a lower notch distinguish selection in grayscale.
        for x in [1.0, w - 4.0] {
            pixel(p, r, x, 1.0, 3.0, 1.0, GOLD_LIGHT);
        }
        pixel(p, r, (w / 2.0).floor() - 1.0, h - 2.0, 3.0, 2.0, GOLD_LIGHT);
    }
}

pub fn focus(p: &Painter, r: Rect) {
    let r = snap(r).shrink(2.0);
    for (x, y, dx, dy) in [
        (r.left(), r.top(), 1.0, 1.0),
        (r.right() - 1.0, r.top(), -1.0, 1.0),
        (r.left(), r.bottom() - 1.0, 1.0, -1.0),
        (r.right() - 1.0, r.bottom() - 1.0, -1.0, -1.0),
    ] {
        for i in 0..3 {
            p.rect_filled(
                Rect::from_min_size(pos2(x + i as f32 * dx, y), vec2(1.0, 1.0)),
                0,
                SILVER_LIGHT,
            );
            p.rect_filled(
                Rect::from_min_size(pos2(x, y + i as f32 * dy), vec2(1.0, 1.0)),
                0,
                SILVER_LIGHT,
            );
        }
    }
}

/// Cover rails, stitched leather, stepped metal corner plates and silver rivets.
/// Use a background painter so fittings never cover interactive content.
pub fn binding(p: &Painter, r: Rect, book: bool) {
    let r = snap(r);
    let (w, h) = (r.width(), r.height());
    pixel(p, r, 4.0, 1.0, w - 8.0, 1.0, GOLD_DARK);
    pixel(p, r, 4.0, h - 2.0, w - 8.0, 1.0, GOLD_DARK);
    for i in 0..((w - 20.0) / 6.0).max(0.0) as i32 {
        let x = 10.0 + i as f32 * 6.0;
        pixel(p, r, x, 3.0, 2.0, 1.0, GOLD_DARK);
        pixel(p, r, x, h - 4.0, 2.0, 1.0, GOLD_DARK);
    }
    for (x, y, dx, dy) in [
        (0.0, 0.0, 1.0, 1.0),
        (w - 1.0, 0.0, -1.0, 1.0),
        (0.0, h - 1.0, 1.0, -1.0),
        (w - 1.0, h - 1.0, -1.0, -1.0),
    ] {
        for k in 0..6 {
            let k = k as f32;
            pixel(p, r, x + dx * k, y, 1.0, 1.0, GOLD);
            pixel(p, r, x, y + dy * k, 1.0, 1.0, GOLD);
        }
        pixel(p, r, x + dx, y + dy, 1.0, 1.0, GOLD_LIGHT);
        pixel(p, r, x + dx * 2.0, y + dy * 2.0, 1.0, 1.0, SILVER_LIGHT);
        pixel(p, r, x + dx * 3.0, y + dy * 3.0, 1.0, 1.0, SILVER_DARK);
    }
    if book && w > 100.0 && h > 80.0 {
        pixel(p, r, 5.0, 8.0, 2.0, h - 16.0, LEATHER);
        for y in [12.0, h - 17.0] {
            pixel(p, r, 3.0, y, 6.0, 3.0, GOLD_DARK);
            pixel(p, r, 3.0, y, 6.0, 1.0, GOLD);
        }
        pixel(
            p,
            r,
            w - 5.0,
            (h / 2.0).floor() - 5.0,
            5.0,
            10.0,
            SILVER_DARK,
        );
        pixel(p, r, w - 4.0, (h / 2.0).floor() - 4.0, 4.0, 1.0, SILVER);
        pixel(
            p,
            r,
            w - 3.0,
            (h / 2.0).floor() - 1.0,
            2.0,
            2.0,
            SILVER_LIGHT,
        );
    }
}
