use super::UiAction;
use crate::world::orbital::{Channel, View, receiver::Action};
use crate::world::orbital::radio::{self, Action as RadioAction};

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    egui::ScrollArea::vertical()
        .id_salt("orbital_service_ledger")
        .max_height((ui.ctx().content_rect().height() - 100.0).max(120.0))
        .show(ui, |ui| ledger(ui, view, action));
}

fn ledger(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    ui.heading("Meridian Lift / open service band");
    ui.label("An old orbital commons still tends its atmospheric corridors. The published band carries schedules and manifests; private traffic remains encrypted.");
    ui.small("Bearings are degrees clockwise from north. Times use the world calendar.");
    ui.small("Atmospheric service network · berths ≥360 m above local terrain · upper relays 18 km above berth. Levitation lattice active.");
    radio_ledger(ui,view,action);
    if view.radio.level>0 {
        ui.collapsing("Published orbital ephemeris",|ui| {
            for c in &view.megastructures {
                ui.strong(format!("{} · {:04X}",c.site.kind.name(),c.site.key&0xffff));
                ui.small(format!("Slant {:.0} km · {:03}° · elevation {:.1}° · orbit {:.1} min",
                    c.distance_m/1000.0,c.bearing,c.elevation_deg,c.period_seconds/60.0));
                ui.small(c.site.kind.description());
            }
            for c in &view.skyhooks {
                let h = &c.hook;
                ui.strong(h.size.name());
                ui.small(format!("Tether {:.0} km · turn {:.1} min · tip {:.0} km · hub elevation {:.1}° · port cusp in {:.0} min",
                    h.tether_m/1000.0, h.turn_period()/60.0, c.tip_altitude_m/1000.0,
                    c.hub_elevation_deg, c.next_port_cusp_s/60.0));
            }
            ui.small("Sealed aerial installations. The Meridian receiver is the local cargo interface.");
            ui.small("Orbital periods use world minutes; the day/night time acceleration applies to motion.");
        });
    }
    ui.separator();
    let receiver = &view.receiver;
    ui.strong("Ground receiver · commissioning contract ML-C01");
    ui.label(receiver.status);
    if let Some(p) = receiver.position {
        ui.small(format!(
            "Console at {:.0}, {:.0}, {:.0} m · operate within 6 m",
            p.x, p.y, p.z
        ));
    } else {
        ui.small("Near corridor (1100, -900), face a dry, flat patch four metres ahead. Clear three metres of headroom.");
    }
    if receiver.minutes > 0 {
        ui.label(format!(
            "Delivery in {} world minutes; cargo waits safely if you leave.",
            receiver.minutes
        ));
    }
    if receiver.quote_minutes > 0 {
        ui.small(format!(
            "Quoted delivery: {} world minutes after ordering.",
            receiver.quote_minutes
        ));
    }
    ui.small("Build: 8 iron + 4 copper ingots. Contract: 4 copper ingots + 1 crystal → 8 steel ingots + 4 ancient circuits. One shipment per world.");
    ui.horizontal_wrapped(|ui| {
        for (enabled, label, request) in [
            (receiver.can_deploy, "Build receiver", Action::Deploy),
            (receiver.can_order, "Pay & order shipment", Action::Order),
            (
                receiver.can_collect,
                "Collect sealed cargo",
                Action::Collect,
            ),
            (
                receiver.can_restore,
                "Restore console · 2 iron",
                Action::Restore,
            ),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                *action = UiAction::OrbitalReceiver(request);
            }
        }
    });
    ui.separator();
    ui.vertical(|ui| {
        if view.contacts.is_empty() {
            ui.label("No service corridors within receiver range.");
        }
        for c in &view.contacts {
            let s = c.service;
            let link=view.radio.link(s.site.cell);
            ui.group(|ui| {
                ui.strong(format!(
                    "ML-{:04X} · {}",
                    s.site.key & 0xffff,
                    s.site.class.name()
                ));
                ui.label(format!("{:.1} km · {:03}° · {}",c.distance_m/1000.0,c.bearing,s.phase.name()));
                if link.receive {ui.small(format!("Next service change in {} world min",s.minutes_to_change));}
                ui.small(format!("Public service carrier {:.3} MHz · signal {}%{}",
                    137.0+(s.site.key%200) as f32*0.005,(link.quality*100.0) as u32,
                    if link.directional {" · dish tracking"} else {""}));
                if link.receive {ui.small(s.site.class.manifest());}
                else {ui.small("Visual contact / carrier unavailable. Improve reception to decode the manifest.");}
                if s.requested {
                    ui.label("Your alignment request is on the service ledger.");
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(c.can_request && link.transmit, egui::Button::new("Radio return"))
                        .clicked()
                    {
                        *action = UiAction::RequestOrbital(s.site.cell, Channel::Radio);
                    }
                    if ui.add_enabled(view.radio.near_dish,egui::Button::new("Point dish")).clicked() {
                        *action=UiAction::OrbitalRadio(RadioAction::Aim(s.site.cell));
                    }
                    if ui
                        .add_enabled(c.can_request, egui::Button::new("Resonant return"))
                        .clicked()
                    {
                        *action = UiAction::RequestOrbital(s.site.cell, Channel::Resonance);
                    }
                });
            });
        }
    });
    ui.small("A return requests a sealed inspection descent at the existing corridor. Allow 36 world minutes for alignment. One request at a time; current visits retain their windows.");
    ui.small("The ground receiver accepts one commissioning shipment. Platforms remain sealed to passengers.");
}

fn radio_ledger(ui:&mut egui::Ui,view:&View,action:&mut UiAction) {
    let r=&view.radio;
    ui.separator();
    ui.strong(format!("Communications · level {} · {}",r.level,radio::NAMES[r.level as usize]));
    ui.label(r.status);
    ui.collapsing("Radio technology and knowledge",|ui| {
        for tier in 1..=5 {
            let capability=["", "Receive public beacons within 1.5 km. Listen to learn carrier waves.",
                "Transmit within 1.5 km; receive within 3 km. Handshakes teach modulation.",
                "Filtered reception to 6 km; portable transmission to 3 km.",
                "Build and aim a physical dish; operate nearby for a 6 km uplink. Handshake to learn tracking.",
                "Upgrade the dish with ancient circuits; coherent reception resists storm interference."][tier];
            ui.label(format!("{} {} · {}",if r.level>=tier as u32 {"✓"} else {"○"},radio::NAMES[tier],radio::COST_TEXT[tier]));
            ui.small(capability);
        }
        for (i,lesson) in radio::LESSONS.iter().enumerate() {
            ui.small(format!("{} {}",if r.knowledge & (1<<i)!=0 {"Learned:"} else {"Unknown:"},lesson));
        }
    });
    ui.horizontal_wrapped(|ui| {
        if r.level<5 && r.level!=3 {
            let label=format!("Build {} · {}",radio::NAMES[r.level as usize+1],radio::COST_TEXT[r.level as usize+1]);
            if ui.add_enabled(r.can_upgrade,egui::Button::new(label)).clicked() {*action=UiAction::OrbitalRadio(RadioAction::Upgrade);}
        }
        for (enabled,label,request) in [(r.can_listen,"Listen / decode beacon",RadioAction::Listen),
            (r.can_handshake,"Establish handshake",RadioAction::Handshake),
            (r.can_build,"Build dish · 8 iron + 6 copper + 2 steel",RadioAction::BuildDish),
            (r.can_repair,"Restore dish · 2 iron",RadioAction::RepairDish)] {
            if ui.add_enabled(enabled,egui::Button::new(label)).clicked() {*action=UiAction::OrbitalRadio(request);}
        }
    });
    ui.small("Receiver grants use a dedicated service port. Portable radio and dishes have their own equipment requirements. Storms reduce signal; knowledge is permanent.");
}
