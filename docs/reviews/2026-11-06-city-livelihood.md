# Civilization prototype and native integration audit

Source audited: `C:\Users\antis0007\Downloads\Java`.
Active game: `C:\dev\voxel_v2_backup`.
Date: 2026-11-06.

## What the earlier work contains

`civ_proto` is a dependency-free Rust library: 27 source files, 3,796 lines, and 128 unit tests. Its `src/lib.rs` explicitly calls it a pure prototype and says engine adoption requires moving modules into the game. There is no executable entry point, renderer, terrain adapter, storage adapter, or socket implementation in this crate. I compiled its actual sources with Rust and ran all 128 tests successfully. No source in Downloads was modified.

`VILLAGER_DESIGN.md` is headed “Technical Design & Implementation Plan”; its opening defines an implementation-ready design, not a completed engine. `doc_villager` contains the component sections of that design. `NPC_SIMULATION_REVERSE_ENGINEERING.md` and `doc` describe Songs of Syx. The reference JAR has 10,992 Java classes and a manifest entry point of `init.Main`. Its classes are not Rust Axiom components. I inspected the archive metadata, without executing or importing the JAR.

## Feature trace

| Feature | Earlier Rust prototype | Axiom before this change | This integration |
|---|---|---|---|
| Jobs | `pawn.rs`: priorities, needs, target IDs, reservation set; caller supplies movement/world | `world/farming/jobs.rs`: local physical worker, pathfinding, mine/haul/build, four-worker limit | Production tasks enter that existing controller |
| Farming | `Work::Farm` category; no crop operations | Actual plot growth, environmental checks, player harvest | Residents collect seed from a chest, walk to beds, plant, harvest, retain one seed for replanting, and carry surplus back |
| Forestry | No tree felling | Player tree felling exists; worker mines marked log cells | Scene uses finite authored fallen logs; autonomous standing-tree forestry remains absent |
| Stockpiles | `economy.rs`: scalar transfer and limiting functions | Physical chest slots and carried stacks | Crafting stages the complete chest transaction and commits only with all inputs and output space |
| Cooking | `Work::Cook` category, no recipe execution | Authored Fernwake pantry consumes its own counters | Resident executes the existing Root stew recipe at a workbench beside the chest |
| Eating | `Needs::eat()` resets needs without consuming food | Authored pantry serving | Resident walks to chest and consumes one actual meal on a persistent meal schedule |
| Construction | `Project` reserves a scalar material count; progress increments | Paid voxel orders, reservations, interruption handling, city housing projects | Produced planks feed the same constructor in a small founding fixture |
| Furniture | No production implementation found | Existing furniture recipes and placeable blocks | One-shot chest recipe feeds a physical furnishing placement |
| Persistence | In-memory structs; no serializer/filesystem adapter found | Main save version 67 plus checksummed farming/other sidecars | Optional defaulted production and carried-seed fields in the existing JOB1 record; main save format unchanged |
| Multiplayer | `coop.rs`: authority/replication enums, 16-byte cohort cost; no transport | Worker controller explicitly disables itself in multiplayer | Still local-only; no new multiplayer claim |
| Scale | Budget selection sorts eligible people; distant schedule scans individuals; no engine benchmark | Maximum three helpers plus primary worker | Same cap; scenario uses three embodied workers |

The prototype is useful for design contracts and future settlement records. It is not a drop-in farming/cooking/city-building runtime. Copying its pawn and scalar inventories over Axiom would create competing owners for workers and goods. This change therefore extends the existing engine controllers instead of adding another parallel simulation.

## Implementation boundaries

- New source: `world/farming/jobs/production.rs` and its `actions`, `scene`, and `check` modules; `assets/city_livelihood.ron`.
- Narrow integration points: `jobs.rs`, `jobs/crew.rs`, and `main.rs`.
- Crop definitions, recipes, chest transfers, collision walking, voxel edit publication, and paid construction all reuse existing engine code.
- Production recipes are referenced by name. Meal and crop types come from existing asset tables.
- Three residents own disjoint farm beds. Each bill has one assigned worker. Resource commits recheck the current state; recipes do not reserve or destroy ingredients during partial work.
- Carried seed moves with the existing per-worker state and persists. Invalidated beds cause the seed to return to storage rather than creating a crop through a player edit.
- The furnishing bill has a lifetime completion limit. An installed chest does not trigger endless replacement production.
- Residents dig 19 marked soil cells, haul the soil, and build 19 work platforms. The platforms provide reach above the shelter walls; without them, eight roof-edge placements remained inaccessible.
- New-world scenario is opt-in with `--city-livelihood`; it requires an explicit scratch save directory. The launcher allocates a fresh directory each time.
- World-speed controls select 1×, 2×, 4× or 8×. They multiply the local simulation clock's elapsed input; fixed step duration, 16-step batch limit and per-frame gameplay budget remain unchanged. Multiplayer always uses the normal clock rate.
- Existing city task 01a1b12e-a1ca-7037-86a6-a27b26737035 was reviewed for ownership and status. Its quarter, road, layout, market, navigation and build changes were retained. No broad checkout reset or file replacement from a stale repository copy was used.

## Verification record

- Earlier prototype: **128 passed**, output in `prototype-tests.log`.
- Active game integration: initial whole-app native typecheck passed against the existing compiled dependencies, output in `native-check.log`.
- Final native acceptance `acceptance-final/livelihood-check.json`: **passed** at 75,000 fixed steps (625 simulated seconds). Three actual residents, 12 plots, 73 harvested units, 42 planting/replanting operations, 10 meals produced, 3 meals consumed, and 58 completed paid placements. Wall-clock test duration was 3.15 seconds, without rendering. An earlier matching run took 2.79 seconds.
- All 58 expected cells are checked in the world: 38 plank shelter cells, 19 paid earthwork cells, and one crafted chest. Input was 12 seeds, 12 finite fallen logs and 19 marked soil cells. No initial planks, meals or furniture were granted.
- The acceptance runner verifies crop/seed/meal accounting, timber/plank/chest accounting, earthwork accounting, no output in the player inventory, recipe refusal with missing input or full output, and legacy JOB1 defaults.
- Farming sidecars were encoded, decoded and restored while a resident carried seed and harvested produce. Persistent jobs matched exactly; simulation then continued to completion. This is a farming-sidecar checkpoint, not a full process-restart or cross-file crash-atomic save test.
- Existing city/market regression `legacy-market-regression/construction-check.json`: **passed**. Three builders completed 561 paid placements and a physical supplier bartered 8 salvaged logs for 8 iron units. Existing geometry, walking and cargo checks passed.
- Native window capture `runs/livelihood-20261106-214033-134/bench-000.png` visually shows the completed shelter, planted crops, dug soil pit, crafted chest and residents; its panel reports Mine 0 / Build 0 and 7 stored meals. It advances real gameplay before capture using `--city-livelihood-preview-ticks=75000`.
- That completed-scene capture measured 113.80 average FPS, 10.62 ms p95 and 11.33 ms p99 frames at 1280×720 on an RTX 4090 using DX12. This is a three-resident scene after construction, not a construction-load or population-scale benchmark.
- Final rendered run `runs/livelihood-20261106-214755-898` starts from the empty site at 8× with no preview ticks. Within the 120-second capture it reaches Mine 0 / Build 0. `bench-010.png` shows the completed shelter and chest, 84 harvested units, 12 meals produced, 6 eaten, and 6 still stored. The launcher exits successfully.
- That full progression measured 119.11 average FPS, 14.66 ms p95 and 16.42 ms p99, with a 10,002 ms worst frame. Startup took 95.65 seconds, including renderer pipeline work. Those stalls are real limitations of this run; the average does not establish smooth performance. Neither their cause nor large-population performance was investigated here.
- Earlier acceptance runs found and corrected authored RON tuple syntax, a furnishing placement that trapped workers, and unreachable roof edges. Failed-run logs and snapshots remain available.

The shared Cargo directory was occupied by another build. This task cancelled only its own waiting check, then used the observed compiler dependency invocation with a separate output path. It did not terminate or overwrite the other build. The compiler arguments and logs are retained in this task directory.

The final speed-control build initially encountered incomplete concurrent torch edits. Subsequent evidence showed those edits had been completed by their owner; the final native build then passed. No torch or model code was changed by this task. The final full repository Cargo gate was not run; validation used a whole-app Rust compiler invocation with existing dependency builds, native acceptance, and rendered gameplay.

The required repository session report was run against HEAD and saved as `session-report.log`. Its roughly 146,000-line shared-tree delta includes many concurrent tasks and must not be attributed to this change. Historical benchmark rows in that report were not rerun by this task.

## Running the playable fixture

The deliverable directory is `C:\Users\antis0007\Documents\Codex\2026-11-06\task-14`.

- Double-click `Launch-City.cmd` to start a new local settlement at 8× speed. The residents begin with seeds and finite raw resources; all production runs visibly from that starting point.
- `Play-CityLivelihood.ps1 -Speed 1` runs at normal speed. The settlement panel can also select the rate during play.
- `Play-CityLivelihood.ps1 -Check` runs the native acceptance into another fresh output directory.
- `Play-CityLivelihood.ps1 -Capture -Seconds 120 -Speed 8` records native screenshots while the settlement progresses.
- `--city-livelihood-preview-ticks=75000` optionally advances real gameplay before showing the scene; it is not enabled by the normal launcher.

The launcher retains the executable hash, arguments, logs and isolated save directory for each run. Source is already integrated in the active repository. `axiom-livelihood-final.patch` contains just this task's eight source/content changes, excluding concurrent main-loop edits. No shared-tree commit was created because these hooks depend on the city task's existing uncommitted controller work.

## Remaining work

This is a small local founding loop, not a mass-population city simulation. The earlier four-worker cap remains. Regional population/cohort scheduling, civilian businesses, automatic settlement site selection, general forestry, household growth, a furniture catalogue, richer needs, and replicated multiplayer jobs are unfinished. A meal schedule is not a complete hunger/health simulation. Root stew follows its existing recipe, which does not currently require a separate fuel item or fire simulation.

The production fixture starts with seeds and finite fallen timber. It does not generate lumber from a scalar stock or claim a closed sustainable forest economy. Normal crop growth runs through the existing environmental model. The separate construction-quarter work remains the stronger architectural/neighborhood scene; this new fixture isolates and proves the production dependencies before combining them at larger scale.

The two fixtures are not yet one self-supplying city. `quarter/fixture.rs` currently supplies most of its construction bill directly to the chest, and its legacy scene installs streets and existing houses. Its timber-framed architecture requires LimePlaster, Log and RoofTiles, whereas this production fixture supplies planks and a chest. The next integration must connect those actual material bills to recipes and gathering orders, retain usable paths while buildings rise, and prove the combined resource ledger. Raising a resident counter or copying the earlier prototype's `Work::Farm` enum would not accomplish that.

Standing-tree forestry needs a real tree/harvest adapter and replanting policy. Scaling beyond the current four-worker controller needs measured scheduling and navigation work, plus a distinct settlement/cohort representation for distant people. The earlier `population.rs`, `budget.rs` and `coop.rs` are reusable design starting points, but they have no demonstrated live-engine scale or network transport. `pawn.rs::step` also changes jobs without releasing a previous reservation; its lifecycle must be completed before adopting that controller.

Farming state and inventory/voxel state are still persisted independently. This change preserves normal snapshot/reload semantics but does not add cross-file crash-atomic commits. Four-worker native behavior and a fast standalone unit suite provide no evidence for thousands of simultaneously embodied agents.
