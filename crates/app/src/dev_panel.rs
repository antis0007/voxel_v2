use crate::{egui_ui::UiAction, local_commands, player::Player, world::{adventure::Adventure, DayNightCycle, World}};
use crate::world::{adventure::{wand_items::WandRecord, wands::{EMPTY, Loadout}}, inventory::{Item, ItemStack}};
use std::{io::{self, BufRead, Read, Write}, sync::mpsc::{self, Receiver}};

pub mod engine;
pub mod factory_lab;
pub mod gilded_layout;
pub mod machine;
pub mod wand_lab;
const MAX_LINE: usize = 32_768;

pub struct Bridge {
    engine: std::cell::RefCell<engine::Engine>,
    commands: Receiver<(u32, String)>,
    spark_speed: Option<f32>,
    recipe: Option<String>,
}

impl Bridge {
    pub fn open() -> Option<Self> {
        if !std::env::args().any(|arg| arg == "--axiom-dev-stdin") {
            return None;
        }
        let (sender, receiver) = mpsc::sync_channel(8);
        std::thread::Builder::new().name("axiom-dev-panel".into()).spawn(move || {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();
            let mut line = String::new();
            loop {
                line.clear();
                match (&mut stdin).take(MAX_LINE as u64).read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if !line.ends_with('\n') => break,
                    Ok(_) => {}
                }
                if let Some((id, command)) = line.trim_end().split_once('\t')
                    && let Ok(id) = id.parse::<u32>()
                    && command.starts_with('/')
                    && sender.send((id, command.to_owned())).is_err() {
                    break;
                }
            }
        }).ok()?;
        let spark_speed = std::env::var_os("VOXEL_SAVE_DIR")
            .filter(|directory| !directory.is_empty())
            .and_then(|_| std::env::var("VOXEL_FORGE_SPARK_SPEED").ok())
            .and_then(|speed| speed.parse::<f32>().ok())
            .filter(|speed| (1.0..=120.0).contains(speed));
        let recipe = std::env::var_os("VOXEL_SAVE_DIR")
            .filter(|directory| !directory.is_empty())
            .and_then(|_| std::env::var("VOXEL_FORGE_RECIPE").ok())
            .filter(|name| !name.is_empty() && name.len() <= 640 && !name.chars().any(char::is_control));
        reply(0, "Game connected");
        if let Some(speed) = spark_speed {
            reply(0, &format!("Forge Spark fixture ready at {speed} m/s · use /forge-spark in a solo world"));
        }
        if let Some(name) = &recipe {
            reply(0, &format!("Forge recipe fixture ready · {name}"));
        }
        Some(Self { commands: receiver, spark_speed, recipe, engine: Default::default() })
    }

    pub fn poll(&self, action: UiAction, world: Option<&mut World>, player: &mut Player,
        clock: &mut DayNightCycle) -> UiAction {
        self.engine.borrow_mut().bind(world.as_deref());
        if !matches!(action, UiAction::None) {
            return action;
        }
        let Ok((id, command)) = self.commands.try_recv() else { return action };
        if let Some(request) = command.strip_prefix("/engine ") {
            let response = self.engine.borrow_mut().handle(request, world, player, clock);
            reply(id, &response.to_string());
            return action;
        }
        let Some(world) = world else {
            reply(id, "Open a solo world before sending commands");
            return action;
        };
        if command == "/convoy-status" {
            reply(id, &crate::world::farming::planet_freight::dev_status(world));
            return action;
        }
        if command == "/ambience-status" {
            crate::world::ambience::refresh(world, player);
            reply(id, &crate::world::ambience::diagnostics(world));
            return action;
        }
        if command == "/faction-status" {
            reply(id, &crate::world::travel::civilization::dev_factions(world));
            return action;
        }
        if command == "/civilization-status" {
            reply(id, &crate::world::farming::jobs::diagnostics(world));
            return action;
        }
        if command == "/dungeon-survey" || command.starts_with("/dungeon-view ") || command.starts_with("/spell-fx ") {
            #[cfg(feature = "multiplayer")]
            if world.multiplayer.is_some() {
                reply(id, "Open a solo playtest world");
                return action;
            }
            let cfg = world.sample_config();
            let eye = player.eye(world.voxel_scale);
            let forward = player.forward_dir();
            let scale = cfg.voxel_meters();
            let message = if let Some(view) = command.strip_prefix("/dungeon-view ") {
                if world.rules.mode != crate::world::rules::GameMode::Spectator {
                    "Spectator mode required for dungeon viewpoints".to_owned()
                } else if crate::dungeon_review::workbench_view(player, &cfg, view) {
                    format!("{view} · generator {} · position {:?} m", cfg.generator_version, player.pos * scale)
                } else { "This world has no matching viewpoint".to_owned() }
            } else if let Some(family) = command.strip_prefix("/spell-fx ") {
                if world.adventure.preview_spell_contact(family, eye * scale, forward) {
                    format!("{family} · cosmetic impact preview")
                } else { "Unknown effect family".to_owned() }
            } else {
                let surface = crate::world::raycast_block(world, eye, forward, 6.0 / scale).map(|(cell, _)| {
                    let block = world.get(cell);
                    format!("{} at {cell:?} · solid {} · mirage {}", block.display_name(), block.is_solid(), block.is_mirage())
                }).unwrap_or_else(|| "No surface in reach".to_owned());
                let mimic = crate::world::adventure::focused_cache(world, eye * scale, forward, 6.0)
                    .map_or_else(|| "no mimic in reach".to_owned(), |(_, site, slot)| format!("mimic {site:?}/{slot}"));
                format!("{surface} · {mimic} · up {:?} · generator {} · {:.2} m voxels", player.up_dir(), cfg.generator_version, scale)
            };
            reply(id, &message);
            return action;
        }
        if command == "/forge-recipe-gear" {
            reply(id, &recipe_gear(world, self.recipe.as_deref()));
            return action;
        }
        if matches!(command.as_str(), "/forge-spark" | "/forge-spark-gear") {
            let message = match self.spark_speed {
                Some(_) if command == "/forge-spark-gear" => spark_gear(world),
                Some(speed) => Adventure::forge_cast_spark(world, player, speed),
                None => "Launch a Spark fixture from the spell editor first".to_owned(),
            };
            reply(id, &message);
            return action;
        }
        let (next, messages) = local_commands::command(&command, world, player, clock);
        if matches!(next, UiAction::SaveWorld) {
            reply(id, "Save requested");
        } else {
            reply(id, &messages.join(" · "));
        }
        next
    }
}

fn recipe_gear(world: &mut World, name: Option<&str>) -> String {
    let Some(name) = name else { return "Open a recipe preview first.".to_owned(); };
    #[cfg(feature = "multiplayer")]
    if world.multiplayer.is_some() {
        return "Open a solo recipe preview.".to_owned();
    }
    let Some(recipe) = crate::world::inventory::all_recipes().iter().find(|row| row.name == name) else {
        return "The game did not load this recipe. Check the playtest output.".to_owned();
    };
    let mut pack = world.inventory.clone();
    for &(item, count) in recipe.ingredients {
        if pack.try_insert(ItemStack::new(item, count)).1 != 0 {
            return "Make room for the recipe ingredients and try again.".to_owned();
        }
    }
    world.inventory = pack;
    world.sync_from_inventory();
    format!("Ingredients added for {name}. Craft it at its usual station.")
}

fn spark_gear(world: &mut World) -> String {
    let loadout = Loadout { slots: [0, EMPTY, EMPTY, EMPTY], ..Loadout::default() };
    let fixture = WandRecord { tier: 1, loadout };
    let is_fixture = |stack: &Option<ItemStack>| {
        matches!(stack, Some(ItemStack { item: Item::Wand { id, .. }, .. })
            if world.adventure.wand_record(*id) == Some(&fixture))
    };
    if let Some(slot) = world.inventory.hotbar.iter().position(is_fixture) {
        return format!("Forge Spark: select hotbar slot {} to equip the fixture wand.", slot + 1);
    }
    if world.adventure.wand_bank.contains(&fixture) {
        return "Forge Spark: return the existing fixture wand to the hotbar.".to_owned();
    }
    let Some(slot) = world.inventory.hotbar.iter().position(Option::is_none) else {
        return "Forge Spark: clear one hotbar slot first.".to_owned();
    };
    let Some(item) = world.adventure.mint_wand(fixture) else {
        return "Forge Spark: wand bank is full.".to_owned();
    };
    world.inventory.hotbar[slot] = Some(ItemStack::new(item, 1));
    format!("Forge Spark: select hotbar slot {} to equip the fixture wand.", slot + 1)
}

fn reply(id: u32, message: &str) {
    let mut stdout = io::stdout().lock();
    let message = message.replace(['\n', '\r', '\t'], " ");
    let _ = writeln!(stdout, "AXIOM_DEV\t{id}\t{message}");
    let _ = stdout.flush();
}
