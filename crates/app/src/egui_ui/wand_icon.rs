//! Pixel-art icon for a wand item, drawn from the look bits carried on the item:
//! a diagonal shaft whose length grows with tier, a grip, and one of six heads.
use crate::world::adventure::wand_items::decode_look;
use egui::{Color32 as C, Pos2, Rect, vec2};

const GRID: i32 = 14;

fn rgb(c: [f32; 3], k: f32) -> C {
    let f = |v: f32| ((v * k).clamp(0.0, 1.0) * 255.0) as u8;
    C::from_rgb(f(c[0]), f(c[1]), f(c[2]))
}

pub(super) fn paint(p: &egui::Painter, r: Rect, look: u16) {
    let l = decode_look(look);
    let unit = (r.width().min(r.height()) / GRID as f32).floor().max(1.0);
    let origin = r.center() - vec2(unit, unit) * GRID as f32 * 0.5;
    let px = |x: i32, y: i32, c: C| {
        if (0..GRID).contains(&x) && (0..GRID).contains(&y) {
            let min = origin + vec2(x as f32, y as f32) * unit;
            p.rect_filled(Rect::from_min_size(Pos2::new(min.x, min.y), vec2(unit, unit)), 0.0, c);
        }
    };
    let outline = rgb(l.dark, 0.45);
    let len = 6 + l.tier as i32 * 2;
    // Shaft from the lower-left corner toward the head, one pixel wide with a rim.
    for i in 0..len {
        let (x, y) = (1 + i, GRID - 2 - i);
        px(x - 1, y, outline);
        px(x, y + 1, outline);
        let grip = i < 3;
        px(x, y, if grip { rgb(l.dark, 1.0) } else { rgb(l.shaft, if i % 3 == 0 { 0.8 } else { 1.0 }) });
    }
    let (hx, hy) = (1 + len, GRID - 2 - len);
    px(hx - 1, hy + 1, rgb(l.accent, 1.0));
    let glow = rgb(l.glow, 1.0);
    let bright = rgb(l.glow, 1.4);
    match l.head {
        // Cradled orb
        0 => {
            for (dx, dy) in [(0, 0), (1, 0), (0, -1), (1, -1)] {
                px(hx + dx, hy + dy, glow);
            }
            px(hx, hy - 1, bright);
            px(hx - 1, hy - 1, rgb(l.accent, 0.9));
            px(hx + 2, hy, rgb(l.accent, 0.9));
        }
        // Resonant crystal: a tall shard.
        1 => {
            for d in 0..4 {
                px(hx + d / 2, hy - d, if d == 3 { bright } else { glow });
            }
        }
        // Twin prisms
        2 => {
            px(hx - 1, hy - 1, glow);
            px(hx - 1, hy - 2, bright);
            px(hx + 1, hy, glow);
            px(hx + 2, hy, bright);
        }
        // Orbit ring around a spark
        3 => {
            for (dx, dy) in [(-1, -1), (0, -2), (1, -2), (2, -1), (2, 0), (1, 1), (0, 1), (-1, 0)] {
                px(hx + dx, hy + dy, rgb(l.accent, 1.0));
            }
            px(hx, hy - 1, bright);
            px(hx + 1, hy - 1, glow);
        }
        // Living flame
        4 => {
            px(hx, hy, glow);
            px(hx + 1, hy - 1, glow);
            px(hx, hy - 2, bright);
            px(hx + 1, hy - 3, rgb([1.0, 0.85, 0.5], 1.0));
        }
        // Watching eye
        _ => {
            for (dx, dy) in [(-1, 0), (0, -1), (1, -1), (2, 0), (1, 1), (0, 1)] {
                px(hx + dx, hy + dy, rgb(l.accent, 1.0));
            }
            px(hx, hy, glow);
            px(hx + 1, hy, C::BLACK);
        }
    }
    // Masterwork wands carry a second gem on the collar.
    if l.tier >= 3 {
        px(hx - 2, hy + 2, glow);
    }
}

#[derive(Clone)]
struct Cards {
    bank: Vec<crate::world::adventure::wand_items::WandRecord>,
    active: Option<u16>,
    rank: u32,
}

/// Makes the wand bank available to item tooltips drawn later this frame.
pub(super) fn publish(ctx: &egui::Context, view: &crate::world::adventure::View) {
    let cards = Cards { bank: view.wand_bank.clone(), active: view.active_wand, rank: view.rank };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("wand_cards"), cards));
}

/// Noita-style stat card for one wand item.
pub(super) fn card(ui: &mut egui::Ui, id: u16) {
    use crate::world::adventure::{spell_color, spell_name, wand_items, wands};
    use egui::RichText;
    let Some(cards) = ui.ctx().data(|d| d.get_temp::<Cards>(egui::Id::new("wand_cards"))) else {
        return;
    };
    let Some(record) = cards.bank.get(id as usize) else {
        ui.weak("Unknown wand");
        return;
    };
    let s = wand_items::stats_for(record.tier, &record.loadout, cards.rank);
    ui.separator();
    ui.horizontal(|ui| {
        ui.strong(RichText::new(s.name).size(13.0));
        if cards.active == Some(id) {
            ui.label(RichText::new("· in hand").color(C::from_rgb(140, 220, 160)).size(11.0));
        }
    });
    let row = |ui: &mut egui::Ui, k: &str, v: String| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(k).size(11.0).color(C::from_gray(150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(v).size(11.0).monospace());
            });
        });
    };
    row(ui, "Shuffle", if s.shuffle { "Yes".into() } else { "No".into() });
    row(ui, "Spells/Cast", s.spells_per_cast.to_string());
    row(ui, "Cast delay", format!("{:.2} s", s.cast_delay));
    row(ui, "Rechrg. Time", format!("{:.2} s", s.recharge_time));
    row(ui, "Mana max", format!("{:.0}", s.mana_max));
    row(ui, "Mana chg. Spd", format!("{:.1}", s.mana_charge_speed));
    row(ui, "Capacity", s.capacity.to_string());
    row(ui, "Spread", format!("{:.1} DEG", s.spread_deg));
    ui.horizontal_wrapped(|ui| {
        for i in 0..s.capacity {
            let spell = record.loadout.slot(i);
            if spell == wands::EMPTY {
                ui.label(RichText::new("·").color(C::from_gray(90)));
            } else {
                ui.label(RichText::new(spell_name(spell)).size(10.5).color(rgb(spell_color(spell), 1.0)));
            }
        }
    });
}
