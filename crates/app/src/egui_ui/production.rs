//! Factorio-style production screen: every resource the factory made and
//! used over the last minute or hour, as items per minute, plus a power graph
//! of generation against load for the last sixty seconds.
use super::icons::{self, Icon};
use super::presentation::{AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::world::expedition::NAMES;
use crate::world::factory::{View, power};
use crate::world::factory::{Action as FactoryAction, ETHANOL, Kind, fuel_order, recipes};
use crate::world::factory::overview::{Health, Outpost, Summary};
use egui::{Color32, RichText, Sense, Stroke, pos2, vec2};

pub(super) fn iron_line(ui: &mut egui::Ui, view: &View) {
    if let Some(state) = view.iron_line {
        ui.label(RichText::new(format!("Iron Line · {state}")).color(PAPER));
    }
}

/// Which base the dashboard shows. `Here` follows the player between bases.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Scope {
    #[default]
    Here,
    Base(u32),
    All,
}

/// A base within this distance (metres) counts as the one the player is at.
const HERE_M: f32 = 48.0;
const RAIL_WIDTH: f32 = 184.0;
/// Below this pane width the base list folds into a drop-down.
const RAIL_MIN_PANE: f32 = 600.0;

fn outpost_title(anchor: glam::IVec3) -> String {
    super::map::outpost_name(anchor)
        .unwrap_or_else(|| format!("Base {} · {}", anchor.x, anchor.z))
}

/// What one dashboard shows: a base, or every base together.
struct Rates<'a> {
    minute: (crate::world::factory::stats::Sample, f32),
    hour: (crate::world::factory::stats::Sample, f32),
    power: &'a [(u64, u64)],
    machines: usize,
    health: &'a Summary,
    stored: std::borrow::Cow<'a, [u64]>,
}

fn base_colour(health: &Summary) -> Color32 {
    match health.bottleneck() {
        Some((h, _)) => health_colour(h),
        None if health.counts[Health::Running as usize] > 0 => JADE,
        None => MUTED,
    }
}

fn balance_text(power: &[(u64, u64)]) -> (String, Color32) {
    let (generated, used) = power.last().copied().unwrap_or((0, 0));
    let balance = generated as f64 - used as f64;
    let sign = if balance < 0.0 { "−" } else { "+" };
    (format!("{sign}{}", power::fmt_power(balance.abs() / 4.0)), if balance < 0.0 { DANGER } else { MUTED })
}

/// One row of the base list: status mark, name, and distance · machines · power.
fn rail_entry(ui: &mut egui::Ui, selected: bool, title: &str, detail: &str, mark: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0, super::grimoire::RECESS);
        p.rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0, AMBER);
    } else if response.hovered() {
        p.rect_filled(rect, 0, super::grimoire::RECESS.gamma_multiply(0.5));
    }
    let dot = egui::Rect::from_center_size(pos2(rect.left() + 12.0, rect.top() + 12.0), vec2(8.0, 8.0));
    p.rect_filled(dot, 0, mark);
    let clip = rect.shrink2(vec2(4.0, 0.0));
    let painter = p.with_clip_rect(clip);
    painter.text(pos2(rect.left() + 22.0, rect.top() + 12.0), egui::Align2::LEFT_CENTER, title,
        egui::FontId::proportional(13.0), if selected { PAPER } else { PAPER.gamma_multiply(0.85) });
    painter.text(pos2(rect.left() + 22.0, rect.top() + 28.0), egui::Align2::LEFT_CENTER, detail,
        egui::FontId::proportional(11.0), MUTED);
    response.clicked()
}

fn stock(ui: &mut egui::Ui, amounts: &[u64]) {
    ui.scope(|ui| {
        let id = ui.id().with("stock_filter");
        let mut query = ui.ctx().data(|d| d.get_temp::<String>(id)).unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut query).hint_text("Find resource"));
        let needle = query.trim().to_ascii_lowercase();
        ui.ctx().data_mut(|d| d.insert_temp(id, query));
        let mut found = false;
        egui::Grid::new("shared_stock_ledger").spacing([12.0, 3.0]).striped(true).show(ui, |ui| {
            for (i, &amount) in amounts.iter().enumerate().filter(|(_, n)| **n > 0) {
                if !NAMES[i].to_ascii_lowercase().contains(&needle) { continue; }
                found = true;
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Resource(i));
                    ui.label(NAMES[i]);
                });
                let (r, _) = ui.allocate_exact_size(vec2(86.0, 18.0), Sense::hover());
                ui.painter().text(r.right_center(), egui::Align2::RIGHT_CENTER,
                    super::icons::amount_text(amount as f32 / 64.0), egui::FontId::monospace(13.0), PAPER);
                ui.end_row();
            }
        });
        if !found { ui.weak(if needle.is_empty() { "Storage Empty" } else { "No Matches" }); }
    });
}

fn fuel_request(ui: &mut egui::Ui, view: &View, action: &mut super::UiAction) {
    use crate::world::inventory::{ArmorTier, EquipSlot, Item};
    let Some(order) = &view.fuel_order else {
        if view.workshop.as_ref().is_some_and(|p| p.delivery) {
            if ui.link("Fuel Requests · Cultivator Recipe").clicked() {
                super::recipe_book::request(ui.ctx(), Item::Block(Kind::Cultivator.block()), false);
            }
        }
        return;
    };
    let recipe = recipes::process(Kind::Cultivator, fuel_order::MODE as usize)
        .expect("Cultivator fermentation recipe");
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{} × {}", fuel_order::BATCH, NAMES[ETHANOL]));
        if order.target > 0 {
            let delivered = fuel_order::REQUEST.saturating_sub(order.target);
            ui.label(format!("{} / {} · {}", super::icons::amount_text(delivered as f32 / 64.0),
                fuel_order::BATCH, order.status));
        } else {
            ui.weak(order.status);
        }
        if ui.add_enabled(order.ready, egui::Button::new("Request Fuel")).clicked() {
            *action = super::UiAction::Factory(FactoryAction::SetTarget(order.pos, fuel_order::REQUEST));
        }
        if order.target != 0 && ui.small_button("Clear & Pause").clicked() {
            *action = super::UiAction::Factory(FactoryAction::SetTarget(order.pos, 0));
        }
        if order.target == 1 && ui.link("Burner Jetpack").clicked() {
            super::recipe_book::request(ui.ctx(), Item::Armor(EquipSlot::Chest, ArmorTier::Burner), false);
        }
    });
    ui.horizontal_wrapped(|ui| {
        for ((item, units), have) in recipe.inputs.into_iter().filter(|(_, units)| *units > 0).zip(order.stock) {
            let need = units * order.batches_needed;
            if need == 0 { continue; }
            ui.colored_label(if have >= need { JADE } else { AMBER }, format!("{} {}/{}",
                NAMES[item], super::icons::amount_text(have as f32 / 64.0),
                super::icons::amount_text(need as f32 / 64.0)));
        }
        ui.weak("Cultivator → Item Pipe → Storage");
    });
}

fn here(view: &View) -> Option<&Outpost> {
    view.outposts.first().filter(|o| o.distance_m <= HERE_M)
}

fn scope_id() -> egui::Id {
    egui::Id::new("factory_scope")
}

fn tab_id() -> egui::Id {
    egui::Id::new("production_tab")
}

/// Every base, nearest first, with an aggregate row on top.
fn base_list(ui: &mut egui::Ui, view: &View, scope: &mut Scope, compact: bool) {
    let here_id = here(view).map(|o| o.id);
    let machines: usize = view.outposts.iter().map(|o| o.machines).sum();
    let all_detail = format!("{} Bases · {machines} Machines", view.outposts.len());
    let all_selected = *scope == Scope::All || (*scope == Scope::Here && here_id.is_none());
    let pick = |o: &Outpost| if Some(o.id) == here_id { Scope::Here } else { Scope::Base(o.id) };
    let is_selected = |scope: Scope, o: &Outpost| {
        scope == Scope::Base(o.id) || (scope == Scope::Here && here_id == Some(o.id))
    };
    if compact {
        let selected = view.outposts.iter().find(|o| is_selected(*scope, o))
            .map_or_else(|| "All Bases".to_string(), |o| outpost_title(o.anchor));
        egui::ComboBox::from_id_salt("base_picker").selected_text(selected).show_ui(ui, |ui| {
            if ui.selectable_label(all_selected, format!("All Bases · {all_detail}")).clicked() {
                *scope = Scope::All;
            }
            for o in &view.outposts {
                if ui.selectable_label(is_selected(*scope, o),
                    format!("{} · {:.0} m", outpost_title(o.anchor), o.distance_m)).clicked() {
                    *scope = pick(o);
                }
            }
        });
        return;
    }
    ui.label(RichText::new("BASES").size(12.0).color(AMBER));
    egui::ScrollArea::vertical()
        .id_salt("base_rail")
        .auto_shrink([false, true])
        .max_height((ui.ctx().content_rect().height() * 0.6).max(160.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if rail_entry(ui, all_selected, "All Bases", &all_detail, base_colour(&view.health)) {
                *scope = Scope::All;
            }
            for o in &view.outposts {
                let (balance, _) = balance_text(&o.power);
                let detail = format!("{:.0} m · {} Machines · {balance}", o.distance_m, o.machines);
                if rail_entry(ui, is_selected(*scope, o), &outpost_title(o.anchor), &detail, base_colour(&o.health)) {
                    *scope = pick(o);
                }
            }
            if view.outposts.is_empty() {
                ui.label(RichText::new("No Bases").color(MUTED));
            }
        });
}

/// The selected base's dashboard: name, then Rates, Health or Storage.
fn dashboard(ui: &mut egui::Ui, view: &View, scope: Scope) {
    let site = match scope {
        Scope::Here => here(view),
        Scope::Base(id) => view.outposts.iter().find(|o| o.id == id),
        Scope::All => None,
    };
    let rates = match site {
        Some(o) => Rates {
            minute: o.minute,
            hour: o.hour,
            power: &o.power,
            machines: o.machines,
            health: &o.health,
            stored: std::borrow::Cow::Borrowed(&o.stored[..]),
        },
        None => {
            let mut totals = vec![0u64; crate::world::factory::RESOURCE_COUNT];
            for o in &view.outposts {
                for (total, amount) in totals.iter_mut().zip(o.stored) {
                    *total += amount;
                }
            }
            Rates {
                minute: view.minute,
                hour: view.hour,
                power: &view.power_history,
                machines: view.outposts.iter().map(|o| o.machines).sum(),
                health: &view.health,
                stored: std::borrow::Cow::Owned(totals),
            }
        }
    };
    ui.horizontal_wrapped(|ui| match site {
        Some(o) => {
            let title = outpost_title(o.anchor);
            ui.label(RichText::new(&title).size(15.0).color(PAPER));
            ui.label(RichText::new(format!("{:.0} m · {} Machines", o.distance_m, o.machines)).size(12.0).color(MUTED));
            if super::map::outpost_name(o.anchor).is_none() && ui.small_button("Pin").clicked() {
                super::map::pin_outpost(o.anchor, title);
            }
        }
        None => {
            ui.label(RichText::new("All Bases").size(15.0).color(PAPER));
            ui.label(RichText::new(format!("{} Bases · {} Machines", view.outposts.len(), rates.machines))
                .size(12.0).color(MUTED));
        }
    });
    iron_line(ui, view);
    let mut tab = ui.ctx().data_mut(|d| d.get_temp::<u8>(tab_id()).unwrap_or(0));
    ui.horizontal(|ui| {
        for (k, name) in ["Rates", "Health", "Storage"].into_iter().enumerate() {
            if ui.selectable_label(tab == k as u8, name).clicked() {
                tab = k as u8;
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(tab_id(), tab));
    match tab {
        1 => draw_health(ui, rates.health),
        2 => stock(ui, &rates.stored),
        _ => draw_rates(ui, &rates),
    }
}

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut super::UiAction) {
    let habitat_id = egui::Id::new("production_habitats");
    let mut habitats = ui.ctx().data_mut(|d| d.get_temp::<bool>(habitat_id).unwrap_or(false));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut habitats, false, "Factory");
        ui.selectable_value(&mut habitats, true, "Habitats");
    });
    ui.ctx().data_mut(|d| d.insert_temp(habitat_id, habitats));
    if habitats { super::habitats::draw(ui, &view.habitats, action); return; }
    if let Some((pos, node)) = &view.focused {
        super::factory::stock_reserve(ui, *pos, node, view.stock_reserve, action);
    }
    fuel_request(ui, view, action);
    let mut scope = ui.ctx().data_mut(|d| d.get_temp::<Scope>(scope_id()).unwrap_or_default());
    if matches!(scope, Scope::Base(id) if !view.outposts.iter().any(|o| o.id == id)) {
        scope = Scope::Here;
    }
    if ui.available_width() < RAIL_MIN_PANE {
        base_list(ui, view, &mut scope, true);
        dashboard(ui, view, scope);
    } else {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(RAIL_WIDTH);
                base_list(ui, view, &mut scope, false);
            });
            ui.separator();
            ui.vertical(|ui| dashboard(ui, view, scope));
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(scope_id(), scope));
}

/// Items per minute and the power graph for one base or all of them.
fn draw_rates(ui: &mut egui::Ui, rates: &Rates) {
    let hour_id = egui::Id::new("production_hour");
    let mut hour = ui.ctx().data_mut(|d| d.get_temp::<bool>(hour_id).unwrap_or(false));
    ui.horizontal(|ui| {
        if ui.selectable_label(!hour, "1 min").clicked() {
            hour = false;
        }
        if ui.selectable_label(hour, "1 h").clicked() {
            hour = true;
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(hour_id, hour));
    let (sample, span) = if hour { rates.hour } else { rates.minute };
    let span = span.max(0.001);
    let per_min = |sub: u32| sub as f32 / 64.0 * 60.0 / span;

    if let Some((health, count)) = rates.health.bottleneck()
        && ui.add(egui::Button::new(
            RichText::new(format!("{count} {}  ·  Inspect machines", health.label()))
                .color(health_colour(health)),
        )).on_hover_text(health.fix()).clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(tab_id(), 1u8));
        }

    // The keyed readings sit outside the plot so a peak never hides a number.
    let last = rates.power.last().copied().unwrap_or((0, 0));
    ui.label(RichText::new("POWER · last 60 s").size(13.0).color(MUTED));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Generated {}", power::fmt_power(last.0 as f64 / 4.0))).color(AMBER).size(12.0));
        ui.label(RichText::new(format!("Used {}", power::fmt_power(last.1 as f64 / 4.0))).color(MANA).size(12.0));
        let balance = last.0 as f64 - last.1 as f64;
        ui.label(RichText::new(format!("{} {}",
            if balance >= 0.0 { "Surplus" } else { "Shortfall" },
            power::fmt_power(balance.abs() / 4.0),
        )).color(if balance >= 0.0 { MUTED } else { DANGER }).size(13.0));
    });
    let graph_height = (ui.ctx().content_rect().height() * 0.2).clamp(40.0, 90.0);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), graph_height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0, super::grimoire::RECESS);
    p.rect_stroke(rect, 0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    let plot = rect.shrink(5.0);
    for fraction in [0.25, 0.5, 0.75] {
        let y = plot.bottom() - plot.height() * fraction;
        p.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, EDGE.gamma_multiply(0.35)));
    }
    let peak = rates
        .power
        .iter()
        .map(|(g, d)| (*g).max(*d))
        .max()
        .unwrap_or(0)
        .max(4) as f32;
    for (k, colour) in [(0usize, AMBER), (1, MANA)] {
        let points: Vec<egui::Pos2> = rates
            .power
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let v = if k == 0 { s.0 } else { s.1 } as f32;
                let x = plot.left() + plot.width() * i as f32 / 59.0;
                pos2(x, plot.bottom() - plot.height() * v / peak)
            })
            .collect();
        if points.len() > 1 {
            p.add(egui::Shape::line(points, Stroke::new(1.5, colour)));
        }
    }
    // A second of history is four ticks of units; per tick is kilowatts.
    if let Some(pointer) = response.hover_pos() {
        let index = (((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0) * 59.0).round() as usize;
        if let Some(&(generated, used)) = rates.power.get(index) {
            let x = plot.left() + plot.width() * index as f32 / 59.0;
            p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0, PAPER));
            response.on_hover_text(format!("{} s ago\nGenerated {} · Used {}",
                rates.power.len().saturating_sub(index + 1),
                power::fmt_power(generated as f64 / 4.0),
                power::fmt_power(used as f64 / 4.0),
            ));
        }
    }
    ui.label(
        RichText::new(format!(
            "Energy over the window: {} generated · {} consumed",
            power::fmt_energy(sample.generated as f64),
            power::fmt_energy(sample.drawn as f64)
        ))
        .size(13.0)
        .color(MUTED),
    );
    ui.label(RichText::new(format!("Storage: {} in · {} out | delivered {} · converters {}",
        power::fmt_energy(sample.charged as f64), power::fmt_energy(sample.discharged as f64),
        power::fmt_energy(sample.delivered as f64), power::fmt_energy(sample.converted as f64)))
        .size(13.0).color(MUTED))
        .on_hover_text("Delivered: power received. Consumed: power used by machines. Storage transfers are excluded.");

    let mut rows: Vec<usize> = (0..NAMES.len())
        .filter(|&i| sample.made[i] > 0 || sample.used[i] > 0)
        .collect();
    rows.sort_by(|a, b| {
        (sample.made[*b] + sample.used[*b]).cmp(&(sample.made[*a] + sample.used[*a]))
    });
    if rows.is_empty() && sample.kits == 0 {
        ui.label(RichText::new("No production yet.").color(PAPER));
        // Machines already placed but silent need a feed; an empty base needs a first machine.
        let steps: [&str; 3] = if rates.machines > 0 {
            [
                "1. Load fuel into each generator, or feed a drill power.",
                "2. Run conduit from every output to a chest or the next machine.",
                "3. Open Inspect machines above to see what each one is waiting on.",
            ]
        } else {
            [
                "1. Place a drill on ore, or a generator that burns fuel.",
                "2. Run conduit from its output to a chest or another machine.",
                "3. Power the machines. Rates show up here once items move.",
            ]
        };
        for line in steps {
            ui.label(RichText::new(line).size(13.0).color(MUTED));
        }
        return;
    }
    let top = rows
        .iter()
        .map(|&i| per_min(sample.made[i]).max(per_min(sample.used[i])))
        .fold(1.0f32, f32::max);
    let rate_width = ((ui.available_width() - 190.0) * 0.5).clamp(55.0, 130.0);
    egui::ScrollArea::both()
        .id_salt("production_rows")
        .auto_shrink([false, false])
        .max_height((ui.ctx().content_rect().height()
            - (ui.cursor().top() - ui.min_rect().top()) - 32.0).clamp(48.0, 260.0))
        .show(ui, |ui| {
            egui::Grid::new("production_grid")
                .spacing([8.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Item").size(13.0).color(MUTED));
                    ui.label(RichText::new("Made / min").size(13.0).color(MUTED));
                    ui.label(RichText::new("Used / min").size(13.0).color(MUTED));
                    ui.label(RichText::new("Net").size(13.0).color(MUTED));
                    ui.end_row();
                    for i in rows {
                        let made = per_min(sample.made[i]);
                        let used = per_min(sample.used[i]);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(ui.painter(), r, Icon::Resource(i));
                            ui.label(RichText::new(NAMES[i]).size(13.0).color(PAPER));
                        });
                        rate_bar(ui, made, top, JADE, rate_width);
                        rate_bar(ui, used, top, AMBER, rate_width);
                        let net = made - used;
                        ui.label(
                            RichText::new(format!("{net:+.1}"))
                                .monospace()
                                .size(13.0)
                                .color(if net >= 0.0 { JADE } else { DANGER }),
                        );
                        ui.end_row();
                    }
                    if sample.kits > 0 {
                        ui.label(RichText::new("Machine kits (assemblers)").size(13.0).color(PAPER));
                        ui.label(
                            RichText::new(format!("{:.1}", sample.kits as f32 * 60.0 / span))
                                .monospace()
                                .size(13.0)
                                .color(JADE),
                        );
                        ui.end_row();
                    }
                });
        });
}

fn rate_bar(ui: &mut egui::Ui, value: f32, top: f32, colour: Color32, width: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(width, 17.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0, super::grimoire::RECESS);
    let mut fill = r;
    fill.set_width(r.width() * (value / top).clamp(0.0, 1.0));
    p.rect_filled(fill, 0, colour.gamma_multiply(0.30));
    p.text(
        r.left_center() + vec2(3.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{value:.1}"),
        egui::FontId::monospace(11.0),
        PAPER,
    );
}

fn health_colour(h: Health) -> Color32 {
    match h {
        Health::Running => JADE,
        Health::Starved => AMBER,
        Health::Blocked => super::pixel_art::EMBER,
        Health::Unpowered => DANGER,
        Health::Idle => MUTED,
        Health::Paused => MUTED,
        Health::Unbuilt | Health::Unconfigured => MANA,
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
    p.rect_filled(rect, 0, super::grimoire::RECESS);
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
                        .size(13.0)
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
            ui.label(RichText::new("No Bottlenecks").size(13.0).color(JADE));
        }
    }
    ui.add_space(6.0);
    egui::ScrollArea::both()
        .id_salt("health_rows")
        .max_height(ui.available_height().min(ui.ctx().content_rect().height() - 300.0).max(120.0))
        .show(ui, |ui| {
            egui::Grid::new("health_grid")
                .spacing([12.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    for head in ["Assembly", "Run", "Starved", "Blocked", "Unpowered", "Idle", "Other", "Main issue"] {
                        ui.label(RichText::new(head).size(13.0).color(MUTED));
                    }
                    ui.end_row();
                    for row in &health.rows {
                        let Some(kind) = row.kind else { continue };
                        ui.label(RichText::new(kind.name()).size(13.0).color(PAPER));
                        let cell = |ui: &mut egui::Ui, n: u32, colour: Color32| {
                            ui.label(
                                RichText::new(format!("{n}"))
                                    .monospace()
                                    .size(13.0)
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
                                    .size(13.0)
                                    .color(health_colour(h)),
                            ),
                            None => ui.label(RichText::new("—").size(13.0).color(MUTED)),
                        };
                        ui.end_row();
                    }
                });
        });
}
