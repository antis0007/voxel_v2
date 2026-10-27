//! Pixel-art icon for a wand item, drawn from the look bits carried on the item:
//! the same authored family sprite and palette as the held and dropped model.

use egui::{Color32 as C, Pos2, Rect, vec2};



fn rgb(c: [f32; 3], k: f32) -> C {
    let f = |v: f32| ((v * k).clamp(0.0, 1.0) * 255.0) as u8;
    C::from_rgb(f(c[0]), f(c[1]), f(c[2]))
}

pub(super) fn paint(p: &egui::Painter, r: Rect, look: u16) {
    // One texture per family and palette, independent of tier and save ID.
    let key=crate::mesh::models::wand_kit::icon_key(look);
    let id=egui::Id::new(("wand_kit_icon",key));
    let cached=p.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id));
    let texture=if let Some(texture)=cached { texture } else {
        let Some((size,pixels))=crate::mesh::models::wand_kit::icon_rgba(look) else { return };
        // load_texture takes the Context lock itself. Creating a first icon
        // inside data_mut recursively acquired that lock and deadlocked.
        let image=egui::ColorImage::from_rgba_unmultiplied(size,&pixels);
        let t=p.ctx().load_texture(format!("wand-{key:?}"),image,egui::TextureOptions::NEAREST);
        p.ctx().data_mut(|d| d.insert_temp(id,t.clone())); t
    };
    let [w,h]=texture.size();
    let height=r.height().min(r.width())*0.94;
    let target=Rect::from_center_size(r.center(),vec2(height*w as f32/h.max(1) as f32,height));
    p.image(texture.id(),target,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),C::WHITE);
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
