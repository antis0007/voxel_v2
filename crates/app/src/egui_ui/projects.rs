//! Player-chosen routes through existing recipes, equipment and quest destinations.
use super::{presentation as style, recipe_book};
use crate::keybinds::{self, Action};
use crate::world::{
    BlockId as B, adventure,
    factory::{Kind, View as Machines, multiblock, power, recipes},
    inventory::{ArmorTier, EquipSlot, Item, SurvivalInventory},
    recipe_index, tool_workshop,
};
use egui::{RichText, Ui};

fn amount(n: f32) -> String {
    if (n - n.round()).abs() < 0.001 {
        format!("{n:.0}")
    } else {
        format!("{n:.2}")
    }
}
fn source_hint(item: Item) -> String {
    let drops = adventure::drop_sources(item);
    if drops.is_empty() {
        "Recipes".into()
    } else {
        format!("Drops: {}", drops.join(" · "))
    }
}

fn recipe(ui: &mut Ui, item: Item, inv: &SurvivalInventory, machines: &Machines) {
    ui.push_id(item.encode_u32(), |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button(item.name()).on_hover_text("Recipes").clicked() {
                recipe_book::request(ui.ctx(), item, false);
            }
            let count = inv.count_item(item);
            if inv
                .equipment
                .iter()
                .flatten()
                .any(|stack| stack.item == item)
            {
                ui.colored_label(style::JADE, "Equipped");
            } else if count > 0 {
                ui.weak(format!("Owned: {count}"));
            }
            if let Item::Block(b) = item
                && let Some(kind) = Kind::from_block(b)
            {
                let placed: u32 = machines
                    .health
                    .rows
                    .iter()
                    .filter(|r| r.kind == Some(kind))
                    .map(|r| r.counts.iter().sum::<u32>())
                    .sum();
                ui.colored_label(
                    if placed > 0 {
                        style::JADE
                    } else {
                        style::MUTED
                    },
                    format!("Placed: {placed}"),
                );
            }
            if ui.small_button("Uses").clicked() {
                recipe_book::request(ui.ctx(), item, true);
            }
        });
        if let Some(entry) = recipe_index::producing(item).first() {
            ui.horizontal_wrapped(|ui| {
                ui.weak(entry.station.name());
                for &(input, need) in &entry.inputs {
                    let have = inv.count_item(input) as f32
                        + match input {
                            Item::Resource(i) => inv.resource_remainders[i as usize] as f32 / 64.0,
                            _ => 0.0,
                        };
                    if ui
                        .link(
                            RichText::new(format!(
                                "{} {}/{}",
                                input.name(),
                                amount(have),
                                amount(need)
                            ))
                            .color(if have >= need {
                                style::JADE
                            } else {
                                style::AMBER
                            }),
                        )
                        .on_hover_text(source_hint(input))
                        .clicked()
                    {
                        recipe_book::request(ui.ctx(), input, false);
                    }
                }
            });
        }
    });
}

fn station(ui: &mut Ui, page: &mut u32, tab: usize, name: &str) {
    if ui.button(name).clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("tool_station_tab"), tab));
        *page = 5;
    }
}

fn orbital_supply(ui: &mut Ui, inv: &SurvivalInventory, m: &Machines) {
    use crate::world::orbital::cargo::Supply;

    let Some(terminal) = m.meridian_terminal else { return };
    egui::CollapsingHeader::new("Orbital Supply").default_open(true).show(ui, |ui| {
        let capacity = |tier| power::fmt_energy(power::battery_capacity(tier) as f64);
        ui.horizontal_wrapped(|ui| {
            if ui.link("Mithril Cell · Battery Bank Upgrade").clicked() {
                recipe_book::request(ui.ctx(), Item::Block(Kind::Battery.block()), false);
            }
            ui.label(format!("{} Bronze → {} Mithril",
                capacity(power::MITHRIL_MIN_TIER), capacity(power::MITHRIL_TIER)));
        });
        ui.small("Station Order · Either Supply");
        for (supply, machine) in [(Supply::Steel, Kind::AlloyKiln), (Supply::Fuel, Kind::Cultivator)] {
            let cost = supply.cost();
            let have = inv.count_item(cost.item);
            ui.horizontal_wrapped(|ui| {
                if ui.link(cost.item.name()).clicked() {
                    recipe_book::request(ui.ctx(), cost.item, false);
                }
                ui.colored_label(if have >= cost.count { style::JADE } else { style::AMBER },
                    format!("{have}/{}", cost.count));
                ui.weak(machine.name());
            });
        }
        if ui.button("Pin Meridian Terminal").clicked() {
            super::map::pin_outpost(terminal, "Meridian Terminal".into());
        }
        match m.orbital_delivery {
            Some((seconds, destination)) => {
                ui.horizontal_wrapped(|ui| {
                    if seconds == 0 {
                        ui.label(format!("Ground Cargo · [{}] Collect", keybinds::hint(Action::Interact)));
                    } else {
                        ui.label(format!("Mithril Freight · {seconds} s"));
                    }
                    if ui.button("Pin Cargo").clicked() {
                        super::map::pin_outpost(destination, "Mithril Freight".into());
                    }
                });
            }
            None if m.orbital_cell => { ui.label("Mithril Cell Ready"); }
            None => {}
        }
        super::orbital::salvage_manifest(ui);
    });
}

fn machinery(ui: &mut Ui, inv: &SurvivalInventory, m: &Machines, page: &mut u32) {
    orbital_supply(ui, inv, m);
    ui.strong("Fuel → Generator → Cable → Machine → Pipe → Storage");
    super::production::iron_line(ui, m);
    ui.collapsing("Iron Routes", |ui| {
        let raw = recipes::process(Kind::Furnace, 2).expect("raw iron recipe");
        let dust = recipes::process(Kind::Furnace, 11).expect("iron dust recipe");
        for (name, first) in [("Raw", None), ("Crush → Smelt", Some(Kind::Crusher)), ("Wash → Smelt", Some(Kind::Washer))] {
            let (ingots, energy) = first.map_or((raw.output.1 / 64, raw.power * raw.ticks), |kind| {
                let prep = recipes::process(kind, 2).expect("iron preparation recipe");
                let batches = prep.output.1 / dust.inputs[0].1;
                (batches * dust.output.1 / 64, prep.power * prep.ticks + batches * dust.power * dust.ticks)
            });
            ui.horizontal_wrapped(|ui| {
                ui.strong(name);
                ui.label(format!("{ingots} iron / ore · {}", power::fmt_joules(energy as f64)));
                if first == Some(Kind::Washer) { ui.weak("250 L water / ore"); }
            });
        }
        if ui.link("Kinetic Greaves · Dash and Mid-air Jump").clicked() {
            recipe_book::request(ui.ctx(), Item::Armor(EquipSlot::Legs, ArmorTier::Kinetic), false);
        }
    });
    egui::CollapsingHeader::new("Steel").default_open(true).show(ui, |ui| {
        recipe(ui, Item::Resource(15), inv, m);
        for kind in [Kind::AlloyKiln, Kind::BlastFurnace] {
            let Some(process) = recipes::process(kind, 1) else { continue; };
            let inputs = process.inputs.into_iter().filter(|(_, units)| *units > 0)
                .map(|(id, units)| format!("{} ×{}", Item::Resource(id as u8).name(), amount(units as f32 / 64.0)))
                .collect::<Vec<_>>().join(" + ");
            let (id, units) = process.output;
            recipe(ui, Item::Block(kind.block()), inv, m);
            ui.label(format!("{inputs} → {} ×{}", Item::Resource(id as u8).name(), amount(units as f32 / 64.0)));
            if kind == Kind::BlastFurnace {
                ui.weak(format!("3 × 4 × 3 m · Blast Bricks ×{} · Port on Second Course",
                    multiblock::bricks_needed(kind)));
                recipe(ui, Item::Block(Kind::CokeOven.block()), inv, m);
                ui.weak(format!("3 × 3 × 3 m · Coke Oven Bricks ×{}", multiblock::bricks_needed(Kind::CokeOven)));
            } else {
                ui.weak(format!("{} · {:.0} s / Batch", power::fmt_power(process.power as f64),
                    process.ticks as f64 * power::TICK_SECONDS));
            }
        }
    });
    recipe(ui, Item::Block(B::Workbench), inv, m);
    recipe(ui, Item::Block(B::Smelter), inv, m);
    for (name, kinds) in [
        (
            "Iron Automation",
            &[
                Kind::Generator,
                Kind::Cable,
                Kind::Furnace,
                Kind::Pipe,
                Kind::Storage,
            ][..],
        ),
        (
            "Ore Processing",
            &[Kind::Crusher, Kind::Washer, Kind::Pump, Kind::FluidPipe][..],
        ),
        ("Fuel & Cultivation", &[Kind::Bioreactor, Kind::Cultivator][..]),
        (
            "Power Storage",
            &[
                Kind::Battery,
                Kind::Transformer,
                Kind::InductionCasing,
                Kind::InductionPort,
                Kind::InductionCell,
                Kind::InductionProvider,
            ][..],
        ),
        ("Components & Automation", &[Kind::Fabricator, Kind::Assembler][..]),
    ] {
        egui::CollapsingHeader::new(name)
            .default_open(name == "Iron Automation")
            .show(ui, |ui| {
                for &kind in kinds {
                    recipe(ui, Item::Block(kind.block()), inv, m);
                }
            });
    }
    if let Some((issue, count)) = m.health.bottleneck() {
        ui.colored_label(style::AMBER, format!("{}: {count}", issue.fix()));
    }
    if ui.button("Production").clicked() {
        *page = 7;
    }
}

fn tools(ui: &mut Ui, inv: &SurvivalInventory, page: &mut u32) {
    use engine_tools::{MATERIALS, PARTS};
    ui.strong("Part Builder → Tool Station → Service");
    let key = egui::Id::new("project_tool_schema");
    let mut selected = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(key))
        .unwrap_or(0)
        .min(PARTS.schemas.len().saturating_sub(1));
    egui::ComboBox::from_id_salt("project_tool_picker")
        .selected_text(&PARTS.schemas[selected].display_name)
        .show_ui(ui, |ui| {
            for (i, schema) in PARTS
                .schemas
                .iter()
                .enumerate()
                .filter(|(_, s)| s.parts.len() <= 8)
            {
                ui.selectable_value(&mut selected, i, &schema.display_name);
            }
        });
    ui.ctx().data_mut(|d| d.insert_temp(key, selected));
    for part in &PARTS.schemas[selected].parts {
        if let Some((index, kind)) = PARTS
            .part_kinds
            .iter()
            .enumerate()
            .find(|(_, k)| k.id == part.kind)
        {
            ui.horizontal(|ui| {
                ui.label(format!("{} ×1", kind.display_name));
                if ui.small_button("Part Builder").clicked() {
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("tool_station_tab"), 1usize);
                        d.insert_temp(egui::Id::new("tool_station_part"), index);
                    });
                    *page = 5;
                }
            });
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Assemble").clicked() {
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("tool_station_tab"), 0usize);
                let key = egui::Id::new("tool_station_choices");
                let old = d
                    .get_temp::<(usize, [u8; 8], usize)>(key)
                    .unwrap_or((0, [0; 8], 0));
                d.insert_temp(
                    key,
                    if old.0 == selected {
                        old
                    } else {
                        (selected, [0; 8], 0)
                    },
                );
            });
            *page = 5;
        }
        station(ui, page, 2, "Repair / Modify");
    });
    ui.collapsing("Materials · Mining Tiers", |ui| {
        for (i, material) in MATERIALS.iter().enumerate() {
            let Some(supply) = tool_workshop::supply(i) else {
                continue;
            };
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · Mining Tier {}",
                    material.display_name, material.props.mining_level
                ));
                if supply.cast {
                    if ui.small_button("Casting").clicked() {
                        recipe_book::request(ui.ctx(), Item::Block(B::SmelteryController), false);
                    }
                } else {
                    station(ui, page, 1, "Part Builder");
                }
            });
        }
    });
    ui.weak(format!(
        "Custom Tools: {}",
        inv.hotbar
            .iter()
            .chain(&inv.pack)
            .flatten()
            .filter(|s| matches!(s.item, Item::Assembled { .. }))
            .count()
    ));
}

fn wands(ui: &mut Ui, view: &adventure::View, inv: &SurvivalInventory, page: &mut u32) {
    // A future comparison should show how a deck changes the next cast;
    // additional slots alone do not explain whether the build works.
    let tier = view
        .wand_bank
        .iter()
        .map(|w| w.tier)
        .max()
        .unwrap_or(view.wand)
        .max(view.wand);
    ui.horizontal(|ui| {
        if view.wand > 0 {
            ui.label(format!("Held: Tier {}", view.wand));
        } else {
            ui.weak("No wand held");
        }
        if tier > view.wand {
            ui.weak(format!("Best: Tier {tier}"));
        }
        if ui.button("Wand Editor").clicked() {
            *page = 1;
        }
    });
    // Match the Workbench action, which uses the active tier.
    let base = view.wand;
    if base < 3 {
        let target = base + 1;
        ui.horizontal_wrapped(|ui| {
            let slots = adventure::wands::capacity(target);
            ui.label(if base == 0 {
                format!("Lumen Wand · {slots} Slots")
            } else {
                format!("Next Upgrade: Tier {target} · {slots} Slots")
            });
            if ui.small_button("Workbench").clicked() {
                *page = 0;
            }
        });
        ui.horizontal_wrapped(|ui| {
            for &(resource, units) in adventure::wand_recipe(target - 1) {
                let item = Item::Resource(resource as u8);
                let held =
                    inv.count_item(item) as f32 + inv.resource_remainders[resource] as f32 / 64.0;
                let need = units as f32 / 64.0;
                if ui
                    .link(
                        RichText::new(format!("{} {}/{}", item.name(), amount(held), amount(need)))
                            .color(if held >= need {
                                style::JADE
                            } else {
                                style::AMBER
                            }),
                    )
                    .on_hover_text(source_hint(item))
                    .clicked()
                {
                    recipe_book::request(ui.ctx(), item, false);
                }
            }
        });
    } else {
        ui.weak("Max Tier");
    }
    ui.collapsing("Spell Order", |ui| {
        ui.label("Modifier → Projectile");
        ui.label("Trigger → Payload");
        ui.weak("Spell Sources: Guardians · Site Clears · Wand Upgrades");
    });
}

pub(super) fn draw(
    ui: &mut Ui,
    view: &adventure::View,
    inv: &SurvivalInventory,
    m: &Machines,
    unlocks: super::inventory::unlocks::Unlocks,
    page: &mut u32,
) {
    ui.heading("Projects");
    let key = egui::Id::new("project_branch");
    let mut branch = ui.ctx().data(|d| d.get_temp::<usize>(key)).unwrap_or(0);
    // Branches open with the system they plan: quests, machines, forge, magic.
    let open = [unlocks.quests, unlocks.factory, unlocks.forge, unlocks.magic, unlocks.factory];
    if !open.get(branch).copied().unwrap_or(false) {
        branch = open.iter().position(|o| *o).unwrap_or(0);
    }
    ui.horizontal_wrapped(|ui| {
        for (i, name) in [
            "Expeditions",
            "Machinery",
            "Tools / Weapons",
            "Wands",
            "Traversal",
        ]
        .iter()
        .enumerate()
        {
            if open[i] {
                ui.selectable_value(&mut branch, i, *name);
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(key, branch));
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt(("projects_scroll", branch))
        .max_height((ui.ctx().content_rect().height() - 180.0).max(120.0))
        .show(ui, |ui| {
            match branch {
                0 => {
                    super::quest_log::draw(ui, true);
                    if m.workshop.as_ref().is_some_and(|progress| progress.reclaimed && !progress.delivery)
                        && ui.button("Workshop Supply Line").clicked()
                    {
                        *page = 7;
                    }
                }
                1 => machinery(ui, inv, m, page),
                2 => {
                    ui.colored_label(
                        if m.bench { style::JADE } else { style::AMBER },
                        if m.bench {
                            "Workbench: Nearby"
                        } else {
                            "Workbench: Not Nearby"
                        },
                    );
                    tools(ui, inv, page);
                }
                3 => {
                    ui.colored_label(
                        if m.bench { style::JADE } else { style::AMBER },
                        if m.bench {
                            "Workbench: Nearby"
                        } else {
                            "Workbench: Not Nearby"
                        },
                    );
                    wands(ui, view, inv, page);
                }
                _ => {
                    let jump = keybinds::hint(Action::Jump);
                    let sneak = keybinds::hint(Action::Sneak);
                    for (item, control) in [
                        (
                            Item::Tool(19),
                            format!("Grounded · Hold/release {}", keybinds::hint(Action::Place)),
                        ),
                        (
                            Item::Armor(EquipSlot::Feet, ArmorTier::Slime),
                            format!("Bounce on landing · {sneak} cancels"),
                        ),
                        (
                            Item::Armor(EquipSlot::Chest, ArmorTier::Glider),
                            format!("Falling · {jump} toggles"),
                        ),
                        (
                            Item::Armor(EquipSlot::Chest, ArmorTier::Burner),
                            format!("Hold {jump} · Bio-ethanol"),
                        ),
                        (
                            Item::Armor(EquipSlot::Chest, ArmorTier::Induction),
                            format!("Hold {jump} · Charged batteries"),
                        ),
                    ] {
                        recipe(ui, item, inv, m);
                        ui.weak(control);
                        ui.add_space(4.0);
                    }
                }
            }
            ui.separator();
            ui.collapsing("Milestones", |ui| super::goals_hud::page(ui));
        });
}
