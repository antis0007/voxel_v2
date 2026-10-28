# Minecraft core features missing here, measured against Horizon

Date: 2026-10-28. Source: [horizon-rust/horizon](https://github.com/horizon-rust/horizon) at `62898b4` (MIT), a Rust rewrite of Minecraft Java 26.3 that claims parity with vanilla over 281 differential harnesses. Horizon therefore stands in for "vanilla Java plus a few extras". Our side comes from an audit of this tree on the same day; paths are under `crates/app/src/` unless they say otherwise.

Status words: **Absent** (no code), **Partial** (some of it, or a different model), **Different** (we cover the need on purpose with our own system; listed so nobody rebuilds it by accident). Horizon paths use `client/` = `crates/horizon-client/src/`, `game/` = `client/game/`, `world/` = `crates/horizon-world/src/`.

## 1. Gaps in the core game

| Feature | Horizon | Ours | Notes |
| --- | --- | --- | --- |
| Block states: stairs, slabs, fences, walls, general rotation | block facts and survival rules (`world/level/survive.rs`) | Partial | Only doors, trapdoors, lever and wall torches (`content/src/fittings.rs`, `world/fittings.rs`), each as its own `BlockId` |
| Item durability | `world/item_durability.rs` | Partial | `crates/tools` has it, but nothing in the app calls it and saves do not store it; `ItemStack` has no wear |
| Block crack animation | `client/breaking.rs` | Partial | HUD progress bar only (`egui_ui/tool_hud.rs`) |
| General item chest | `client/containers.rs`, spill on break (`game/spill.rs`) | Partial | Output crate holds resource indices only; no chest for blocks, gear or food |
| Offhand slot | `world/player_inventory.rs` | Absent | |
| Portable storage (shulker, bundle) | `crates/horizon-items/src/bundle.rs` | Absent | |
| XP orbs, levels, XP bar | 25 files | Absent | Only habitat "XP" as a resource |
| Hunger saturation | `world/player_food.rs` | Absent | Hunger exists; drains only with Hard difficulty |
| Player status effects and potions | `world/active_effects.rs`, `game/potions.rs` | Partial | Elemental statuses exist for enemies only (`world/adventure/status.rs`) |
| Brewing | `client/brewing.rs` | Absent | |
| Anvil repair and renaming | `client/stations.rs` | Absent | Repair logic sits unused in `crates/tools` |
| Enchanting table | `crates/horizon-items/src/enchant.rs` | Different | Wand decks and weapon modifiers (`world/adventure/tinkering.rs`) |
| Grindstone, stonecutter, loom, cartography, smithing trims | `client/stations.rs`, `game/loom.rs` | Absent | Smelter weapon fitting is the nearest |
| Bow, crossbow, arrows, trident | `game/crossbow.rs`, `client/arrows.rs` | Different | Wand spells take the ranged role |
| Player shield and blocking | `game/shield.rs`, `world/item_blocking.rs` | Absent | Only the Warden enemy has a shield |
| Totem, mace and other special weapons | `game/totem.rs`, `game/mace.rs` | Absent | |
| Villagers, jobs, trading, gossip | `client/mobs/villager*.rs`, `game/trading.rs` | Absent | Planned as N09 in `docs/MASTER_PLAN.md` |
| Raids and patrols | `game/raids.rs` | Absent | Factions planned as N08 |
| Taming, pets, leashes | `client/mobs/` | Partial | A fed wild animal joins the herd; no followers |
| Mounts (horse, llama, camel) | `game/mounts.rs` | Absent | |
| Boats and minecarts with rails | `game/boats.rs`, `game/minecarts.rs`, `world/level/rails.rs` | Absent | Boats in `docs/GAME_DESIGN.md`, rails in `docs/specs` |
| Elytra and fireworks | `game/fireworks.rs` | Different | Kinetic gear gives dash, air jump and wall-run |
| Ladders, vines, scaffolding | climbable blocks | Absent | |
| Saplings and player-planted trees | `world/level/growth.rs` | Absent | Trees are worldgen only |
| Bonemeal | `world/level/bonemeal.rs` | Absent | Soil nutrients exist (`world/environment/cultivation.rs`) |
| Composter, beehives | `game/composter.rs`, `client/beehive.rs` | Absent | |
| Fishing | `game/fishing.rs` | Absent | Mentioned in `docs/GAME_DESIGN.md` |
| Redstone signal: wire, torches, repeaters, comparators, observers | `world/level/redstone.rs` | Partial | Electrical power and chip nets instead (`world/factory/power.rs`, `crates/chip`); no block-level signal |
| Pistons | `world/level/pistons.rs` | Absent | |
| Buttons, pressure plates, tripwire | `world/level/rails.rs`, `redstone.rs` | Absent | The lever toggles fittings within 3 voxels directly, no wiring |
| Dispensers, droppers, crafter | `game/dispenser.rs`, `game/machines/crafter.rs` | Partial | Feed hopper, item pipes and drills exist |
| TNT and block-breaking explosions | `game/explosion.rs`, `game/tnt.rs` | Absent | Spell blasts exist; no placeable explosive |
| Other dimensions and portals | `world/level/portal.rs`, `game/end.rs` | Absent | Undercrown and orbital sit inside one world; planets are coordinates and rendering only |
| Snowfall, freezing, lightning strikes | `world/level/weather.rs`, `game/lightning.rs` | Partial | Rain and storms render and sound; weather changes no gameplay |
| Light levels in blocks (block light and skylight) wired to meshing | `client/light.rs` | Partial | `propagate_light` adds light on torch placement and never removes it, and the mesher does not read `chunk.light`; visible light comes from the renderer |
| Generic random and scheduled block ticks | `world/level/tick.rs`, `crates/horizon-core/src/ticks/` | Partial | Crop rolls and activity wake only |
| World border | border render and rules | Absent | Mentioned in `docs/design/MULTIPLAYER_SOCIAL_20261026.md` |
| Adventure and spectator game modes | `game/` (F3+F4 switcher) | Partial | Survival and Creative only (`world/rules.rs`); noclip comes close to spectator |
| Signs, books, lecterns | `client/signs.rs`, `game/books.rs` | Absent | Bookcase is decorative |
| Map and clock items | `game/maps.rs` | Partial | Map and compass exist as UI, not as items |
| Advancements | `game/advancements.rs` | Absent | |
| Player statistics | `game/stats.rs` | Partial | Factory production stats only |
| Beacons, jukebox, item frames, armour stands, paintings | `game/beacon.rs` and others | Absent | |
| Commands /give, /weather, /summon, /effect | `crates/horizon-command` | Partial | `crates/net/src/commands.rs` has 12 commands, used in hosted sessions only |
| Per-block break and place sounds | `client/sound_engine.rs` | Partial | Synthesised cues; one generic `Mine` cue for all blocks |
| General particle system | `client/gfx/` | Partial | Spell and tool effects only |
| LAN discovery | `client/lan/` | Partial | Direct IP and the relay lobby, no broadcast |
| Resource packs, data packs, mod API | `crates/horizon-mods` (WASM), `client/datapack.rs` | Absent | Content RON is compiled in with `include_str!`, so it cannot be swapped at runtime |

## 2. Horizon extras worth noting

- **Agent control:** `horizon-ctl` reads the game as JSON and plays it (goto, break, click slots), and the same commands serve as an MCP server.
  - Code: `client/control.rs`, `client/bin/horizon-ctl/`.
  - Our nearest equivalents are headless scenarios and `--benchmark-*` review hooks.
- **WebAssembly mods with capabilities:** a mod declares what it may touch (HUD, chat, commands, recipes, storage), and the mod SDK ships official mods for minimap, zoom, a recipe viewer, far view and keystrokes.
  - Code: `crates/horizon-mods`, `crates/horizon-mod-sdk`.
- **Server supervisor:** backups and restore, auto-restart, signed updates with rollback, and a web panel showing TPS, MSPT and RAM.
  - Code: `crates/horizon-server`.
- **Play with Friends:** invite codes over an end-to-end encrypted tunnel, with a relay fallback.
  - Code: `crates/horizon-friends`, `crates/horizon-relay`.
- **Controller extras:** DualSense adaptive triggers and haptics, and an on-screen keyboard.
  - Code: `crates/horizon-dualsense`, `client/osk.rs`.
  - We have basic gamepad support (`gamepad.rs`).
- **Discord Rich Presence:** `client/discord_presence.rs`.
- **Differential parity harnesses:** 348 `tools/test_*_parity.py` scripts. This is the closest match to our A/B capture discipline.

## 3. Already covered here

These came up in the comparison and need no work:
- **Systems:** tool tiers, stack sizes, dropped items, shaped crafting with a recipe browser (R/U), armour, creative catalogue.
- **Player state:** health, regeneration, fall damage, breath, fire and lava damage, death and respawn, beds, difficulty, gamerules.
- **Mobs:** melee cooldown, knockback, hostile and passive mobs with pathfinding, breeding, bosses, structures with loot.
- **World:** biomes, ores, day and night, falling blocks (cellular).
- **Saves:** versioned saves with upgrades, a world list, seeds.
- **UI:** F3, chat, keybind remapping, gamepad, zoom, minimap and world map, screenshots, subtitles, music, ambience, positional audio, dedicated server, auth.
- **Rendering:** far terrain, HDR post-processing, clouds and sky, held items.

Fluids are deliberately different: mass-conserving cellular water rather than source and flow blocks.

## 4. Smallest steps with the most reach

These close common Minecraft expectations without fighting our design:
1. Item durability through `crates/tools`, stored in saves.
2. A general item chest.
3. Ladders.
4. Saplings and bonemeal on top of the existing crop roll.
5. Offhand.
6. XP and an XP bar.
7. Signs.

Larger items such as redstone signals, villagers, dimensions and a mod API need an owner decision first; several already have plan entries in `docs/MASTER_PLAN.md`.
