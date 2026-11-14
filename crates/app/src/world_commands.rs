//! Applies parsed world commands to one player; solo chat and the host share it.
use crate::{
    player::Player,
    world::{DayNightCycle, WeatherPreset, World, adventure, inventory::{Item, ItemStack}},
};
use engine_net::commands::{StatusOp, Weather, WorldChange};

/// The player a command lands on, and what the session lets it change.
pub struct Body<'a> {
    pub player: &'a mut Player,
    /// `/give` needs creative tools.
    pub creative: bool,
}

/// Cumulonimbus share for rain: above the rain bed, below the thunder threshold.
const RAIN_CUMULONIMBUS: f32 = 0.5;

/// Applies `change`; the text is the reply for the player who typed it.
///
/// `body` is `None` for a dedicated host's console. Its `world.inventory`,
/// `world.adventure` and `world.progression.effects` must be the body's own:
/// online, the caller mounts a joiner's and sends the joiner the result.
pub fn apply(change: &WorldChange, world: &mut World, body: Option<Body>, clock: &mut DayNightCycle) -> Result<String, String> {
    match change {
        WorldChange::TeleportTo(at) => {
            let body = body.ok_or("The console has no player")?;
            if world.adventure.is_dead() {
                return Err("Respawn before teleporting".into());
            }
            let scale = world.voxel_scale.meters();
            let here = body.player.pos * scale;
            let metres = glam::Vec3::new(at[0].resolve(here.x), at[1].resolve(here.y), at[2].resolve(here.z));
            let cells = metres / scale;
            let cfg = world.sample_config();
            if !cells.is_finite() || cells.abs().max_element() > 1_000_000.0
                || cells.y < cfg.min_world_y() as f32 || cells.y > cfg.max_world_y() as f32 {
                return Err("Coordinates are outside the world".into());
            }
            place(body.player, cells);
            Ok(format!("Teleported to {:.1} {:.1} {:.1} m", metres.x, metres.y, metres.z))
        }
        WorldChange::Weather(weather) => {
            match weather {
                Weather::Clear => clock.set_weather_preset(WeatherPreset::Clear),
                Weather::Rain => {
                    clock.set_weather_preset(WeatherPreset::Stormy);
                    let mut controls = clock.cloud_controls();
                    controls.cumulonimbus = RAIN_CUMULONIMBUS;
                    clock.set_cloud_controls(controls);
                }
                Weather::Thunder => clock.set_weather_preset(WeatherPreset::Stormy),
            }
            Ok(format!("Weather: {}", capitalised(weather.label())))
        }
        WorldChange::Gamerule { rule, value } => {
            let mut rules = world.rules;
            let flag = match rule.to_ascii_lowercase().as_str() {
                "keepinventory" => &mut rules.keep_inventory,
                "domobspawning" => &mut rules.mob_spawning,
                "dodaylightcycle" => &mut rules.daylight_cycle,
                "pvp" => &mut rules.pvp,
                _ => return Err("Rules: keepInventory, doMobSpawning, doDaylightCycle, pvp".into()),
            };
            if let Some(value) = value {
                *flag = *value;
            }
            let text = format!("{rule}: {flag}");
            if value.is_some() {
                world.set_rules(rules);
            }
            Ok(text)
        }
        WorldChange::Give { item, count } => {
            if !body.ok_or("The console has no player")?.creative {
                return Err("Use /gamemode creative before /give".into());
            }
            let item = resolve_item(item).ok_or("Unknown item")?;
            let given = give(world, item, *count);
            match count - given {
                0 => Ok(format!("Given: {} × {given}", item.name())),
                left => Ok(format!("Given: {} × {given} · Pack full: {left} not given", item.name())),
            }
        }
        WorldChange::Summon(name) => {
            let body = body.ok_or("The console has no player")?;
            if world.adventure.is_dead() {
                return Err("Respawn before summoning".into());
            }
            let (kind, label) = adventure::species_named(name).ok_or("Unknown creature")?;
            let scale = world.voxel_scale.meters();
            let ahead = body.player.forward_dir() * glam::Vec3::new(1.0, 0.0, 1.0);
            let feet = body.player.pos * scale + ahead.normalize_or_zero() * 3.0;
            adventure::summon(world, kind, feet)?;
            Ok(format!("Summoned: {label}"))
        }
        WorldChange::WorldBorder(metres) => {
            // Session-only, so the rules sidecar is left as it is.
            world.rules.border_m = *metres;
            Ok(if *metres == 0 { "World Border: Off".to_owned() } else { format!("World Border: {metres} m") })
        }
        WorldChange::Status(op) => {
            use crate::world::progression::effects::{Kind, MAX_LEVEL, MAX_SECONDS};
            body.ok_or("The console has no player")?;
            match op {
                StatusOp::Clear => {
                    world.progression.effects.clear();
                    Ok("Effects: Cleared".into())
                }
                StatusOp::Give { name, seconds, level } => {
                    let kind = Kind::parse(name)
                        .ok_or("Effects: regeneration, speed, slowness, strength, weakness, poison, fire_resistance")?;
                    if *seconds > MAX_SECONDS {
                        return Err(format!("Seconds: 1–{MAX_SECONDS:.0}"));
                    }
                    if *level > MAX_LEVEL {
                        return Err(format!("Level: 1–{MAX_LEVEL}"));
                    }
                    world.progression.effects.add(kind, *seconds, *level);
                    Ok(format!("Effect: {} {level} · {seconds:.0}s", kind.name()))
                }
            }
        }
    }
}

/// Moves a body to `cells` at rest, without the fall it was in.
pub fn place(player: &mut Player, cells: glam::Vec3) {
    player.pos = cells;
    player.precise_pos = cells.as_dvec3();
    player.vel = glam::Vec3::ZERO;
    player.movement.fall_speed = 0.0;
    player.air_seconds = 0.0;
    let _ = player.take_pending_harm();
}

/// Inserts up to `count` of `item` into the mounted pack, one stack at a time.
fn give(world: &mut World, item: Item, count: u32) -> u32 {
    let limit = world.inventory.stack_limit(item).max(1);
    let mut given = 0;
    while given < count {
        let (inserted, remaining) = world.inventory.try_insert_pack_first(ItemStack::new(item, (count - given).min(limit)));
        given += inserted;
        if remaining > 0 || inserted == 0 {
            break;
        }
    }
    world.sync_from_inventory();
    if let Item::Weapon(tier) = item
        && given > 0
    {
        let _ = world.adventure.select_weapon(tier as u32);
    }
    given
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Catalogue item or placeable block named `name`, ignoring case, spaces and underscores.
pub fn resolve_item(name: &str) -> Option<Item> {
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let wanted = key(name.trim_start_matches("minecraft:"));
    if let Some(item) = crate::egui_ui::creative_items().iter().copied().find(|i| key(i.name()) == wanted) {
        return Some(item);
    }
    engine_content::ALL_BLOCK_IDS
        .iter()
        .copied()
        .find(|b| key(&format!("{b:?}")) == wanted || key(b.display_name()) == wanted)
        .and_then(Item::canonical_block)
        .map(Item::Block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldType;
    use engine_net::commands::Coord;

    fn body(player: &mut Player) -> Option<Body<'_>> {
        Some(Body { player, creative: true })
    }

    #[test]
    fn teleport_resolves_relative_axes_and_rejects_the_void() {
        let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::HalfMeter);
        let mut player = Player::new();
        player.pos = world.suggested_spawn();
        player.vel = glam::Vec3::ONE;
        let mut clock = DayNightCycle::default();
        let start_m = player.pos * world.voxel_scale.meters();
        let up = WorldChange::TeleportTo([
            Coord { relative: true, value: 0.0 },
            Coord { relative: true, value: 4.0 },
            Coord { relative: false, value: -12.0 },
        ]);
        apply(&up, &mut world, body(&mut player), &mut clock).unwrap();
        let now_m = player.pos * world.voxel_scale.meters();
        assert!((now_m - glam::Vec3::new(start_m.x, start_m.y + 4.0, -12.0)).abs().max_element() < 1e-3, "metres, not cells");
        assert_eq!(player.vel, glam::Vec3::ZERO);
        let void = WorldChange::TeleportTo([Coord { relative: false, value: 0.0 }, Coord { relative: false, value: -100_000.0 }, Coord { relative: false, value: 0.0 }]);
        assert!(apply(&void, &mut world, body(&mut player), &mut clock).is_err());
    }

    #[test]
    fn bodies_take_effects_and_the_console_sets_the_border() {
        let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
        let mut player = Player::new();
        let mut clock = DayNightCycle::default();
        assert!(apply(&WorldChange::WorldBorder(200), &mut world, None, &mut clock).is_ok());
        assert_eq!(world.rules.border_m, 200);
        let speed = WorldChange::Status(StatusOp::Give { name: "speed".into(), seconds: 30.0, level: 1 });
        assert!(apply(&speed, &mut world, body(&mut player), &mut clock).is_ok());
        assert_eq!(world.progression.effects.level(crate::world::progression::effects::Kind::Speed), 1);
        let long = WorldChange::Status(StatusOp::Give { name: "speed".into(), seconds: 99_999.0, level: 1 });
        assert!(apply(&long, &mut world, body(&mut player), &mut clock).is_err());
        assert_eq!(apply(&speed, &mut world, None, &mut clock), Err("The console has no player".into()));
        assert!(apply(&WorldChange::Weather(engine_net::commands::Weather::Rain), &mut world, None, &mut clock).is_ok(), "the console sets weather");
    }
}
