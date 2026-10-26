# Variable scale and the underground

Owner brief 2026-10-25. Two gaps: 1 m gameplay and 0.25 m matter in one world,
and an underground worth descending into. This plan says what already exists,
what the brief changes, the order of work and the test that closes each item.

Companion decisions: ADR 0007 (bricks and scale), ADR 0015 (cells and planes),
ADR 0023 (fine sub-bricks). Strata and cave *rock* are already planned in
`docs/plans/2026-10-25-underground-biomes.md` (generator 55); this plan adds the
network, dungeon assembly and the scale rules those features sit on.

## What exists (2026-10-25)

- `engine_sim`: 8³ bricks of 1 m cells, `BrickStore` pool, `Cell` = 8 B,
  64 volume levels, material table, Margolus kernel. Not read by the game yet
  (`docs/PLAN.md` Phase A3+).
- Legacy `World` owns play: 32³ `ChunkData`, water map, sandbox, thermal,
  environment authority, mapless procedural read path.
- Caves: `worldgen::cave_carves`, four threshold noises producing Air only,
  plus rare 72 m `sites::cavern` chambers. Eight named underground archetypes
  in `zones.rs` with no rock or cave shape of their own.
- Dungeons: `worldgen/src/dungeon` builds ruin layers inside generated terrain.
  There is no module format, no socket assembly and no procedural interior.
- World floor is -64 m. Depth bands below that do not exist yet.

## Part 1: three resolutions

### Rule

A block is one 8-byte record until an edit, fluid or reaction makes it uneven.
It then promotes to 64 fine cells. It demotes when the cells agree again.
Expected fine ratio in play is 2-8% of resident blocks, steady state under 1%.

### Landed

- `MatterStore` owns block/fine writes and sparse 512-bit brick masks. Uniform
  fine reads allocate nothing; replacement commits before releasing fine state.
  Only identical cells demote through this owner; mixed-material averaging is
  deliberately excluded until composition and energy ledgers exist.
- `FineJobs` supplies bounded, deduplicated background representation jobs:
  2000 attempts/tick, a parent-clipped 0.5 ms wall slice, 60-tick hysteresis,
  and a configurable focus radius (24 m default). Deferred jobs retain FIFO
  rotation; direct fine edits must explicitly request demotion maintenance.
  These are library foundations, not app wiring or measured timing acceptance.
- `FineStore` with `promote`, `set`, `demote_uniform`, `demote_averaged`,
  `collapsed`, hysteresis and slot reuse (ADR 0023, `crates/sim/src/fine.rs`).
- Volume conservation: one block unit = 64 fine units. Demotion returns the
  rounding remainder to the caller; nothing is destroyed.
- Regression tests for the click path and for breaking placed and natural
  blocks (`creative::tests::queued_break_reaches_the_world_and_frees_the_queue`,
  `headless::tests::placed_and_natural_blocks_break_and_read_back`).
- The coordinated tick (2026-10-26): `MatterStore::step` runs the block kernel
  with fine-owned blocks skipped, then the fine Margolus pass inside `R_fine`
  with the 2000-block awake cap, the budget cut, dormant demotion after the
  60-tick hysteresis and `FineGovernor` (ADR 0023, "Coordinated tick"). Bench
  `fine_step_2000_sub_bricks_idle`: 7.0 ms on the desktop, about 3.5 µs per
  awake sub-brick, so the 3 ms H1 budget means a few hundred awake sub-bricks
  there.

### Next, in order

1. **Wire the tick.** `World` and the headless CPU region adopt `MatterStore`
   and call `MatterStore::step` with the governor's region and the window's
   fine budget. Disturbance jobs still go through `FineJobs`; demotion of
   settled fine blocks is the tick's own. Queue-full requests are retained for
   retry. The H1 budget test follows the first H1 measurement.
2. **Play semantics.** A pick chips fine cells from the face and removes the
   block on the last hit. Explosions and acid leave fine geometry. Placement
   fills to the block grid. Slabs and stairs are fine masks with a template id;
   the template mesh is used until the mask deviates from it.
3. **Bodies.** Capsule sweeps test the block grid plus fine masks inside
   `R_fine` only.
4. **Save.** `SAVE_VERSION` bump for the pool; migration writes no masks for
   worlds that had none.

### Budgets (H1)

| Stage | Cap | Mechanism |
|---|---|---|
| Block CA | 4.0 ms | awake bricks only |
| Fine CA | 3.0 ms | at most 2000 awake sub-bricks |
| Promote/demote | 0.5 ms | 2000 blocks/tick |
| Thermal, light, support | 1.5 ms | dirty bricks |
| Bodies, mobs, machines | 3.0 ms | caps 128/300/1500 |
| Mesh dirty bricks | 2.0 ms | templates for fine masks |
| Readback, compact | 1.0 ms | |

The governor lowers `R_fine` (24 → 16 → 12 m) before any block radius, and
never drops the player's own edit.

### Machine layer

Machines, belts, pipes, cables and multiblocks are block records with transport
flags. Their effects reach fine matter only through planes (heat, charge, gas).
The reactor meltdown case promotes its surroundings instead of teaching the
machine layer about fine cells.

## Part 2: the underground as the main event

### Depth bands

Strata, rock, hazards and resources follow the generator-55 plan. This plan adds
the two columns that make a band a place to enter: cave family and dungeon
family.

| Band | Depth | Cave family | Dungeon family |
|---|---|---|---|
| Karst | 0-60 | sinkholes, cenotes, dry caves | animal dens, bandit camps |
| Fractured | 60-250 | spaghetti, ravines, aquifers | mine shafts, rail stubs |
| Engineer | 250-600 | cheese caverns, flooded halls | city, foundry, dam works |
| Deep fractured | 600-1000 | lava tubes, geodes, fungal hollows | lairs, geode shrines |
| Cathedral | 1000-1500 | vaulted halls, shattered ravines | naves, crypts, golem forges |
| The Deep | 1500-2500 | vast voids, lit chasms | archives, sealed vaults, compiler halls |
| Mantle | above 2500 | lava ocean | core approach |

Band depth needs the world floor to move first (its own design: streaming,
bedrock, physics and save checks). Until then the table is a target, not a
promise.

### Cave network

1. **Skeleton.** A 3D graph per band grown from surface entrances and the water
   table. Karst follows joints, lava tubes follow old flow paths, tectonic caves
   follow faults. Each edge carries width profile, floor material, water, gas
   and a flow direction that hazard placement uses.
2. **Chambers.** SDF templates with parameters: tube, cheese, ravine, dome,
   shaft, geode, vaulted hall, void. Secondary detail: stalactites, flowstone,
   breakdown piles, fissures, surface chimneys.
3. **Flooded and gassed sections** from the water table and gas sources. Sumps
   as gates, CO2 in low spots, methane in high spots.
4. **Verticality.** At least two vertical connections per km² per 200 m band so
   descent never requires digging; one of them is hidden behind water, a lake
   floor or a mechanism.

### Module assembly

- Authored modules at block scale with material zones, not fixed blocks:
  `assets/dungeons/<family>/<name>.ron`, sizes 8³, 16³, 32³, at least 40 per
  family. Each module carries material, spawn, loot, hazard, connector, secret
  and lore zones. A validator requires at least three reactive zones per module.
- Assembly by connector sockets with family grammars (avenue → blocks;
  nave → transepts → crypt), rotated or mirrored where the family allows.
  Breach modules join a dungeon to a natural cave.
- Reactive zones make the room a reaction: oil-soaked timber burns, a
  pressurised tank floods the corridor with acid, a load-bearing pillar drops
  the floor above, a gas vault ignites.
- Powered and conditional families: a restored dynamo turns on lights, doors,
  lifts and turrets; a chord opens a door and a wrong note shakes rock loose;
  Founder rooms reconfigure walls.
- Waystations at band boundaries: safe room, archive, trader, one Primer
  chapter, a bed. One per band per km².
- Secrets: hooks satisfied by the secrets generator (a mural that names a real
  condition elsewhere, a door that needs the chord, a chest under the sump).
  At least one hint object per three modules.
- Loot tables per family and depth: salvage, wands and cards, Founder shards,
  and rare materials that add a reaction row.

### Performance underground

Caves are uniform air and uniform rock, so bricks stay uniform; only water, gas
and lava surfaces are awake. A big cavern at rest should hold fewer than 800
awake bricks. Dungeons are block schematics; fine work happens only where a
fight or an accident does. Far caves keep a catch-up record (gas level, water
level, machine powered) and are realized on approach. Cave chunks favor the
player's vertical column in the page cache.

## Acceptance tests

| Check | Where |
|---|---|
| `scale_promote_demote` | `engine_sim` integration: acid spill promotes, eats, settles, demotes; volume ledger balances; fine ratio under 1% after 2 simulated minutes |
| `mine_face_chip` | `headless` scenario: hits chip fine cells, block leaves on the last hit, placement fills to the grid |
| `fine_budget_h1` | bench: three fine events inside `R_fine` at or under 3 ms |
| `cave_vertical_connectivity` | `worldgen` over 100 seeds: every realized band reachable without digging |
| `dungeon_reactive_rooms` | 50 assembled modules: a spark or breach chains in at least 40 |
| `dungeon_power_restore` | module graph test: dynamo powers lights, doors and turrets |
| `waystation_per_band` | 100 seeds: one found within 300 m of each boundary |
| `secret_hint_density` | module validator: one hint per three modules, every secret satisfiable |
| `underground_awake_count` | scenario: big cavern under 800 awake bricks at rest |
| Capture board | one capture per band: lighting, atmosphere, one reaction |

The two break-path tests and the `engine_sim` tick tests in
`crates/sim/src/matter/step_tests.rs` (spill, stall, freeze, cap, budget cut,
demotion, governor, replay) exist today. The rest name work that does not.

## Order of work

1. Fine mask on bricks and the block/fine read boundary: done (ADR 0023).
2. Block-scale CA authority plus the fine pass behind `R_fine`, with the fine
   budget and the governor: done in `engine_sim` on 2026-10-26; wiring into
   play is open.
3. Mining, placement and blasts across both scales, with the tool regression
   tests extended to partial blocks.
4. World floor and band depth, then the cave skeleton per band.
5. Module format, validator, assembly and the first two families (Engineer,
   Cathedral).
6. Waystations, secrets and loot tables.
7. Save format for the fine pool and the cave catch-up record.

## Non-goals

- New liquids and blocks (acid, oil, sulfur, obsidian) belong to content work;
  this plan only says what happens when they exist.
- Surface changes: everything new stays at least 14 m below the surface.
- No generator edit before its version gate; every old version keeps generating
  byte-identically.
