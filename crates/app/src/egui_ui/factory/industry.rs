use super::*;
use crate::world::{factory::industry as machines, industry::config::CONFIG};

pub fn readings(ui: &mut egui::Ui, view: &View) {
    for (i, &value) in view.air.iter().enumerate().filter(|(_, v)| **v >= 0.5) {
        ui.label(
            RichText::new(format!("{}  {:.1} mg/m³", CONFIG.species[i].name, value)).color(
                if CONFIG.species[i].harm_mg_m3 > 0.0 && value > CONFIG.species[i].harm_mg_m3 {
                    icons::GOLD
                } else {
                    icons::PAPER
                },
            ),
        );
    }
    if view.contamination_mg > 0 {
        ui.small(format!(
            "Deposited  {:.2} g",
            view.contamination_mg as f64 / 1000.0
        ));
    }
}

pub fn panel(ui: &mut egui::Ui, pos: IVec3, n: &Node, view: &View, action: &mut UiAction) {
    ui.set_width(360.0);
    ui.heading(n.kind.name());
    ui.label(n.status);
    let modes = n.kind.modes();
    if !modes.is_empty() {
        egui::ComboBox::from_id_salt("gas_machine_mode")
            .selected_text(modes.get(n.mode as usize).copied().unwrap_or(modes[0]))
            .show_ui(ui, |ui| {
                for (i, label) in modes.iter().enumerate() {
                    if ui.selectable_label(n.mode as usize == i, *label).clicked() {
                        *action = UiAction::Factory(Action::SetRecipe(pos, i as u32));
                    }
                }
            });
    }
    if n.ruptured
        && ui
            .button("Repair")
            .on_hover_text(
                CONFIG
                    .repair_cost
                    .iter()
                    .map(|&(i, n)| format!("{} {:.2}", NAMES[i], n as f32 / 64.0))
                    .collect::<Vec<_>>()
                    .join(" · "),
            )
            .clicked()
    {
        *action = UiAction::Factory(Action::RepairGas(pos));
    }
    gas_slots(ui, n);
    outlet_controls(ui, pos, n, action);
    if n.kind == Kind::GasTank || machines::processor(n) {
        if ui.button("Fill jetpack fuel cell").on_hover_text("Requires an empty bucket and 0.51 g fuel gas").clicked() {
            *action = UiAction::Factory(Action::FillFuelCell(pos));
        }
    }
    if n.kind == Kind::GasGenerator {
        power_section(ui, n, view.focused_grid);
        ui.small(format!("Buffer  {} / {}", power::fmt_joules(n.energy as f64), power::fmt_joules(power::GENERATOR_BUFFER as f64)));
        ui.small(format!("Condensate  {}", condensate_volume(machines::turbine::condensate(n))));
        if machines::turbine::active(n) {
            let t = &CONFIG.steam_turbine;
            ui.small(format!("Steam  {} mg / 250 J", t.steam_mg_per_energy));
            ui.small(format!("Output Limit  {} kW", t.max_energy_per_tick));
        }
    }
    if n.kind == Kind::WetScrubber {
        ui.small(format!(
            "Water  {:.1} L",
            (f64::from(n.water) * f64::from(machines::WATER_UNIT_MG) + f64::from(n.separator_water_mg)) / 1_000_000.0
        ));
        power_section(ui, n, view.focused_grid);
        if n.mode == 3 {
            ui.horizontal(|ui| {
                ui.small(format!("Bio-ethanol  {:.2}", n.input[crate::world::factory::ETHANOL] as f32 / 64.0));
                if ui.button("Load fuel").clicked() {
                    *action = UiAction::Factory(Action::Load(pos));
                }
            });
        }
    }
    if n.kind == Kind::DustFilter || (n.kind == Kind::WetScrubber && !machines::processor(n)) {
        waste_controls(ui, pos, n, action);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(if n.enabled { "Pause" } else { "Run" }).clicked() {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
        let drain_label = if n.kind == Kind::GasGenerator { "Bottle Condensate" } else if n.water == 0 { "Bottle remaining water" } else { "Drain water" };
        if (n.water > 0 || n.separator_water_mg > 0) && ui.button(drain_label).clicked() {
            *action = UiAction::Factory(Action::Drain(pos));
        }
    });
    if n.kind == Kind::ExhaustDuct {
        super::face_row(ui, pos, n, action);
    }
    readings(ui, view);
    ui.small(format!("Wind  {:.1}, {:.1} m/s", view.wind[0], view.wind[1]));
}

pub(super) fn gas_slots(ui: &mut egui::Ui, n: &Node) {
    let mass = n.exhaust.iter().sum::<u32>();
    ui.add(
        egui::ProgressBar::new(mass as f32 / machines::GAS_CAPACITY as f32).text(format!(
            "Gas  {:.1} / {:.1} g",
            mass as f32 / 1000.0,
            machines::GAS_CAPACITY as f32 / 1000.0
        )),
    );
    for (i, &amount) in n.exhaust.iter().enumerate() {
        if amount > 0 || ((n.kind == Kind::GasTank || machines::processor(n)) && i >= 3) {
            ui.small(format!(
                "{}  {:.2} g",
                CONFIG.species[i].name,
                amount as f32 / 1000.0
            ));
        }
    }
}

fn waste_controls(ui: &mut egui::Ui, pos: IVec3, n: &Node, action: &mut UiAction) {
        let waste = n.residue.iter().sum::<u32>();
        ui.add(
            egui::ProgressBar::new(waste as f32 / machines::WASTE_CAPACITY as f32)
                .text(format!("Waste  {:.1} g", waste as f32 / 1000.0)),
        );
        if n.kind == Kind::DustFilter {
            ui.small(format!(
                "Filter  {:.0}%",
                n.filter_credit as f32 * 100.0 / machines::FILTER_CAPACITY as f32
            ));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Load media").clicked() {
                *action = UiAction::Factory(Action::Load(pos));
            }
            if ui.button("Pack waste").clicked() {
                *action = UiAction::Factory(Action::PackWaste(pos));
            }
            if ui.button("Clean ground").clicked() {
                *action = UiAction::Factory(Action::CleanPollution(pos));
            }
        });
}

pub(super) fn outlet_controls(ui: &mut egui::Ui, pos: IVec3, n: &Node, action: &mut UiAction) {
    let sides = ["Auto", "East", "West", "Up", "Down", "South", "North"];
    let gas_label = |i: usize| i.checked_sub(1).and_then(|i| CONFIG.species.get(i))
        .map_or("All gases", |gas| gas.name.as_str());
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("gas_outlet_face")
            .selected_text(sides.get(n.gas_vent_side as usize).copied().unwrap_or(sides[0]))
            .show_ui(ui, |ui| {
                for (i, label) in sides.iter().enumerate() {
                    if ui.selectable_label(n.gas_vent_side as usize == i, *label).clicked() {
                        *action = UiAction::Factory(Action::GasOutlet(pos, i as u8, n.gas_vent_species));
                    }
                }
            });
        egui::ComboBox::from_id_salt("gas_outlet_species")
            .selected_text(gas_label(n.gas_vent_species as usize))
            .show_ui(ui, |ui| {
                for i in 0..=CONFIG.species.len() {
                    if ui.selectable_label(n.gas_vent_species as usize == i, gas_label(i)).clicked() {
                        *action = UiAction::Factory(Action::GasOutlet(pos, n.gas_vent_side, i as u8));
                    }
                }
            });
        if ui
            .button(if n.gas_vent { "Close vent" } else { "Vent gas" })
            .clicked()
        {
            *action = UiAction::Factory(Action::VentGas(pos));
        }
    });
}

pub(super) fn condensate_volume(mg: u64) -> String {
    if mg < 1_000_000 {
        format!("{:.3} mL", mg as f64 / 1_000.0)
    } else {
        format!("{:.3} L", mg as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fired_boiler_ui_only_shows_electrical_panel_for_retained_charge() {
        use crate::world::factory::{self, Kind, industry::boiler};
        let mut world = crate::world::World::with_seed(engine_voxel::WorldType::Superflat,
            engine_core::VoxelScalePreset::OneMeter, 7);
        let n = factory::fixture_place(&mut world, glam::IVec3::new(0, 64, 0), Kind::Generator).unwrap();
        n.mode = boiler::MODE;
        assert!(!super::super::generator_power_visible(n));
        n.energy = 1;
        assert!(super::super::generator_power_visible(n));
        n.energy = 0; n.mode = 0;
        assert!(super::super::generator_power_visible(n));
        n.kind = Kind::Bioreactor;
        assert!(super::super::generator_power_visible(n));
    }
    #[test]
    fn turbine_condensate_reading_keeps_small_recoverable_amounts_visible() {
        for (mg, expected) in [(0, "0.000 mL"), (1, "0.001 mL"), (96, "0.096 mL"),
            (192, "0.192 mL"), (999_999, "999.999 mL"), (1_000_000, "1.000 L"),
            (16_000_000_000, "16000.000 L")] {
            assert_eq!(super::condensate_volume(mg), expected);
        }
    }
}
