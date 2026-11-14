# Undercrown inhabited — enemies, loot and spells (design)

Companion to `2026-11-12-undercrown-overhaul-design.md`. That plan lists "combat or AI rework" as a non-goal and fixes the props, layout loops and lighting. This plan fills the spaces it builds with things to fight, flee from and find.

## Why

Owner, 2026-11-13: the Undercrown "feels bleak and barebones, unfinished" compared with our plans and with dungeon-overhaul modpacks. The owner wants a ten-minute visit to mean finding items and spells, and seeing unique, intimidating enemies that may or may not give chase or hunt you.

What exists today (survey, 2026-11-13):
- No Undercrown-specific enemies. Rooms only get the generic `Realm::Underground` ambient table in `assets/spawns.ron`, mostly at night.
- No chests, spawners, found wands or spell drops. Dungeons have all of them (`worldgen/src/dungeon.rs`, `adventure/wand_items.rs::drop_roll`, `adventure/discovery.rs::drop_roll`).
- Loot is 10 one-shot plinths (`assets/undercrown_rewards.ron`). The relics they name have no item of their own.
- There is one spell discovery (spell 95 at district 1, room 9) and one boss-like encounter (the observatory, a synthetic site id `IVec3(99999, 99999, 5)`).
- Unused building blocks already in the engine:
  - burrowers with Roam/Stalk/Lunge and footstep sensing through rock (`burrowers.rs`);
  - five encounter recipes, including a cache mimic and a cistern stalker (`encounters.rs`);
  - the boss wiring path (Old Copper, `thunderhen.rs`, `bosses.rs`);
  - guardian, relic and reward persistence (`reliquaries.rs`, `rewards.rs`).

## Research borrowed (patterns, not IP)

| Source | Pattern | Use here |
|---|---|---|
| Alien: Isolation | A director nudges a predator toward the player's area. The predator must find the player by its own senses. A menace gauge peaks, then the predator is sent away. | Stalker with a designed give-up phase |
| Minecraft Warden, Deeper and Darker | Blind, noise-driven anger that decays. Player noise gates the pressure. | A noise meter fed by mining, casting, sprinting and mechanisms |
| Alex's Caves | A per-biome roster of 3–5 mobs, one gear-gated hazard, and a book entry on first sighting | A district roster plus a bestiary entry |
| Minecraft trial chambers | A fight drops a key. A vault opens once per player. An ominous remix gives rarer loot. | Trial room plus vault |
| Noita, Hades | Hand-made rooms offer a spell or wand choice. The door previews the reward type. | Spell alcoves and a choice room |
| Hollow Knight charms, Terraria biome chests | Relics with real effects. A key comes from the area's own enemies. | District relics that change traversal |
| Deep Rock Galactic | Objective-triggered waves followed by a lull | Mechanism-triggered surge |
| Dwarf Fortress forgotten beasts, Cataclysm arenas | One unique lair creature per area with readable phases | District apex creature |

## Goals

- **E1. Every district has its own roster.** It has 3–4 creature roles and at least one that hunts.
- **E2. Hunters can lose you.** Each hunter detects by sound and/or light. It has an observable alert state and a designed give-up. Sneaking, darkness and breaking line of sight are real counterplay.
- **E3. The player controls pressure.** A per-district noise meter decays over time. It rises with mining, casting, sprinting, combat and operating mechanisms. Pressure never comes from a wall-clock timer.
- **E4. A reward beat every 2–3 rooms.** This uses the overhaul plan's pacing rule. Rewards come in three tiers: niche caches, chests at the end of a loop, and vault or apex rewards.
- **E5. Spells are found here.** Each district places at least two spell finds (an alcove scroll or a reliquary) and one found-wand chance.
- **E6. Relics do something.** The 10 plinth relics become real items with one effect each. At least two of those effects open routes.
- **E7. Telegraphs before danger.** Every ambusher has a tell, such as a sound, a block that is slightly off, or a bone pile. Every hunter has a distinct sound that can be heard before it is seen.

## Non-goals

- A general AI rework outside the Undercrown. New behaviour is scoped to Undercrown creatures, but can be reused later.
- An NPC dialogue system.
- The Hollow Sea district. The overhaul plan reserves it.
- Multiplayer-specific balancing beyond existing enemy replication.

## Anti-goals (failure even if it works)

- Constant chase. If a hunter is always on you, the give-up phase failed.
- Cheap death from an untelegraphed ambush.
- Numeric loot bloat. A reward must be a capability (spell, wand, relic) or useful material, not +2% stats.
- Enemies spawning inside a lit, player-secured room. Origins must be visible: lairs, nests, water, vents.
- Breaking old worlds. Old gates must generate and spawn identically.

## Constraints

- **Generator gate.**
  - Placement of lairs, chests, alcoves and trial rooms goes behind a new gate, `UNDERCROWN_INHABITED_GENERATOR`.
  - Claim the gate in `docs/worldgen/GENERATOR_GATES.md` first (301 is the highest seen on 2026-11-13) and grep every branch.
  - The gate is dormant until the owner approves the first district.
- **Save compatibility.**
  - Read the current save version constant before touching state.
  - Chest, alcove, vault and relic state rides on the existing `undercrown/state.rs` object flags, keyed by `ObjectId` (CLAIMED, OPEN, DISCOVERED).
  - Kills use the site/slot defeat records with a synthetic Undercrown site id, which is the observatory pattern.
  - New item ids (relics) need a save bump. Batch them into one bump.
- **Content in RON.**
  - Rosters, loot tables, relic effects and creature strains go in `assets/*.ron`: new `assets/undercrown_life.ron` plus rows in `bestiary.ron`, `burrowers.ron` and `undercrown_rewards.ron`.
  - Note that `encounters.rs` currently keeps its recipes as a Rust `RECIPES` table. This plan moves new recipes into RON. The existing five may stay where they are.
- **Determinism.** Placement is a pure function of the plan seed and `ObjectId`. AI and director logic run on the fixed sim step with positional seeded hashes and no wall-clock.
- **Names.** Every new creature, relic and place name needs owner approval before it ships. This document uses role labels. Existing names (Cistern Stalker, Patient Cache Mimic, district names) may be reused.
- **UI text.** Follows `egui_ui/UI_RULES.md`.
- **Visuals.** New creature models and props ship with A/B captures from `VOXEL_REVIEW_DUNGEON=undercrown:<room>` poses. The owner approves the looks.
- **Performance.** Live Undercrown enemies are capped (proposed: 10 active, including ambient). Frame time in review rooms must be no worse than baseline. Measure with interleaved passes.

## Design

### 1. Placement layer (the missing piece)

A pure planner, `undercrown/life.rs` in worldgen, assigns *life anchors* to rooms by the role each room already has in the overhaul plan:

| Room role | Anchors placed |
|---|---|
| Arena | 1 encounter group from the district roster, or a trial anchor |
| Vault | 1 vault (locked; its key comes from a trial or apex elsewhere on the loop) |
| Secret | 1 cache chest or spell alcove |
| Traverse | 0–1 ambusher with a tell |
| Rest | none (safe; ambient spawns are suppressed in a radius) |
| Vista | 0–1 hunter lair visible from the vista, a "see it before it sees you" beat |
| Side niche (any role) | minor cache, about 1 in 3 |

Each anchor is `{ object_id, kind, roster_row, loot_row }`. Worldgen stamps only the static parts: chests, alcoves, lair dressing such as bone piles and nests, and trial plates. The app spawns creatures from anchors when the player is within the activation radius, using the same per-slot defeat records so kills persist.

### 2. Creature roles (per district roster)

Every roster is built from these roles. Most reuse existing motions.

| Role | Behaviour | Built from |
|---|---|---|
| **Hunter** | Roams its territory and investigates noise and light. Chases on sight and gives up when menace peaks or contact is lost. Returns to its lair. | New `hunter` state machine on top of navigation and burrower sensing |
| **Ambusher** | Concealed until triggered by proximity, a step or opening a chest. It has a tell. | Existing `encounters.rs` mechanics (RubbleAmbush, CacheAmbush, Interceptor) |
| **Swarm** | Small, weak, numerous. Comes from nests at the room edges during a surge. | Existing night-mob kinds with a district palette |
| **Sentry** | Static or slow. On detecting the player it calls the district hunter, or raises noise sharply. | Existing Suppressor/Hexer with a new "alarm" effect |
| **Apex** (one per district) | Lair arena with telegraphed phases and a signature drop. | Boss wiring path (Old Copper template) |

### 3. Hunter state machine and director

The hunter has five states: `Dormant → Roam → Investigate(point) → Chase → Withdraw → Roam`.

- **Senses.**
  - Hearing: player noise events within a radius scaled by event loudness. Damping through rock reuses the burrower `SENSE_M` idea.
  - Sight: a view cone with line-of-sight raycast. Range scales with the light level at the player's position, so torches and glowing spells make you visible from farther away.
  - Each district picks one sense as dominant, so the rule is learnable (see section 6).
- **Menace.** A per-district scalar that rises while the hunter is close or has line of sight and decays otherwise. At the peak threshold the hunter enters `Withdraw`: it retreats to its lair or through a water or vent exit for a cooldown, then resumes `Roam`.
- **Director.** It never teleports the hunter. It only biases `Roam` waypoints toward the room graph node the player is in, weighted by the noise meter.
- **Telegraphs.**
  - Each state has a distinct sound: a distant roam call, a sharp investigate cue, a chase roar, a withdraw fade.
  - Proximity has a heartbeat-style audio layer.
  - Optional HUD: none. Audio and the creature itself carry it.
- **Counterplay.**
  - Crouching halves your noise.
  - Dousing or not carrying light shrinks sight range.
  - Breaking line of sight and staying quiet for N seconds drops `Chase` to `Investigate`.
  - Rest rooms are never entered.

### 4. Noise meter

Per district, held in the sim. It decays over time. Events add to it, with weights in RON: mining a block, casting by spell mana, sprinting steps, landing from height, combat hits, operating a mechanism (large), opening a chest (small).

The meter drives three things:
- director waypoint bias;
- surge chance;
- sentry sensitivity.

It is transient and not saved.

### 5. Loot and spells

- **Caches.** Minor niches drawn from a district loot table in RON. Contents are materials, food, potions and torches. They are one-shot via the CLAIMED flag.
- **Chests.** Placed at the end of a loop, using the existing dungeon chest blocks. The loot table rolls one capability roll (`wand_items::drop_roll` or `discovery::drop_roll` seeded by `ObjectId`) plus materials.
- **Spell alcoves.** A visible scroll on a lit lectern (overhaul goal G2: visible before claimed). Claiming adds the spell to the found-spell log. Each district authors which spell ids can appear. Ids come from `assets/spells.ron` and stay within the existing 0–95 found-spell range.
- **Trial room.**
  - A sealed arena with a trial plate. Stepping on it starts N waves from the roster.
  - Clearing it drops a district key item, a reuse of the dungeon-bound key concept.
  - The key opens that district's vault once.
  - Proposed rule: the trial can be re-run after a long in-game cooldown measured in sim ticks, but the vault opens once per world.
- **Vault.** Holds the district relic plus one guaranteed found wand at a tier scaled by district depth.
- **Relics.** The 10 `undercrown_rewards.ron` rows become real items. Each has exactly one effect, defined in RON and applied by an existing buff or modifier path. Examples, effects only, names pending:
  - Drowned Borough: longer breath underwater, and you can see the hunter's wake. Opens the flooded streets before the sluice is lowered.
  - Ashen Foundry: heat resistance on lava-adjacent blocks. Opens the furnace route.
  - Frozen Choir: quieter footsteps on rime (noise ×0.5).
  - Glass Archive: spell light no longer raises sight range.

### 6. District rosters (role labels; names for approval)

| District | Dominant sense | Hunter | Ambusher | Swarm/Sentry | Apex | Hazard |
|---|---|---|---|---|---|---|
| Drowned Borough (**first**) | Hearing (blind, through water) | Water hunter: swims canals, lunges at noise near the water edge (burrower `Swim` strain) | Cistern Stalker (existing Interceptor) | Swarm of small eels from drains during a surge | Courthouse lair beast, reached after the final sluice stage | Deep water: breath, slowed movement |
| Crown Chasm | Sight | Cliff hunter that climbs walls and drops from above | Rubble ambusher on ledges | Bell sentry: wakes the hunter when you ring it carelessly | Bridge guardian | Falls, wind gusts |
| Rootbound Halls | Hearing through rock | Burrower (`Burrow`) under the halls | Root-snare ambusher | Swarm from root nests | Counterweight-shaft lair | Collapsing floors |
| Lantern Gardens | Light, inverted: it hunts in darkness and fears the lanterns | Dark-dweller | Cache mimic among planters | Moth swarm drawn to your torch | Orchard-heart apex | Spores |
| Ashen Foundry | Heat | Furnace hound | Kiln Trapper (existing Hexer) | Slag-mites | Foundry engine beast | Lava, heat |
| Glass Archive | Light (lens) | Reflection stalker that moves through light beams | Glass mimic | Archive sentry (alarm) | Archive keeper | Blinding light beams |
| Frozen Choir | Sound (resonance) | Rime singer that hunts by echo | Ice-encased ambusher | Shard swarm | Choir apex | Cold |

### 7. First district: Drowned Borough

This is a vertical slice that ships before any other district.
- Lair anchors and a water-hunter strain (RON row in `burrowers.ron` plus a hunter state).
- Cistern ambushers in cisterns and canal side rooms.
- Each sluice stage is a mechanism, so lowering the water causes a noise spike and a drain-eel surge, followed by a lull and a chest.
- Two spell alcoves, three caches, one chest, one trial room with its key, and one vault holding the relic and a found wand.
- An apex lair in the courthouse, reachable after the last sluice stage.
- Acceptance: a 10-minute route through the district on the review seed hits at least 4 reward beats, at least 1 hunter chase that ends in a withdraw, at least 1 telegraphed ambush and at least 1 spell find. This is checked by a scripted playtest plus captures.

### 8. Dev tools

- `VOXEL_REVIEW_DUNGEON=undercrown:<room>` gains a `life` overlay. Its log lists anchors per room, and a debug draw shows hunter state, menace, noise and sight cone.
- The engine workbench gets a `life` op with three uses: dump anchors for a district, force the hunter state, and set the noise meter.
- The Wand Lab balance sheet gets "vs district" kill-time columns, using bestiary health rows for the new creatures. Loot wands and alcove spells can then be checked against what they will face.

### 9. Acceptance criteria

- **Placement.** On the review seed, every Arena, Vault and Secret room in a done district has its anchor. No anchor lies in a Rest room. Test over the plan.
- **Old gates.** Below the new gate, the plan generates and spawns byte-identically. Existing fixtures plus `just test save`.
- **Give-up.** In a headless sim, a hunter chasing a stationary hidden player reaches `Withdraw` within a bounded number of ticks after line of sight is lost. A visible, noisy player is re-acquired.
- **Determinism.** The same seed and the same input log produce the same hunter path.
- **Pacing.** For each done district, reward beats per room are no lower than 1/3 along the main loop.
- **Visuals.** A/B captures for each new creature model and lair; the owner approves.
- **Performance.** Frames in the borough review rooms with the roster active show p95 no worse than baseline plus agreed slack (proposed +0.5 ms).

### 10. Delivery slices

1. **Placement layer plus loot.** `life.rs` anchors, caches, chests, spell alcoves and the dormant gate. The visible reward is immediate, with no AI work.
2. **Noise meter plus hunter state machine**, using one hunter (the borough water hunter) and debug overlays.
3. **Drowned Borough full slice:** ambushers, surge, trial and vault, apex, relic items (save bump). The owner plays it.
4. Other districts in order: Crown Chasm, Rootbound Halls, Ashen Foundry, Lantern Gardens, Glass Archive, Frozen Choir. Each is its own reviewable slice.
5. Wand Lab "vs district" columns (can land at any point after slice 3).

## Settled decisions (Claude's calls, owner may override)

- One district first (Drowned Borough), because it already has a staged map-changing mechanism, water for a blind hunter, and existing cistern ambushers.
- The trial can be re-run, but the vault opens once per world.
- Hunters never teleport and never enter Rest rooms.
- No HUD threat meter. Audio carries the tension.
- **One dormant gate** for the whole borough slice (slices 1–3). Nothing has been generated at a dormant gate, so its layout may change freely until it is bumped. It is bumped only after the owner approves the borough. Every later district claims its own gate.
- **Trial is single-run per world**, overriding the re-runnable proposal in section 5. This avoids storing a persistent cooldown. A re-runnable "ominous" variant can come later with its own state.
- **Creature persistence.** Before slice 1, the builder must check whether the site/slot defeat record width fits one synthetic site id per district. If it does not, the save bump moves into slice 1 and is batched with everything else.
- **Relics are passive while carried**, like charms in the pack, and use the existing buff path. Plinths already claimed in old worlds keep their stand-in rewards, and relic items exist only from the new gate. The two effects that need new systems ("see the hunter's wake" and "spell light does not raise sight range") read flags that the hunter code checks.
- **Hunter movement.** The builder confirms in slice 2 whether burrower `Swim` and navigation cover room-graph roaming and lair exits. If they do not, slice 2 includes a minimal waypoint graph taken from the plan's room graph. This is not general pathfinding.
- **Multiplayer.** Hunter AI and the director run host-only, like other enemy AI. There is one noise meter per district, shared by all players. The director biases toward the player whose noise contributed most. The determinism test is single-player: same seed plus same scripted input produce the same hunter path.
- **Scripted playtest.** Use the existing headless bot and engine-workbench ops (fill/probe/command). If they cannot drive a route, the acceptance falls back to a recorded manual route plus captures. A harness is not a deliverable of this plan.
- **Caps.** 10 active Undercrown enemies and p95 +0.5 ms are working targets, to be confirmed from the slice 2 baseline capture.

## Left to the builder

- Exact radii, decay rates and noise weights, which go in RON.
- Creature model dimensions and silhouettes, within the authored-voxel direction.
- Loot table contents per district.
- Wave counts and sizes in trial rooms.

## Progress (2026-11-13)

- Gate 302 claimed (`undercrown::v2::life::VERSION`), dormant.
- Slice 1, borough:
  - Prop kinds `Cache` (banded strongbox) and `Lectern` (lit page) have models in `v2/props.rs`.
  - `v2/life.rs` places 4 caches and 3 lecterns at prop index 2.
  - Reward rows in `assets/undercrown_rewards.ron` gained `prop`, `spell` and `wand` fields.
  - The lecterns teach Soaked strike, Undertow and Flood bolt.
  - The evidence chest (under the sluice flood) and the ferry strongbox each mint a seeded found wand.
- Slice 2, first pass:
  - Burrower strains gained the optional fields `district`, `patience_s`, `rest_s` and `hears`, plus a `Withdraw` mode.
  - Kind 88 is the borough water hunter. It rises only in borough rooms on generator 302 worlds, one at a time.
  - It hears by player speed, so creeping or standing still slips it.
  - It breaks off after 14 s of pursuit and rests 25 s at its lair.
  - Its bestiary name "Drowned Hunter" is a placeholder awaiting approval.

## Progress (2026-11-14): live without a gate

The default generator is 241 and many dormant gates sit between it and 302,
so nothing that changes voxels can ship yet. These parts are runtime or data
and are live in every world now:

- District hunters spawn in any world with the v2 layout. The gen-302 check
  on `district_at` was removed; it now needs only `plan.v2`.
- New resident hunters (placeholder names, approval owed):
  - kind 89 "Root Gnawer": Rootbound Halls, tunnels through stone, gives up after 10 s.
  - kind 90 "Chasm Breacher": Crown Chasm, breaches from walls, gives up after 9 s.
  - kind 91 "Hollow Sea Leviathan": Hollow Sea, 40-50 segments, gives up after 18 s.
- Noise: `burrowers::State::ruckus` (0..1, decays 0.5/s). Casting adds 0.85 and a
  finished block break adds 0.6 (`Adventure::make_noise`, called in main.rs
  after `mining_progress.tick`). Hearing hunters take the louder of footsteps and ruckus.
- The ten existing live plinth rows in `undercrown_rewards.ron` now teach a spell
  and/or mint a found wand:
  - Grapple, Teleport bolt, Chain arc, Dig bolt, Freezing shard, Growth, Meteor,
    Leap and Lightning arc;
  - wands from the Jeweler's balcony (tier 1), Sea cache (2) and Archive reliquary (3).
  - Ids stay at or below 95, because the found-spell log is limited to that range in the save.
- Uncompiled and untested as of writing (owner asked for no build).
  Backup branch: `undercrown-life-wip`.

- Not yet built:
  - the trial room, the vault and its key;
  - the apex;
  - relic items;
  - drain-eel surges tied to the sluice;
  - debug overlays;
  - captures.
