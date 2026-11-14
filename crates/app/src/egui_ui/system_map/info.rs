//! Selected body card: physical data, range, transfer figures, target control.
use super::super::flight_hud::body_color;
use super::super::presentation::{AMBER, MUTED};
use super::View;
use crate::space_nav::{self, Fix, TICKS_PER_SECOND, Transfer, text};
use egui::RichText;
use engine_space::{BodyId, System};

fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(RichText::new(label).color(MUTED));
    ui.label(RichText::new(value).monospace());
    ui.end_row();
}

/// Surface gravity shown for a body, m/s²; home's is the game's own.
fn surface_gravity(system: &System, body: BodyId) -> f64 {
    if system.name(body) == "home" { 9.81 } else { system.surface_gravity(body) }
}

pub(super) fn draw(ui: &mut egui::Ui, system: &System, fix: &Fix, view: &mut View, plan: Option<Transfer>, target: Option<BodyId>) {
    let body = view.selected;
    let kind = system.kind(body);
    ui.label(RichText::new(text::name(system, body)).color(body_color(kind)).size(20.0));
    ui.label(RichText::new(text::kind(kind)).color(MUTED).small());
    ui.add_space(4.0);
    egui::Grid::new("system_map_body").num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
        row(ui, "Radius", text::range(space_nav::surface_radius(system, body)));
        row(ui, "Gravity", format!("{:.2} m/s²", surface_gravity(system, body)));
        if let Some(spin) = system.spin(body) {
            row(ui, "Day", text::duration(spin.period_ticks as f64 / TICKS_PER_SECOND));
        }
        if let Some(orbit) = system.orbit(body) {
            row(ui, "Orbit", text::duration(system.period_ticks(body) as f64 / TICKS_PER_SECOND));
            row(ui, "Orbit Radius", text::range(orbit.a));
        }
        if let Some(soi) = system.soi(body) {
            row(ui, "Influence", text::range(soi));
        }
        row(ui, "Range Now", text::range(fix.contact(system, body).range_m));
    });
    if let Some(plan) = plan {
        ui.separator();
        ui.label(RichText::new("Transfer Estimate").color(AMBER));
        egui::Grid::new("system_map_transfer").num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
            row(ui, "From", text::name(system, plan.from));
            row(ui, "Depart", text::speed(plan.depart_mps));
            row(ui, "Capture", text::speed(plan.capture_mps));
            row(ui, "Total Δv", text::speed(plan.depart_mps + plan.capture_mps));
            row(ui, "Coast", text::duration(plan.coast_s));
            let window = match plan.window_s {
                Some(s) if s >= 1.0 => text::duration(s),
                Some(_) => "Open".to_owned(),
                None if plan.child => "Open".to_owned(),
                None => "None".to_owned(),
            };
            row(ui, "Window", window);
        });
        if let Some(wait) = plan.window_s.filter(|s| *s >= 1.0)
            && ui.button("Show Window").clicked()
        {
            let tick = view.preview_tick.unwrap_or(fix.tick).saturating_add((wait * TICKS_PER_SECOND).round() as u64);
            view.preview_tick = Some(tick);
            view.offset_s = tick.saturating_sub(fix.tick) as f64 / TICKS_PER_SECOND;
        }
    } else {
        ui.separator();
        ui.label(RichText::new(if body == fix.reference { "Local Body" } else { "No Direct Transfer" }).color(MUTED));
    }
    ui.separator();
    ui.horizontal(|ui| {
        if Some(body) == target {
            if ui.button("Clear Target").clicked() {
                space_nav::set_target(None);
            }
        } else if ui.button("Set Target").clicked() {
            space_nav::set_target(Some(body));
        }
        if view.focus != body && ui.button("Focus").clicked() {
            view.focus_on(body);
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn home_gravity_is_the_ground_pull_and_the_moon_is_lighter() {
        let system = crate::space_nav::system().unwrap();
        let (home, moon) = (system.id("home").unwrap(), system.id("moon").unwrap());
        assert_eq!(super::surface_gravity(system, home), 9.81);
        let g = super::surface_gravity(system, moon);
        assert!(g > 0.1 && g < 9.81, "moon {g}");
    }
}
