//! Plain-data habitat inspection and local production controls.
use super::{
    UiAction,
    presentation::{AMBER, JADE, MANA, MUTED, PAPER},
};
use crate::world::{
    factory,
    habitats::{Action, BEACON_RANGE, Beacon, Family, HARVEST_DAMAGE, HARVEST_MANA, MEND_HEALTH, MEND_MANA, View},
};
use egui::RichText;

fn button(ui: &mut egui::Ui, action: &mut UiAction, text: &str, enabled: bool, command: Action) {
    if ui.add_enabled(enabled, egui::Button::new(text)).clicked() {
        *action = UiAction::Factory(factory::Action::Habitat(command));
    }
}

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    ui.label(RichText::new("HABITATS").color(AMBER).size(13.0));
    if view.mastered != 0 {
        ui.label(RichText::new("Matron breeding unlocked").color(JADE));
    }
    ui.label(
        RichText::new(format!(
            "XP stored: {}  ·  loose: {}",
            view.xp, view.loose_xp
        ))
        .color(JADE),
    );
    ui.small("Crates: 2.5 m · Mana Font: 6 m at 1 kJ/mana · Mana Flower: 8 m");
    if view.lantern {
        ui.label(RichText::new("Witness Lantern · collects loose essence within 6 m").color(MANA));
    } else {
        button(
            ui,
            action,
            "Recover Witness Lantern",
            view.lantern_near,
            Action::ClaimLantern,
        );
    }
    button(
        ui,
        action,
        "Imprint moss binding · 64 XP · melee hits restore health",
        view.xp >= 64,
        Action::ImprintMoss,
    );
    if view.sources.is_empty() {
        ui.label("Find habitat sources in Rootlit Hollow.");
    }
    egui::ScrollArea::vertical().id_salt("habitat_list").show(ui, |ui| {
        for (s, distance) in &view.sources {
            ui.push_id(s.id, |ui| {
                ui.separator();
                ui.label(RichText::new(format!("{} #{}", s.family.name(), s.id)).color(PAPER));
                if let Some(p) = s.pos {
                    ui.label(RichText::new(format!("{:.0} m · {:.0}, {:.0}, {:.0}  |  {}", distance, p.x,p.y,p.z,s.blocker)).color(MUTED));
                } else { ui.label("Ready to plant"); }
                let contract = match s.family {
                    Family::Nursery => "Rooted soil + water + biomass → fibre + mana",
                    Family::Spider => "Dry overhead anchor + water + biomass → silk",
                    Family::Matron => "Rooted soil + water + 4 biomass + 32 mana → Matron",
                };
                ui.small(contract);
                ui.small("Water within 2 m; 1 biomass / 20 s. Divide mature beds to plant more.");
                if s.resident.is_none() {
                    let ticks = match s.family { Family::Nursery => 120.0, Family::Spider => 160.0, Family::Matron => 720.0 };
                    ui.add(egui::ProgressBar::new(s.development as f32/ticks).text("Developing source"));
                } else { ui.label(RichText::new("Ready to harvest").color(JADE)); }
                ui.small(format!("Feed {:.1} · supplied mana {} · ready products {:.1} · harvested {:.1}",
                    s.feed as f32/64.0,s.charge,s.products as f32/64.0,s.harvested as f32/64.0));
                ui.label(RichText::new(format!("Receiver: {} mana · {} XP",s.mana,s.xp)).color(MANA));
                ui.small(format!("Lifetime: {} births · {} deaths · {:.1} biomass used · {} mana made / {} used · {} XP made",
                    s.births,s.deaths,s.feed_used as f32/64.0,s.mana_made,s.mana_used,s.xp_made));
                let near = *distance <= 7.0;
                ui.horizontal_wrapped(|ui| {
                    button(ui,action,"Supply 4 biomass",near,Action::Feed(s.id));
                    button(ui,action,"Supply 32 mana",near,Action::Charge(s.id));
                    button(ui,action,"Harvest product",near && s.products >= 64,Action::Harvest(s.id));
                    button(ui,action,"Collect buffers",near,Action::Collect(s.id));
                });
                if s.receiver {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("Beacon · {BEACON_RANGE:.0} m"));
                        for (mode, label, detail) in [
                            (Beacon::Off, "Off", String::new()),
                            (Beacon::Mending, "Mending", format!("+{MEND_HEALTH:.0} HP/s · {MEND_MANA} mana per player")),
                            (Beacon::Harvest, "Harvest", format!("{HARVEST_DAMAGE:.0} damage/s · {HARVEST_MANA} mana per marked brood · up to 4")),
                        ] {
                            if ui.add_enabled(near, egui::Button::selectable(s.beacon == mode, label))
                                .on_hover_text(detail).clicked()
                            {
                                *action = UiAction::Factory(factory::Action::Habitat(Action::SetBeacon(s.id, mode)));
                            }
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    button(ui,action,if s.receiver { "Remove empty receiver" } else { "Build receiver · 1 circuit + 1 crystal" },near,Action::Receiver(s.id));
                    button(ui,action,if s.automatic { "Stop auto harvest" } else { "Auto harvest; preserve resident" },near && s.receiver,Action::Automatic(s.id));
                    button(ui,action,if s.emitter { "Stop Spark turret" } else { "Start Spark turret · 6 m range" },near && s.receiver,Action::Emitter(s.id));
                    button(ui,action,if s.reserve { "Brood Protected" } else { "Brood Harvestable" },near,Action::Reserve(s.id));
                    button(ui,action,"Divide viable source · 1 product + 4 biomass",near && s.products >= 64 && s.resident.is_some(),Action::Propagate(s.id));
                    if s.pos.is_none() { button(ui,action,"Place at feet",true,Action::Place(s.id)); }
                    else { button(ui,action,"Carry source and buffers",near && s.resident.is_none(),Action::Carry(s.id)); }
                    button(ui,action,"Discard empty source",near && s.resident.is_none() && s.feed == 0 && s.charge == 0 && s.products == 0 && s.harvested == 0 && s.mana == 0 && s.xp == 0 && !s.receiver,Action::Destroy(s.id));
                });
            });
        }
    });
}
