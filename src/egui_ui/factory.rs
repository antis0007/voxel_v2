use super::{
    UiAction,
    icons::{self, Icon},
};
use crate::world::{
    expedition::{Expedition, NAMES},
    factory::{Action, Kind, View, recipes},
};
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, vec2};
use glam::IVec3;

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
    } else if n % 64 == 0 {
        format!("{}", n / 64)
    } else {
        format!("{:.1}", n as f32 / 64.0)
    }
}

/// Fixed cells. Empty compartments stay in the grid, so gaining or losing a
/// stack never changes the panel's size.
fn supply_grid(ui: &mut egui::Ui, salt: &str, slots: &[(usize, u32)]) {
    const SLOT: f32 = 36.0;
    const COLUMNS: usize = 6;
    let cells = slots.len().max(COLUMNS);
    let rows = cells.div_ceil(COLUMNS);
    egui::Grid::new(salt)
        .spacing([2.0, 2.0])
        .show(ui, |ui| {
            for index in 0..rows * COLUMNS {
                let (rect, _) = ui.allocate_exact_size(vec2(SLOT, SLOT), Sense::hover());
                icons::frame(ui.painter(), rect, false);
                if let Some(&(item, count)) = slots.get(index) {
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
                }
                if (index + 1) % COLUMNS == 0 {
                    ui.end_row();
                }
            }
        });
}

fn container_cell(
    ui: &mut egui::Ui,
    item: Option<usize>,
    count: u32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
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
    salt: impl std::hash::Hash,
    pos: IVec3,
    columns: usize,
    cells: usize,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    egui::Grid::new(("container", salt, pos))
        .spacing([2.0, 2.0])
        .show(ui, |ui| {
            for index in 0..cells {
                let item = (index < NAMES.len()).then_some(index);
                let count = item.map(|i| node.input[i]).unwrap_or(0);
                let response = container_cell(ui, item, count);
                if let Some(item) = item {
                    if response.clicked() && count > 0 {
                        *action = UiAction::Factory(Action::BufferMove(pos, item, 64, false));
                    } else if response.secondary_clicked() {
                        let (amount, deposit) = if ui.input(|i| i.modifiers.shift) && count > 0 {
                            (u32::MAX, false)
                        } else {
                            (64, true)
                        };
                        *action = UiAction::Factory(Action::BufferMove(pos, item, amount, deposit));
                    }
                }
                if (index + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

fn container_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    ui.set_width(360.0);
    ui.set_max_width(360.0);
    let (title, blurb) = match node.mode {
        1 => (
            "Chest",
            "27 slots. Left click takes a stack. Right click puts a stack in. Shift right-click takes the compartment.",
        ),
        2 => (
            "Barrel",
            "One item, in bulk. The first thing you put in locks it until it is empty.",
        ),
        _ => (
            "Crate",
            "Logistics crate. Filter it, then let the bus pipe in and pipe out.",
        ),
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
    let (hint, _) = ui.allocate_exact_size(vec2(360.0, 28.0), Sense::hover());
    ui.painter().with_clip_rect(hint).text(
        hint.left_top(),
        Align2::LEFT_TOP,
        blurb,
        FontId::proportional(11.0),
        icons::PAPER,
    );
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
                    .unwrap_or_else(|| "Empty barrel".to_string());
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
    resource_grid(ui, node.mode, pos, columns, cells, node, action);
    ui.label(RichText::new(node.status).color(icons::GOLD));
}

fn pipe_panel(
    ui: &mut egui::Ui,
    pos: IVec3,
    node: &crate::world::factory::Node,
    action: &mut UiAction,
) {
    ui.set_width(420.0);
    ui.set_max_width(420.0);
    ui.label(RichText::new("Item pipe").size(20.0).color(icons::PAPER));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        for (mode, label) in ["Pipe in", "Pipe out", "Distribute"].into_iter().enumerate() {
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
            if response.clicked() {
                *action = UiAction::Factory(Action::SetRecipe(pos, mode as u32));
            }
        }
    });
    let blurb = node
        .kind
        .modes()
        .get(node.mode as usize)
        .copied()
        .unwrap_or("Item pipe");
    let (hint, _) = ui.allocate_exact_size(vec2(420.0, 32.0), Sense::hover());
    ui.painter().with_clip_rect(hint).text(
        hint.left_top(),
        Align2::LEFT_TOP,
        blurb,
        FontId::proportional(11.0),
        icons::PAPER,
    );
    if node.mode != 0 {
        ui.label(RichText::new("FILTER").size(11.0).color(icons::GOLD));
        egui::Grid::new(("pipe-filter", pos))
            .spacing([2.0, 2.0])
            .show(ui, |ui| {
                for index in 0..20 {
                    let item = (index < NAMES.len()).then_some(index);
                    let selected = item.is_some_and(|i| node.filter == Some(i)) || (index == 19 && node.filter.is_none());
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
                    if (index + 1) % 10 == 0 {
                        ui.end_row();
                    }
                }
            });
    }
    ui.label(RichText::new(node.status).color(icons::GOLD));
}
fn detail(kind: Kind) -> &'static str {
    match kind {
        Kind::Generator => {
            "Coal furnace: 64 power/s. Arcane pile: 256 power/s from arcane cores, hot, needs piped coolant water."
        }
        Kind::Bioreactor => "Burns fungal biomass. 128 power/s. Cultivators supply renewable fuel.",
        Kind::Crusher => "1 ore → 2 dust. 4 seconds. 128 power per batch.",
        Kind::Furnace => "1 dust → 1 ingot. 3 seconds. 72 power per batch.",
        Kind::Feeder => {
            "Load supplies, then export through copper item pipes. A filter selects the exported resource."
        }
        Kind::Storage => {
            "Crate, chest or barrel. Pipe in fills it. Pipe out pulls from it. A distributor evens stock between containers on the line."
        }
        Kind::Pipe => {
            "Pipe in pushes outputs onward. Pipe out pulls from the neighbour. Distributor splits stock across chests, crates and barrels."
        }
        Kind::Cable => {
            "Teal power transport. Connect a continuous path from a generator to consumers."
        }
        Kind::AlloyKiln => {
            "Choose bronze or steel. Bronze opens components; steel opens advanced tools and extraction."
        }
        Kind::Fabricator => {
            "Choose induction coils or arcane cores. Cores require wild essence recovered from guardians."
        }
        Kind::Cultivator => {
            "Grow biomass with a reusable culture and lumen fibre, or turn biomass into fibre."
        }
        Kind::MiningDrill => {
            "Excavates a 3 × 3 metre shaft, 24 metres down. Requires power. Ore enters its output; waste rock is removed. Stops for players, structures, fluids and full output. Operates within 64 metres."
        }
        Kind::Pump => {
            "Place directly above water. Powered intake removes real water beneath its footprint. Stores up to 16 m³. Operates within 64 metres."
        }
        Kind::FluidPipe => {
            "Blue water transport. Pump → pipes → washer or tank. Water is conserved; paused machines block transfer."
        }
        Kind::Tank => {
            "Buffers 16 m³ of water. Pumps fill tanks; tanks supply washers through fluid pipes. Does not need power."
        }
        Kind::Washer => {
            "1 ore + 250 L water → 3 dust. 6 seconds, 288 power. Smelt dust for triple ore yield. Stops safely when water, power or output space runs out."
        }
    }
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
    let active = node.process();
    let next = active.or_else(|| {
        node.choose_recipe()
            .and_then(|id| recipes::process(node.kind, id))
    });
    ui.horizontal(|ui| {
        icons::slot(ui, Icon::Machine(node.kind), None, true, "Smelting furnace");
        ui.vertical(|ui| {
            ui.label(RichText::new("SMELTING").size(19.0).color(icons::GOLD));
            let status = if !node.enabled {
                "Paused"
            } else if node.status == "Processing" {
                "Smelting"
            } else if node.status == "Needs cable / power" {
                "Awaiting power"
            } else if node.status == "Output blocked" {
                "Output full"
            } else if node.status == "Needs input" {
                "Awaiting ore"
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

    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.small("POWER");
        icons::gauge(
            ui,
            node.energy as f32 / 64.0,
            icons::GOLD,
            &format!("{} / 64 energy", node.energy),
        );
        if node.energy == 0 {
            ui.small("Connect a generator with power cable.");
        }
    });

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
                        "{:.0}% · batch {}/{} ticks",
                        progress * 100.0,
                        node.progress,
                        recipe.ticks
                    )
                } else {
                    format!("Ready · {} ticks · {} power", recipe.ticks, recipe.power)
                })
                .fill(icons::TEAL),
        );
        ui.small(format!(
            "Recipe: {} → {}. Item pipes deliver the finished ingots.",
            NAMES[recipe.inputs[0].0], NAMES[recipe.output.0]
        ));
    } else {
        ui.add_space(4.0);
        ui.label(
            RichText::new("No smeltable ore in the input.")
                .color(crate::egui_ui::presentation::MUTED),
        );
    }

    if node.kind == Kind::Furnace {
        ui.add_space(6.0);
        ui.label(
            RichText::new("RECIPE")
                .size(11.0)
                .color(crate::egui_ui::presentation::MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            for id in 9..12 {
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
    } else {
        ui.add_space(6.0);
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

    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.48).min(300.0));
            ui.small("ORE / DUST IN");
            let inputs: Vec<_> = (0..NAMES.len())
                .filter(|&i| node.accepts(i))
                .map(|i| (i, node.input[i]))
                .collect();
            supply_grid(ui, "smelt-in", &inputs);
        });
        ui.vertical(|ui| {
            ui.small("INGOTS OUT");
            let outputs: Vec<_> = next
                .map(|r| vec![(r.output.0, node.output[r.output.0])])
                .unwrap_or_else(|| {
                    (0..NAMES.len())
                        .filter(|&i| node.output[i] > 0 || matches!(i, 6..=8))
                        .map(|i| (i, node.output[i]))
                        .collect()
                });
            supply_grid(ui, "smelt-out", &outputs);
        });
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Load ore")
            .on_hover_text("Load all matching ore/dust from your pack")
            .clicked()
        {
            *action = UiAction::Factory(Action::Load(pos));
        }
        if ui
            .button("Unload")
            .on_hover_text("Return all input and output ingots to your pack")
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
        if node.upgrade < 2 {
            let cost: &[(usize, u32)] = if node.upgrade == 0 {
                &[(14, 128), (16, 64)]
            } else {
                &[(15, 128), (16, 128), (17, 64)]
            };
            if ui
                .add_enabled(
                    affordable(inventory, cost),
                    egui::Button::new(format!("Drive {} → {}", node.upgrade + 1, node.upgrade + 2)),
                )
                .on_hover_ui(|ui| {
                    ingredients(ui, cost, inventory);
                    ui.small("Faster batches at higher power demand.");
                })
                .clicked()
            {
                *action = UiAction::Factory(Action::UpgradeMachine(pos));
            }
        }
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
            super::presentation::hud_frame().show(ui,|ui| {
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
                return;
            }
            ui.horizontal(|ui| {
                icons::slot(ui,Icon::Machine(node.kind),None,true,detail(node.kind));
                ui.vertical(|ui| {
                    ui.label(RichText::new(node.kind.name()).size(20.0).color(icons::PAPER));
                    ui.label(RichText::new(node.status).color(if node.status=="Processing"||node.status=="Generating" {icons::TEAL}else{icons::GOLD}));
                    if node.kind.powered() {icons::gauge(ui,node.energy as f32/64.0,icons::GOLD,&format!("{} / 64 power",node.energy));}
                    if node.kind==Kind::Generator && node.mode==1 {
                        let heat=node.progress as f32/crate::world::factory::pile::MAX_HEAT as f32;
                        icons::gauge(ui,heat,if node.progress>=crate::world::factory::pile::WARN_HEAT {egui::Color32::from_rgb(235,80,60)} else {icons::GOLD},&format!("Core heat {} / {}",node.progress,crate::world::factory::pile::MAX_HEAT));
                    }
                    if node.kind.water_port() && (node.kind!=Kind::Generator || node.mode==1) {icons::gauge(ui,node.water as f32/8192.0,icons::TEAL,&format!("{:.0} / 16000 L {}",node.water as f32*1000.0/512.0,if node.kind==Kind::Generator {"coolant"} else {"water"}));}
                });
            });
            if let Some(recipe)=node.process() {
                ui.horizontal_wrapped(|ui| {
                    for &(i,n) in &recipe.inputs {if n>0 {icons::slot(ui,Icon::Resource(i),Some(n),false,NAMES[i]);}}
                    ui.label(RichText::new("→").size(26.0).color(icons::GOLD));
                    icons::slot(ui,Icon::Resource(recipe.output.0),Some(recipe.output.1),true,NAMES[recipe.output.0]);
                });
                icons::gauge(ui,node.progress as f32/recipe.ticks as f32,icons::TEAL,"Batch progress");
            }
            for (i,label) in node.kind.modes().iter().enumerate() {
                let short=label.split(':').next().unwrap_or(label);
                if ui.selectable_label(node.mode==i as u32,short).on_hover_text(*label).clicked(){*action=UiAction::Factory(Action::SetRecipe(*pos,i as u32));}
            }
            if !node.kind.conduit() {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() * 0.48).min(330.0));
                        ui.small("INPUT");
                        let inputs: Vec<_> = (0..NAMES.len())
                            .filter(|&i| node.accepts(i) || node.input[i] > 0)
                            .map(|i| (i, node.input[i]))
                            .collect();
                        supply_grid(ui, "machine-in", &inputs);
                    });
                    ui.vertical(|ui| {
                        ui.small("OUTPUT");
                        let outputs: Vec<_> = (0..NAMES.len())
                            .filter(|&i| node.output[i] > 0)
                            .map(|i| (i, node.output[i]))
                            .collect();
                        supply_grid(ui, "machine-out", &outputs);
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Load").on_hover_text("Load matching supplies from your pack").clicked(){*action=UiAction::Factory(Action::Load(*pos));}
                    if ui.button("Unload").on_hover_text("Return all input and output items to your pack").clicked(){*action=UiAction::Factory(Action::Collect(*pos));}
                    if ui.button(if node.enabled {"Pause"}else{"Start"}).clicked(){*action=UiAction::Factory(Action::Toggle(*pos));}
                    if node.kind.water_port() && node.water>0 {
                        if ui.button(if node.draining {"Close drain"}else{"Drain water"}).on_hover_text("Pauses the machine and releases stored water into free neighbouring cells. Clear the outlet before dismantling.").clicked(){*action=UiAction::Factory(Action::Drain(*pos));}
                    }
                    if node.kind.powered() && node.upgrade<2 {
                        let cost:&[(usize,u32)]=if node.upgrade==0 {&[(14,128),(16,64)]}else{&[(15,128),(16,128),(17,64)]};
                        if ui.add_enabled(affordable(inventory,cost),egui::Button::new(format!("Drive {} → {}",node.upgrade+1,node.upgrade+2))).on_hover_ui(|ui|{ingredients(ui,cost,inventory);ui.small("Faster work, higher power demand.");}).clicked(){*action=UiAction::Factory(Action::UpgradeMachine(*pos));}
                    }
                });
            }
            if matches!(node.kind,Kind::Feeder|Kind::Storage) || (node.kind==Kind::Pipe && node.mode==1) {
                ui.collapsing("Filter",|ui|{ui.horizontal_wrapped(|ui|{
                    if ui.selectable_label(node.filter.is_none(),"Any").clicked(){*action=UiAction::Factory(Action::SetFilter(*pos,None));}
                    for (i,name) in NAMES.iter().enumerate(){if icons::slot(ui,Icon::Resource(i),None,node.filter==Some(i),name).clicked(){*action=UiAction::Factory(Action::SetFilter(*pos,Some(i)));}}
                });});
            }
        });
        } else {
            ui.label("No machine selected. Aim at a machine and press F to inspect it.");
        }
        return;
    }
    if tab == 0 {
        super::presentation::hud_frame().show(ui, |ui| {
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
                        "Third strike releases a radial impulse that launches loose assemblies.",
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
                                ui.label(RichText::new(name).size(9.5).color(icons::TEAL))
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
            ui.weak("No sword owned. Craft a copper sword from the Crafting tab, or recover a warden's steel blade.");
        }
        ui.separator();
        super::wands::draw(ui, adventure, action);
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
                &[6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18][..],
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
