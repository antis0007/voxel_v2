//! Solo chat uses the same history widget as network chat and the same command
//! grammar as the host (`engine_net::commands`). World changes go through
//! `world_commands`, so a command behaves the same alone and online.
use crate::{egui_ui::UiAction, player::Player, social::Social, world::{World, DayNightCycle, rules::GameMode}, world_commands};
use engine_net::{commands::{self, Command, Effect, ServerView}, protocol::{ChatKind, PlayerId}};

const SOLO: PlayerId = 0;
const HELP: &str = "/gamemode survival|creative|adventure|spectator · /gamerule keepInventory true|false · /time set day|noon|night|midnight|ticks · /time add ticks · /weather clear|rain|thunder · /tp x y z (supports ~) · /give item [count] · /summon creature · /worldborder metres|off · /effect give name seconds [level] · /effect clear · /seed · /mill build|status|load|bake|collect · /save-all · /me text";

pub fn execute(line: &str, chat: &mut Social, world: &mut World, player: &mut Player, clock: &mut DayNightCycle) -> UiAction {
    if !line.starts_with('/') {
        chat.push_text(ChatKind::Say, format!("<You> {line}"));
        return UiAction::None;
    }
    let (action, replies) = command(line, world, player, clock);
    for text in replies {
        chat.push_text(ChatKind::System, text);
    }
    action
}

/// Solo session as the shared command layer sees it: one player who is the operator.
struct Solo {
    mode: GameMode,
    time: f32,
    seed: u64,
}

impl ServerView for Solo {
    fn players(&self) -> Vec<(PlayerId, String)> {
        vec![(SOLO, "You".to_owned())]
    }
    fn is_op(&self, _: PlayerId) -> bool {
        true
    }
    fn gamemode(&self, _: PlayerId) -> commands::GameMode {
        match self.mode {
            GameMode::Survival => commands::GameMode::Survival,
            GameMode::Creative => commands::GameMode::Creative,
            GameMode::Adventure => commands::GameMode::Adventure,
            GameMode::Spectator => commands::GameMode::Spectator,
        }
    }
    fn time_of_day(&self) -> f32 {
        self.time
    }
    fn seed(&self) -> u64 {
        self.seed
    }
    fn supports(&self, _: commands::GameMode) -> bool {
        true
    }
}

pub(crate) fn command(line: &str, world: &mut World, player: &mut Player, clock: &mut DayNightCycle) -> (UiAction, Vec<String>) {
    if line == "/realm" || line.starts_with("/realm ") {
        let feet = player.pos * world.voxel_scale.meters();
        let reply = crate::world::travel::civilization::politics::command(world, feet, &line[6..]);
        return (UiAction::None, vec![reply]);
    }
    if line == "/mill" || line.starts_with("/mill ") {
        let reply = crate::world::mill::command(world, player, line[5..].trim());
        return (UiAction::None, vec![reply]);
    }
    let parsed = match commands::parse(line) {
        Ok(command) => command,
        Err(reason) => return (UiAction::None, vec![reason]),
    };
    match parsed {
        Command::Help => return (UiAction::None, vec![HELP.to_owned()]),
        Command::List => return (UiAction::None, vec!["Players: You (local owner)".to_owned()]),
        Command::Msg { .. } | Command::Name(_) | Command::GamemodeOf { .. } | Command::Kick { .. } | Command::Tp { .. } | Command::Op { .. }
        | Command::Deop { .. } | Command::Whitelist { .. } | Command::Ban { .. } | Command::Pardon { .. }
        | Command::Banlist | Command::Stop => return (UiAction::None, vec!["Hosted sessions only".to_owned()]),
        _ => {}
    }
    let view = Solo { mode: world.rules.mode, time: clock.time_of_day(), seed: u64::from(world.sample_config().seed) };
    let outcome = commands::execute(SOLO, &parsed, &view);
    let mut replies = outcome.private;
    let mut action = UiAction::None;
    // The crate words broadcasts for other players; a lone player reads the local state line instead.
    let mut broadcast = outcome.broadcast;
    for effect in outcome.effects {
        match effect {
            Effect::Gamemode { mode, .. } => {
                broadcast.clear();
                let mut rules = world.rules;
                rules.mode = match mode {
                    commands::GameMode::Survival => GameMode::Survival,
                    commands::GameMode::Creative => GameMode::Creative,
                    commands::GameMode::Adventure => GameMode::Adventure,
                    commands::GameMode::Spectator => GameMode::Spectator,
                };
                world.set_rules(rules);
                replies.push(format!("Game mode: {}", rules.mode.label()));
            }
            Effect::SetTime(time) => {
                let eye = player.pos.as_dvec3() * f64::from(world.voxel_scale.meters());
                crate::planet::set_local_time(world.world_type, eye, clock, time);
            }
            Effect::Save => action = UiAction::SaveWorld,
            Effect::World { change, .. } => {
                let creative = world.rules.mode.creative_tools();
                let body = world_commands::Body { player: &mut *player, creative };
                match world_commands::apply(&change, world, Some(body), clock) {
                    Ok(text) | Err(text) => replies.push(text),
                }
            }
            // Access, rename and player teleports are refused above.
            Effect::Kick { .. } | Effect::Teleport { .. } | Effect::SetOp { .. } | Effect::Rename { .. }
            | Effect::Whitelist { .. } | Effect::SetWhitelist(_) | Effect::Ban { .. } | Effect::Pardon { .. }
            | Effect::Stop => {}
        }
    }
    replies.extend(broadcast);
    (action, replies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{WorldType, adventure, inventory::Item, rules::Rules};
    use world_commands::resolve_item;

    fn run(line: &str, world: &mut World, player: &mut Player, clock: &mut DayNightCycle) -> (UiAction, Vec<String>) {
        command(line, world, player, clock)
    }

    #[test]
    fn items_resolve_by_display_name_or_snake_case() {
        assert_eq!(resolve_item("steel_sword"), Some(Item::Weapon(2)));
        assert_eq!(resolve_item("STEEL SWORD"), Some(Item::Weapon(2)));
        assert!(matches!(resolve_item("stone"), Some(Item::Block(_))));
        assert_eq!(resolve_item("no_such_thing"), None);
    }

    #[test]
    fn creatures_resolve_from_the_bestiary() {
        assert_eq!(adventure::species_named("ruin_skitterer").map(|(k, _)| k), Some(0));
        assert_eq!(adventure::species_named("Ruin Skitterer").map(|(_, n)| n), Some("Ruin Skitterer"));
        assert!(adventure::species_named("dragon").is_none());
    }

    #[test]
    fn world_commands_change_mode_time_weather_and_border() {
        let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
        let mut player = Player::new();
        player.pos = world.suggested_spawn();
        let mut clock = DayNightCycle::default();

        run("/gamemode spectator", &mut world, &mut player, &mut clock);
        assert_eq!(world.rules.mode, GameMode::Spectator);
        let (_, replies) = run("/gamemode hardcore", &mut world, &mut player, &mut clock);
        assert!(replies[0].contains("gamemode takes"), "{replies:?}");

        run("/time set midnight", &mut world, &mut player, &mut clock);
        assert!((clock.time_of_day() - 0.0).abs() < 1e-4 || (clock.time_of_day() - 1.0).abs() < 1e-4);
        run("/time set 6000", &mut world, &mut player, &mut clock);
        assert!((clock.time_of_day() - 0.5).abs() < 1e-4, "ticks count from sunrise");

        run("/weather thunder", &mut world, &mut player, &mut clock);
        assert!(clock.cloud_controls().cumulonimbus >= crate::simulation::storms::THUNDER_CUMULONIMBUS);
        run("/weather rain", &mut world, &mut player, &mut clock);
        let rain = clock.cloud_controls().cumulonimbus;
        assert!(rain < crate::simulation::storms::THUNDER_CUMULONIMBUS && crate::simulation::storms::precipitation(rain) > 0.3);
        run("/weather clear", &mut world, &mut player, &mut clock);
        assert_eq!(clock.cloud_controls().cumulonimbus, 0.0);

        run("/worldborder 200", &mut world, &mut player, &mut clock);
        assert_eq!(world.rules.border_m, 200);
        assert_eq!(Rules::decode(&world.rules.encode()).unwrap().border_m, 0);
        run("/worldborder off", &mut world, &mut player, &mut clock);
        assert_eq!(world.rules.border_m, 0);
        run("/worldborder 3", &mut world, &mut player, &mut clock);
        assert_eq!(world.rules.border_m, 0);

        run("/gamerule pvp true", &mut world, &mut player, &mut clock);
        assert!(world.rules.pvp);
        let (_, replies) = run("/kick Bo", &mut world, &mut player, &mut clock);
        assert_eq!(replies, vec!["Hosted sessions only"]);
    }

    #[test]
    fn give_lands_in_the_pack_and_summon_reaches_the_world() {
        let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
        let mut player = Player::new();
        player.pos = world.suggested_spawn();
        let mut clock = DayNightCycle::default();
        world.rules.mode = GameMode::Survival;
        let (_, replies) = run("/give stone 5", &mut world, &mut player, &mut clock);
        assert_eq!(replies, vec!["Use /gamemode creative before /give"]);

        world.rules.mode = GameMode::Creative;
        let stone = resolve_item("stone").unwrap();
        let before = world.inventory.count_item(stone);
        let (action, replies) = run("/give stone 70", &mut world, &mut player, &mut clock);
        assert!(matches!(action, UiAction::None));
        assert_eq!(world.inventory.count_item(stone), before + 70, "{replies:?}: more than one stack arrives");
        let (_, replies) = run("/give copper_sword 0", &mut world, &mut player, &mut clock);
        assert!(replies[0].starts_with("count is"), "{replies:?}");

        let before = world.adventure.enemies.len();
        run("/summon ruin_skitterer", &mut world, &mut player, &mut clock);
        assert_eq!(world.adventure.enemies.len(), before + 1);
        let (_, replies) = run("/summon dragon", &mut world, &mut player, &mut clock);
        assert_eq!(replies, vec!["Unknown creature"]);
    }
}
