# Implementation tracking

Date: 2026-10-26. Branch `growth-xmb`, HEAD `bed5b195` at the time of writing.
Method: four read-only source audits (orbital structures, first-person wand and
hand, multiplayer, repo-wide), the native captures listed in §7, and the
documents named in each row. Line numbers are from the working tree on this
date and move as peers edit; file names are the stable reference.

Status words follow `docs/NEW/04_IMPLEMENTATION_AND_VERIFICATION.md` §8:
PROPOSED, INSPECTED, IMPLEMENTED, BUILT, UNIT_TESTED, INTEGRATION_TESTED,
NATIVE_SEEN, PLAYTESTED, PROFILED. A row can hold different words in
different columns. "Untracked" means the file has never been committed.

Tree state when this was written: about 300 modified or untracked paths;
two live sessions besides this one (one on `crates/sim` fine sub-bricks and
the orbital monument, one historical sweep session that owns every dirty
path in the task gate); `just test` and `cargo clippy` for the whole
workspace fail on peers' in-flight files (`crates/sim/src/scenario_tests.rs`
`step_skipping`, `crates/content/src/fittings.rs:95`), not on committed code.

## 1. Space elevators and megastructures

### 1.1 What the three screenshots show

The tall tower with six curved buttresses and a hairline rising from its top
is the generator-65 **anchor monument** at the first Meridian corridor
(metres 1100, −900): a 256 m shaft, six buttresses into buried sockets,
annular collars, control wing, dock roof (`crates/worldgen/src/meridian_terminal/monument.rs`,
untracked, last edited 17:26 today by a live peer session). The hairline is
the atmospheric braid to the crown anchorage 18 km up. "Ember Deep" is the
name of the Ember underground zone archetype (`crates/worldgen/src/zones.rs:45`),
shown because the player stands over that zone; it is not the structure's name.
The dark doorway under the slab canopy and the plain gate are the monument's
control-wing openings as generated at that revision. The peer's own final
review of the previous revision (generator 64 exchange) is under
`target/orbital-art-r2/native-final/` with the release manifest
`release.json` (exe SHA-256 `6454F2D5…`).

### 1.2 Inventory

| Piece | Where | Status | Evidence |
|---|---|---|---|
| Meridian corridors: Courier 12 m, Freight 32 m, Conservatory 80 m carriages; crown rings 38 / 220 / 480 m; hold 360 m above terrain; anchorage 18 km | `crates/app/src/world/orbital.rs` (modified) | IMPLEMENTED, NATIVE_SEEN; 0 tests on placement | seeded 4096 m cells, first corridor fixed at (1100, −900); 3-day traffic cycle |
| Ground exchange (generator 64) and anchor monument (generator 65) | `crates/worldgen/src/meridian_terminal.rs`, `meridian_terminal/monument.rs` (both untracked); hook at `crates/worldgen/src/lib.rs` | IMPLEMENTED, NATIVE_SEEN (peer captures 17:00); 0 tests; version bump 53 → 65 uncommitted in `crates/voxel/src/config.rs` | `crates/worldgen/GENERATOR_64.md`, `GENERATOR_65.md` |
| Orbiting megastructures: Loom foundry 9.6 km, Continuum twin habitat 7.6 km, Dawn mirror array 10 km, derelict relay 2.8 km, one each on circular Kepler orbits at 110–200 km (sky scale ÷4) | `crates/app/src/world/orbital/megastructures.rs`, `crates/app/src/mesh/models/orbital/megastructures.rs` (modified) | IMPLEMENTED, NATIVE_SEEN; full geometry every frame, no LOD, no distance fade | `docs/design/ORBITAL_VISUAL_REFINEMENT_20261026.md` |
| Rotating skyhooks: Needle 40 km, Talon 105 km, Ark 287.5 km two-stage; elastic 17-node line, tapered by design tension | `world/orbital/skyhook.rs`, `world/orbital/dynamics.rs` (untracked), `mesh/models/orbital/skyhook.rs` | IMPLEMENTED; dynamics 0 tests | `docs/design/ORBITAL_DYNAMICS_PIXEL_20261026.md` |
| Receiver: build or reclaim console; commissioning ML-C01 once per world; relay recovery once per world | `world/orbital/receiver.rs` (modified) | IMPLEMENTED, NATIVE_SEEN in `receiver_review.rs` fixture; 0 tests | review fixture commands `terminal-*`, `freight-*` |
| Radio: six levels, Listen and Handshake lessons, dish build, aim, track | `world/orbital/radio.rs` | IMPLEMENTED; only in-game learning-by-observation in the repo | |
| Talon freight: lift, lower, collect iron in 8-ingot crates; momentum ledger `hook_delta + orbital_stock + in_transit − imported = 0` | `receiver.rs`, `dynamics.rs` | IMPLEMENTED, NATIVE_SEEN; saves at version 39; untracked file that tracked code depends on | |
| Meteors | `world/orbital/meteors.rs`, `mesh/models/orbital/meteors.rs` | IMPLEMENTED, UNIT_TESTED | |
| `engine_space` crate and `assets/star_systems.ron` | `crates/space` | UNIT_TESTED, not used by the game; home planet radius 20,371 m there against 1,592,750 m in the app | decision 0025: phase 5b, integer sun, planet phases pending |

### 1.3 Unfinished, in order of player impact

1. **Hold platform has no landing, collision or passenger transfer**
   (`world/orbital.rs` ~490). The carriage is scenery.
2. **Corridor booking does nothing.** `Booking.channel` is written and only
   serialized; Radio and Resonance return the same text; `requested` only
   changes a lamp colour and a label (`orbital.rs` ~150, ~414–441).
3. **Courier and Ark hooks carry no cargo**; `dynamics::cargo` returns None
   unless the hook is the Talon; the Ark drop line is visual only.
4. **Only the first corridor has a ground base**; the others end on bare
   terrain. Ground fittings cull at 1 km (gen 64) and 2.6 km (monument).
5. **Distance presentation** (the Titanfall direction, §1.4): near corridors
   fade out over 5–6 km and reappear at full strength as far braids past 6 km
   (pop, read from code, not yet captured); skyhooks and megastructures have
   no distance fade and are clipped only by the planet limb; at most 3 far
   corridors within 24 km. Shader: `shaders/world_shader.wgsl` celestial path
   (~2323–2391) and corridor fades (~3658–3664). Mesh: `mesh/models/orbital.rs`.
6. **No tests** in `dynamics.rs`, `receiver.rs`, `radio.rs`, `passes.rs`,
   `meridian_terminal*`, or any `mesh/models/orbital/*` except
   megastructures and meteors.
7. **Rule breaches:** `passes.rs` uses `Instant::now()` for a 250 µs budget
   (rule: no wall clock in simulation); the review script
   `tools/Review-OrbitalStructure.ps1` pins generator 25, so the exchange and
   monument never appear in its captures; review commands `skyhook-pod` and
   `skyhook-cusp` aim at `Contact::pod`, which the renderer no longer draws.
8. **Static UI text** after completion ("ML-C01 … Once per world." and the
   recovery description); no Upgrade button at radio level 3 (level 4 comes
   only from building the dish).
9. **Untracked dependencies:** `dynamics.rs`, `meridian_terminal*`, five
   mesh helper files. A commit of the modified tracked files alone does not
   build.

### 1.4 Distant-landscape direction (Titanfall 2 references)

References read at full resolution from Downloads: Tu Bui `dmz-overall`,
`switchback-overall`, `bnw-arch-sky`, Jung Park `boomtown`. Lessons that
transfer, each tied to a code location:

| Lesson from the references | Change | Where |
|---|---|---|
| Far structures lift toward the sky hue with saturation kept; silhouette survives, detail does not | Add an aerial-perspective ramp to the celestial and corridor paths keyed to slant distance through the atmosphere, not a hard fade to zero | `world_shader.wgsl` celestial path; `render/atmosphere.rs` already owns transmittance |
| Repeated pylons receding in depth give scale | Let more than 3 far corridors draw past 24 km as silhouette-only braids and crowns (no carriage, no detail) | `world/orbital.rs` far-corridor cap; `mesh/models/orbital.rs` far branch |
| Cables thin into haze, never clip | Replace the 5–6 km near fade plus 6 km far pop with one continuous fade that hands near geometry to the far braid at the same opacity | `world_shader.wgsl` ~3658–3664, `mesh/models/orbital.rs` ~81–96 |
| Bases sit in built ground, routes read from afar | Generator 65 already does this at the first corridor; extend the cut-and-fill terrace idea to the other corridors' footings | `meridian_terminal/monument.rs` (peer's file), new footing module for secondary corridors |
| Hairline tether tops dissolve into sky | Fade the braid's upper third against sky luminance instead of ending at the crown terminal | `mesh/models/orbital.rs` braid spans |

Ownership: the orbital model and shader files are being edited by a live
session (monument.rs at 17:26 today, final captures at 17:00). This document
specifies the slice; the code change waits for that session's commit or an
explicit hand-over. Before-captures for the comparison are in §7.

### 1.5 Completion checklist (what "done" means for this feature)

- [ ] Hold platform: stand, ride, arrive (collision and transfer).
- [ ] Booking has an effect (window, class, channel) and a failure mode.
- [ ] Courier and Ark cargo contracts, or remove them from the UI.
- [ ] Secondary corridors have footings; far corridors draw as silhouettes.
- [ ] One continuous distance fade; aerial perspective on structures; no pop at 6 km (before and after captures at `corridor-far`, `terminal-ground`, `orbital-anchorage`).
- [ ] Tests: dynamics ledger invariant, Kepler drift bounds, receiver actions, passes budget without wall clock, meridian_terminal determinism across scales.
- [ ] Review script on the current generator; pod commands repointed to `dynamics.cargo`.
- [ ] Untracked files committed with their dependents; generator and save bumps recorded.

## 2. First-person wand and hand

### 2.1 Current build method

| Part | HEAD (committed) | Working tree (peer, uncommitted) |
|---|---|---|
| Wand | 1,035 lines of procedural primitives (`voxel_sphere` orbs of 12 mm cubes, beam shaft, cuboid plank forearm) in `mesh/models/wand.rs` | 127-line orchestrator over untracked `wand_sprite.rs` (PNG extruded to 4 mm × 24 mm cubes from `assets/source/wand_ecosystem/png_modular/source.png`, 3 columns × 3 sections = 27 assemblies), hung from the `GripSocket` of `assets/runtime/viewmodels/artificer_tally/wand.glb` |
| Hand | cuboid planks 0.15–0.10 m wide | CPU-skinned glTF `assets/runtime/viewmodels/hand_v5/hand.glb` with a procedurally bent sleeve (`wayfarer.rs`) |
| Pose | code constants | `assets/runtime/viewmodels/presentation.ron` (scale 0.84, 72° viewmodel FOV, rest/aim/inspect/interact keys, finger bones) |
| Texture | vertex colours | 512×256 atlas bound at group 1 binding 1 (`render/viewmodel_texture.rs`, untracked); the PNG wand still uses vertex colours |

Status: IMPLEMENTED and NATIVE_SEEN for the hand (`hand_v5/native-review.json`
says art acceptance is still open), UNIT_TESTED for the glTF loader
(7 required clips). Every authored asset (GLBs, atlases, 17 `.blend` files,
RON meshes, 22 Blender scripts under `tools/blender/`) is untracked, and the
code `include_bytes!`s them: a commit of the four modified tracked files
alone does not build.

### 2.2 Animation coverage

Present: idle loop with breathing, equip and holster, cast windup, release and
recovery, aim, inspect, interact, hit recoil, idle fidget after 16 s, turn
sway. Missing against `docs/design/WAND_ECOSYSTEM_20261026.md`: walk and
sprint clips (only a 5 mm / 3 mm gait bob), continuous cast, rapid-fire
accumulation, failed-cast response, spell-dependent recoil.

### 2.3 Defects from the 2026-10-25 audit, re-checked

| Defect | State today |
|---|---|
| Shaft styles are recolours of one silhouette | Replaced by aliasing: six seeded shaft/head/grip indices are reduced `% 3` at render time, so styles 0 and 3 are identical (`wand_sprite.rs`) |
| Arm is a plank next to a 3 cm shaft | Fixed in code by the skinned hand; art acceptance open ("fingers still read as parallel bars", `minimal_v2/README.md`) |
| Heads collapse into one glow | Still present: one `glow_voxel` at the muzzle for every head and spell (`wand.rs` ~61–66); `WandView.mana/deck/slots/cursor/tier` are filled and never drawn |
| Four pixel sizes in one object | Still present: sprite cube 4 mm × 0.84 ≈ 3.4 mm against the 4.5–5.5 mm target in `PIXEL_WORKSHOP_RESET_20261026.md`; the Artificer atlas is 5.6 mm; the source PNG is a point-sampled AI-generated image, which the same document forbids |
| Idle wand points 33° off axis | Unverified; rest axis now data (`presentation.ron`); needs one capture |

### 2.4 Wand ecosystem (twelve families, five lineages)

In code: 3 PNG families in play, plus 3 review-only GLBs (bone needle, runic
key, cinder rod) behind `VOXEL_REVIEW_ITEM`. No family or lineage exists as
data in any `assets/*.ron`; `WandRecord` has no family or construction
field. The spec in `docs/design/WAND_ECOSYSTEM_20261026.md` and
`docs/NEW/03` lists 12 families; 9 do not exist in any form.

### 2.5 Next, in order

1. Commit hygiene: land the untracked viewmodel modules and assets together
   with the four modified files, separated from the unrelated conduit,
   casing, chip-socket and orbital changes mixed into the same diff.
2. One pixel size: pick the metric (the documents say 4.5–5.5 mm) and make the
   sprite, atlas and glow cube obey it; replace the AI-generated source PNG
   with an authored sheet.
3. Per-head and per-spell feedback instead of one glow cube; read
   `WandView.deck/slots/cursor`.
4. Family and lineage as data: a `family` field on `WandRecord`, a RON table,
   seeded picks over real families instead of `% 3` aliasing.
5. Walk, sprint, continuous-cast and failed-cast states.
6. Fix stale documents: `docs/VIEWMODEL_ASSET_PIPELINE.md` (Wayfarer GLBs
   are referenced nowhere), `docs/design/PIXEL_ART_DIRECTION.md` (RON kit
   loader is test-only), `assets/source/wand_ecosystem/artificer_tally/README.md`,
   `.claude/skills/render/SKILL.md` (`WAND_PIXEL voxel_sphere` no longer exists).

Capture recipe: `tools/Review-Artificer.ps1 -Executable <exe> -Label <unique>`
with `-Mode`, `-Shaft/-Head/-Grip`, `-Pose`, `-Item`, `-Spell`; output
`target/artificer-20261025/<Label>/camera.png`.

## 3. Multiplayer

### 3.1 What exists

| Layer | Fact | Where |
|---|---|---|
| Transport | Valve GameNetworkingSockets 0.3.0 over direct IP/UDP, every message reliable and ordered, 1 KiB packets, 16 players max, protocol `VXMP` v2 | `crates/net/` (untracked crate), `crates/net/src/steam.rs`, `protocol.rs` |
| Authority | Host runs every actor and `simulation::fixed::finish` at 1/120 s from a wall-clock accumulator; clients run no simulation and have no prediction, lockstep or rollback | `crates/app/src/multiplayer.rs` (untracked) |
| Replication | Terrain snapshot (RLE edited chunks, ≤ 4096) then cell deltas; poses at 30 Hz; a full gameplay image (inventory, adventure, factory, chips, orbital, habitats, environment, all water) per client at most every 100 ms, RON + zlib, up to 16 MiB | `multiplayer/gameplay.rs`, `multiplayer/actions.rs`, `authority.rs` |
| Identity | Exe FNV-64 hash plus seed, generator, cell size; both peers need a byte-identical exe | `multiplayer.rs` ~313–338 |
| Entry | CLI only: `--multiplayer-host=IP:port`, `--multiplayer-join=IP:port`, `-seed`, `-generator`, `-save`, `-ephemeral`, `-headless`; no menu | `multiplayer.rs`, `main.rs` |
| Status | Two clients connected on a 14:05 binary that predates `actions.rs`, `authority.rs`, `gameplay.rs`, `inventory.rs` (17:03–17:24). The current source has compiled (50 warnings) and never run | `target/dedicated-bridge-client.log`, `crates/net/README.md` |

Feature `multiplayer` is not in any default set and `just test` does not
enable it. Zero tests in `crates/net`, `multiplayer.rs`, `multiplayer/*`.
No ADR or master-plan row covers it; `docs/ENGINE_CORE.md:209` cites an ADR
that does not exist.

### 3.2 Unfinished

1. Cross-player damage is dead code: `ContextFrame.damage` is always empty,
   `Hit` is never built, `Adventure::multiplayer_damage` never called.
2. Client hand slot is hard-coded to 0; edge presses (cast, cycle, interact,
   dash) on non-tick frames are dropped.
3. Water and environment simulate around one anchor player only.
4. No resync: a late transfer packet is fatal and the session closes for good;
   no state digest; no delta encoding for the gameplay image.
5. README stale in both directions (claims features missing that exist;
   wrong chunk cap, generator and whitelist).
6. Commit hazard: `mod network_player;` and the `engine_net` dependency are
   unconditional, so committing the modified tracked files without the
   untracked crate and modules breaks the default build.

### 3.3 Next, in order

1. Run the current source natively (host plus one client) and record it;
   then commit `crates/net`, `multiplayer*`, `network_player*` with the
   `Cargo.toml` edits in one change, feature-gated so the default build is
   unaffected.
2. Client slot and edge-press fixes (small, local).
3. Cross-player damage through the existing `Hit` path.
4. Resync and digest; per-recipient image deltas.
5. An ADR for the transport and authority model; a master-plan row (co-op
   would have touched the parked C-PF05 `cooperative_discovery_credit`).

## 4. Everything else

Test counts are `#[test]` attributes found by grep, including untracked
files; "(k ign)" marks ignored tests. Status: absent, library-only (tested
but no consumer in play), partial, playable, tested.

| Subsystem | Where | Status | Evidence | Next unfinished item |
|---|---|---|---|---|
| Cube-sphere planet | `crates/core/src/cube_sphere/` | library-only | 30 tests, bench baseline 6 keys, decision 0024 | No crate outside `core` uses it; the C02 SDD (face id, cross-edge rotation) comes first; no SDD exists yet |
| Content tables, RON round trips | `crates/content`, `assets/*.ron` | tested | 22 tests; byte-equal round trips for materials, reactions, machines, recipes, spells, bestiary, chip_modules, star_systems, undercrown_rewards, tools/{parts,materials,traits} | `descriptions.ron` has only a lint; `environment.ron`, `atmosphere.ron`, `groundcover/library.ron` have weaker checks; `content/src/fittings.rs` untracked; 7 asset files modified |
| Integer CA kernel `engine_sim` (block level) | `crates/sim/src/` | library-only | 77 tests, 5 bench baselines; PLAN2 A1–A5 committed | PLAN2 A6 not started: `WorldSimulation` drives no `CaBackend`; `cellular.rs`, `thermal.rs`, `fluid_compute.wgsl` not removed |
| GPU port `engine_sim_gpu` | `crates/sim_gpu/src/` | partial | 11 tests (8 GPU, ignored by `just test`); A5 byte-identical result in PLAN2 | `resident_async.rs`, `resident_edits.rs` untracked; ADRs 0027 and 0028 say GPU execution pending |
| Fine sub-bricks (ADR 0023) | `crates/sim/src/fine.rs`, `matter.rs`, `matter/*` | library-only, live peer work (commits today a5f12acb … 526ca45e) | 23 tests, bench `fine_step_2000_sub_bricks_idle` | Zero references from `crates/app`; promotion scheduling, fine masks on `ChunkData`, meshes, mob sweeps, save bump all deferred |
| CA in play (`ca_view`, `cpu_region`; ADRs 0018, 0025–0029, 0033–0035) | `crates/app/src/simulation/cpu_region*`, `world/ca.rs`, `ca_window.rs`, `world/storage/session/` | partial, opt-in only | 0 tests in these files; `--ca-window`, `--ca-backend`, headless `--ca-cpu-region` | ADR 0025 "runtime acceptance pending", 0026 "runtime pending"; most files untracked |
| Legacy cellular water/sand, rigid, thermal | `crates/app/src/simulation/` | playable | water 10 (2 ign), environment 20, climate 6, heat 3, sandbox 2, rigid 1 | Removal scheduled by PLAN2 A6; `cellular.rs` and `thermal.rs` have no direct tests |
| Worldgen version | `crates/voxel/src/config.rs` | tested | tree 65, HEAD 53 | `GENERATOR_54..65.md` untracked; no note for 53 or 57 |
| Landscape, terrain variety | `crates/worldgen/src/landscape.rs` (96 K) | playable | 5 tests; bench `terrain_generate_8_chunks` (+59 % against baseline at last run) | `docs/plans/2026-10-24-worldgen-spec-gap.md`: plates, stream-power erosion, wonders not started |
| Drainage, lakes, river ice | `drainage.rs`, `lakes.rs`, `river_ice.rs` | tested | 23 tests | "initial drainage plan, not a continuously replenished water source" |
| Sites, boss access | `sites.rs` (89 K), `sites/*` | tested | 25 tests (3 ign) | `sites/review.rs` untracked |
| Dungeon v2 | `dungeon.rs` (98 K), `dungeon/v2/` (29 files) | tested | 43 tests; pin covers gens 32–35 | `v2/arcades.rs`, `v2/industry.rs` untracked |
| Undercrown v2 | `undercrown/v2/` (24 files) | tested | 30 tests incl. `the_shipped_undercrown_never_changes` | `v2/observatory.rs` (gens 60/63) untracked |
| Valley, settlement | `valley.rs`, `valley/settlement/*` | partial | 5 tests; GENERATOR_56/58/61 | settlement module and `skyline.rs` untracked; "no semantic furnishings or authored windows" yet |
| Amberroot grove | `amberroot/` | tested | 10 tests (1 ign) | One authored reference grove (gen 35) |
| Zones | `zones.rs` | tested | 7 tests | — |
| Surface routes, bridges | `surface/routes/*` | partial | 3 tests; GENERATOR_62 | routes module untracked; bridges have 0 tests |
| Trees | `trees/*` | tested | 9 tests | `trees/conifer.rs` untracked |
| Architecture kit | `architecture.rs` (52 K), `architecture/*` | partial | 8 tests | `composition.rs`, `plan.rs` untracked |
| Anchor districts | — | absent | mentioned once in a design note | memory says a `voxel_anchor` worktree (gate 55) exists; nothing in this tree |
| Golden generation tests | `gen_golden.rs` | partial | ignored; prints digests, compares nothing; pins only in dungeon v2 and undercrown v2 | No committed digest for generators 36–65 |
| Saves, regions | `world/storage.rs` (tree 39, HEAD 34), `regions.rs` | tested | 15 tests; migration tests for v12, 17, 18/19, 20, 21, 22, 28→29, 36→37; fixtures synthesized, none committed | Versions 35–39 uncommitted; fine pool and CA session outside ordinary saves |
| Factory, automation | `world/factory.rs`, `factory/` (16 files) | playable | 27 machine kinds; 51 tests incl. `authority_tests` 2 | Windowed factory tick still batched `dt` (PLAN P2); assembler has 1 test |
| Chips | `crates/chip`, `factory/chips*`, 7 UI panels | playable | chip crate 33, app 17 | `chips/orders.rs`, `chips/production.rs` untracked |
| Inventory, crafting, recipes | `world/inventory.rs` (2,220 lines), `recipe_index.rs`, `recipes.ron` (42) | tested | 23 tests + round trip | `chests` branch (save 30) unmerged and conflicting with the wand-bank layout |
| Spells table | `assets/spells.ron` (95), `spell_defs*` | tested | 7 tests incl. round trip | both files modified |
| Wand deck evaluator | `adventure/wands.rs` (2,456 lines), `wand_items.rs` | tested | 43 tests | no global execution budget for nested payloads; only multicast capped at 12 |
| Enemies, bestiary | `bestiary.ron` (11 species), `encounters.rs`, `tactics.rs` | tested | 4 tests | — |
| Bosses | hart, tollkeeper, matron, glassweaver, Ossuary engine | playable | glassweaver 6, engine 5, boss_look 2; hart/tollkeeper/matron 0 | Ossuary: "no manual play session, no unit tests"; `engine/{mechanisms,review,save}.rs` untracked; save v38 |
| Discovery, quests, status | `discovery.rs`, `quest.rs`, `status.rs` | tested | 16 / 1 / 11 tests | — |
| Habitats | `world/habitats*` | playable | 0 tests; save v26 | No unit tests |
| Wildlife | `adventure/wildlife*` | partial | 7 tests | whole module untracked; plan `docs/plans/2026-10-26-outdoor-wildlife.md` |
| growth.rs | `adventure/growth.rs` | in progress, do not touch | 4 tests | dead-code warnings (PLAN debt 7) |
| Tools crate `engine_tools` (ADR 0022) | `crates/tools` | library-only | 16 tests (3 ign) | crate untracked; no dependent; "no save or UI integration" |
| Renderer | `render.rs` (5,131 lines), `render/` (26 files), 37 WGSL + `fluid_compute.wgsl` | playable | horizon 22 (5 ign), streaming ~36 (17 ign), coverage 10, origin 8 | `Gpu::step_inner` ~1,100 lines; arena/horizon evidence plan has every box unchecked; two surface shaders untracked |
| Captures policy | `policy.json` `capture_ab`, `seam_route` (manual) | partial | manual attestation only | offscreen PNG hashes deferred (PLAN P2) |
| Meshing | `mesh.rs` (2,004 lines), `mesh/*`, `models.rs` (1,977) + 12 model files | playable | 24 tests | `ground_detail/library.rs`, `assets/groundcover/` untracked |
| Far LOD, horizon | `render/horizon/` (15 files) | playable | tests in the renderer row; worldgen `horizon_agreement` 4 | far-field/water design "for review"; `streaming/columns_tests.rs` untracked |
| UI | `egui_ui.rs` (3,279 lines) + 43 panels | playable | `text_rules.rs` enforces 19 entries; `just lint-ui` | every other panel outside `CHECKED`; `chip_art.rs`, `semiconductor.rs` untracked |
| Audio | `crates/audio` | playable | 8 tests | lib/synth modified |
| Headless scenario runner | `headless.rs` (875 lines), `headless/{options,session}.rs` (untracked) | tested | 18 tests; 3 scenarios; claims dir has only a README | pending `sand_falls.snap.new` with hash `e34f1a32…` against committed `2a6d6444…`: behaviour changed and nobody accepted it; no machine step or ledger check |
| Benches, perf gate | `crates/{core,sim,space,worldgen}/benches`, `scripts/bench.py`, `just bench-check` (+8 % fail) | tested | baselines core 6, sim 5, space 2, worldgen 1 | no benches for app, chip, sim_gpu; app benches need a lib/bin split (PLAN debt 6) |
| Task gate | `scripts/workflow.py` (699 lines), `policy.json`, hooks | tested | 33 + 11 tests | classifier disabled; attribution counts peers' edits for pre-ownership sessions |

### Counts per crate

| Crate | tests | TODO-style markers | files over 300 lines / total |
|---|---|---|---|
| app | 582 | 0 | 111 / 367 |
| worldgen | 196 | 0 | 21 / 139 |
| sim | 77 | 0 | 1 / 27 |
| space | 60 | 0 | 0 / 16 |
| core | 39 | 0 | 0 / 17 |
| chip | 33 | 0 | 0 / 20 |
| content | 22 | 0 | 1 / 10 |
| tools | 16 | 0 | 0 / 8 |
| sim_gpu | 11 | 0 | 1 / 7 |
| voxel | 9 | 0 | 0 / 5 |
| audio | 8 | 0 | 2 / 3 |
| gpu | 0 | 0 | 0 / 1 |
| net | 0 | 0 | 1 / 7 |

Largest files: `render.rs` 5,131, `main.rs` 4,969, `egui_ui.rs` 3,279,
`world/adventure.rs` 3,024. The 300-line rule is met by 230 of 367 app files.

### Plans and milestones

- `docs/PLAN.md`: P0–P3 have actuals (2026-10-22); P4 reduction and P5 docs do not.
- `docs/PLAN2.md`: Phase 0 and A1–A5 committed; A6–A8 and Phases B–F have no actuals.
- `docs/MASTER_PLAN.md`: approved 2026-10-24; no spike, SDD or claim has passed; the M1 exit scenario `dam_break_200k` exists in no file; `docs/spikes/` holds only a README; no SDD files; `docs/systems/` has only the tooltip design.
- Untracked documents: 15 `docs/design/*_20261026.md`, `FIRST_PERSON_VIEWMODEL_20261025.md`, ADRs 0022 and 0024–0035, the `docs/NEW/` U1 set (07 is the only tracked one).

### Most blocked areas

1. **CA authority on the M1 path.** Kernel and GPU port are tested as
   libraries; play reaches them only through the opt-in region path with 0
   unit tests; ADRs 0025–0028 pending runtime; PLAN2 A6 not started.
2. **Fine sub-bricks.** Committed in `crates/sim` today, never referenced
   from `crates/app`.
3. **M1 milestone.** No `dam_break_200k`, no spike results, no SDDs, no claim
   scenarios; cube-sphere has no consumer.
4. **Traceability.** Save 35–39 and generators 54–65, the `tools` and `net`
   crates and 153 untracked paths exist only in the working tree.
5. **Determinism signal.** `sand_falls` snapshot drift unaccepted; no golden
   digests for generators 36–65.

## 5. Repair queue across subsystems

1. Peers' in-flight breaks that block workspace verification:
   `crates/sim/src/scenario_tests.rs` (`step_skipping` missing),
   `crates/content/src/fittings.rs:95` (clippy). Owners: the live sim
   session. Until they land, verify per crate with
   `cargo nextest run -p <crate>`.
2. Untracked files that tracked code depends on (build breaks if the tracked
   half is committed alone): `world/orbital/dynamics.rs`,
   `worldgen/src/meridian_terminal*`, five `mesh/models/orbital/*` helpers,
   all viewmodel modules and assets, `crates/net`, `multiplayer*`,
   `network_player*`, `render/viewmodel_texture.rs`,
   `shaders/architectural_surfaces.wgsl`, `furnishing_surfaces.wgsl`.
3. Version bumps pending in the working tree: save 34 → 39, generator
   53 → 65, both uncommitted; `CLAUDE.md` matches HEAD and must be updated
   with the commit that lands them.
4. Rule breaches: `Instant::now()` in `world/orbital/passes.rs`; 118 of 231
   Rust files over 300 lines at the last audit (`docs/AUDIT.md`).
5. Stale documents listed in §2.5 item 6, plus `crates/net/README.md` and
   `tools/Review-OrbitalStructure.ps1` (generator 25).
6. Review fixtures that no longer match the renderer (`skyhook-pod`,
   `skyhook-cusp`).

## 6. Sessions and ownership

- This session (`9be8b4b1`): 07 player-freedom addendum routing and audit,
  `authority_tests.rs` (commit `bed5b195`), this document.
- Live peer (`134e86d4`): `crates/sim` fine sub-brick kernel, orbital
  monument, `main.rs`, `multiplayer.rs` (17:24–17:50 today).
- Earlier today (untracked outputs): orbital exchange art (R2, R3, final
  17:00), wand ecosystem and pixel workshop documents, viewmodel assets.

## 7. Evidence index

| Evidence | Path | Exe SHA-256 |
|---|---|---|
| Peer's final orbital exchange review (generator 64) | `target/orbital-art-r2/native-final/{ground,gallery,rim,court-dusk}-reviewed.png`, `release.json` | `6454F2D5C29E77071B38531506B370374845366B7FDF5D4AEDEE144CF387B381` |
| Before-captures for the distance slice, this session (release-fast exe built 17:21, newer than the peer's reviewed `6454F2D5…`) | `target/elevator-tracking-20261026/before/` | `DF01AC750AF283B33D90A9AEB7D7211FC69448A21982140FAE8A3A907ABC794A` |
| Factory authority test run | `target/workflow/evidence-pf/authority_tests_nextest.txt` | n/a |

Before-captures, seed 74, Hills, 1 m, clear, indirect lighting off, 1280×900,
exe default generator (65 in this build), driven through the receiver review
fixture (`VOXEL_RECEIVER_REVIEW=1`, commands in `command.txt`):

| File | Fixture command | Verdict | What it shows |
|---|---|---|---|
| `ground-shaft-b.png` | `terminal-ground` | valid | The same view as the owner's first screenshot: generator-65 monument, hairline braid to the top of the frame at constant opacity, a second corridor's braid crossing the sky, pale far mountains with hard edges and no aerial perspective, a dark wire-frame object on the right-hand slope |
| `dock.png` | `terminal-dock` | valid | The owner's second screenshot: slab canopy, orange receiver disc, unlit black opening |
| `gallery.png` | `terminal-gallery` | valid | Inside the control wing, aim at stone bricks |
| `anchorage.png` | `orbital-anchorage` | valid | Crown ring at 18 km with mirror leaves, braid continuing down; no haze between ring and ground |
| `skyhook-detail.png` | `skyhook-detail` | valid | Talon grapple descending on its tapered line, crown ring below; pose at 19.2 km altitude |
| `corridor-far-b.png` | `corridor-far` (ground, 9 km east, aimed at the crown) | valid | Two hairlines against a cloud; no crown, carriage or shaft silhouette readable at 9 km. This is the distant-landscape gap in §1.4 |
| `ground-shaft.png`, `corridor-far.png`, `mirror-array.png`, `mirror-array-b.png` | first attempts, `megastructure-mirror` | rejected | Loading screen: the capture fired before streaming settled; the mirror pose at orbital altitude never finishes "Loading terrain" and needs a fixture fix |

Capture driver scripts (scratch, not committed): a PowerShell loop that starts
the exe with the benchmark flags above, writes `NN-label command` then
`label capture` to `command.txt` with a retrying writer (the game holds the
file open while reading), waits for `[capture] saved`, and stops the process
by PID. Settle 9 s near the station, 40 s after a far teleport.
