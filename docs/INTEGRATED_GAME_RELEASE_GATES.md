# Integrated game acceptance ledger — 21 October 2026

This is an incomplete release program. The thirteen-step ordinary-player gate
in the supplied request has **not passed**. Rust + raw wgpu and the pinned
dependency set are retained. No reference screenshots accompanied the request;
the only attachment was its text. No player roads, structures or saves were moved.

## Verified foundation changes in this increment

- Battery banks, transformers, induction cells and ports pack into non-stackable
  power items. Exact integer charge, installed upgrades, mode and battery I/O
  limit travel with the item through slots, the cursor, drops and saves.
  `src/world/factory/portable.rs`, `src/world/inventory.rs`.
- Dismantling preflights the assembly, assembler kits, ingredient/output buffers
  and recovered upgrade parts against a copied inventory. A full pack leaves
  the assembly intact. Kit stock is no longer silently discarded, and changing
  an assembler target requires collecting its previous outputs first.
- Matrix identity is the lowest-coordinate **cell**, independent of port choice.
  A topology edit scatters the pool into sorted physical cells, up to 64 kWh
  each, before removal. Removing a cell takes its charge; removing a port takes
  no healthy matrix charge. Splits retain their cells' ownership; merges sum
  their actual charge. Incomplete structures remain offline. Excess legacy
  charge is retained offline rather than clamped away. This is a deterministic
  ownership policy, not a globally permanent UUID across cell removal.
  `src/world/factory/induction.rs`.
- Shared pool rate budgets across networks remain in `power.rs`. No cable or
  provider tier was weakened to make an accounting check pass.
- Production counts source production once per generator and consumption when
  work commits, including assembly, pumping, drilling and mana conversion.
  Consumer-buffer deliveries, storage charging/discharging and converter intake
  have separate counters. The hour is a sliding 3,600-second window.
- Save **22** introduces the power-item tag and cell-held matrix pools; record
  lengths stay unchanged. Loader repairs cover biomass-generator inputs,
  assembler inputs, partial crafts, resource outputs and kit identity. A real
  region-file round trip covers new state; v21 layout loading is exercised too.
  This does not establish real-fixture coverage for every v1–20 save.
- The assembler panel displays 10 kJ per craft and rounds drive timing to whole
  0.25-second simulation ticks. Production reserves a scrollable table area and
  shrinks its graph on small/scaled screens. Fixed benchmark windows now retain
  requested pixel dimensions; intentional resize reviews remain resizable.

Electrical units remain 250 J per unit, with 0.25 s factory ticks. Cables and
transformers are ideal capacity-limited energy links; there is no modeled
voltage, AC waveform, resistive loss or transformer efficiency. Biomass's
200 kJ/item is usable electrical output; fuel portions remain in `burn` when
buffer capacity curtails production. A fully powered base assembler commits
5 kW × 2 s = 10 kJ per craft; drives shorten time at proportionately higher draw.
Non-portable machine power buffers and partially burned fuel do not become
portable items in this increment. Production history resets on load.

Source: [OpenStax, electrical energy and power](https://openstax.org/books/university-physics-volume-2/pages/9-5-electrical-energy-and-power).
Future electrolysis must obey the balanced water reaction and explicit losses;
the existing catalog is not evidence of a functioning chemical factory.
[OpenStax, electrolysis](https://openstax.org/books/chemistry-2e/pages/17-7-electrolysis).

## Baseline and timing contract

Evidence lives in `target/integrated-gate-a`. `baseline-source.zip` contains the
pre-change source, required untracked source/assets, Cargo.lock and .cargo
configuration. `baseline-files.json` records per-file SHA-256 hashes;
`baseline-head.txt` identifies HEAD. The frozen tree was extracted and compiled
with the same local target directory. Only an ignored copy of HEAD's original
timing method was added to its probe file. It leaves the workload and threshold
unchanged. This snapshot is the dirty pre-increment baseline, not clean HEAD.

The existing live test uses alternating version order and passes around
1.11–1.14×. Restoring HEAD's v15-then-v16 method reproduces the failure in the
frozen **pre-change** tree:

| Repeat | v15 / seconds | v16 / seconds | Ratio |
|---|---:|---:|---:|
| 1 | 0.807 | 1.604 | 1.99× |
| 2 | 0.779 | 1.593 | 2.04× |
| 3 | 0.782 | 1.602 | 2.05× |

Candidate repetitions with the same method measured 0.791/1.621 s (2.05×),
0.789/1.583 s (2.01×), and 0.797/1.602 s (2.01×). Those differences do not
establish a performance improvement. Both snapshots reproduce the failure.

Same `test` profile (opt-level 1, debug 1), 24 logical processors exposed,
one test thread, seed 424242, 80 padded chunks, one warm-up and best of three
per version; original threshold `new < old * 1.35 + 0.05`. Raw logs retain the
absolute times. The earlier stage probe measured v15/v16 full passes at
0.817/0.907 s, site chains 0.199/0.263 s, structure 0.243/0.243 s and caves
0.112/0.113 s, with solid counts 2,247,121/2,245,886. Stage timings are separate
whole-pass probes, not additive costs or proof of the original failure's cause.

An explicit affinity-to-CPU-0 diagnostic passed at 0.779/0.929 s (1.19×).
That run overlapped compilation and is only a diagnostic lead. Scheduler/cache
sensitivity is still unproven as the cause. The original-order failure remains
an **open Gate A blocker**. Factory changes did not cause the frozen baseline's
failure. No threshold, workload, generation feature or build profile was changed
to manufacture a pass. No generation optimization is claimed here.
This is a shared host with unrelated concurrent sessions; external workload
was not controlled. These repetitions reproduce the failure but do not provide
an isolated hardware performance comparison or identify its cause.

Reproduce:

```powershell
cargo test --locked --offline --target-dir target terrain_generation_cost_stays_close_to_the_previous_generator -- --nocapture --test-threads=1
cargo test --locked --offline --target-dir target gen_probe_original_order -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --target-dir target gen_probe_stage_costs -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --target-dir target -- --test-threads=1
cargo build --release --locked --offline --target-dir target
```

Do not run timing repetitions while compiling or running renderer reviews.
The baseline ordinary suite also fails the causeway deck test (329 pass,
1 fail, 29 ignored). An audio normalization failure appeared in the live suite
after unrelated concurrent audio edits; it is absent from the frozen baseline.
Neither failure is attributed to the factory changes. Hash drift is recorded in
`changed-since-baseline.txt`; this workspace is still being edited concurrently.

## Actual system inventory and open gates

“Present” below means reachable source paths exist, not release acceptance.

| Area | Actual path / authority | Current classification and acceptance still required |
|---|---|---|
| Terrain, climate, geology, sites | `world.rs`, `world/landscape.rs`, `terrain_variety.rs`, `sites.rs`, `dungeon.rs` | Present, incomplete. Versioned metre-space generation, regional erosion/biomes and authored sites. Generated paths exist; identifying the user's pictured roads requires the missing image/save. Connected regional drainage and several-seed route quality are unverified. |
| Edits, liquids, storage | `world/regions*`, `world/storage*`, `world/water.rs`, `simulation/*` | Present. Committed chunk data and partial-water state, bounded residency/publication and atomic checkpoint infrastructure. Dam/drain/excavation unload + LOD + recovery playthrough remains open. |
| Near/far terrain and foliage | `mesh/solid.rs`, `render/streaming*`, `render/horizon*` | Present, incomplete. Conservative mesh LOD and separate horizon paths. Edited-structure coverage, continuous ascent and origin-rebase acceptance remain open. |
| Materials, water, atmosphere | `shaders/world_shader.wgsl`, `mesh/water_depth_probe.rs`, atmosphere/cloud shaders | Present, incomplete. One world texel lattice and existing water/atmosphere paths. No new material/water quality claim from this increment. Matched ground/aerial/shore diagnostics still required. |
| Power and processing | `world/factory*`, `egui_ui/factory.rs`, `production.rs` | Playable implementation present: batteries, matrices, tiered cables, ideal converters, biomass mode, assembler and production history. Pickup/accounting/save repairs implemented here; recipe reachability and ordinary-player throughput still require playthrough. |
| Item/water transport | `factory/transit.rs`, `extraction.rs` | Present, incomplete. Reserved item packets, filters/storage and real pump/tank water buffers. Machine kits are collectable; they are not transported by the resource-only packet schema. Conveyors, kit sorters, priority overflow and powered haulers remain open. |
| Chemistry/thermal engineering | `fluid/catalog.rs`, `containers.rs`, `network.rs`, `machine.rs`, `processes.rs` | Disconnected from the world factory and its save schema. Definitions and isolated checks are not playable boilers, turbines, electrolysis, gas storage, fuel cells or scientific experiments. Do not add a second competing authority. |
| Signals/forestry/augments | existing factory modes, drive upgrades, cultivator and biomass recipes | Incomplete. Existing fermentation/drive modes are useful foundations. No complete sensor/thermostat loop, sawmill or controlled thermal chain is established. |
| Wands and shared materials | `adventure/wands.rs`, `adventure/spells.rs`, `adventure/inscriptions.rs`, `world.rs` | Present, incomplete. Programmed casts and payload modifiers exist. Nested modifier isolation, trigger identity, delayed-program snapshots, save semantics and placed emitter parity remain acceptance work. |
| Cultivation/creatures | `adventure/growth.rs`, `bestiary.rs`, `hart.rs` | Present, incomplete. Persistent mushroom companion identity/health/size/markers and clearance helpers exist. Bounded owned hauler inventories, reservations and useful resize/task playthrough remain unproven. |
| Survival/discovery/construction | inventory, recipe index/browser, adventure quest/discovery, creative tools | Present, incomplete. Food/equipment/death recovery, starter wand, recipes/uses/tree and building controls exist. Bootstrap balance, accessibility/audio and the complete thirteen-step route remain open. |

Finite release target: one coherent reference region; a reachable powered
workshop and transport loop; one conserving thermal/chemical experiment with
sensor control; one shared handheld/emitter utility spell; one persistent
growable/resizeable helper doing useful work; and the supplied thirteen-step
playthrough with reload and failure recovery. Broader biome, machine, creature
and advanced-project branches remain long-term scope.

Gate A is still open for the original timing failure and full runtime/save
acceptance. B is open for geographic/aerial validation; C for connected
thermal/chemical/control/logistics; D for emitter and spell-event proof; E for
the ordinary-player route and polish. There is no claim that all gates shipped.

## Safe visual review

`tools/Review-FactoryGate.ps1` launches the rebuilt renderer in isolated storage
and captures Production, battery, matrix, converter, cable and assembler panels.
The matrix fixture includes providers as well as cells. `-UiScale 1.25` checks
scaled presentation. These are development fixtures, not acquisition proof.

```powershell
./tools/Review-FactoryGate.ps1 -Label candidate -Panel production
./tools/Review-FactoryGate.ps1 -Label candidate -Panel battery
./tools/Review-FactoryGate.ps1 -Label candidate -Panel matrix
./tools/Review-FactoryGate.ps1 -Label candidate -Panel assembler -UiScale 1.25
```

Manifests record executable hash, camera fixture, seed, time/weather, resolution,
view range and UI scale. Logs record the adapter and frame statistics. Original
images, player's infrastructure and ordinary-play saves are preserved.

## Runtime evidence and limits

The final frozen source is `target/integrated-gate-a/delivery-verified-source.zip`
with per-file hashes in `delivery-verified-sources.json`. Earlier archives and
executables are retained as diagnostic history. The frozen final test suite is
**340 passed, 2 failed, 29 ignored**; failures remain the audio normalization and
causeway checks above. Focused factory/storage/inventory checks pass (47/5/10).
`delivery-all-tests.log` records the complete result. This is not a green suite.

Native Vulkan captures use the reported NVIDIA RTX 4090 adapter. `candidate-v2`
contains battery, matrix, transformer, assembler and cable panels, plus Production;
`candidate-v3` contains the corrected Production and assembler at 125% scale.
These optimized `release-fast` captures were inspected. `capture-dimensions.json`
records 1280×720 images after the benchmark-window correction. The earlier
`candidate-fast` run changed window dimensions and is not a matched-resolution
performance comparison. The captures exposed and drove the table/scale repair.
One-frame startup images with incomplete terrain are not visual acceptance.

The final **release** build also passed, including composed WGSL validation:
`target/integrated-gate-a/voxel-gate-a-delivery.exe`, SHA-256
`31116F3BDB87EA8EB7CCE052066BC62AB9ECBD7DC8441F643E52C921D3FC7613`.
`release-final` contains eight completed native panel reviews, 44 PNGs, all
1280×720, with no detected renderer validation errors or device-loss messages.
All eight panel cases were inspected, including Production and assembler at
125%. `release-capture-verification.json` records completion/dimensions/error
checks. This is fixture-based visual verification, not the ordinary-player gate.
The first release attempt failed renaming an archive in the sandbox temp folder;
using `target/integrated-gate-a/build-temp` for TMP/TEMP resolved it. Builds also
waited behind other sessions' artifact locks; those sessions were not stopped.

`world-baseline/{ground,oblique,overhead}/camera.png` records the actual current
world at seed 2738958700, noon and partly cloudy weather, with camera/arguments
and executable hashes alongside. These are fresh generated review worlds with
isolated save directories. Their starter workshop and trail network are generated
(`sites.rs`, `surface.rs`); the missing user's exact image/save still cannot be
classified. No placement was changed. The oblique capture includes a horizon;
the overhead shows the sand rim, canopy stippling and ocean detail that still
need the Gate B art/geography work. These are baselines, not improvement claims.

### Gate B increment — 22 October 2026

- **Aerial hole fixed.** Above about 640 m the near-chunk cull projection's far
  plane rejected every resident chunk, while the horizon still suppressed those
  columns, leaving a sky-coloured disc below the camera. `build_draw_list`
  (`render/streaming/visibility.rs`) no longer far-culls; residency already
  bounds reach. Verified at 700 m, seed 2738958700, 10 view chunks.
- **Generator 30: home landmass is no longer a circle.** The radial `inland`
  floor (110→380 m) made a round island with a round sand rim. Beyond the
  unchanged 110 m core, the falloff distance is now stretched/squeezed by a
  slow fbm field (0.45–2.0×), giving bays and headlands. Older versions
  generate identically. Verified at 900 m overhead.
- **Water depth is continuous (generator 30).** Resident water depth came from
  whole-voxel beds, so every shelf showed one colour ring per metre. Chunks
  that hold water now carry `bed_frac` (true bed height minus the integer
  bed, from the same landscape sample), applied in `GeneratedChunk::water_optics`
  only where the bed is still the generated one. Identical on both sides of a
  chunk border; older generators are unchanged. Same-pose A/B at seed
  2738958700: rings and the dark terrace blotches are gone. `water_depth_probe`
  tests pass (6/6). An earlier shader-side depth wobble made no visible
  difference and was removed.
- World tests: 215 pass, 2 fail (causeway deck, listed above; the
  `VXWSAV28` save-version assertion from a concurrent save bump).
- Still open for B: near-field water reads lighter than horizon water (a
  stepped disc around the camera); drainage connectivity and several-seed
  route quality remain unmeasured.

Renderer reviews overlapped compilation and other sessions. Their raw frame/GPU
statistics are retained, but do not establish the 16.67 ms budget, an isolated
performance delta, continuous-flight stability or final visual acceptance.
Empty/long-history UI interaction, all legacy save fixtures, the thirteen-step
ordinary-player route and the broader gates remain open.
