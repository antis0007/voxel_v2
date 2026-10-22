//! Factorio-style production screen: every resource the factory made and
//! used over the last minute or hour, as items per minute, plus a power graph
//! of generation against load for the last sixty seconds.
use super::icons::{self, Icon};
use super::presentation::{AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::world::expedition::NAMES;
use crate::world::factory::{View, power};
use crate::world::factory::overview::{Health, Summary};
use egui::{Color32, RichText, Sense, Stroke, pos2, vec2};

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut super::UiAction) {
    let habitat_id = egui::Id::new("production_habitats");
    let mut habitats = ui.ctx().data_mut(|d| d.get_temp::<bool>(habitat_id).unwrap_or(false));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut habitats, false, "Factory");
        ui.selectable_value(&mut habitats, true, "Habitats");
    });
    ui.ctx().data_mut(|d| d.insert_temp(habitat_id, habitats));
    if habitats { super::habitats::draw(ui, &view.habitats, action); return; }
    let mode_id = egui::Id::new("production_mode");
    let mut health_mode = ui.ctx().data_mut(|d| d.get_temp::<bool>(mode_id).unwrap_or(false));
    ui.horizontal(|ui| {
        ui.label(RichText::new("PRODUCTION").size(12.0).color(AMBER));
        if ui.selectable_label(!health_mode, "Rates").clicked() {
            health_mode = false;
        }
        if ui
            .selectable_label(health_mode, "Health")
            .on_hover_text("Which assemblies run, starve, block or lack power, and what limits the factory")
            .clicked()
        {
            health_mode = true;
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(mode_id, health_mode));
    if health_mode {
        draw_health(ui, &view.health);
        return;
    }
    let hour_id = egui::Id::new("production_hour");
    let mut hour = ui.ctx().data_mut(|d| d.get_temp::<bool>(hour_id).unwrap_or(false));
    ui.horizontal(|ui| {
        if ui.selectable_label(!hour, "1 min").clicked() {
            hour = false;
        }
        if ui.selectable_label(hour, "1 h").clicked() {
            hour = true;
        }
        ui.label(
            RichText::new(format!("{} assemblies", view.count))
                .size(11.0)
                .color(MUTED),
        );
    });
    ui.ctx().data_mut(|d| d.insert_temp(hour_id, hour));
    let (sample, span) = if hour { view.hour } else { view.minute };
    let per_min = |sub: u32| sub as f32 / 64.0 * 60.0 / span;

    // Power graph: last 60 s, generation (amber) against load (blue).
    ui.label(RichText::new("POWER · last 60 s").size(11.0).color(MUTED));
    let graph_height = (ui.ctx().content_rect().height() * 0.2).clamp(40.0, 90.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), graph_height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0, Color32::from_rgb(17, 15, 20));
    p.rect_stroke(rect, 0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    let peak = view
        .power_history
        .iter()
        .map(|(g, d)| (*g).max(*d))
        .max()
        .unwrap_or(0)
        .max(4) as f32;
    for (k, colour) in [(0usize, AMBER), (1, MANA)] {
        let points: Vec<egui::Pos2> = view
            .power_history
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let v = if k == 0 { s.0 } else { s.1 } as f32;
                let x = rect.left() + rect.width() * i as f32 / 59.0;
                pos2(x, rect.bottom() - 4.0 - (rect.height() - 8.0) * v / peak)
            })
            .collect();
        if points.len() > 1 {
            p.add(egui::Shape::line(points, Stroke::new(1.5, colour)));
        }
    }
    // A second of history is four ticks of units; per tick is kilowatts.
    let last = view.power_history.last().copied().unwrap_or((0, 0));
    p.text(
        rect.left_top() + vec2(4.0, 3.0),
        egui::Align2::LEFT_TOP,
        format!(
            "made {}  used {}  peak {}",
            power::fmt_power(last.0 as f64 / 4.0),
            power::fmt_power(last.1 as f64 / 4.0),
            power::fmt_power(peak as f64 / 4.0)
        ),
        egui::FontId::monospace(10.0),
        PAPER,
    );
    ui.label(
        RichText::new(format!(
            "Energy over the window: {} generated · {} consumed",
            power::fmt_energy(sample.generated as f64),
            power::fmt_energy(sample.drawn as f64)
        ))
        .size(11.0)
        .color(MUTED),
    );
    ui.label(RichText::new(format!("Storage: {} in · {} out | delivered {} · converters {}",
        power::fmt_energy(sample.charged as f64), power::fmt_energy(sample.discharged as f64),
        power::fmt_energy(sample.delivered as f64), power::fmt_energy(sample.converted as f64)))
        .size(11.0).color(MUTED))
        .on_hover_text("Delivered fills machine buffers; consumed is committed work. Converters are ideal capacity-limited bridges with zero modeled electrical loss. Storage transfers are not fresh generation.");

    let mut rows: Vec<usize> = (0..NAMES.len())
        .filter(|&i| sample.made[i] > 0 || sample.used[i] > 0)
        .collect();
    rows.sort_by(|a, b| {
        (sample.made[*b] + sample.used[*b]).cmp(&(sample.made[*a] + sample.used[*a]))
    });
    if rows.is_empty() && sample.kits == 0 {
        ui.label(
            RichText::new("Nothing has been produced yet in this window. Machines report here as they finish batches.")
                .color(MUTED),
        );
        return;
    }
    let top = rows
        .iter()
        .map(|&i| per_min(sample.made[i]).max(per_min(sample.used[i])))
        .fold(1.0f32, f32::max);
    egui::ScrollArea::vertical()
        .id_salt("production_rows")
        .auto_shrink([false, false])
        .max_height((ui.ctx().content_rect().height()
            - (ui.cursor().top() - ui.min_rect().top()) - 32.0).clamp(48.0, 260.0))
        .show(ui, |ui| {
            egui::Grid::new("production_grid")
                .spacing([8.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Item").size(11.0).color(MUTED));
                    ui.label(RichText::new("Made / min").size(11.0).color(MUTED));
                    ui.label(RichText::new("Used / min").size(11.0).color(MUTED));
                    ui.label(RichText::new("Net").size(11.0).color(MUTED));
                    ui.end_row();
                    for i in rows {
                        let made = per_min(sample.made[i]);
                        let used = per_min(sample.used[i]);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(ui.painter(), r, Icon::Resource(i));
                            ui.label(RichText::new(NAMES[i]).size(11.0).color(PAPER));
                        });
                        rate_bar(ui, made, top, JADE);
                        rate_bar(ui, used, top, AMBER);
                        let net = made - used;
                        ui.label(
                            RichText::new(format!("{net:+.1}"))
                                .monospace()
                                .size(11.0)
                                .color(if net >= 0.0 { JADE } else { DANGER }),
                        );
                        ui.end_row();
                    }
                    if sample.kits > 0 {
                        ui.label(RichText::new("Machine kits (assemblers)").size(11.0).color(PAPER));
                        ui.label(
                            RichText::new(format!("{:.1}", sample.kits as f32 * 60.0 / span))
                                .monospace()
                                .size(11.0)
                                .color(JADE),
                        );
                        ui.end_row();
                    }
                });
        });
}

fn rate_bar(ui: &mut egui::Ui, value: f32, top: f32, colour: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(130.0, 14.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0, Color32::from_rgb(20, 18, 22));
    let mut fill = r;
    fill.set_width(r.width() * (value / top).clamp(0.0, 1.0));
    p.rect_filled(fill, 0, colour.gamma_multiply(0.55));
    p.text(
        r.left_center() + vec2(3.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{value:.1}"),
        egui::FontId::monospace(10.0),
        PAPER,
    );
}

fn health_colour(h: Health) -> Color32 {
    match h {
        Health::Running => Color32::from_rgb(120, 214, 150),
        Health::Starved => AMBER,
        Health::Blocked => Color32::from_rgb(240, 140, 60),
        Health::Unpowered => DANGER,
        Health::Idle => MUTED,
        Health::Paused => Color32::from_rgb(110, 110, 124),
        Health::Unbuilt | Health::Unconfigured => Color32::from_rgb(120, 150, 220),
    }
}

/// Factorio's factory overview: a stacked bar of every assembly by state, the
/// main limit with its fix, and one row per kind ordered by trouble.
fn draw_health(ui: &mut egui::Ui, health: &Summary) {
    let total = health.total();
    if total == 0 {
        ui.label(RichText::new("Nothing has been built yet.").color(MUTED));
        return;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 16.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0, Color32::from_rgb(17, 15, 20));
    let mut x = rect.left();
    for h in Health::ALL {
        let n = health.counts[h as usize];
        if n == 0 {
            continue;
        }
        let w = rect.width() * n as f32 / total as f32;
        p.rect_filled(
            egui::Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height())),
            0,
            health_colour(h),
        );
        x += w;
    }
    p.rect_stroke(rect, 0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(RichText::new(format!("{total} assemblies")).size(12.0).color(AMBER));
        for h in Health::ALL {
            let n = health.counts[h as usize];
            if n > 0 {
                ui.label(
                    RichText::new(format!("{n} {}", h.label()))
                        .size(11.0)
                        .color(health_colour(h)),
                );
            }
        }
    });
    match health.bottleneck() {
        Some((h, n)) => {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Main limit: {n} {}", h.label()))
                    .size(12.0)
                    .color(health_colour(h)),
            );
            ui.small(h.fix());
        }
        None => {
            ui.add_space(4.0);
            ui.label(RichText::new("No limits: nothing is starved, blocked or unpowered.").size(11.0).color(JADE));
        }
    }
    ui.add_space(6.0);
    egui::ScrollArea::vertical()
        .id_salt("health_rows")
        .max_height(ui.available_height().min(ui.ctx().content_rect().height() - 300.0).max(120.0))
        .show(ui, |ui| {
            egui::Grid::new("health_grid")
                .spacing([12.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    for head in ["Assembly", "Run", "Starved", "Blocked", "Unpowered", "Idle", "Other", "Main issue"] {
                        ui.label(RichText::new(head).size(11.0).color(MUTED));
                    }
                    ui.end_row();
                    for row in &health.rows {
                        let Some(kind) = row.kind else { continue };
                        ui.label(RichText::new(kind.name()).size(11.0).color(PAPER));
                        let cell = |ui: &mut egui::Ui, n: u32, colour: Color32| {
                            ui.label(
                                RichText::new(format!("{n}"))
                                    .monospace()
                                    .size(11.0)
                                    .color(if n == 0 { MUTED } else { colour }),
                            );
                        };
                        let c = row.counts;
                        cell(ui, c[Health::Running as usize], health_colour(Health::Running));
                        cell(ui, c[Health::Starved as usize], health_colour(Health::Starved));
                        cell(ui, c[Health::Blocked as usize], health_colour(Health::Blocked));
                        cell(ui, c[Health::Unpowered as usize], health_colour(Health::Unpowered));
                        cell(ui, c[Health::Idle as usize], health_colour(Health::Idle));
                        let other = c[Health::Paused as usize]
                            + c[Health::Unbuilt as usize]
                            + c[Health::Unconfigured as usize];
                        cell(ui, other, health_colour(Health::Paused));
                        match row.main_issue() {
                            Some((h, n)) => ui.label(
                                RichText::new(format!("{n} {}", h.label()))
                                    .size(11.0)
                                    .color(health_colour(h)),
                            ),
                            None => ui.label(RichText::new("—").size(11.0).color(MUTED)),
                        };
                        ui.end_row();
                    }
                });
        });
}
