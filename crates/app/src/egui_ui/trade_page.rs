//! Trade slots and optional supply details.
use super::{presentation as style, quest_log::{self, Lead}};
use crate::world::World;
use crate::world::farming::{freight, pantry, watershed};
use crate::world::travel::{self, ResidentAction, trades};

const SLOT_PX: f32 = 40.0;
/// The page closes beyond this distance from the villager, in metres.
const RANGE_M: f32 = 6.0;

fn stack(ui: &mut egui::Ui, s: trades::Stack, have: Option<u32>) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SLOT_PX, SLOT_PX), egui::Sense::hover());
    let painter = ui.painter();
    super::icons::frame(painter, rect, response.hovered());
    super::inventory::draw_item_icon(painter, rect.shrink(3.0), s.item);
    let short = have.is_some_and(|n| n < s.count);
    let color = if short { style::DANGER } else { style::PAPER };
    painter.text(rect.right_bottom() - egui::vec2(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, s.count.to_string(), egui::FontId::proportional(11.0), color);
    response.on_hover_text(format!("{} × {}", s.item.name(), s.count));
}

fn arrow(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, SLOT_PX), egui::Sense::hover());
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0, style::MUTED);
    ui.painter().line_segment([egui::pos2(rect.left() + 4.0, y), egui::pos2(rect.right() - 4.0, y)], stroke);
    for dy in [-5.0, 5.0] {
        ui.painter().line_segment([egui::pos2(rect.right() - 10.0, y + dy), egui::pos2(rect.right() - 4.0, y)], stroke);
    }
}

/// Draws the open villager's page; closes it when the villager leaves reach or Done is pressed.
pub fn draw(ctx: &egui::Context, open: &mut Option<u32>, world: &mut World, eye: glam::Vec3, network: bool)
    -> Vec<(u32, ResidentAction)>
{
    let Some(villager) = *open else { return Vec::new() };
    if travel_centre(world, villager).is_none_or(|c| c.distance(eye) > RANGE_M) {
        *open = None;
        return Vec::new();
    }
    if !network { world.travel.restock(travel::day(world)); }
    let page = trades::page_for(world, villager);
    let (mut chosen, mut done) = (None, false);
    let workplace_issue = trades::workplace_issue(world, villager);
    let conversation = travel::residents::conversation(world, villager, workplace_issue);
    let history = engine_worldgen::civilization::history(&world.sample_config(), villager);
    let mut service=false;
    let mut accompany=false;
    let mut donation = None;
    let screen_h = ctx.content_rect().height();
    egui::Window::new(trades::name(villager))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            // Rows scroll within the screen; Done stays pinned below them.
            let scroll_h = (screen_h - 150.0).max(120.0);
            egui::ScrollArea::vertical().id_salt("trade_scroll").max_height(scroll_h).auto_shrink([true, true]).show(ui, |ui| {
            egui::Grid::new("trade_rows").spacing(egui::vec2(6.0, 4.0)).show(ui, |ui| {
                for (row, t) in page.iter().enumerate() {
                    let Some(quote) = trades::quote(world, villager, row, t) else { continue };
                    ui.horizontal(|ui| {
                        for &s in &t.give {
                            stack(ui, s, Some(world.inventory.count_item(s.item)));
                        }
                    });
                    arrow(ui);
                    stack(ui, t.get, None);
                    let left = quote.remaining;
                    let stock_backed = quote.stock.is_delivery() || quote.stock.is_stocked();
                    let count = if stock_backed { left.to_string() } else { format!("{left}/{}", t.uses) };
                    ui.label(count).on_hover_text(if quote.stock.is_delivery() { "Lots Wanted" } else { "Trades Left" });
                    let affordable = t.affordable(&world.inventory);
                    let reason = quote.issue.map(trades::WorkplaceIssue::label)
                        .or_else(|| (left == 0).then_some(if quote.stock.is_delivery() { "Orders Closed" } else { "Out of Stock" }))
                        .or_else(|| (!affordable).then_some("Items Missing"));
                    let button = ui.add_enabled(reason.is_none(), egui::Button::new("Trade"));
                    if button.clicked() {
                        chosen = Some(row);
                    }
                    if let Some(reason) = reason {
                        button.on_disabled_hover_text(reason);
                    }
                    ui.end_row();
                }
            });
            let nursery = villager == watershed::SELLER && watershed::active(world);
            let freight_active = freight::active(&world.sample_config());
            let clerk = freight_active && villager == freight::SELLER;
            if nursery || clerk {
                ui.collapsing("Supplies", |ui| {
                    if nursery && let Some(status) = watershed::status(world) {
                        ui.label(status);
                    }
                    if freight_active {
                        ui.label(freight::status(world));
                    }
                });
            }
            if clerk && let Some(report) = pantry::report(world) {
                ui.separator();
                ui.strong("Fernwake Town Board");
                ui.label(report.summary);
                if let Some(cause) = report.cause {
                    ui.label(format!("Waiting for a meal · {}", report.missing));
                    ui.label(cause);
                }
                for (item, label) in [(pantry::FOOD, "Deliver Trailcake"), (pantry::BIOMASS, "Deliver Biomass")] {
                    if ui.add_enabled(pantry::can_donate(world, item), egui::Button::new(label)).clicked() {
                        donation = Some(item);
                    }
                }
            }
            if let Some(label)=travel::services::label(villager) {
                ui.separator();
                ui.horizontal(|ui| {
                    if let Some(item)=travel::services::cost(villager) {
                        stack(ui,trades::Stack {item,count:1},Some(world.inventory.count_item(item)));
                    }
                    let available=travel::services::availability(world,villager,eye);
                    let button=ui.add_enabled(available.is_ok(),egui::Button::new(label));
                    service=button.clicked();
                    if let Err(reason)=available {button.on_disabled_hover_text(reason);}
                });
            }
            if !network {
                use crate::world::farming::jobs;
                let guarding = world.travel.guard_posts.contains_key(&villager);
                let label = if guarding { "Release Guard" } else { "Post Guard Here" };
                let hint = if world.travel.following == Some(villager) {
                    "Station your companion here"
                } else {
                    "1 Trailcake to post · Intercepts nearby hostiles"
                };
                if ui.button(label).on_hover_text(hint).clicked() {
                    travel::toggle_guard_post(world, villager);
                }
                ui.weak(format!("Guards {}/{}", world.travel.guard_posts.len(), travel::MAX_GUARDS));
                let assigned = jobs::assigned(world, villager);
                if ui.button(if assigned { "Release Settler" } else { "Assign Settler" }).clicked() {
                    jobs::assign(world, villager);
                }
                ui.weak(format!("Crew {}/{}", jobs::crew::count(world), jobs::crew::CAPACITY));
                if jobs::assigned(world, villager) {
                    ui.label(jobs::status_for(world, Some(villager)));
                    if ui.small_button("Recruit Constructors").clicked() {
                        match jobs::crew::recruit(world, eye) {
                            Ok(count) => world.expedition.say(format!("Constructors Recruited {count}")),
                            Err(reason) => world.expedition.say(reason),
                        }
                    }
                    ui.weak("Empty hand + Sneak + Use on a chest or terrain block.");
                    let mut priorities = jobs::priorities_for(world, villager);
                    let previous = priorities;
                    ui.horizontal(|ui| {
                        for (label, value) in ["Haul", "Mine", "Build"].into_iter().zip(&mut priorities) {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).range(0..=4).speed(0.2));
                        }
                    });
                    ui.weak("0 off · 1 first · 4 last");
                    if priorities != previous {
                        jobs::set_priorities(world, villager, priorities);
                    }
                    if ui.small_button("Clear Build Orders").clicked() {
                        jobs::clear_builds(world, villager);
                    }
                }
            }
            let following = world.travel.following == Some(villager);
            if network {
                accompany = ui.button(if following { "Stay" } else { "Follow · 1 Trailcake" }).clicked();
            } else {
                let label = if following {
                    "Move In Here"
                } else if world.travel.homes.contains_key(&villager) {
                    "Move to a New Home"
                } else {
                    "Invite to Base"
                };
                if ui.button(label).clicked() {
                    match travel::services::invite_to_base(world, villager, eye) {
                        Ok(()) => done = true,
                        Err(reason) => world.expedition.say(reason),
                    }
                }
            }
            let freight_site = engine_worldgen::fernwake::freight::active(&world.sample_config());
            let directions: &[Lead] = match (freight_site, villager) {
                (true, watershed::SELLER) => &[Lead::Nursery, Lead::FreightOffice],
                (true, freight::SELLER) => &[Lead::ServiceLedger],
                _ => &[],
            };
            if !directions.is_empty() {
                ui.collapsing("Directions", |ui| {
                    for &lead in directions {
                        ui.push_id(lead.name(), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(lead.name());
                                ui.weak(lead.directions());
                                let saved = quest_log::remembered(world, lead);
                                if ui.add_enabled(!saved, egui::Button::new(if saved { "Recorded" } else { "Record" })).clicked() {
                                    quest_log::remember(world, lead);
                                }
                            });
                        });
                    }
                });
            }
            if let Some(lines) = conversation {
                ui.separator();
                if ui.input(|i| i.modifiers.shift) {
                    for line in lines {
                        ui.label(egui::RichText::new(line).italics().color(style::MUTED));
                    }
                    if let Some(line)=travel::residents::waterworks(world,villager) {
                        ui.label(egui::RichText::new(line).color(style::MUTED));
                    }
                } else {
                    ui.label(egui::RichText::new("[Shift] Conversation").small().color(style::MUTED));
                }
                for record in travel::residents::records(villager) {
                    ui.collapsing(&record.title,|ui| {ui.label(&record.text);});
                }
            }
            if let Some(site) = travel::civilization::summary(world, villager) {
                ui.label(format!("{} · {} · {}", site.charter, site.settlement, site.priority));
                ui.label(format!("{} households · {} vacant homes · {} arrivals", site.households, site.vacant, site.arrivals));
                if site.in_transit != 0 || site.awaiting_housing != 0 {
                    ui.label(format!("{} in transit · {} awaiting housing", site.in_transit, site.awaiting_housing));
                }
                if let Some(project) = site.project {
                    ui.label(project);
                }
                if let Some(stock) = crate::world::farming::jobs::communities::stock_summary(world, villager) {
                    ui.label(stock);
                }
            }
            if let Some(history) = history {
                ui.separator();
                ui.collapsing(history.livelihood.name(), |ui| {
                    ui.label(format!("Founded in year {}. The hold reached {} households in year {}. {} remained at the end of the old survey.",
                        history.founded, history.peak_households, history.peak_year, history.households));
                    if let Some(year) = history.flood_year {
                        ui.label(format!("The last flood in the ledger was in year {year}."));
                    }
                    if let Some(year) = history.rockfall_year {
                        ui.label(format!("The ledger records a rockfall in year {year}."));
                    }
                    if history.refugees_received > 0 {
                        ui.label(format!("We took in {} displaced households.", history.refugees_received));
                    }
                    if let Some(neighbor) = history.neighbor {
                        ui.label(format!("In lean years, we drew on the stores of the {}.", neighbor.name()));
                    }
                    ui.label(history.burial.description());
                    ui.label("The old survey pier by the lane marks the storehouse. The wand-lords once claimed salvage rights over it.");
                });
            }
            });
            ui.separator();
            done |= ui.button("Done").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if done {
        *open = None;
    }
    [
        chosen.map(ResidentAction::Trade),
        donation.map(ResidentAction::Donate),
        service.then_some(ResidentAction::Service),
        accompany.then_some(ResidentAction::Accompany),
    ].into_iter().flatten().filter_map(|action| {
        if network {
            Some((villager, action))
        } else {
            travel::resident_action(world, villager, eye, action, None);
            None
        }
    }).collect()
}

fn travel_centre(world: &World, villager: u32) -> Option<glam::Vec3> {
    crate::world::adventure::wildlife::vehicle::villager_centre(world, villager)
}
