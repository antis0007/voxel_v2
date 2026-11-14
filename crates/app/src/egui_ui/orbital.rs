use super::UiAction;
use crate::world::{BlockId, factory::Kind, inventory::Item};
use crate::world::orbital::{Channel, View, receiver::Action};
use crate::world::orbital::cargo::SALVAGE_MANIFEST;
use crate::world::orbital::radio::{self, Action as RadioAction};

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    egui::ScrollArea::vertical()
        .id_salt("orbital_service_ledger")
        .max_height((ui.ctx().content_rect().height() - 100.0).max(120.0))
        .show(ui, |ui| {
            if view.navigation { super::nav_console::menu(ui, action); }
            if view.receiver.can_board {
                salvage_manifest(ui);
            }
            cargo_section(ui, view, action);
            radio_section(ui, view, action);
            pass_forecasts(ui,view);
            ui.separator();
            corridors(ui, view, action);
            ui.separator();
            receiver_section(ui, view, action);
            ephemeris(ui, view);
        });
}

pub(super) fn salvage_manifest(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("Station Salvage Manifest").default_open(true).show(ui, |ui| {
        ui.weak("Original Locker Contents");
        for (item, count) in SALVAGE_MANIFEST {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{} ×{count}", item.name()));
                let example = match item {
                    Item::Resource(15) => Some(Kind::MiningDrill.block()),
                    Item::Resource(12) => Some(Kind::Fabricator.block()),
                    Item::Resource(5) => Some(BlockId::ManaFlower),
                    _ => None,
                };
                if let Some(block) = example {
                    let build = Item::Block(block);
                    if ui.link(format!("→ {}", build.name())).clicked() {
                        super::recipe_book::request(ui.ctx(), build, false);
                    }
                } else {
                    ui.weak("Melee Equipment");
                }
            });
        }
        ui.weak("Freight Loading · Defense Robots");
    });
}

fn cargo_section(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    let v = &view.cargo;
    if v.can_collect && ui.button("Collect Cargo").clicked() {
        *action = UiAction::OrbitalReceiver(Action::CollectLoot);
    }
    if !v.at_station { return; }
    ui.heading("Station Freight");
    ui.label(v.status);
    if v.seconds > 0 { ui.label(format!("{} s", v.seconds)); }
    ui.label(format!("Security: {} / {}", v.guards, crate::world::orbital::cargo::MAX_GUARDS));
    if v.paused_seconds > 0 { ui.label(format!("Constructors: Paused · {} s", v.paused_seconds)); }
    let id = egui::Id::new("station_cargo_receiver");
    let mut receiver = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if !v.receiver_available { receiver = false; }
    egui::ComboBox::from_label("Destination")
        .selected_text(if receiver { "Ground Receiver" } else { "Elevator Base" })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut receiver, false, "Elevator Base");
            if v.receiver_available { ui.selectable_value(&mut receiver, true, "Ground Receiver"); }
        });
    ui.ctx().data_mut(|d| d.insert_temp(id, receiver));
    ui.strong("Salvage Freight");
    for stack in &v.items { ui.label(format!("{} × {}", stack.item.name(), stack.count)); }
    if ui.add_enabled(v.can_dispatch, egui::Button::new("Ship Locker Salvage")).clicked() {
        *action = UiAction::OrbitalReceiver(Action::DispatchCargo { receiver });
    }
    ui.strong("Mithril Cell Order");
    ui.horizontal_wrapped(|ui| {
        for (supply, enabled) in crate::world::orbital::cargo::Supply::ALL.into_iter().zip(v.can_order_cell) {
            let cost = supply.cost();
            if ui.add_enabled(enabled, egui::Button::new(format!("Trade {} ×{}", cost.item.name(), cost.count)))
                .on_hover_text("Mithril Cell ×1 · Ground Pickup").clicked()
            {
                *action = UiAction::OrbitalReceiver(Action::OrderCell { supply, receiver });
            }
        }
    });
    ui.weak("Ground Pickup → Bronze or Resonant Battery Bank");
    ui.separator();
}

/// Radio level, what it can do now, and the actions available right now.
fn radio_section(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    let r = &view.radio;
    ui.heading("Radio");
    ui.label(format!("Level {} · {}", r.level, radio::NAMES[r.level as usize]));
    ui.label(r.status);
    ui.horizontal_wrapped(|ui| {
        if r.level>=4 && r.knowledge&4!=0 && r.near_dish && ui.button("Track derelict relay").clicked() {
            *action=UiAction::OrbitalRadio(RadioAction::TrackRelay);
        }
        if r.level < 5 && r.level != 3 {
            let next = r.level as usize + 1;
            let label = format!("Build {} · {}", radio::NAMES[next], radio::COST_TEXT[next]);
            if ui.add_enabled(r.can_upgrade, egui::Button::new(label)).clicked() {
                *action = UiAction::OrbitalRadio(RadioAction::Upgrade);
            }
        }
        for (enabled, label, request) in [
            (r.can_listen, "Listen", RadioAction::Listen),
            (r.can_handshake, "Connect", RadioAction::Handshake),
            (r.can_build, "Build dish · 8 iron, 6 copper, 2 steel", RadioAction::BuildDish),
            (r.can_repair, "Restore dish · 2 iron", RadioAction::RepairDish),
        ] {
            if enabled && ui.button(label).clicked() {
                *action = UiAction::OrbitalRadio(request);
            }
        }
    });
    let learned = (0..radio::LESSONS.len()).filter(|i| r.knowledge & (1 << i) != 0).count();
    ui.small(format!("Learned {learned} of {} radio lessons.", radio::LESSONS.len()));
}

/// One compact card per service corridor in range.
fn corridors(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    ui.heading("Contacts");
    if view.contacts.is_empty() {
        ui.label("None in range.");
        return;
    }
    for c in &view.contacts {
        let s = c.service;
        let link = view.radio.link(s.site.cell);
        ui.group(|ui| {
            ui.strong(format!("{} · ML-{:04X}", s.site.class.name(), s.site.key & 0xffff));
            ui.label(format!("{:.1} km · {:03}° · {}", c.distance_m / 1000.0, c.bearing, s.phase.name()));
            if link.receive {
                ui.small(format!(
                    "Next change in {} min · signal {}%",
                    s.minutes_to_change,
                    (link.quality * 100.0) as u32
                ));
                ui.small(s.site.class.manifest());
            } else {
                ui.small("Radio upgrade required.");
            }
            if s.requested {
                ui.label("Requested");
            }
            ui.horizontal_wrapped(|ui| {
                if c.can_request && link.transmit && ui.button("Radio return").clicked() {
                    *action = UiAction::RequestOrbital(s.site.cell, Channel::Radio);
                }
                if view.radio.near_dish && ui.button("Point dish").clicked() {
                    *action = UiAction::OrbitalRadio(RadioAction::Aim(s.site.cell));
                }
                if c.can_request && ui.button("Resonant return").clicked() {
                    *action = UiAction::RequestOrbital(s.site.cell, Channel::Resonance);
                }
            });
        });
    }
}

/// The ground receiver and its one commissioning shipment.
fn receiver_section(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    let receiver = &view.receiver;
    ui.heading("Receiver");
    ui.label(receiver.status);
    if let Some(p) = receiver.position {
        ui.small(format!("Console at {:.0}, {:.0}, {:.0}. Stay within 6 m.", p.x, p.y, p.z));
    } else if receiver.can_deploy {
        ui.small("Needs a flat, dry spot with 3 m of headroom. Costs 8 iron and 4 copper.");
    }
    if receiver.minutes > 0 {
        ui.label(format!("Delivery {}min", receiver.minutes));
    }
    ui.small("ML-C01: 4 copper + 1 crystal for 8 steel + 4 circuits. Once per world.");
    ui.strong("Relay recovery");
    ui.small(receiver.recovery_status);
    ui.small("2 steel + 1 circuit for 6 circuits + 2 crystals. One recovery per world. Place the dish near the receiver and choose Track derelict relay.");
    ui.horizontal_wrapped(|ui| {
        for (enabled, label, request) in [
            (receiver.can_deploy, "Build receiver", Action::Deploy),
            (receiver.can_order, "Order shipment", Action::Order),
            (receiver.can_collect, "Collect cargo", Action::Collect),
            (receiver.can_restore, "Restore console · 2 iron", Action::Restore),
            (receiver.can_recover, "Survey relay & order recovery", Action::RecoverRelay),
            (receiver.can_board, "Board express lift", Action::Board),
        ] {
            if enabled && ui.button(label).clicked() {
                *action = UiAction::OrbitalReceiver(request);
            }
        }
    });
    if receiver.freight_unlocked {
        ui.collapsing("Talon Freight",|ui| {
            ui.label(receiver.freight_status);
            ui.label(format!("Orbit {} iron · Ground {} iron",receiver.orbital_iron,receiver.delivered_iron));
            ui.small(format!("Lift Capacity {:.0} kg · Strain {:.2}%",receiver.capacity_kg,receiver.strain*100.0));
            ui.small(format!("Lifted {} kg · Lowered {} kg",receiver.lifted_kg,receiver.lowered_kg));
            ui.horizontal_wrapped(|ui| {
                for (enabled,label,request) in [
                    (receiver.can_lift,format!("Lift Iron · {}",crate::world::orbital::dynamics::IRON_PER_CRATE),Action::LiftIron),
                    (receiver.can_lower,format!("Lower Iron · {}",crate::world::orbital::dynamics::IRON_PER_CRATE),Action::LowerIron),
                    (receiver.can_collect_iron,"Collect Iron".into(),Action::CollectIron),
                ] {
                    if ui.add_enabled(enabled,egui::Button::new(label)).clicked() {*action=UiAction::OrbitalReceiver(request);}
                }
            });
        });
    }
}

fn pass_forecasts(ui:&mut egui::Ui,view:&View) {
    if view.radio.level==0 {return;}
    ui.collapsing("Upcoming passes",|ui| {
        if view.pass_forecasts_pending {ui.small("Computing local ephemerides...");}
        else if view.pass_forecasts.is_empty() {ui.small("No qualifying passes in the next world day.");}
        for event in &view.pass_forecasts {
            let wait=((event.opens-view.calendar).max(0.0)*1440.0).ceil() as u32;
            let remaining=((event.closes-view.calendar).max(0.0)*1440.0).ceil() as u32;
            ui.strong(event.kind.name());
            if wait==0 {ui.small(format!("Window open / {remaining} world min remaining / peak {:.0} degrees",event.peak_degrees));}
            else {ui.small(format!("In {wait} world min / window {:.0} min / peak {:.0} degrees",(event.closes-event.opens)*1440.0,event.peak_degrees));}
        }
        ui.small("Bad weather can interrupt the signal.");
    });
}

/// Everything overhead, one line each.
fn ephemeris(ui: &mut egui::Ui, view: &View) {
    if view.radio.level == 0 || (view.megastructures.is_empty() && view.skyhooks.is_empty()) {
        return;
    }
    ui.collapsing("In the sky", |ui| {
        for c in &view.megastructures {
            ui.label(format!(
                "{} · {:.0} km away · {:03}° · {:.0}° up",
                c.site.kind.name(),
                c.distance_m / 1000.0,
                c.bearing,
                c.elevation_deg
            ));
        }
        for c in &view.skyhooks {
            ui.label(format!(
                "{} · tip {:.0} km up · next dock in {:.0} min",
                c.hook.size.name(),
                c.tip_altitude_m / 1000.0,
                c.next_port_cusp_s / 60.0
            ));
        }
    });
}
