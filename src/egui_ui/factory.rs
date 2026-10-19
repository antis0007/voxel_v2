use super::{
    UiAction,
    icons::{self, Icon},
};
use crate::world::{
    expedition::{Expedition, NAMES},
    factory::{Action, Kind, View},
};
use egui::{Color32, RichText};

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
fn supplies(ui: &mut egui::Ui, items: &[u32; 19]) {
    ui.horizontal_wrapped(|ui| {
        let mut empty = true;
        for (i, &n) in items.iter().enumerate() {
            if n > 0 {
                empty = false;
                icons::slot(ui, Icon::Resource(i), Some(n), false, NAMES[i]);
            }
        }
        if empty {
            ui.weak("Empty");
        }
    });
}
fn detail(kind: Kind) -> &'static str {
    match kind {
        Kind::Generator => "Burns coal. 64 power/s. Connect consumers with teal power cables.",
        Kind::Bioreactor => "Burns fungal biomass. 128 power/s. Cultivators supply renewable fuel.",
        Kind::Crusher => "1 ore → 2 dust. 4 seconds. 128 power per batch.",
        Kind::Furnace => "1 dust → 1 ingot. 3 seconds. 72 power per batch.",
        Kind::Feeder => {
            "Load supplies, then export through copper item pipes. A filter selects the exported resource."
        }
        Kind::Storage => {
            "Receives items after processors. Set a filter to dedicate a crate to one resource."
        }
        Kind::Pipe => {
            "Copper item transport. Connect a continuous path between machine faces. Machines do not relay connections."
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
            ui.horizontal(|ui| {
                icons::slot(ui,Icon::Machine(node.kind),None,true,detail(node.kind));
                ui.vertical(|ui| {
                    ui.label(RichText::new(node.kind.name()).size(20.0).color(icons::PAPER));
                    ui.label(RichText::new(node.status).color(if node.status=="Processing"||node.status=="Generating" {icons::TEAL}else{icons::GOLD}));
                    if node.kind.powered() {icons::gauge(ui,node.energy as f32/64.0,icons::GOLD,&format!("{} / 64 power",node.energy));}
                    if node.kind.water_port() {icons::gauge(ui,node.water as f32/8192.0,icons::TEAL,&format!("{:.0} / 16000 L water",node.water as f32*1000.0/512.0));}
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
                    ui.vertical(|ui| {ui.set_width((ui.available_width()*0.48).min(330.0));ui.small("INPUT");supplies(ui,&node.input);});
                    ui.vertical(|ui| {ui.small("OUTPUT");supplies(ui,&node.output);});
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
            if matches!(node.kind,Kind::Feeder|Kind::Storage) {
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
            ui.label(RichText::new("Field supplies").size(18.0).color(icons::GOLD));
            ui.columns(3, |columns| {
                {
                    let ui=&mut columns[0];
                    icons::slot(ui, Icon::Sword(1), None, adventure.weapons & 1 != 0, "Copper sword · balanced melee weapon");
                    ui.label("Copper sword");
                    ingredients(ui, crate::world::adventure::sword_recipe(1), inventory);
                    if ui.add_enabled(view.bench && adventure.weapons & 1 == 0 && affordable(inventory, crate::world::adventure::sword_recipe(1)), egui::Button::new("Craft copper sword"))
                        .clicked() { *action = UiAction::Factory(Action::CraftSword(1)); }
                }
                {
                    let ui=&mut columns[1];
                    icons::slot(ui, Icon::Sword(2), None, adventure.weapons & 2 != 0, "Steel sword · heavy melee weapon");
                    ui.label("Steel sword");
                    ingredients(ui, crate::world::adventure::sword_recipe(2), inventory);
                    if ui.add_enabled(view.bench && adventure.weapons & 2 == 0 && affordable(inventory, crate::world::adventure::sword_recipe(2)), egui::Button::new("Craft steel sword"))
                        .clicked() { *action = UiAction::Factory(Action::CraftSword(2)); }
                }
                {
                    let ui=&mut columns[2];
                    ui.label("Fungal rations ×2");
                    ingredients(ui, crate::world::adventure::ration_recipe(), inventory);
                    ui.small("Restores hunger and a little health.");
                    if ui.add_enabled(view.bench && adventure.rations <= 65533 && affordable(inventory, crate::world::adventure::ration_recipe()), egui::Button::new("Prepare rations"))
                        .clicked() { *action = UiAction::Factory(Action::CookRations); }
                }
            });
            if !view.bench { ui.small("F at a workbench to craft weapons and prepare food."); }
        });
        ui.separator();
        ui.label(RichText::new("Workshop kits").size(17.0).color(icons::PAPER));
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
        super::relics::codex(ui,adventure);
        ui.label(RichText::new("Weapons").size(18.0).color(icons::GOLD));
        let mut owns_any = false;
        for tier in 1..=4 {
            if adventure.weapons & (1 << (tier - 1)) == 0 { continue; }
            owns_any = true;
            ui.horizontal(|ui| {
                icons::slot(ui, Icon::Sword(tier), None, adventure.weapon == tier, crate::world::adventure::sword_name(tier));
                ui.vertical(|ui| {
                    ui.label(crate::world::adventure::sword_name(tier));
                    ui.small(match tier {3 => "Third strike releases a flame cleave and restores mana",4 => "Chill, then shatter; shards chill nearby enemies",2 => "High damage · recovered from wardens or crafted",_ => "Balanced damage · crafted at a workbench"});
                    if ui.button(if adventure.weapon == tier { "Put away" } else { "Equip" }).clicked() {
                        *action = UiAction::Factory(Action::SelectSword(if adventure.weapon == tier { 0 } else { tier }));
                    }
                });
            });
        }
        if !owns_any { ui.weak("No sword owned. Craft a copper sword from the Crafting tab, or recover a warden's steel blade."); }
        ui.separator();
        super::wands::draw(ui,adventure,action);
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
        icons::gauge(ui, adventure.hunger / 100.0, icons::TEAL, &format!("Hunger · {:.0}%", adventure.hunger));
        ui.horizontal(|ui| {
            ui.label(format!("Fungal rations · {}", adventure.rations));
            if ui.add_enabled(adventure.rations > 0 && adventure.hunger < 99.0, egui::Button::new("Eat one"))
                .clicked() { *action = UiAction::Factory(Action::EatRation); }
        });
        ui.small("Eat to restore hunger and health. Hunger must stay above 75% for natural recovery.");
        ui.separator();
        ui.label("Collected materials");
        let search_id = ui.id().with("pack_material_filter");
        let mut filter = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter materials…").desired_width(ui.available_width()));
        ui.ctx().data_mut(|d| d.insert_temp(search_id, filter.clone()));
        if inventory.resources.iter().all(|&n| n == 0) {
            ui.weak("No supplies yet. Mine with left mouse or open a salvage cache with F.");
        }
        let filter = filter.trim().to_lowercase();
        let groups: [(&str, &[usize]); 2] = [
            ("Mined and gathered", &[0, 1, 2, 3, 4, 5, 13][..]),
            ("Refined and relics", &[6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18][..]),
        ];
        let mut shown = 0;
        for (heading, ids) in groups {
            let entries: Vec<_> = ids.iter().copied().filter(|&i| inventory.resources[i] > 0 && NAMES[i].to_lowercase().contains(&filter)).collect();
            if entries.is_empty() { continue; }
            shown += entries.len();
            egui::CollapsingHeader::new(format!("{} · {}", heading, entries.len()))
                .id_salt(heading).default_open(true).show(ui, |ui| {
                egui::Grid::new(("supplies_list", heading)).spacing([12.0, 6.0]).show(ui, |ui| {
                    for i in entries {
                        icons::slot(ui, Icon::Resource(i), None, false, NAMES[i]);
                        ui.label(NAMES[i]);
                        ui.monospace(format!("{:.2}", inventory.resources[i] as f32 / 64.0));
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
