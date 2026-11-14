# Fantasy battle sandbox: MTG-style summoning, TABS-style battles, and creative army tools

Status: design, 2026-11-06. Owner direction: a fantasy battle sandbox where Magic-the-Gathering
style summoning meets Totally-Accurate-Battle-Simulator style configurable fights between
custom armies, in the fully destructible voxel world, with creative army tools.

Extends, and must agree with:

- [Army command and battles](2026-11-06-army-command-and-battles-design.md) (if written; the
  division/order/formation model from the same direction).
- [Walls, sieges, epochs and other strategy inspirations](2026-11-06-walls-sieges-ages-and-rts-inspiration-design.md)
  (fortifications, destruction, the counters from the prototype).
- [World AI, companions and conquest](2026-11-06-world-ai-companions-and-conquest-design.md)
  (the three tiers, one owner per quantity, the F1 rule).

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 0. Decisions taken (defaults; owner may override)

| # | Question | Decision |
|---|---|---|
| D1 | Where the arena lives | a **game mode** (a `GameMode` in `crates/app/src/world/rules.rs`, beside survival, creative, adventure and spectator) registered in `GameMode::ALL`, selectable and enterable as a scene; not a separate save format |
| D7 | Survival summoning | **not a survival activity**, except at a very high level with a **custom summon spell** the player composes and modifies with spell mods (see section 3.0) |
| D2 | Deck depth | a **lean spellbook with costs** (no hand, library or graveyard); depth comes from keywords, creature types, counters and terrain, not card economy |
| D3 | Summon default | a **real entity (permanent)**; temporary tokens are opt-in with a duration |
| D4 | Unit editor scope (v1) | **stats and keywords**; appearance and models later |
| D5 | Scale budget | roughly **300 individually simulated units** per staged battle, up to about **1,000** with the world-AI design's LOD tiers |
| D6 | Names | keywords, cards and units keep **placeholder names**; any new proper noun needs owner approval |

Off limits until the owner clears them: `crates/app/src/world/adventure/growth.rs` and
`crates/app/src/egui_ui/wand_xmb.rs` (the latter holds the Summons category UI).

## 1. The vision

A TABS-style arena inside the one voxel world: stage two custom armies, press Play, and watch
a physics- and cellular-automaton-driven, fully destructible brawl between fantasy units and
summoned creatures, with creative army tools and a bridge back into the campaign.

Four ideas, one system:

- **Summoning** (Magic the Gathering): a spellbook of cards; casting conjures real creatures.
- **Battle staging** (TABS): place two sides, set a budget and a win condition, watch it play.
- **Destruction**: the cellular world and rigid bodies own the consequences; walls break,
  terrain collapses, fire spreads.
- **Creative tools**: spawn, paint, formation drag, unit editor, scenario save and load.

## 2. What exists (audited 2026-11-06)

| Piece | Where | State |
|---|---|---|
| Summoning function | `crates/app/src/world/adventure.rs` (`summon(world, kind, feet_m)`) | L |
| `/summon creature`, `WorldChange::Summon` | `crates/app/src/local_commands.rs`, `crates/app/src/world_commands.rs` | L |
| Spell cards, Summons category (`SUMMONS = [36]`) | `crates/app/src/world/adventure/wand_items/`, `crates/app/src/egui_ui/wands.rs`, `assets/spells.ron`, `crates/app/src/world/adventure/spell_defs/card.rs` | L |
| Creature data (role, health, body, speed, range, windup, cooldown) | `assets/bestiary.ron` | L |
| Spawn table (habitat, weight, biome, realm, time) | `assets/spawns.ron` | L |
| Creative mode and tools (`ToolKind`, `BrushShape`, `AreaFootprintShape`, `CreativeState`) | `crates/app/src/creative.rs` | L |
| Gamemodes survival, creative, adventure, spectator | `crates/app/src/local_commands.rs` | L |
| Destruction (cellular water/sand/fire, rigid bodies, thermal, collapse) | `crates/sim`, `crates/app/src/simulation/`, `crates/app/src/world/effects.rs` | L |
| Teams, arena builder, unit editor, points budget, army save/load | searched across `crates/app/src` | Absent (`faction` zero; no staging layer) |

## 3. The design

### 3.0 Game mode and survival gating

The sandbox is a **game mode**, registered beside the existing `GameMode` values in
`crates/app/src/world/rules.rs` (`Survival`, `Creative`, `Adventure`, `Spectator`) and listed
in `GameMode::ALL`, so the world-rules screen and `/gamemode` pick it up like any other.

- **In the battle mode**: army command, the arena palette, placement tools, the unit editor
  and the sim controls are all available.
- **In survival**: none of that. Survival stays the modpack loop. The only bridge is a
  **custom summon spell** unlocked at a very high level.
- **Custom summon spells.** A survival player at a very high level can compose a summon spell:
  a base creature kind plus a stack of **spell mods**, the existing modifier rows the wand
  already carries (`assets/spells.ron` holds the modifier rows; the wand composes a base with
  modifiers, in `crates/app/src/world/adventure/wand_items/`). Mods change the count, the cost,
  the duration and the creature's keywords (Flight, Reach, Haste, and so on).
- **Mods stack with rules.** A mod applies once per spell and within caps, so a summon cannot
  be made free or unbounded.
- The same spell content is used in the battle mode, where the palette offers it directly.
- The spell carries the same `unlock:` gate the other spells use (`assets/spells.ron`), so a
  high-level custom summon is a real unlock, not a menu toggle.

### 3.1 Summoning (Magic the Gathering inspired)

- A **Summoner** (the player, or a companion commander) holds a **spellbook** of summon cards.
  A card is cost + creature kind + count + **keywords** + abilities, reusing the existing card
  model and the Summons category in `assets/spells.ron`.
- **Costs from real resources.** A card spends mana and materials. No new universal currency.
- **Keywords** are new columns on creature and spell rows, not a new system: *Flying, Reach,
  Trample, Haste, Braced, Fragile*, plus simple abilities (an on-death burst, an **aura** that
  binds a modifier to a creature or a place).
- **Tokens and permanents.** A summon is a **real entity** through `summon()`, so the cellular
  world and physics own it. A card may be temporary (a token with a duration) or permanent
  (the default, D3).
- **Summoning sickness.** A fresh summon cannot act for a short interval, a status that ends
  on the tick. This is the one MTG rule worth keeping, because it stops a summon from being a
  free instant kill.
- The card is a promise; the creature is real.

### 3.2 Battle staging (TABS style, in the battle mode)

- **The battle mode.** A game mode of its own (D1). Stage **Blue** and **Red** units, then
  press Play.
- **Unit palette.** Bestiary creatures, army division types and summoned creatures, with a
  **points budget** per side.
- **Placement tools**, extending `crates/app/src/creative.rs`: spawn brush, clone, erase,
  **drag-to-formation** (a drag defines a footprint; a modifier or the drag shape picks the
  formation), **team paint**, a facing dial, props and ladders, terrain sculpt and blocks.
- **Unit editor** (D4): role, size, health, speed, range, keywords and abilities, saved as a
  RON content row, so a custom unit is content like every other unit.
- **Scenario editor**: terrain, initial units, a **win condition**, a time limit and hazards,
  saved and loaded through the existing scenario and sidecar machinery.
- **Simulation controls**: play and pause, speed and slow motion, tick step, restart, and a
  deterministic **seed**; a free-fly and follow camera; and **possess a unit** to drop into
  first person (the seam with the main game).

### 3.3 Fully destructible environment

Combat damage writes **voxels** through the fortification body, so a battle breaks walls,
collapses terrain, spreads fire and floods channels with the systems already present
(`crates/sim`, `crates/app/src/simulation/`, `crates/app/src/world/effects.rs`). Destruction
is bounded by the same budgets and LOD as the rest of the world, so a large staged fight does
not leave the frame budget.

### 3.4 Configurable fights between custom armies

- **Army builder**: units, counts, a budget cap, stances, formations and leaders per side;
  armies save and load.
- **Win conditions**: annihilate, capture a relic, hold a point, survive a timer, break a wall.
- **Determinism**: the same seed and setup give the same battle, checked by a headless scenario
  with a state hash, matching the existing scenario runner.

### 3.5 Bridges back to the campaign

- A staged battle can be **exported as a real engagement**: the two armies become world armies
  (the army and strategic direction) that resolve in the campaign.
- **Summoning is a campaign capability**: the spellbook you carry. The arena is where you test
  it; the world is where it counts.
- Creature rows, keywords and costs are the **same content** in both modes, so nothing is
  duplicated.

### 3.6 Keeping the main game first-person (F1)

- The arena is a **world mode**, never forced; the campaign is untouched.
- **Possession** (drop into any unit) is the TABS to first-person seam, inside the destructible
  world.
- Everything is **budgeted** and costs only while the sandbox runs.

## 4. Mapping to the engine and the prototype

| Feature | Reuses |
|---|---|
| Summon by card | `crates/app/src/world/adventure.rs::summon`, spell cards, `assets/bestiary.ron` |
| Arena placement tools | `crates/app/src/creative.rs` (`ToolKind`, `BrushShape`, `AreaTool`) |
| Teams and battle setup | new `Team`, `BattleSetup` records (prototype `battle`) |
| Units editor | a new unit content row in `assets/` |
| Armies, divisions, formations | the army direction and `civ_proto` (`counters`, `fortify`, `territory`) |
| Destruction | `crates/sim` cellular world, `crates/app/src/simulation/`, `crates/app/src/world/effects.rs` |
| Scenarios and saves | the existing scenario runner and sidecar pattern |
| Game mode | `crates/app/src/world/rules.rs` (`GameMode`, `GameMode::ALL`), `/gamemode`, the world-rules screen |
| Spell mods | the modifier rows in `assets/spells.ron` and the wand composer (`crates/app/src/world/adventure/wand_items/`) |

The pure prototype already carries `counters`, `fortify`, `territory`, `epochs`, `pawn`,
`brain`, `politics`, `worldmap`, `settlement`. Slices B1 to B3 add `summon`, `keywords` and
`battle`, so the whole staging model is testable with no engine coupling.

## 5. Slices

| # | Deliverable | Acceptance |
|---|---|---|
| B0 | The battle game mode | a `GameMode` registered in `GameMode::ALL`, selectable and enterable; survival refuses the arena tools |
| B1 | Summon by card | a card summons the right kind, the pool is debited, the sickness applies, a token expires and a permanent does not |
| B2 | Keywords and abilities as content rows | a keyword changes behaviour; a RON round-trip test |
| B3 | Team and BattleSetup records | two sides, a budget cap, a win condition; an over-budget army is refused with a why |
| B4 | Placement tools in creative | spawn brush, clone, erase, team paint; capture |
| B5 | Drag-to-formation placement | a straight drag places a line; a blob places a square |
| B6 | Unit editor to content row | RON round-trip; the unit spawns with its stats |
| B7 | Play, pause, speed and determinism | the same seed gives an identical state hash |
| B8 | Destruction assertion | a battle breaks a wall; the voxels and the fortification record agree |
| B9 | Scenario save and load | a reload restores both armies exactly |
| B10 | Export to a campaign engagement | the staged battle becomes a world army |
| B11 | Custom summon spells with spell mods | a composed base plus mods changes count, cost, duration and keywords; a duplicate mod is refused; survival casts it only above the level gate |

B1 to B3 and B5 are pure logic and can be built and tested like the existing prototype. Each
slice lands behind the repo pre-commit path: `just check`, `just clippy`, `just test <name>`,
a headless scenario, then `just verify`, plus captures for any visible change.

## 6. Risks and open follow-ups

- **Naming** stays placeholder until the owner approves keywords, cards and units (D6).
- **Scope**: the arena must not become a second game. It shares creatures, cards, armies,
  destruction and scenarios with the campaign, and exports back.
- **Cost**: the arena is a mode, budgeted, and free to the campaign when closed.
- **Off limits**: `crates/app/src/world/adventure/growth.rs` and
  `crates/app/src/egui_ui/wand_xmb.rs`.
- **Follow-ups for the owner**: confirm D1 to D5; decide whether the arena ships with a small
  set of authored scenarios; decide whether possession is a full mode or a spectator pose.


