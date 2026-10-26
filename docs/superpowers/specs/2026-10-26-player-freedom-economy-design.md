# Player freedom and the non-coercive economy: audit and first slice

Owner brief, 2026-10-26: `07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md`, preserved
byte-for-byte at [docs/NEW/07](../../NEW/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md)
(SHA-256 in `docs/specs/README.md`). Its §14 kickoff asks for an audit of the
real repository before any edit, integration into the claims harness, and a
smallest "two-path resource + one functioning site" demo. This note is the
audit and the slice design. Routing is proposed in
[decision 0034](../../decisions/0034-player-freedom-addendum.md); nothing here
is an input of record until the human adopts it (decision 0022 froze intake).

Two other pastes arrived with the brief. The "Axiom Magic Overhaul" paste is
already the wand chapter of `docs/NEW/03_ART_CONTENT_AND_WORLDGEN.md` and a
peer session answered it in `docs/design/WAND_ECOSYSTEM_20261026.md` on
2026-10-26; this note does not touch magic. The "Make Exploring the World as
Rewarding as Mastering It" paste was cut at the paste limit inside its §7
table; the 07 document is its complete form, so the fragment was not preserved.

Status vocabulary follows `docs/NEW/04` §8: INSPECTED means read in source
this pass; nothing below is NATIVE_SEEN or PLAYTESTED.

## 1. Audit: the twelve promises against the code

Line numbers are from the working tree on 2026-10-26 and move as peers edit.

| Claim | Promise | What exists (INSPECTED) | Gap |
|---|---|---|---|
| C-PF01 | No tutorial or AFK compulsion for ordinary building, repair, first magic, first machine | Starter wand and three rations in the starting inventory; workbench and smelter placed in every settlement and site (`worldgen/src/sites.rs:944-947`); no Primer, no tutorial text system | Nothing measures it. Needs the C-MC5 bot run and a human cohort; both M4 |
| C-PF02 | Several routes to ordinary necessities | Manual: any block drops itself (`world/inventory.rs:171-184`); ore plus 0.25 coal in a hand Smelter gives an ingot (`world/expedition.rs:722-748`). Machine: drill, crusher or washer, furnace, blast furnace (`world/factory/recipes.rs:10-41`). Caches: SurveyCache, SalvageCrate, CultivatorCache in sites | Drills delete stone, dirt, sand, gravel and limestone with no yield (`factory/extraction.rs:298-345`). No recipe makes sand or gravel. No trade of any kind |
| C-PF03 | Expert factory has authentic throughput advantage | Processors run every 0.25 s per node, 64 units per batch, upgrades multiply steps (`factory.rs:1560-1640`); output cap 4096 per slot, item bus 64 units per source per step | No measured comparison against hand gathering. Add to `technical_factory_honored` at M5 |
| C-PF04 | Novel exploration still pays after automation | Site clear gives 128 wild essence, 64 circuits, one spell (`adventure/rewards.rs:25-65`); Undercrown plinths give metal blocks and armour once (`assets/undercrown_rewards.ron`); boss shrines drop a tier 1–3 wand | Every late reward is a commodity or a spell id. No site keeps a use after its caches open except Undercrown mechanisms |
| C-PF05 | Machines may discover and haul | No `is_player`, `by_player` or `player_only` check anywhere; shrine keys and seals read world blocks, so any actor works (`reliquaries/access.rs:9-25`) | Discovery credit exists only as the spell log bitset; there is no observation record to credit anyone with |
| C-PF06 | Humane play is practical | Habitats produce fibre, biomass and mana without death, 64 units per 20 s per resident (`world/habitats.rs:376-390`); sheep and goats drop nothing (`adventure.rs:2424`); hostile loot is once per site slot, so no kill farm exists | No guardian construct to repair or employ; the Ossuary Engine is a boss captured for a drill (`adventure/engine/mechanisms.rs:187-241`). Kills drop smelted metals 7 and 15 that only metallurgy should make |
| C-PF07 | Peace can be earned locally | Hostiles spawn only from seeded site slots within 36–68 m (`worldgen/src/sites.rs:1803-1930`); cleared slots are saved and never respawn; no night or light rule | There is no "secured region" state; peace is a side effect of finite slots, not something a wall, ward or beacon changes |
| C-PF08 | Low chore friction | 9 hotbar + 27 pack slots, resources and plain blocks stack 64 (512 at 0.5 m); machine blocks stack 16; three foods (`inventory.rs:109-148`); no durability on any item (ADR 0022's tool crate is unintegrated) | Hunger drains 0.04/s, 2.2× while sprinting, and passive regeneration stops below 75 (`adventure.rs:1183-1186`): the exact loop the brief removes by default. No portable container; the `chests` branch adds placed chests at save 30 and conflicts with the wand-bank save layout |
| C-PF09 | Death has stakes, keeps gear | Respawn keeps every item, full health, 65 % mana; only unbanked Resonance salvage is lost (`adventure.rs:2584-2630`); max health never drops | `respawn_point` is not serialized (`Adventure::encode`, `adventure.rs:2689-2727`), so after a reload death returns the player to the world spawn |
| C-PF10 | Deeper and farther places keep distinct identities | Surface sites (6 kinds), caverns, Holy Mountain landings, dungeon v2 (labyrinth, pitfall, forge rooms with lava and ore), Undercrown v2 districts, boss shrines, Amberroot groves | Generator 54 notes say "the strip is visual, not a rail system"; forge rooms and settlements are geometry plus a workbench. Depth identity is a style, not a mechanism |
| C-PF11 | Knowledge by observation or experiment | Spell log bitset (`adventure/discovery.rs`), 11 inscriptions from clear records, Undercrown markers set a DISCOVERED flag, radio lessons from listening (`world/orbital/radio.rs:140-154`) | No Primer, no evidence graph, no clue placement. Recipe book shows every item from the start |
| C-PF12 | Factories work while the player explores | `Factory::tick` reads only `self.nodes` and runs every fixed step (`fixed.rs:58-63`); drills and pumps read chunks through simulation residency, not distance; the only distance rule is the 6 m Battery mana font (`extraction.rs:88-117`) | Measured by `authority_tests.rs` this pass (see §4). Time stops outside `AppMode::Playing` and the 16-step frame cap drops time on slow frames (`simulation.rs:29,47`); a CA region suspends the factory entirely |

Supporting facts: trade, traders, villagers and NPC code do not exist in any
crate. `SitePlan`, `PlacePlan`, `Opportunity`, `Evidence` and `Clue` do not
exist; every `Plan` struct is a layout blueprint. The only mechanisms that
write voxels on restore are the eight Undercrown props (stair, bridge, bell,
winch, sluice, cooling, lens, freight; `world/undercrown/interact.rs:208-240`),
restored by one key press at no cost.

## 2. Value kinds mapped to existing systems

| Brief §2 | Repository today | Owner row |
|---|---|---|
| Commodity value | `Item::Resource(u8)` ids 0–21, `Item::Block`, per-node `input`/`output` buffers, `stats::Production` deltas | M01, M03, M16 |
| Capability value | Spell log bits 0–95, inscriptions, radio lessons, chip library (save 34+) | P14, P15, G02 |
| Opportunity value | Site records bitmask (slot bits, 256 = completed), Undercrown claim and discovered flags, relic CLAIMED | W04, W05, P13 |

The brief's `SiteOpportunity`, `ObservationReceipt`, `EconomyRoute` and
`HabitatSummary` sketches map onto those three stores. `HabitatSummary`
already exists in substance (`habitats.rs`: bed, water, feed, residents, next
cycle). The other three are new sparse records and need the save version.

## 3. Smallest slice: two routes to the same materials and one working ruin

Goal, from the brief §14: an explorer with a portable storage kit and a
technical player with a crusher and furnace both obtain stone, sand and metal,
and both finish a small home plus a workstation; a nearby ruin has a
recoverable mechanism and one grounded clue.

Parts, in build order. Each line names the owner row and what proves it.

1. **Machine route to building material** (M16). Drills return the
   ordinary blocks they cut instead of deleting them, and a Crusher mode
   turns stone into gravel and gravel into sand with a declared loss. Blocks
   are not resources, so outputs go through `Node::stock` as `Item::Block`
   stacks or through new resource ids. The second needs a save bump and is
   the human's call (decision 0034). Proof: a Rust test on `fixture_place`
   plus a throughput number per machine in `technical_factory_honored`.
2. **Explorer route** (P03, W04). SalvageCrates in sites already hold
   building blocks; the gap is carrying them. Either the `chests` branch's
   placed chests (peer, save 30) or a portable kit item for a Storage node
   (the `PowerKit` pattern in `factory/portable.rs`, extended to
   `Kind::Storage`). Proof: `explorer_building_materials` as a headless
   scenario once the runner can read inventories.
3. **Factory authority** (M01, C17). Done as a test this pass; the RON
   runner needs `Step::Machine` and `Check::MachineOutput` before it becomes
   a claims scenario. Proof: `authority_tests.rs`.
4. **One functioning ruin** (W04, P13). Pick one of: Sunken waterworks
   with a real `Kind::Pump` node placed on first visit and a broken cable to
   replace; Abandoned foundry whose valve sequence already exists and could
   gate a working smelter; an Undercrown sluice given a material cost.
   The waterworks is the closest to "Engineer ruin with a pump network" in
   the brief's §9.1 table. Proof: `legendary_site_not_loot_only` plus a
   save round trip of the restored state.
5. **One grounded clue** (W06, P14). A plate or inscription in the ruin that
   states a measurable fact about a second place (a water level, a
   temperature, a bearing), checkable with existing tools. This is the first
   `coherent_rare_hint_chain` row. Design only until W06 has an SDD.
6. **Respawn persistence** (P13). Serialize `respawn_point` and
   `last_death_pos`; save bump. Proof: `normal_death_recovery` as a storage
   test loading an older fixture.

Not in the slice: hunger defaults (product decision), trade (P11 has no
code), guardian constructs (G08), drones (B09), region security (X02).

## 4. Verification this pass

- `crates/app/src/world/factory/authority_tests.rs` (new, declared from
  `factory.rs`): two tests build a `headless::Session` on Superflat, place a
  furnace with 640 dust and energy, and run 3,600 fixed steps with the
  player beside it, 500 m away, or with the machine 600 m away. The ledger
  (input, output, energy, progress, status) must be identical and the output
  must be exactly 640 ingots. The second test stops at "Needs input" with
  half the input and compares the stalled ledger.
- Result, 2026-10-26, shared tree with peers' edits in flight:
  `cargo nextest run -p minecraft_style_voxel_prototype authority` ran 2
  tests, 2 passed (0.9 s). Both configurations finished with 640 ingots,
  0 dust and the same energy, progress and status, so C-PF12 holds for
  node-based processing today. The workspace-wide `just test` was blocked by
  a peer's unfinished `engine_sim` test module, not by this diff. Drills
  and pumps (chunk reads through residency) are not covered by this test.

## 5. Scenario rows proposed for the claims harness

| Scenario | Claim | Kind | Milestone |
|---|---|---|---|
| `afk_vs_adventure_factory` | C-PF12 | automated, Rust test now; RON once the runner has machine steps | M1.5 |
| `normal_death_recovery` | C-PF09 | automated storage fixture | M3 |
| `explorer_building_materials` | C-PF02 | headless once inventories are checkable | M4 |
| `technical_factory_honored` | C-PF03 | automated throughput | M5 |
| `inventory_first_hour`, `starter_cooking_intrinsics`, `player_freedom_home_without_farm` | C-PF08, C-PF01 | human playtest | M4 |
| `tnt_material_recovery` | C-PF02 | automated mass accounting | M4 |
| `legendary_site_not_loot_only`, `coherent_rare_hint_chain` | C-PF04, C-PF11 | automated plus human cold run | M6, M9 |
| `humane_material_alternatives`, `golem_guard_vs_scrap` | C-PF06 | automated plus playtest | M8 |
| `regional_peace_victory` | C-PF07 | automated persistence | M6 |
| `science_discovery_via_drone`, `cooperative_discovery_credit` | C-PF05 | automated | M8 |
| `new_deep_frontier` | C-PF10 | human review across seeds | M9 |
| `trading_no_infinite_arbitrage` | C-PF02 | automated ledger | M5 |

## 6. Questions only the owner can answer

1. Hunger: keep the drain as default, demote it to a world mode, or run the
   two prototypes the brief asks for before choosing?
2. Sand and gravel from machines: block-item outputs (no save change) or new
   resource ids (save bump, cleaner logistics)?
3. First functioning ruin: waterworks, foundry or Undercrown sluice?
4. Does 07 become an input of record (adopt decision 0034), or stay a
   reference like the 2026-10-25 briefs?
5. The U1 set in `docs/NEW/` is untracked. Commit it as supplied, or keep it
   as a staging folder?
