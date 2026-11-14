//! Express lift cabin panel: speed and arrival large, drive, cabin field,
//! altitude and phase beneath. Numbers refresh at 10 Hz; the frame follows
//! the cabin at display rate.
use super::UiAction;
use crate::world::orbital::express::View;
use crate::world::orbital::receiver::Action;
use egui::{Align2, Color32, RichText};
use engine_space::express::Phase;

const REFRESH_S: f64 = 0.1;

#[derive(Clone, Copy, Default)]
struct Shown {
    at: f64,
    speed_mps: f64,
    arrival_s: f64,
    drive_g: f64,
    cabin_g: f64,
    altitude_m: f64,
}

fn g_text(g: f64) -> String {
    if g.abs() >= 10.0 { format!("{g:+.0} g") } else { format!("{g:+.1} g") }
}

fn speed_text(mps: f64) -> String {
    if mps >= 1000.0 { format!("{:.2} km/s", mps / 1000.0) } else { format!("{mps:.0} m/s") }
}

fn altitude_text(m: f64) -> String {
    if m >= 10_000.0 { format!("{:.1} km", m / 1000.0) } else { format!("{m:.0} m") }
}

/// Local time from the day fraction, 24 h clock.
fn clock_text(time_of_day: f64) -> String {
    let minutes = (time_of_day.rem_euclid(1.0) * 1440.0).floor() as u64;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn arrival_text(s: f64) -> String {
    let s = s.ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub(super) fn draw(ctx: &egui::Context, view: &View, action: &mut UiAction) {
    if view.on_deck && (view.pressure.is_some() || view.board.is_some()) {
        // Anchorage board: the station's own readings, not the cabin's.
        let p = super::themes::current(ctx);
        let dim = p.edge;
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        egui::Area::new(egui::Id::new("habitat_pressure"))
            .anchor(Align2::LEFT_TOP, egui::vec2(16.0, 16.0))
            .order(egui::Order::Background)
            .interactable(false).show(ctx, |ui| {
                super::presentation::hud_frame(ctx).show(ui, |ui| {
                    ui.set_min_width(230.0);
                    if let Some(b) = view.board {
                        ui.label(RichText::new(b.class).color(dim).small());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(altitude_text(b.altitude_m)).size(22.0).monospace());
                            ui.add_space(12.0);
                            ui.label(RichText::new(clock_text(b.time_of_day)).size(22.0).monospace());
                        });
                        ui.separator();
                    }
                    egui::Grid::new("anchorage_board").num_columns(2).spacing([18.0, 2.0]).show(ui, |ui| {
                        if let Some(b) = view.board {
                            ui.label(RichText::new(if b.next_is_sunrise { "Sunrise" } else { "Sunset" }).color(dim));
                            ui.label(RichText::new(arrival_text(b.next_light_s)).monospace());
                            ui.end_row();
                            ui.label(RichText::new("Rim spin").color(dim));
                            ui.label(RichText::new(format!("{:.1} rpm · {:.1} g", b.rim_rpm, b.rim_g)).monospace());
                            ui.end_row();
                            ui.label(RichText::new("Braids in sight").color(dim));
                            ui.label(RichText::new(format!("{}", b.braids_in_sight)).monospace());
                            ui.end_row();
                        }
                        if let Some((pressure, oxygen)) = view.pressure {
                            ui.label(RichText::new("Pressure").color(dim));
                            ui.label(RichText::new(format!("{:.1} kPa", pressure / 1000.0)).monospace());
                            ui.end_row();
                            ui.label(RichText::new("O₂").color(dim));
                            ui.label(RichText::new(format!("{:.1} kPa", oxygen / 1000.0)).monospace());
                            ui.end_row();
                        }
                        if let Some(b) = view.board {
                            ui.label(RichText::new("Cabin").color(dim));
                            ui.label(if b.door_open { "Docked · Door open" } else { "Away" });
                            ui.end_row();
                        }
                    });
                });
            });
    }
    let Some(t) = view.telemetry else { return };
    if view.on_deck { return; }
    let now = ctx.input(|i| i.time);
    let id = egui::Id::new("lift_panel_readings");
    let mut shown: Shown = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if now - shown.at >= REFRESH_S || shown.at == 0.0 {
        shown = Shown {
            at: now,
            speed_mps: t.speed_mps,
            arrival_s: t.arrival_s,
            drive_g: t.drive_g,
            cabin_g: t.cabin_g,
            altitude_m: t.altitude_m,
        };
        ctx.data_mut(|d| d.insert_temp(id, shown));
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
    let p = super::themes::current(ctx);
    let dim = p.edge;
    let calm = Color32::from_rgb(0xc9, 0xd6, 0xc2);
    egui::Area::new(egui::Id::new("lift_panel"))
        // Top left: the minimap and its coordinates own the top right.
        .anchor(Align2::LEFT_TOP, egui::vec2(16.0, 16.0))
        .order(egui::Order::Background)
        .interactable(view.can_descend)
        .show(ctx, |ui| {
            super::presentation::hud_frame(ctx).show(ui, |ui| {
                ui.set_min_width(230.0);
                ui.label(RichText::new("Express Lift").color(dim).small());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(speed_text(shown.speed_mps)).color(calm).size(26.0).monospace());
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Arrival").color(dim).small());
                        ui.label(RichText::new(arrival_text(shown.arrival_s)).color(calm).size(18.0).monospace());
                    });
                });
                ui.separator();
                egui::Grid::new("lift_readings").num_columns(2).spacing([18.0, 2.0]).show(ui, |ui| {
                    ui.label(RichText::new("Drive").color(dim));
                    ui.label(RichText::new(g_text(shown.drive_g)).monospace());
                    ui.end_row();
                    ui.label(RichText::new("Cabin").color(dim));
                    ui.label(RichText::new(format!("{:.1} g", shown.cabin_g)).monospace());
                    ui.end_row();
                    if let Some((pressure, _)) = view.pressure {
                        ui.label("Pressure");
                        ui.label(format!("{:.1} kPa", pressure / 1000.0));
                        ui.end_row();
                    }
                    ui.label(RichText::new("Altitude").color(dim));
                    ui.label(RichText::new(altitude_text(shown.altitude_m)).monospace());
                    ui.end_row();
                    ui.label(RichText::new("Phase").color(dim));
                    ui.label(t.phase.label());
                    ui.end_row();
                    ui.label(RichText::new("Rated").color(dim));
                    ui.label(RichText::new(format!("{} · {}", g_text(view.rated_g).trim_start_matches('+'), speed_text(view.rated_mps))).small());
                    ui.end_row();
                });
                let bar = egui::ProgressBar::new(t.progress as f32).desired_width(230.0).desired_height(4.0);
                ui.add(bar);
                if view.can_descend && t.phase == Phase::Docked && ui.button("Descend").clicked() {
                    *action = UiAction::OrbitalReceiver(Action::Descend);
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_keep_restrained_precision() {
        assert_eq!(g_text(8.43), "+8.4 g");
        assert_eq!(g_text(-21.1), "-21 g");
        assert_eq!(g_text(0.0), "+0.0 g");
        assert_eq!(speed_text(327.0), "327 m/s");
        assert_eq!(speed_text(3636.4), "3.64 km/s");
        assert_eq!(altitude_text(812.0), "812 m");
        assert_eq!(altitude_text(18_100.0), "18.1 km");
        assert_eq!(arrival_text(119.2), "2:00");
        assert_eq!(arrival_text(0.0), "0:00");
        assert_eq!(clock_text(0.5), "12:00");
        assert_eq!(clock_text(0.26), "06:14");
        assert_eq!(clock_text(1.0), "00:00");
    }
}
