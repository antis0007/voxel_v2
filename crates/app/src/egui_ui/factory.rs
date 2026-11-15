use super::{
    UiAction,
    icons::{self, Icon},
};
use crate::egui_ui::presentation::MUTED;
use crate::world::{
    expedition::{Expedition, NAMES},
    factory::{
        Action, Kind, Node, View, assembler, biomatter, blocks::Material, induction, multiblock, pile, power, quarry,
        power::NetStats, recipes,
    },
};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, vec2};
use glam::IVec3;
use crate::world::factory::{GOLD_CONCENTRATE, placer};
use crate::world::factory::industry as gas_tank;
use super::slot_grid::SlotGrid;
mod container_tools;
mod industry;

fn ingredients(ui: &mut egui::Ui, cost: &[(usize, u32)], inventory: &Expedition) {
    ui.horizontal_wrapped(|ui| {
        for &(i, n) in cost {
            let response = icons::slot(
                ui,
                Icon::Resource(i),
                Some(n),
                false,
                &format!(
                    "{}: {:.2} / {:.2}",
                    NAMES[i],
                    inventory.resources[i] as f32 / 64.0,
                    n as f32 / 64.0
                ),
            );
            if inventory.resources[i] < n {
                ui.painter().line_segment(
                    [response.rect.left_bottom(), response.rect.right_bottom()],
                    egui::Stroke::new(3.0, Color32::from_rgb(197, 84, 67)),
                );
            }
        }
    });
}
fn stack_text(n: u32) -> String {
    if n == 0 {
        String::new()
    } else if n.is_multiple_of(64) {
        format!("{}", n / 64)
    } else {
        format!("{:.1}", n as f32 / 64.0)
    }
}

fn block_slot(ui: &mut egui::Ui, material: Material, count: u32) {
    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
    let item = material.item();
    icons::frame(ui.painter(), rect, false);
    super::inventory::draw_item_icon(ui.painter(), rect.shrink(3.0), item);
    super::inventory::stack_count(ui.painter(), rect, 1.0, count);
    response.on_hover_text(format!("{} · {count}", item.name()));
}

fn quarry_settings(ui: &mut egui::Ui, pos: IVec3, node: &Node, size: i32, action: &mut UiAction) {
    if node.kind == Kind::Quarry {
        ui.small("Width");
    }
    mode_picker(ui, pos, node, action);
    if node.kind == Kind::Quarry {
        ui.horizontal(|ui| {
            for (label, enabled) in [("Blocks", false), ("Placer", true)] {
                if ui.selectable_label(node.placer.enabled == enabled, label).clicked() {
                    *action = UiAction::Factory(Action::SetPlacer(pos, enabled));
                }
            }
        });
        if node.placer.enabled || node.placer.has_feed() {
            sediment_buffer(ui, node.placer.feed);
        }
        ui.horizontal_wrapped(|ui| {
            ui.small("Pit Direction");
            let selected = quarry::direction(node);
            for (index, &(label, _)) in quarry::DIRECTIONS.iter().enumerate() {
                if ui.selectable_label(selected == index, label).clicked() {
                    *action = UiAction::Factory(Action::SetQuarryDirection(pos, index as u8));
                }
            }
        });
    }
    let mut depth = quarry::configured_depth(node.target);
    ui.horizontal(|ui| {
        ui.small("Depth");
        if ui.add(egui::DragValue::new(&mut depth).range(1..=quarry::MAX_DEPTH_M).suffix(" m")).changed() {
            *action = UiAction::Factory(Action::SetTarget(pos, depth as u32));
        }
    });
    let size = size.max(1);
    let limit = quarry::limit(size, node.mode, node.target) as u32;
    let done = node.excavation.min(limit);
    let metres = done as f32 / (quarry::layer(size, node.mode) * size) as f32;
    let label = format!("{metres:.1} / {depth} m");
    ui.small(&label);
    icons::gauge(ui, done as f32 / limit as f32, icons::TEAL, &label);
}

fn sediment_buffer(ui: &mut egui::Ui, feed: placer::Feed) {
    let capacity = placer::FEED_CAPACITY as f32;
    let volume = feed.volume as f32 / placer::VOLUME_PER_M3 as f32;
    let label = format!("Sediment · {volume:.2} m³ · Grade Band {} ‰", feed.grade());
    ui.small(&label);
    icons::gauge(ui, feed.volume as f32 / capacity, icons::GOLD, &label);
}

fn grass_process(ui: &mut egui::Ui, node: &Node, recipe: &biomatter::Recipe) {
    ui.horizontal_wrapped(|ui| {
        block_slot(ui, Material::Grass, recipe.grass);
        if recipe.water > 0 {
            ui.label(format!("{} L Water", recipe.water * 1_000 / 512));
        }
        ui.label(RichText::new("→").size(26.0).color(icons::GOLD));
        icons::slot(ui, Icon::Resource(biomatter::RESOURCE), Some(64), true, NAMES[biomatter::RESOURCE]);
    });
    let seconds = recipe.ticks as f32 * power::TICK_SECONDS as f32 / (1 + node.upgrade) as f32;
    let label = format!("{seconds:.1} s · {}", power::fmt_joules((recipe.power * recipe.ticks) as f64));
    ui.small(&label);
    icons::gauge(ui, node.progress as f32 / recipe.ticks as f32, icons::TEAL, &label);
}

struct InputTransfer<'a> {
    pos: IVec3,
    node: &'a Node,
    inventory: &'a Expedition,
    action: &'a mut UiAction,
}

impl InputTransfer<'_> {
    fn slot(&mut self, response: egui::Response, item: usize, count: u32) {
        let accepts = self.node.accepts(item);
        if response.clicked() && count > 0 {
            *self.action = UiAction::Factory(Action::BufferMove(self.pos, item, 64, false));
        } else if response.secondary_clicked() && accepts && self.inventory.resources[item] > 0 {
            *self.action = UiAction::Factory(Action::BufferMove(self.pos, item, 64, true));
        }
        let controls = if accepts {
            "[LMB] Take 1 · [RMB] Load 1"
        } else {
            "[LMB] Take 1"
        };
        response.on_hover_text(format!(
            "{} · {} in Buffer\n{controls}",
            NAMES[item],
            stack_text(count),
        ));
    }
}

/// Fixed cells. Empty compartments stay in the grid, so gaining or losing a
/// stack never changes the panel's size.
fn supply_grid(
    ui: &mut egui::Ui,
    salt: &str,
    slots: &[(usize, u32)],
    mut input: Option<InputTransfer<'_>>,
) {
    const SLOT: f32 = 36.0;
    let columns = SlotGrid::columns_for(ui.available_width(), SLOT, 6);
    let cells = slots.len().max(columns).div_ceil(columns) * columns;
    SlotGrid::new(columns, SLOT).sense(Sense::hover()).show(ui, salt, cells, |ui, index, rect, hover| {
        let Some(&(item, count)) = slots.get(index) else { return };
        ui.painter().rect_filled(rect, 0, ui.visuals().faint_bg_color);
        if let Some(input) = input.as_mut() {
            input.slot(hover.interact(Sense::click()), item, count);
        } else {
            hover.on_hover_text(format!("{} · {} in Buffer", NAMES[item], stack_text(count)));
        }
        icons::paint(ui.painter(), rect.shrink(4.0), Icon::Resource(item));
        if count > 0 {
            ui.painter().text(
                rect.right_bottom() - vec2(2.0, 1.0),
                Align2::RIGHT_BOTTOM,
                stack_text(count),
                FontId::monospace(13.0),
                icons::PAPER,
            );
        }
    });
}

const CONTAINER_PX: f32 = 36.0;

fn container_cell(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: egui::Response,
    item: Option<usize>,
    count: u32,
) -> egui::Response {
    icons::frame(ui.painter(), rect, response.hovered());
    if let Some(item) = item {
        icons::paint(ui.painter(), rect.shrink(4.0), Icon::Resource(item));
        if count > 0 {
            ui.painter().text(
                rect.right_bottom() - vec2(2.0, 1.0),
                Align2::RIGHT_BOTTOM,
                stack_text(count),
                FontId::monospace(10.0),
                icons::PAPER,
            );
        }
    } else {
        ui.painter().rect_filled(
            rect.shrink(3.0),
            0,
            Color32::from_black_alpha(120),
        );
    }
    response
}

fn resource_grid(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    pos: IVec3,
    columns: usize,
    cells: usize,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    let order = container_tools::cell_order(&node.input, cells, container_tools::sorted(ui.ctx(), pos));
    SlotGrid::new(columns, CONTAINER_PX).show(ui, ("container", salt, pos), order.len(), |ui, index, rect, response| {
        let item = order[index];
        let count = item.map(|i| node.input[i]).unwrap_or(0);
        let response = container_cell(ui, rect, response, item, count);
        if let Some(item) = item
            && (response.clicked() || response.secondary_clicked()) {
            *action = UiAction::Factory(Action::ContainerClick(pos, item, response.secondary_clicked(), ui.input(|i| i.modifiers.shift)));
        }
    });
}

pub(super) fn container_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    ui.set_width(360.0);
    ui.set_max_width(360.0);
    let title = match node.mode {
        1 => "Chest",
        2 => "Barrel",
        _ => "Crate",
    };
    ui.label(RichText::new(title).size(20.0).color(icons::PAPER));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        for (mode, label) in ["Crate", "Chest", "Barrel"].into_iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(vec2(88.0, 24.0), Sense::click());
            let on = node.mode == mode as u32;
            ui.painter().rect_filled(
                rect,
                0,
                if on {
                    Color32::from_rgb(93, 73, 48)
                } else if response.hovered() {
                    Color32::from_rgb(55, 46, 43)
                } else {
                    Color32::from_rgb(29, 25, 29)
                },
            );
            ui.painter().rect_stroke(
                rect,
                0,
                Stroke::new(1.0, if on { icons::GOLD } else { icons::TEAL }),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                icons::PAPER,
            );
            if response.clicked() {
                *action = UiAction::Factory(Action::SetRecipe(pos, mode as u32));
            }
        }
    });
    if node.mode == 2 {
        let locked = node
            .filter
            .or_else(|| node.input.iter().position(|&n| n > 0));
        ui.horizontal(|ui| {
            let count = locked.map(|i| node.input[i]).unwrap_or(0);
            let response = {
                let (rect, response) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::click());
                icons::frame(ui.painter(), rect, response.hovered());
                if let Some(item) = locked {
                    icons::paint(ui.painter(), rect.shrink(8.0), Icon::Resource(item));
                    ui.painter().text(
                        rect.right_bottom() - vec2(3.0, 2.0),
                        Align2::RIGHT_BOTTOM,
                        stack_text(count),
                        FontId::monospace(12.0),
                        icons::PAPER,
                    );
                }
                response
            };
            if let Some(item) = locked {
                if response.clicked() && count > 0 {
                    *action = UiAction::Factory(Action::BufferMove(pos, item, 64, false));
                } else if response.secondary_clicked() {
                    *action = UiAction::Factory(Action::BufferMove(pos, item, 64, true));
                }
            }
            ui.vertical(|ui| {
                ui.allocate_exact_size(vec2(220.0, 4.0), Sense::hover());
                let (bar, _) = ui.allocate_exact_size(vec2(220.0, 12.0), Sense::hover());
                ui.painter().rect_filled(bar, 0, Color32::from_rgb(17, 15, 20));
                let fill = count as f32 / crate::world::factory::CAPACITY as f32;
                ui.painter().rect_filled(
                    Rect::from_min_size(bar.min, vec2(bar.width() * fill.clamp(0.0, 1.0), bar.height())),
                    0,
                    icons::GOLD,
                );
                let label = locked
                    .map(|i| format!("{}  {}", NAMES[i], stack_text(count)))
                    .unwrap_or_else(|| "Empty".to_string());
                let (line, _) = ui.allocate_exact_size(vec2(220.0, 16.0), Sense::hover());
                ui.painter().with_clip_rect(line).text(
                    line.left_center(),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::monospace(11.0),
                    icons::PAPER,
                );
            });
        });
    }
    let (columns, cells) = match node.mode {
        1 => (9, 27),
        2 => (9, 18),
        _ => (5, 20),
    };
    container_tools::toolbar(ui, pos, action);
    resource_grid(ui, node.mode, pos, columns, cells, node, action);
    ui.label(RichText::new(status_label(node.status)).color(icons::GOLD));
    container_tools::middle_click_sort(ui, pos);
    face_row(ui, pos, node, action);
}

fn face_row(ui: &mut egui::Ui, pos: IVec3, node: &Node, action: &mut UiAction) {
    if node.kind.conduit() || node.kind.structural() {
        return;
    }
    ui.small(format!("Connections: {}", crate::world::factory::ports::names(node.kind)));
    if node.kind.processor() || node.kind == Kind::Assembler {
        ui.small("Touching chests use these faces")
            .on_hover_text("In draws the current recipe's ingredients. Out deposits finished items. Chest filters, reserved stock and capacity still apply.");
    }
    ui.small("Item Faces");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for face in 0..6 {
            let role = crate::world::factory::sides::role(node, face) as u8;
            let label = format!(
                "{} {}",
                crate::world::factory::sides::FACE_NAME[face],
                crate::world::factory::sides::ROLE_NAME[role as usize],
            );
            let tint = match role {
                1 => icons::TEAL,
                2 => icons::GOLD,
                3 => super::presentation::MUTED,
                _ => icons::PAPER,
            };
            if ui.button(RichText::new(label).color(tint))
                .on_hover_text("Cycle: Both → In → Out → Off")
                .clicked() {
                *action = UiAction::Factory(Action::SetFace(pos, face as u8, (role + 1) % 4));
            }
        }
    });
}

fn conduit_faces(ui: &mut egui::Ui, pos: IVec3, node: &Node, links: u8, action: &mut UiAction) {
    use crate::world::factory::sides::FACE_NAME;
    ui.horizontal_wrapped(|ui| {
        for (side, name) in FACE_NAME.into_iter().enumerate() {
            let closed = node.target & (1 << side) != 0;
            let (state, tint) = if closed {
                ("Closed", super::presentation::MUTED)
            } else if links & (1 << side) != 0 {
                ("Linked", icons::TEAL)
            } else {
                ("Open", icons::PAPER)
            };
            if ui.small_button(RichText::new(format!("{name}: {state}")).color(tint))
                .on_hover_text(if closed { "Open face" } else { "Close face" })
                .clicked()
            {
                *action = UiAction::Factory(Action::ToggleSide(pos, side as u8));
            }
        }
    });
}

fn pipe_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    ui.set_width(420.0);
    ui.set_max_width(420.0);
    ui.label(RichText::new("Item Pipe").size(20.0).color(icons::PAPER));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        for (mode, label) in ["Transfer", "Extract", "Distribute"].into_iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(vec2(120.0, 28.0), Sense::click());
            let on = node.mode == mode as u32;
            ui.painter().rect_filled(
                rect,
                0,
                if on {
                    Color32::from_rgb(93, 73, 48)
                } else if response.hovered() {
                    Color32::from_rgb(55, 46, 43)
                } else {
                    Color32::from_rgb(29, 25, 29)
                },
            );
            ui.painter().rect_stroke(
                rect,
                0,
                Stroke::new(
                    1.0,
                    if mode == 2 {
                        icons::GOLD
                    } else if mode == 1 {
                        icons::TEAL
                    } else {
                        Color32::from_rgb(180, 110, 50)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(13.0),
                icons::PAPER,
            );
            if response.on_hover_text([
                "Carry machine outputs",
                "Pull from adjacent buffers",
                "Balance connected storage",
            ][mode]).clicked() && !on {
                *action = UiAction::Factory(Action::SetRecipe(pos, mode as u32));
            }
        }
    });
    if node.mode != 0 {
        ui.label(RichText::new("FILTER").size(13.0).color(icons::GOLD));
        egui::Grid::new(("pipe-filter", pos))
            .spacing([2.0, 2.0])
            .show(ui, |ui| {
                // One cell per resource, then "Any".
                for index in 0..=NAMES.len() {
                    let item = (index < NAMES.len()).then_some(index);
                    let selected = item.is_some_and(|i| node.filter == Some(i)) || (item.is_none() && node.filter.is_none());
                    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                    icons::frame(ui.painter(), rect, selected || response.hovered());
                    if let Some(item) = item {
                        icons::paint(ui.painter(), rect.shrink(4.0), Icon::Resource(item));
                        if response.clicked() {
                            *action = UiAction::Factory(Action::SetFilter(pos, Some(item)));
                        }
                    } else {
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "Any",
                            FontId::proportional(10.0),
                            icons::PAPER,
                        );
                        if response.clicked() {
                            *action = UiAction::Factory(Action::SetFilter(pos, None));
                        }
                    }
                    if (index + 1) % 12 == 0 {
                        ui.end_row();
                    }
                }
            });
    }
    ui.label(RichText::new(status_label(node.status)).color(icons::GOLD));
}
fn detail(kind: Kind) -> &'static str {
    kind.name()
}

/// Shared presentation names; simulation status strings remain unchanged.
pub(super) fn status_label(status: &str) -> &str {
    match status {
        "Needs cable / power" => "No Power",
        "Output blocked" => "Output Full",
        "Tank full" => "Tank Full",
        "Waiting for world data" | "Waiting for excavation" => "Waiting",
        "Excavation unavailable" => "Blocked",
        "Intake clogged: clear sediment or relocate pump" => "Intake Blocked",
        "Intake dry" | "Needs water" => "No Water",
        "Stand clear of shaft" => "Area Occupied",
        "Assembly in shaft" | "Obstruction in shaft" => "Obstructed",
        "Bedrock reached" => "Bedrock",
        "Shaft complete" | "Quarry complete" => "Complete",
        "Needs input" => "No Input",
        "Processing" => "Working",
        "Held by controller" => "Held",
        "Stock ready" => "Stock Ready",
        other => other,
    }
}

fn recovery_hint(ui: &mut egui::Ui, node: &Node) {
    if !node.enabled {
        ui.small("Resume the machine to continue.");
        return;
    }
    let hint = match node.status {
        "Needs cable / power" => "Connect a power cable and check generator fuel.",
        "Output blocked" => "Collect output or connect storage with item pipe.",
        "Exhaust blocked" => "Vent exhaust outdoors or connect a gas duct.",
        "Needs fuel" if node.kind == Kind::Generator && node.mode == 0 =>
            "Load coal from your pack below.",
        "Needs input" if node.kind == Kind::Furnace && node.mode == 0 =>
            "Add crushed ore, or switch to Ore mode.",
        "Needs input" if node.kind == Kind::Furnace && node.mode == 1 =>
            "Add a whole piece of copper, tin or iron ore.",
        "Shaft complete" if node.kind == Kind::MiningDrill =>
            "Move the drill over another vein or change its depth.",
        "Stand clear of shaft" => "Step away from the area below the drill.",
        "Assembly in shaft" | "Obstruction in shaft" => "Clear the shaft or move the machine.",
        _ => return,
    };
    ui.small(hint);
}
fn affordable(inventory: &Expedition, cost: &[(usize, u32)]) -> bool {
    cost.iter().all(|(i, n)| inventory.resources[*i] >= *n)
}

fn smelter_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &crate::world::factory::Node,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    let alloy = node.kind == Kind::AlloyKiln;
    let gold = node.kind == Kind::Furnace && node.mode == recipes::GOLD_MODE;
    let missing_input = if alloy { "Missing Ingredients" }
        else if gold { "Missing Concentrate" }
        else if node.mode == 1 { "Missing Ore" }
        else { "Missing Dust" };
    let active = node.process();
    let next = active.or_else(|| if gold {
        recipes::process(node.kind, recipes::GOLD_RECIPE)
    } else if alloy {
        recipes::process(node.kind, node.mode as usize)
    } else {
        node.choose_recipe(1)
            .and_then(|id| recipes::process(node.kind, id))
    });
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, node.kind.name());
        ui.vertical(|ui| {
            ui.label(RichText::new(node.kind.name()).size(19.0).color(icons::GOLD));
            let status = if !node.enabled {
                "Paused"
            } else if node.status == "Processing" {
                "Smelting"
            } else if node.status == "Needs cable / power" {
                "Awaiting power"
            } else if node.status == "Output blocked" {
                "Output full"
            } else if node.status == "Needs input" {
                missing_input
            } else {
                node.status
            };
            ui.label(RichText::new(status).size(12.5).color(if node.enabled {
                icons::TEAL
            } else {
                crate::egui_ui::presentation::MUTED
            }));
        });
    });

    recovery_hint(ui, node);
    ui.separator();
    if alloy { mode_picker(ui, pos, node, action); }

    if let Some(recipe) = next {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for &(i, n) in recipe.inputs.iter().filter(|&&(_, n)| n > 0) {
                icons::slot(ui, Icon::Resource(i), Some(n), false, NAMES[i]);
            }
            ui.label(RichText::new("→").size(24.0).color(icons::GOLD));
            icons::slot(
                ui,
                Icon::Resource(recipe.output.0),
                Some(recipe.output.1),
                true,
                NAMES[recipe.output.0],
            );
        });
        let progress = if active.is_some() {
            node.progress as f32 / recipe.ticks.max(1) as f32
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(progress)
                .text(if active.is_some() {
                    format!(
                        "{:.0}% · {:.1} s left",
                        progress * 100.0,
                        (recipe.ticks - node.progress.min(recipe.ticks)) as f32 * 0.25
                            / (1 + node.upgrade) as f32
                    )
                } else {
                    format!(
                        "Ready · {:.1} s at {} · {} per batch",
                        recipe.ticks as f32 * 0.25 / (1 + node.upgrade) as f32,
                        power::fmt_power((recipe.power * (1 + node.upgrade)) as f64),
                        power::fmt_joules((recipe.power * recipe.ticks) as f64)
                    )
                })
                .fill(icons::TEAL),
        );
    } else {
        ui.add_space(4.0);
        ui.label(
            RichText::new(missing_input)
                .color(crate::egui_ui::presentation::MUTED),
        );
    }

    if node.kind == Kind::Furnace {
        ui.add_space(6.0);
        ui.label(
            RichText::new("FEED")
                .size(13.0)
                .color(crate::egui_ui::presentation::MUTED),
        );
        mode_picker(ui, pos, node, action);
        ui.label(
            RichText::new("RECIPE")
                .size(13.0)
                .color(crate::egui_ui::presentation::MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            let ids: &[usize] = if node.mode == 0 { &[9, 10, 11] } else { &[] };
            if node.mode == 1 {
                ui.small("Ore mode picks copper, tin or iron ore automatically.");
            }
            for &id in ids {
                let Some(recipe) = recipes::process(Kind::Furnace, id) else {
                    continue;
                };
                let selected = node.recipe == Some(id);
                let label = format!("{} → {}", NAMES[recipe.inputs[0].0], NAMES[recipe.output.0]);
                if ui.selectable_label(selected, label).clicked() {
                    *action = UiAction::Factory(Action::SetRecipe(pos, id as u32));
                }
            }
        });
    }

    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.48).min(300.0));
            ui.small("Input");
            let inputs: Vec<_> = (0..NAMES.len())
                .filter(|&i| node.accepts(i) || node.input[i] > 0)
                .map(|i| (i, node.input[i]))
                .collect();
            supply_grid(ui, "smelt-in", &inputs, Some(InputTransfer { pos, node, inventory, action }));
        });
        ui.vertical(|ui| {
            ui.small("Output");
            // A new batch or mode never hides stock left by an earlier recipe.
            let outputs: Vec<_> = (0..NAMES.len())
                .filter(|&i| {
                    node.output[i] > 0
                        || next.is_some_and(|r| r.output.0 == i
                            || r.byproduct.is_some_and(|(side, _)| side == i))
                        || (next.is_none()
                            && ((gold && i == crate::world::factory::GOLD_INGOT)
                                || (!gold && matches!(i, 6..=8))))
                })
                .map(|i| (i, node.output[i]))
                .collect();
            if !outputs.is_empty() || !gold {
                supply_grid(ui, "smelt-out", &outputs, None);
            }
            let blocks = node.blocks.output[Material::GoldBlock.index()];
            if blocks > 0 {
                block_slot(ui, Material::GoldBlock, blocks);
            }
        });
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Load All")
            .clicked()
        {
            *action = UiAction::Factory(Action::Load(pos));
        }
        if ui
            .button("Unload All")
            .on_hover_text("Inputs and outputs · May reset batch progress")
            .clicked()
        {
            *action = UiAction::Factory(Action::Collect(pos));
        }
        if ui
            .button(if node.enabled { "Pause" } else { "Start" })
            .clicked()
        {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
        upgrade_button(ui, pos, node, inventory, action);
    });
}

/// A bar with its reading written inside, so the number is visible without
/// hovering.
fn meter(ui: &mut egui::Ui, value: f32, color: Color32, caption: &str) {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width().min(340.0), 22.0), Sense::hover());
    response.on_hover_text(caption);
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    p.rect_filled(r, 0, Color32::from_rgb(17, 15, 20));
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(r.width() * value.clamp(0.0, 1.0), r.height())),
        0,
        color.gamma_multiply(0.7),
    );
    p.rect_stroke(r, 0, Stroke::new(1.0, icons::TEAL.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    p.text(
        r.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        caption,
        FontId::monospace(13.0),
        icons::PAPER,
    );
}

/// Live electrical readings for any assembly on a cable network.
fn power_section(ui: &mut egui::Ui, node: &Node, grid: Option<NetStats>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("POWER").size(13.0).color(icons::GOLD));
        let flow = if node.flow > 0 {
            format!("+{}", power::fmt_power(node.flow as f64))
        } else if node.flow < 0 {
            format!("−{}", power::fmt_power(-node.flow as f64))
        } else {
            "0 W".into()
        };
        ui.label(RichText::new(flow).monospace().color(icons::PAPER));
        if node.kind.powered() {
            ui.label(
                RichText::new(format!(
                    "buffer {} / {}",
                    power::fmt_joules(node.energy as f64),
                    power::fmt_joules(power::CONSUMER_BUFFER as f64)
                ))
                .small()
                .color(MUTED),
            );
        }
    });
    match grid {
        Some(g) => {
            let balance = g.generation as i64 - g.load as i64;
            ui.label(
                RichText::new(format!(
                    "Grid: {} generated · {} drawn · {} gen / {} machines / {} batteries",
                    power::fmt_power(g.generation as f64),
                    power::fmt_power(g.load as f64),
                    g.generators,
                    g.machines,
                    g.batteries
                ))
                .small()
                .color(if balance >= 0 { MUTED } else { icons::GOLD }),
            );
            if g.capacity > 0 {
                meter(
                    ui,
                    g.stored as f32 / g.capacity as f32,
                    icons::TEAL,
                    &format!(
                        "Stored {} / {}{}",
                        power::fmt_energy(g.stored as f64),
                        power::fmt_energy(g.capacity as f64),
                        g.runtime_seconds()
                            .map(|s| format!(" · {:.0} min at this deficit", s / 60.0))
                            .unwrap_or_default()
                    ),
                );
            }
        }
        None => {
            ui.label(
                RichText::new("Not on a power network. Touch a power cable to a face.")
                    .small()
                    .color(icons::GOLD),
            );
        }
    }
}

fn upgrade_button(ui: &mut egui::Ui, pos: IVec3, node: &Node, inventory: &Expedition, action: &mut UiAction) {
    if !node.kind.upgradable() || node.upgrade >= 2 {
        return;
    }
    let cost = node.kind.upgrade_cost(node.upgrade);
    let label = if node.kind == Kind::Battery {
        format!("Cells → {}", power::battery_tier_name(node.upgrade + 1))
    } else if node.kind == Kind::GasTank {
        format!("Tank → {}", gas_tank::tank_tier_name(node.upgrade + 1))
    } else {
        format!("Drive {} → {}", node.upgrade + 1, node.upgrade + 2)
    };
    if ui
        .add_enabled(affordable(inventory, cost), egui::Button::new(label))
        .on_hover_ui(|ui| {
            ingredients(ui, cost, inventory);
            if node.kind == Kind::Battery {
                ui.small(format!(
                    "{} at up to {}. Stored charge is kept.",
                    power::fmt_energy(power::battery_capacity(node.upgrade + 1) as f64),
                    power::fmt_power(power::battery_rate(node.upgrade + 1) as f64)
                ));
            } else if node.kind == Kind::GasTank {
                let mut next = node.clone();
                next.upgrade += 1;
                ui.small(format!(
                    "Holds {:.0} g and moves {:.1} g per face each step. Stored gas is kept.",
                    gas_tank::gas_capacity(&next) as f32 / 1000.0,
                    gas_tank::port_flow(&next) as f32 / 1000.0
                ));
            } else {
                ui.small("Faster batches; the same energy per batch at a higher draw.");
            }
        })
        .clicked()
    {
        *action = UiAction::Factory(Action::UpgradeMachine(pos));
    }
}

fn mode_picker(ui: &mut egui::Ui, pos: IVec3, node: &Node, action: &mut UiAction) {
    ui.horizontal_wrapped(|ui| {
        for (i, label) in node.kind.modes().iter().enumerate() {
            let short = label.split(':').next().unwrap_or(label);
            if ui
                .selectable_label(node.mode == i as u32, short)
                .on_hover_text(*label)
                .clicked()
            {
                *action = UiAction::Factory(Action::SetRecipe(pos, i as u32));
            }
        }
    });
}

pub(super) fn stock_reserve(ui: &mut egui::Ui, pos: IVec3, node: &Node, view: Option<crate::world::factory::stock_order::View>, action: &mut UiAction) {
    use crate::world::factory::stock_order::{self, Order};
    let outputs = stock_order::outputs(node.kind, node.mode);
    let Some(&first) = outputs.first() else { return; };
    let active = stock_order::read(node.kind, node.mode, node.target);
    egui::CollapsingHeader::new("Maintain Stock").default_open(active.is_some()).show(ui, |ui| {
        let key = ui.id().with((pos, node.mode));
        let default = active.unwrap_or(Order { item: first, count: 8 });
        let mut draft = ui.ctx().data(|d| d.get_temp::<(u32, Order)>(key))
            .filter(|(target, _)| *target == node.target).map_or(default, |(_, draft)| draft);
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("reserve_item").selected_text(NAMES[draft.item]).show_ui(ui, |ui| {
                for &item in outputs {
                    ui.selectable_value(&mut draft.item, item, NAMES[item]);
                }
            });
            for count in stock_order::AMOUNTS {
                ui.selectable_value(&mut draft.count, count, count.to_string());
            }
            if ui.add_enabled(active.is_none_or(|order| order.target() != draft.target()) || !node.enabled,
                egui::Button::new("Set Reserve")).clicked()
            {
                *action = UiAction::Factory(Action::SetTarget(pos, draft.target()));
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(key, (node.target, draft)));
        if let Some(order) = active {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{} × {} · {}", order.count, NAMES[order.item], node.status));
                if ui.small_button("Clear & Pause").clicked() {
                    *action = UiAction::Factory(Action::SetTarget(pos, 0));
                }
            });
            if let Some(view) = view {
                let amount = |units| super::icons::amount_text(units as f32 / 64.0);
                ui.label(format!("{} / {} stored · {} in transit", amount(view.stored), order.count, amount(view.incoming)));
            }
            if order.item == crate::world::factory::ETHANOL {
                let seconds = crate::player::traversal::BURN_SECONDS * order.count as f32;
                ui.weak(format!("{seconds:.0} s burner jetpack thrust"));
            }
        }
        ui.weak("Item pipes → storage filtered to the product");
    });
}

fn battery_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    inventory: &Expedition,
    orbital_cell: bool,
    action: &mut UiAction,
) {
    ui.set_width(380.0);
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new("Battery bank").size(19.0).color(icons::PAPER));
            ui.label(
                RichText::new(format!(
                    "{} · {} · up to {}",
                    power::battery_tier_name(node.upgrade),
                    node.status,
                    power::fmt_power(power::battery_rate(node.upgrade) as f64)
                ))
                .color(icons::TEAL),
            );
        });
    });
    let cap = power::battery_capacity(node.upgrade);
    meter(
        ui,
        node.energy as f32 / cap as f32,
        icons::TEAL,
        &format!(
            "{} / {}  ({:.0}%)",
            power::fmt_energy(node.energy as f64),
            power::fmt_energy(cap as f64),
            node.energy as f32 * 100.0 / cap as f32
        ),
    );
    power_section(ui, node, grid);
    let max = power::battery_rate(node.upgrade);
    let mut limit = if node.target == 0 { max } else { node.target };
    ui.horizontal(|ui| {
        ui.label(RichText::new("I/O LIMIT").size(13.0).color(MUTED));
        if ui
            .add(egui::Slider::new(&mut limit, 1..=max).suffix(" kW"))
            .on_hover_text("Maximum charge and discharge rate")
            .changed()
        {
            *action = UiAction::Factory(Action::SetTarget(pos, if limit >= max { 0 } else { limit }));
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new("BEHAVIOUR").size(13.0).color(MUTED));
    mode_picker(ui, pos, node, action);
    if let Some(text) = node.kind.modes().get(node.mode as usize) {
        ui.small(*text);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(if node.enabled { "Isolate" } else { "Connect" }).on_hover_text("An isolated bank neither charges nor discharges").clicked() {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
        upgrade_button(ui, pos, node, inventory, action);
        if node.upgrade < power::MITHRIL_TIER && (orbital_cell || power::accepts_mithril(node.upgrade))
            && ui.add_enabled(orbital_cell && power::accepts_mithril(node.upgrade), egui::Button::new("Install Mithril"))
                .on_hover_text("Orbital Mithril ×1 · Capacity 64 kWh")
                .on_disabled_hover_text(if orbital_cell { "Upgrade to Bronze Cells" } else { "Orbital Mithril Required" })
                .clicked()
        {
            *action = UiAction::Factory(Action::InstallMithril(pos));
        }
    });
}

fn generator_power_visible(node: &Node) -> bool {
    !crate::world::factory::industry::boiler::active(node) || node.energy > 0
}

fn generator_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    ui.set_width(400.0);
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new(node.kind.name()).size(19.0).color(icons::PAPER));
            ui.label(RichText::new(status_label(node.status)).color(if node.status == "Generating" {
                icons::TEAL
            } else {
                icons::GOLD
            }));
        });
    });
    recovery_hint(ui, node);
    if node.kind == Kind::Generator {
        mode_picker(ui, pos, node, action);
    }
    let pile = pile::is_pile(node);
    let boiler = crate::world::factory::industry::boiler::active(node);
    let passive = (node.kind == Kind::Generator).then(|| power::passive_rating(node.mode)).flatten();
    let rate = if boiler {
        crate::world::industry::config::CONFIG.fired_boiler.max_fuel_per_tick as f64
    } else if let Some(rate) = passive {
        rate as f64
    } else if node.kind == Kind::Bioreactor {
        32.0
    } else if pile {
        pile::RATE as f64
    } else if node.kind == Kind::Generator && node.mode == 2 {
        power::BIOMASS_RATE as f64
    } else {
        16.0
    };
    ui.label(
        RichText::new(format!("{} {}", if boiler { "Thermal Input Limit" } else { "Rated output" }, power::fmt_power(rate)))
            .small()
            .color(MUTED),
    );
    if boiler {
        let b = &crate::world::industry::config::CONFIG.fired_boiler;
        ui.small(format!("Steam Limit {:.3} g/s", b.steam_mg_per_energy as f64 * rate / power::TICK_SECONDS / 1000.0));
        ui.small(format!("Steam {} mg / 250 J", b.steam_mg_per_energy));
    }
    if pile {
        let heat = node.progress as f32 / pile::MAX_HEAT as f32;
        meter(
            ui,
            heat,
            if node.progress >= pile::WARN_HEAT { Color32::from_rgb(235, 80, 60) } else { icons::GOLD },
            &format!("Core heat {} / {}", node.progress, pile::MAX_HEAT),
        );
        meter(
            ui,
            node.water as f32 / 8192.0,
            icons::TEAL,
            &format!("Coolant {:.0} / 16000 L", node.water as f32 * 1000.0 / 512.0),
        );
    }
    if passive.is_none() {
        meter(
            ui,
            node.burn as f32 / power::MAX_COMBUSTION_BURN.max(pile::CAPACITY) as f32,
            Color32::from_rgb(230, 120, 50),
            &format!(
                "Burning {} · {:.0} s left",
                power::fmt_joules(node.burn as f64),
                node.burn as f64 / rate * power::TICK_SECONDS
            ),
        );
    }
    if generator_power_visible(node) {
        meter(
            ui,
            node.energy as f32 / power::GENERATOR_BUFFER as f32,
            icons::GOLD,
            &format!(
                "Buffer {} / {}",
                power::fmt_energy(node.energy as f64),
                power::fmt_energy(power::GENERATOR_BUFFER as f64)
            ),
        );
        power_section(ui, node, grid);
    }
    ui.add_space(4.0);
    if node.kind.water_port() && (boiler || node.water > 0 || node.separator_water_mg > 0) {
        let mass = crate::world::factory::industry::turbine::condensate(node);
        ui.small(format!("Water {}", industry::condensate_volume(mass)));
        if mass > 0 {
            let label = if node.water == 0 { "Bottle Remaining Water" }
                else if node.draining { "Close Drain" } else { "Drain Water" };
            if ui.button(label).clicked() { *action = UiAction::Factory(Action::Drain(pos)); }
        }
    }
    if ui.button(if node.enabled { "Pause" } else { "Start" }).clicked() {
        *action = UiAction::Factory(Action::Toggle(pos));
    }
    if passive.is_some() {
        return;
    }
    ui.label(RichText::new("FUEL").size(13.0).color(MUTED));
    egui::Grid::new(("fuel", pos)).spacing([8.0, 2.0]).show(ui, |ui| {
        let fuels: Vec<(usize, u32, u32)> = if node.kind == Kind::Bioreactor {
            vec![(13, 32, 800)]
        } else if pile {
            vec![(pile::FUEL, pile::PORTION, pile::CAPACITY)]
        } else if node.kind == Kind::Generator && node.mode == 2 {
            vec![power::BIOMASS_FUEL]
        } else {
            power::COMBUSTION_FUELS.to_vec()
        };
        for (fuel, portion, units) in fuels {
            let per_item = units as f64 * 64.0 / portion as f64;
            ui.horizontal(|ui| {
                let (r, response) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                icons::paint(ui.painter(), r, Icon::Resource(fuel));
                InputTransfer { pos, node, inventory, action }.slot(response, fuel, node.input[fuel]);
                ui.label(NAMES[fuel]);
            });
            ui.monospace(stack_text(node.input[fuel]));
            ui.label(
                RichText::new(format!(
                    "{} / item · {:.0} s",
                    power::fmt_joules(per_item),
                    per_item / rate * power::TICK_SECONDS
                ))
                .small()
                .color(MUTED),
            );
            ui.end_row();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Load All Fuel").on_hover_text("Vent combustion exhaust outdoors").clicked() {
            *action = UiAction::Factory(Action::Load(pos));
        }
        if ui.button("Unload").clicked() {
            *action = UiAction::Factory(Action::Collect(pos));
        }
    });
}

/// Top-down plan of one course of a shell: `#` brick, `D` controller,
/// `·` the course's interior when hollow is allowed.
fn shell_plan(ui: &mut egui::Ui, kind: Kind) {
    let (courses, controller_course) = if kind == Kind::BlastFurnace { (4, 1) } else { (3, 1) };
    let brick = if kind == Kind::BlastFurnace { "blast brick" } else { "coke oven brick" };
    ui.label(
        RichText::new(format!(
            "BUILD · 3 × {courses} × 3 of {brick}, {} total. The {} sits in the middle of one wall, course {} from the bottom; pipes attach to its outer face.",
            multiblock::bricks_needed(kind),
            if kind == Kind::BlastFurnace { "tuyere" } else { "door" },
            controller_course + 1
        ))
        .small()
        .color(MUTED),
    );
    ui.horizontal(|ui| {
        for course in 0..courses {
            let (r, _) = ui.allocate_exact_size(vec2(44.0, 50.0), Sense::hover());
            let p = ui.painter();
            for z in 0..3 {
                for x in 0..3 {
                    let cell = Rect::from_min_size(r.min + vec2(x as f32 * 13.0 + 2.0, z as f32 * 13.0 + 2.0), vec2(12.0, 12.0));
                    let controller = course == controller_course && x == 1 && z == 2;
                    p.rect_filled(
                        cell,
                        0,
                        if controller {
                            Color32::from_rgb(240, 150, 60)
                        } else if kind == Kind::BlastFurnace {
                            Color32::from_rgb(110, 92, 84)
                        } else {
                            Color32::from_rgb(140, 82, 56)
                        },
                    );
                }
            }
            p.text(
                r.center_bottom() - vec2(0.0, 2.0),
                Align2::CENTER_BOTTOM,
                format!("course {}", course + 1),
                FontId::proportional(10.0),
                icons::PAPER,
            );
        }
    });
}

fn multiblock_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    ui.set_width(420.0);
    let title = if node.kind == Kind::CokeOven { "Coke oven" } else { "Blast furnace" };
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(19.0).color(icons::PAPER));
            let (found, needed) = node.structure;
            ui.label(
                RichText::new(if node.formed {
                    format!("{} · shell formed", node.status)
                } else {
                    format!("Incomplete shell: {found} / {needed} bricks")
                })
                .color(if node.formed { icons::TEAL } else { icons::GOLD }),
            );
        });
    });
    let t = multiblock::temperature(node);
    let target = multiblock::working_temperature(node.kind, node.mode);
    let max = if node.kind == Kind::CokeOven { multiblock::COKE_OVEN_MAX_C } else { multiblock::BLAST_MAX_C };
    meter(
        ui,
        t as f32 / max as f32,
        if t >= target { Color32::from_rgb(240, 120, 50) } else { icons::GOLD },
        &format!("Hearth {t} °C · needs {target} °C"),
    );
    if node.kind == Kind::BlastFurnace {
        mode_picker(ui, pos, node, action);
    }
    if let Some(process) = recipes::process(node.kind, node.mode as usize) {
        ui.horizontal_wrapped(|ui| {
            for &(i, n) in process.inputs.iter().filter(|&&(_, n)| n > 0) {
                icons::slot(ui, Icon::Resource(i), Some(n), false, NAMES[i]);
            }
            ui.label(RichText::new("→").size(22.0).color(icons::GOLD));
            icons::slot(ui, Icon::Resource(process.output.0), Some(process.output.1), true, NAMES[process.output.0]);
            if let Some((side, amount)) = process.byproduct {
                icons::slot(ui, Icon::Resource(side), Some(amount), false, NAMES[side]);
            }
        });
        let progress = node.progress as f32 / process.ticks as f32;
        meter(
            ui,
            progress,
            icons::TEAL,
            &format!(
                "{:.0}% · {:.1} s of {:.0} s",
                progress * 100.0,
                (process.ticks - node.progress.min(process.ticks)) as f32 * 0.25,
                process.ticks as f32 * 0.25
            ),
        );
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(200.0);
            ui.small("Charge");
            let inputs: Vec<_> = (0..NAMES.len())
                .filter(|&i| node.accepts(i) || node.input[i] > 0)
                .map(|i| (i, node.input[i]))
                .collect();
            supply_grid(ui, "mb-in", &inputs, Some(InputTransfer { pos, node, inventory, action }));
        });
        ui.vertical(|ui| {
            ui.small("Output");
            let outputs: Vec<_> = (0..NAMES.len())
                .filter(|&i| node.output[i] > 0)
                .map(|i| (i, node.output[i]))
                .collect();
            supply_grid(ui, "mb-out", &outputs, None);
        });
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Load All").clicked() {
            *action = UiAction::Factory(Action::Load(pos));
        }
        if ui.button("Unload").clicked() {
            *action = UiAction::Factory(Action::Collect(pos));
        }
        if ui.button(if node.enabled { "Bank the fire" } else { "Light" }).clicked() {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
    });
    if !node.formed {
        shell_plan(ui, node.kind);
    }
    let _ = inventory;
}

fn induction_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    pool: Option<(u32, u32, u32)>,
    action: &mut UiAction,
) {
    ui.set_width(400.0);
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new("Induction matrix").size(19.0).color(icons::PAPER));
            if node.formed {
                ui.label(
                    RichText::new(format!(
                        "{} · {} cells · {} providers",
                        node.status, node.structure.0, node.structure.1
                    ))
                    .color(icons::TEAL),
                );
            } else {
                ui.label(
                    RichText::new(format!(
                        "Not formed: {}",
                        induction::problem(node.structure.1)
                    ))
                    .color(icons::GOLD),
                );
            }
        });
    });
    if let Some((stored, cap, rate)) = pool {
        meter(
            ui,
            stored as f32 / cap.max(1) as f32,
            icons::TEAL,
            &format!(
                "{} / {}  ({:.1}%)",
                power::fmt_energy(stored as f64),
                power::fmt_energy(cap as f64),
                stored as f64 * 100.0 / cap.max(1) as f64
            ),
        );
        ui.small(format!("Transfer limit {} across all ports (one provider = 256 kW).", power::fmt_power(rate as f64)));
    }
    power_section(ui, node, grid);
    ui.label(RichText::new("PORT").size(13.0).color(MUTED));
    mode_picker(ui, pos, node, action);
    ui.small("Hollow casing: 3-9 m per side. Add wall ports, storage cells and providers.");
}

fn transformer_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    pool: Option<(u32, u32, u32)>,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    ui.set_width(380.0);
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new("Transformer").size(19.0).color(icons::PAPER));
            ui.label(
                RichText::new(format!(
                    "{} · up to {}",
                    node.status,
                    power::fmt_power(power::transformer_rate(node.upgrade) as f64)
                ))
                .color(icons::TEAL),
            );
        });
    });
    if let Some((stored, cap, _)) = pool {
        meter(ui, stored as f32 / cap.max(1) as f32, icons::GOLD, &format!("Buffer {}", power::fmt_joules(stored as f64)));
    }
    power_section(ui, node, grid);
    ui.small("Connect two cable networks to transfer surplus power.");
    ui.horizontal(|ui| {
        if ui.button(if node.enabled { "Open breaker" } else { "Close breaker" }).clicked() {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
        upgrade_button(ui, pos, node, inventory, action);
    });
}

fn cable_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    ui.set_width(380.0);
    ui.label(RichText::new(power::cable_tier_name(node.upgrade)).size(19.0).color(icons::PAPER));
    let cap = power::cable_capacity(node.upgrade);
    let used = grid.map(|g| g.throughput).unwrap_or(0);
    meter(
        ui,
        used as f32 / cap as f32,
        if used >= cap { Color32::from_rgb(235, 80, 60) } else { icons::TEAL },
        &format!("Carrying {} of {}", power::fmt_power(used as f64), power::fmt_power(cap as f64)),
    );
    if let Some(g) = grid {
        ui.small(format!(
            "{} generators · {} machines · {} storage ports on this line",
            g.generators, g.machines, g.batteries
        ));
    }
    if node.upgrade < 2 {
        let cost = Kind::Cable.upgrade_cost(node.upgrade);
        if ui
            .add(egui::Button::new(format!("Upgrade line → {}", power::cable_tier_name(node.upgrade + 1))))
            .on_hover_ui(|ui| {
                ui.small("Per cable piece:");
                ingredients(ui, cost, inventory);
                ui.small(format!(
                    "Every connected piece of this tier is upgraded while materials last. {} capacity. Different tiers no longer connect: bridge them with a transformer.",
                    power::fmt_power(power::cable_capacity(node.upgrade + 1) as f64)
                ));
            })
            .clicked()
        {
            *action = UiAction::Factory(Action::UpgradeMachine(pos));
        }
    }
}

fn assembler_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &Node,
    grid: Option<NetStats>,
    inventory: &Expedition,
    action: &mut UiAction,
) {
    use crate::world::inventory::{Item, all_recipes};
    ui.set_width(440.0);
    let recipe = assembler::recipe(node);
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
        ui.vertical(|ui| {
            ui.label(RichText::new("Assembling machine").size(19.0).color(icons::PAPER));
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    node.status,
                    recipe.map(|r| r.name).unwrap_or("no recipe")
                ))
                .color(icons::TEAL),
            );
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Production");
        for (mode, label) in node.kind.modes().iter().enumerate() {
            if ui.selectable_label(node.mode == mode as u32, *label).clicked() {
                *action = UiAction::Factory(Action::SetRecipe(pos, mode as u32));
            }
        }
        let start = if node.enabled { "Pause" }
            else if node.mode == assembler::SINGLE_BATCH {
                if node.progress > 0 { "Resume Batch" } else { "Craft Batch" }
            } else { "Start" };
        if ui.add_enabled(recipe.is_some() || node.enabled, egui::Button::new(start)).clicked() {
            *action = UiAction::Factory(Action::Toggle(pos));
        }
    });
    if node.mode == assembler::SINGLE_BATCH && let Some(recipe) = recipe {
        ui.small(format!("Batch Output: {}x {}", recipe.output.count, recipe.output.item.name()));
    }
    if let Some(r) = recipe {
        ui.horizontal_wrapped(|ui| {
            for &(item, n) in r.ingredients {
                if let Item::Resource(i) = item {
                    icons::slot(ui, Icon::Resource(i as usize), Some(node.input[i as usize]), false, &format!("{} · needs {n} per craft", NAMES[i as usize]));
                }
            }
            for (material, count) in assembler::block_needs(r) {
                ui.label(format!("{} {}/{}", material.item().name(), node.blocks.input[material.index()], count));
            }
            ui.label(RichText::new("→").size(22.0).color(icons::GOLD));
            match r.output.item {
                Item::Resource(i) => {
                    icons::slot(ui, Icon::Resource(i as usize), Some(node.output[i as usize]), true, NAMES[i as usize]);
                }
                item => { ui.label(format!("{} ×{}", item.name(), node.stock)); }
            }
        });
        let progress = node.progress as f32 / assembler::CRAFT_TICKS as f32;
        meter(
            ui,
            progress,
            icons::TEAL,
            &format!(
                "{:.0}% · {:.2} s · {} / craft",
                progress * 100.0,
                assembler::CRAFT_TICKS.div_ceil(1 + node.upgrade) as f32 * 0.25,
                power::fmt_joules((assembler::POWER * assembler::CRAFT_TICKS) as f64)
            ),
        );
    }
    power_section(ui, node, grid);
    ui.label(RichText::new("RECIPE").size(13.0).color(MUTED));
    let search_id = ui.id().with(("assembler_search", pos));
    let mut query = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
    ui.add(egui::TextEdit::singleline(&mut query).hint_text("Filter recipes").desired_width(200.0));
    let q = query.to_lowercase();
    egui::ScrollArea::vertical().id_salt(("assembler_recipes", pos)).max_height(150.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for i in assembler::choices() {
                let r = &all_recipes()[i];
                if !q.is_empty() && !r.name.to_lowercase().contains(&q) {
                    continue;
                }
                let on = node.target == i as u32 + 1;
                if ui.selectable_label(on, r.name).clicked() {
                    *action = UiAction::Factory(Action::SetTarget(pos, if on { 0 } else { i as u32 + 1 }));
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(search_id, query));
    ui.horizontal_wrapped(|ui| {
        if ui.button("Load").on_hover_text("Input Transfer").clicked() {
            *action = UiAction::Factory(Action::Load(pos));
        }
        let collect = if node.stock > 0 { format!("Collect {}", node.stock) } else { "Unload".to_string() };
        if ui.button(collect).clicked() {
            *action = UiAction::Factory(Action::Collect(pos));
        }
        upgrade_button(ui, pos, node, inventory, action);
    });
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    view: &View,
    inventory: &Expedition,
    adventure: &crate::world::adventure::View,
    action: &mut UiAction,
    tab: u32,
) {
    if tab == 3 {
        if let Some((pos, node)) = &view.focused {
            super::presentation::hud_frame(ui.ctx()).show(ui,|ui| {
            if crate::world::factory::sides::is_conduit(node.kind) {
                conduit_faces(ui, *pos, node, view.focused_links, action);
            }
            if crate::world::factory::industry::component(node.kind) {
                industry::panel(ui,*pos,node,view,action);
                upgrade_button(ui,*pos,node,inventory,action);return;
            }
            industry::readings(ui,view);
            if crate::world::factory::industry::port(node.kind) {
                ui.collapsing("Gas outlet", |ui| {
                    industry::gas_slots(ui, node);
                    industry::outlet_controls(ui, *pos, node, action);
                });
            }
            if node.kind == Kind::Storage {
                container_panel(ui, *pos, node, action);
                return;
            }
            if node.kind == Kind::Pipe {
                pipe_panel(ui, *pos, node, action);
                return;
            }
            if matches!(node.kind, Kind::Furnace | Kind::AlloyKiln) {
                smelter_panel(ui, *pos, node, inventory, action);
                power_section(ui, node, view.focused_grid);
                super::chip_panel::section(ui, view, *pos, action);
                face_row(ui, *pos, node, action);
                return;
            }
            if node.kind == Kind::Battery {
                battery_panel(ui, *pos, node, view.focused_grid, inventory, view.orbital_cell, action);
                return;
            }
            if node.kind == Kind::InductionPort {
                induction_panel(ui, *pos, node, view.focused_grid, view.focused_pool, action);
                return;
            }
            if node.kind == Kind::Transformer {
                transformer_panel(ui, *pos, node, view.focused_grid, view.focused_pool, inventory, action);
                return;
            }
            if node.kind == Kind::Cable {
                cable_panel(ui, *pos, node, view.focused_grid, inventory, action);
                return;
            }
            if node.kind == Kind::Assembler {
                assembler_panel(ui, *pos, node, view.focused_grid, inventory, action);
                face_row(ui, *pos, node, action);
                return;
            }
            if node.kind.generator() {
                generator_panel(ui, *pos, node, view.focused_grid, inventory, action);
                if crate::world::factory::chips::controllable(node.kind) {
                    super::chip_panel::section(ui, view, *pos, action);
                }
                face_row(ui, *pos, node, action);
                return;
            }
            if matches!(node.kind, Kind::CokeOven | Kind::BlastFurnace) {
                multiblock_panel(ui, *pos, node, inventory, action);
                face_row(ui, *pos, node, action);
                return;
            }
            if node.kind.structural() {
                ui.horizontal(|ui| {
                    icons::slot(ui, Icon::Machine(node.kind), None, true, detail(node.kind));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(node.kind.name()).size(19.0).color(icons::PAPER));
                        ui.label(RichText::new(if node.formed { "Structure Complete" } else { "Structure Incomplete" }).color(if node.formed { icons::TEAL } else { icons::GOLD }));

                    });
                });
                return;
            }
            ui.horizontal(|ui| {
                icons::slot(ui,Icon::Machine(node.kind),None,true,detail(node.kind));
                ui.vertical(|ui| {
                    ui.label(RichText::new(node.kind.name()).size(20.0).color(icons::PAPER));
                    let status = if node.kind == Kind::Quarry && !node.enabled { "Paused" } else { status_label(node.status) };
                    ui.label(RichText::new(status).color(if status == "Working" || status == "Generating" {icons::TEAL}else{icons::GOLD}));
                    if node.kind.powered() {power_section(ui,node,view.focused_grid);}
                    if node.kind == Kind::SmelteryController {
                        ui.small(if node.formed { "Shell Complete" } else { "Shell Incomplete" });
                        let fuel = node.burn as f32 / crate::world::smeltery::DEF.fuel_ticks as f32;
                        icons::gauge(ui, fuel, icons::GOLD, "Fuel");
                    }
                    if node.kind==Kind::Generator && node.mode==1 {
                        let heat=node.progress as f32/crate::world::factory::pile::MAX_HEAT as f32;
                        icons::gauge(ui,heat,if node.progress>=crate::world::factory::pile::WARN_HEAT {egui::Color32::from_rgb(235,80,60)} else {icons::GOLD},&format!("Core heat {} / {}",node.progress,crate::world::factory::pile::MAX_HEAT));
                    }
                    if node.kind.water_port() && (node.kind!=Kind::Generator || node.mode==1) {icons::gauge(ui,node.water as f32/8192.0,icons::TEAL,&format!("{:.0} / 16000 L {}",node.water as f32*1000.0/512.0,if node.kind==Kind::Generator {"coolant"} else {"water"}));}
                });
            });
            recovery_hint(ui, node);
            if crate::world::factory::chips::controllable(node.kind) {
                super::chip_panel::section(ui, view, *pos, action);
            }
            let lab_owned = view.fabrication.owns((pos.x, pos.y, pos.z));
            let sluice = placer::sluice(node);
            if node.kind == Kind::Washer && (sluice || node.placer.has_feed()) {
                let mut feed = node.placer.feed;
                feed.add(node.placer.batch);
                sediment_buffer(ui, feed);
                if sluice {
                    let ticks = placer::sluice_ticks(node.placer.batch);
                    icons::gauge(ui, node.progress as f32 / ticks as f32, icons::TEAL, "Batch Progress");
                }
            }
            let grass_recipe = if node.kind == Kind::Cultivator { biomatter::recipe(node.mode) } else { None };
            if let Some(recipe) = grass_recipe {
                grass_process(ui, node, recipe);
            }
            let selected_process = node.process().or_else(|| {
                if node.kind == Kind::Fabricator {
                    recipes::process(node.kind, node.mode as usize)
                } else {
                    None
                }
            });
            if let Some(recipe) = selected_process.filter(|_| !lab_owned) {
                ui.horizontal_wrapped(|ui| {
                    if let Some((material, count)) = recipe.block_cost(view.cells_per_metre) {
                        block_slot(ui, material, count);
                    }
                    for &(i,n) in &recipe.inputs {if n>0 {icons::slot(ui,Icon::Resource(i),Some(n),false,NAMES[i]);}}
                    ui.label(RichText::new("→").size(26.0).color(icons::GOLD));
                    icons::slot(ui,Icon::Resource(recipe.output.0),Some(recipe.output.1),true,NAMES[recipe.output.0]);
                });
                icons::gauge(ui,node.progress as f32/recipe.ticks as f32,icons::TEAL,"Batch progress");
                if node.kind == Kind::Fabricator {
                    ui.small(format!(
                        "Base cycle {:.2} s · {}",
                        recipe.ticks as f64 * power::TICK_SECONDS,
                        power::fmt_power(recipe.power as f64),
                    ));
                }
            }
            if node.kind.excavates() {
                quarry_settings(ui, *pos, node, view.cells_per_metre, action);
            } else if !lab_owned {
                mode_picker(ui, *pos, node, action);
            }
            if !lab_owned {
                stock_reserve(ui, *pos, node, view.stock_reserve, action);
            }
            if !node.kind.conduit() {
                ui.horizontal_top(|ui| {
                    if !node.kind.excavates() {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() * 0.48).min(330.0));
                            ui.small("Input");
                            let inputs: Vec<_> = (0..NAMES.len())
                                .filter(|&i| node.accepts(i) || node.input[i] > 0)
                                .map(|i| (i, node.input[i]))
                                .collect();
                            if !inputs.is_empty() || (grass_recipe.is_none() && !sluice) {
                                let transfer = (node.kind != Kind::Feeder).then_some(InputTransfer {
                                    pos: *pos, node, inventory, action,
                                });
                                supply_grid(ui, "machine-in", &inputs, transfer);
                            }
                            let grass = node.blocks.input[Material::Grass.index()];
                            if grass_recipe.is_some() || grass > 0 {
                                block_slot(ui, Material::Grass, grass);
                            }
                            let paydirt = node.blocks.input[Material::Paydirt.index()];
                            if sluice || paydirt > 0 {
                                block_slot(ui, Material::Paydirt, paydirt);
                            }
                            if node.kind == Kind::Crusher || (node.kind == Kind::Washer && !sluice)
                                || node.blocks.input[Material::GoldQuartz.index()] > 0
                            {
                                block_slot(ui, Material::GoldQuartz, node.blocks.input[Material::GoldQuartz.index()]);
                            }
                        });
                    }
                    ui.vertical(|ui| {
                        ui.small("Output");
                        let outputs: Vec<_> = (0..NAMES.len())
                            .filter(|&i| node.output[i] > 0 || (sluice && i == GOLD_CONCENTRATE))
                            .map(|i| (i, node.output[i]))
                            .collect();
                        if !outputs.is_empty() || node.blocks.output.iter().all(|&n| n == 0) {
                            supply_grid(ui, "machine-out", &outputs, None);
                        }
                        ui.horizontal_wrapped(|ui| {
                            for &material in Material::ALL {
                                let count = node.blocks.output[material.index()];
                                if count > 0 || (sluice && material == Material::Sand) {
                                    block_slot(ui, material, count);
                                }
                            }
                        });
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    if !node.kind.excavates() && ui.button("Load All").clicked(){*action=UiAction::Factory(Action::Load(*pos));}
                    let collect = if node.kind.excavates() { "Collect" } else { "Unload" };
                    if ui.button(collect).on_hover_text("Output Transfer").clicked(){*action=UiAction::Factory(Action::Collect(*pos));}
                    if ui.button(if node.enabled {"Pause"}else{"Start"}).clicked(){*action=UiAction::Factory(Action::Toggle(*pos));}
                    if node.placer.has_feed() && ui.add_enabled(!node.enabled, egui::Button::new("Flush Sediment"))
                        .on_hover_text("Sediment → Sand · Gold Lost").clicked()
                    {
                        *action = UiAction::Factory(Action::FlushSediment(*pos));
                    }
                    if node.kind.water_port() && node.water>0
                        && ui.button(if node.draining {"Close drain"}else{"Drain water"}).on_hover_text("Fluid Outlet").clicked(){*action=UiAction::Factory(Action::Drain(*pos));}
                    upgrade_button(ui,*pos,node,inventory,action);
                });
                face_row(ui, *pos, node, action);
            }
            if matches!(node.kind,Kind::Feeder|Kind::Storage) || (node.kind==Kind::Pipe && node.mode==1) {
                ui.collapsing("Filter",|ui|{ui.horizontal_wrapped(|ui|{
                    if ui.selectable_label(node.filter.is_none(),"Any").clicked(){*action=UiAction::Factory(Action::SetFilter(*pos,None));}
                    for (i,name) in NAMES.iter().enumerate(){if icons::slot(ui,Icon::Resource(i),None,node.filter==Some(i),name).clicked(){*action=UiAction::Factory(Action::SetFilter(*pos,Some(i)));}}
                });});
            }
        });
        } else {
            ui.label("No Selection");
        }
        return;
    }
    if tab == 0 {
        if let Some(progress) = &view.workshop {
            ui.strong("Workshop");
            for (name, done) in [("Guardian Vault", progress.vault), ("Workshop Reclaimed", progress.reclaimed), ("Iron Delivery", progress.delivery)] {
                ui.horizontal(|ui| {
                    ui.label(if done { "✓" } else { "○" });
                    ui.label(name);
                });
            }
            if !progress.delivery {
                ui.label("Electric Furnace → Item Pipe → Storage");
                super::production::iron_line(ui, view);
            }
            if progress.reclaimed && progress.delivery { ui.strong("Complete"); }
            ui.separator();
        }
        super::presentation::hud_frame(ui.ctx()).show(ui, |ui| {
            ui.label(
                RichText::new("Field supplies")
                    .size(18.0)
                    .color(icons::GOLD),
            );
            ui.columns(3, |columns| {
                {
                    let ui = &mut columns[0];
                    icons::slot(
                        ui,
                        Icon::Sword(1),
                        None,
                        adventure.weapons & 1 != 0,
                        "Copper sword · balanced melee weapon",
                    );
                    ui.label("Copper sword");
                    ingredients(ui, crate::world::adventure::sword_recipe(1), inventory);
                    if ui
                        .add_enabled(
                            view.bench
                                && adventure.weapons & 1 == 0
                                && affordable(inventory, crate::world::adventure::sword_recipe(1)),
                            egui::Button::new("Craft copper sword"),
                        )
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::CraftSword(1));
                    }
                }
                {
                    let ui = &mut columns[1];
                    icons::slot(
                        ui,
                        Icon::Sword(2),
                        None,
                        adventure.weapons & 2 != 0,
                        "Steel sword · heavy melee weapon",
                    );
                    ui.label("Steel sword");
                    ingredients(ui, crate::world::adventure::sword_recipe(2), inventory);
                    if ui
                        .add_enabled(
                            view.bench
                                && adventure.weapons & 2 == 0
                                && affordable(inventory, crate::world::adventure::sword_recipe(2)),
                            egui::Button::new("Craft steel sword"),
                        )
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::CraftSword(2));
                    }
                }
                {
                    let ui = &mut columns[2];
                    ui.label("Fungal rations ×2");
                    ingredients(ui, crate::world::adventure::ration_recipe(), inventory);
                    ui.small("Restores hunger and a little health.");
                    if ui
                        .add_enabled(
                            view.bench
                                && adventure.rations <= 65533
                                && affordable(inventory, crate::world::adventure::ration_recipe()),
                            egui::Button::new("Prepare rations"),
                        )
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::CookRations);
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                icons::slot(
                    ui,
                    Icon::Sword(5),
                    None,
                    adventure.weapons & 16 != 0,
                    "Kinetic maul · heavy impact weapon with a physics shockwave",
                );
                ui.vertical(|ui| {
                    ui.label("Kinetic maul");
                    ingredients(ui, crate::world::adventure::sword_recipe(5), inventory);
                    ui.small(
                        "Third hit knocks back loose objects.",
                    );
                    if ui
                        .add_enabled(
                            view.bench
                                && adventure.weapons & 16 == 0
                                && affordable(inventory, crate::world::adventure::sword_recipe(5)),
                            egui::Button::new("Forge kinetic maul"),
                        )
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::CraftSword(5));
                    }
                });
            });
            if !view.bench {
                ui.small("F at a workbench to craft weapons and prepare food.");
            }
        });
        ui.separator();
        ui.label(
            RichText::new("Workshop kits")
                .size(17.0)
                .color(icons::PAPER),
        );
        let id = ui.id().with("selected_recipe");
        let mut selected = ui
            .ctx()
            .data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0))
            .min(Kind::ALL.len() - 1);
        ui.horizontal_wrapped(|ui| {
            for (i, kind) in Kind::ALL.into_iter().enumerate() {
                if icons::slot(
                    ui,
                    Icon::Machine(kind),
                    Some(view.kits[i] * 64),
                    i == selected,
                    kind.name(),
                )
                .clicked()
                {
                    selected = i;
                }
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, selected));
        let kind = Kind::ALL[selected];
        ui.add_space(8.0);
        ui.label(RichText::new(kind.name()).size(20.0).color(icons::GOLD))
            .on_hover_text(detail(kind));
        ingredients(ui, kind.recipe(), inventory);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    view.bench && affordable(inventory, kind.recipe()),
                    egui::Button::new(format!("Craft ×{}", kind.yield_count())),
                )
                .clicked()
            {
                *action = UiAction::Factory(Action::Craft(kind));
            }
            if ui
                .add_enabled(view.kits[selected] > 0, egui::Button::new("Equip kit"))
                .clicked()
            {
                *action = UiAction::Factory(Action::Select(kind));
            }
            ui.label(RichText::new("?").color(icons::TEAL))
                .on_hover_text(detail(kind));
            if !view.bench {
                ui.small("F at a workbench to craft");
            }
        });
        ui.add_space(8.0);
    } else if tab == 1 {
        super::wands::draw(ui, adventure, action);
        ui.collapsing("Equipment", |ui| {
        ui.separator();
        super::relics::codex(ui, adventure);
        ui.label(RichText::new("Weapons").size(18.0).color(icons::GOLD));
        let mut owns_any = false;
        for tier in 1..=5 {
            if adventure.weapons & (1 << (tier - 1)) == 0 {
                continue;
            }
            owns_any = true;
            ui.horizontal(|ui| {
                icons::slot(
                    ui,
                    Icon::Sword(tier),
                    None,
                    adventure.weapon == tier,
                    crate::world::adventure::sword_name(tier),
                );
                ui.vertical(|ui| {
                    let equipped = adventure.weapon == tier;
                    ui.label(if equipped {
                        crate::world::adventure::tinkering::display_name(
                            crate::world::adventure::sword_name(tier),
                            adventure.tool_head,
                            adventure.tool_bind,
                            adventure.tool_grip,
                        )
                    } else {
                        crate::world::adventure::sword_name(tier).to_owned()
                    });
                    if equipped {
                        ui.small(crate::world::adventure::tinkering::stat_summary(
                            adventure.tool_head,
                            adventure.tool_bind,
                            adventure.tool_grip,
                        ));
                        let traits = crate::world::adventure::tinkering::traits(
                            adventure.tool_head,
                            adventure.tool_bind,
                            adventure.tool_grip,
                        );
                        ui.horizontal_wrapped(|ui| {
                            for (name, detail) in traits {
                                ui.label(RichText::new(name).size(13.0).color(icons::TEAL))
                                    .on_hover_text(detail);
                            }
                        });
                    }
                    ui.small(match tier {
                        3 => "Third strike releases a flame cleave and restores mana",
                        4 => "Chill, then shatter; shards chill nearby enemies",
                        5 => {
                            "Third strike releases a radial impulse that launches loose assemblies"
                        }
                        2 => "High damage · recovered from wardens or crafted",
                        _ => "Balanced damage · crafted at a workbench",
                    });
                    if ui
                        .button(if adventure.weapon == tier {
                            "Put away"
                        } else {
                            "Equip"
                        })
                        .clicked()
                    {
                        *action =
                            UiAction::Factory(Action::SelectSword(if adventure.weapon == tier {
                                0
                            } else {
                                tier
                            }));
                    }
                });
            });
        }
        if !owns_any {
            ui.weak("No sword. Craft one or loot a warden.");
        }
        ui.separator();
        if adventure.wand < 3 {
            let cost = crate::world::adventure::wand_recipe(adventure.wand);
            ingredients(ui, cost, inventory);
            let label = match adventure.wand {
                0 => "Craft lumen wand",
                1 => "Add third spell slot",
                _ => "Add fourth spell slot",
            };
            if ui
                .add_enabled(
                    view.bench && affordable(inventory, cost),
                    egui::Button::new(label),
                )
                .clicked()
            {
                *action = UiAction::Factory(Action::Wand);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            icons::slot(ui, Icon::Bore, None, true, inventory.tool_name());
            ui.label(inventory.tool_name());
        });
        if inventory.tool_tier < 2 {
            let cost: &[(usize, u32)] = if inventory.tool_tier == 0 {
                &[(6, 128), (7, 64), (4, 128)]
            } else {
                &[(8, 256), (5, 128), (4, 128)]
            };
            ingredients(ui, cost, inventory);
            if ui
                .add_enabled(
                    view.bench && affordable(inventory, cost),
                    egui::Button::new("Upgrade bore"),
                )
                .clicked()
            {
                *action = UiAction::Factory(Action::Upgrade);
            }
        }
        ui.separator();
        ui.label(RichText::new(format!("{} sanctums cleared", adventure.defeated)).color(icons::GOLD))
            .on_hover_text("Clearing 1, 3 and 7 sanctums increases maximum health and mana, damage and regeneration.");
        let goal = match adventure.rank {
            0 => 1,
            1 => 3,
            _ => 7,
        };
        icons::gauge(
            ui,
            adventure.defeated as f32 / goal as f32,
            icons::GOLD,
            &format!("{} / {} sanctums", adventure.defeated, goal),
        );
        });
    } else {
        ui.label(RichText::new("Survival pack").size(18.0).color(icons::GOLD));
        icons::gauge(
            ui,
            adventure.hunger / 100.0,
            icons::TEAL,
            &format!("Hunger · {:.0}%", adventure.hunger),
        );
        ui.horizontal(|ui| {
            ui.label(format!("Fungal rations · {}", adventure.rations));
            if ui
                .add_enabled(
                    adventure.rations > 0 && adventure.hunger < 99.0,
                    egui::Button::new("Eat one"),
                )
                .clicked()
            {
                *action = UiAction::Factory(Action::EatRation);
            }
        });
        ui.small(
            "Eat to restore hunger and health. Hunger must stay above 75% for natural recovery.",
        );
        ui.separator();
        ui.label("Collected materials");
        let search_id = ui.id().with("pack_material_filter");
        let mut filter = ui
            .ctx()
            .data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.add(
            egui::TextEdit::singleline(&mut filter)
                .hint_text("Filter materials…")
                .desired_width(ui.available_width()),
        );
        ui.ctx()
            .data_mut(|d| d.insert_temp(search_id, filter.clone()));
        if inventory.resources.iter().all(|&n| n == 0) {
            ui.weak("No supplies yet. Mine with left mouse or open a salvage cache with F.");
        }
        let filter = filter.trim().to_lowercase();
        let groups: [(&str, &[usize]); 2] = [
            ("Mined and gathered", &[0, 1, 2, 3, 4, 5, 13][..]),
            (
                "Refined and relics",
                &[6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18, 19, 20, 21, GOLD_CONCENTRATE, crate::world::factory::GOLD_INGOT][..],
            ),
        ];
        let mut shown = 0;
        for (heading, ids) in groups {
            let entries: Vec<_> = ids
                .iter()
                .copied()
                .filter(|&i| {
                    inventory.resources[i] > 0 && NAMES[i].to_lowercase().contains(&filter)
                })
                .collect();
            if entries.is_empty() {
                continue;
            }
            shown += entries.len();
            egui::CollapsingHeader::new(format!("{} · {}", heading, entries.len()))
                .id_salt(heading)
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new(("supplies_list", heading))
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            for i in entries {
                                icons::slot(ui, Icon::Resource(i), None, false, NAMES[i]);
                                ui.label(NAMES[i]);
                                ui.monospace(format!(
                                    "{:.2}",
                                    inventory.resources[i] as f32 / 64.0
                                ));
                                ui.end_row();
                            }
                        });
                });
        }
        if shown == 0 && !filter.is_empty() {
            ui.weak("No collected materials match this search.");
        }
        if view.kits.iter().any(|&n| n > 0) {
            ui.separator();
            ui.label("Machine kits · click to equip");
            ui.horizontal_wrapped(|ui| {
                for (i, kind) in Kind::ALL.into_iter().enumerate() {
                    if view.kits[i] > 0
                        && icons::slot(
                            ui,
                            Icon::Machine(kind),
                            Some(view.kits[i] * 64),
                            false,
                            kind.name(),
                        )
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::Select(kind));
                    }
                }
            });
        }
    }
    ui.separator();
}
