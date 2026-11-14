# Engine and world refinement — 2026-10-24

Status: current execution guide; 12–18-month scope is a planning envelope, not a completion promise. This refines ENGINE_CORE/GAME_DESIGN, keeps D1–D7 subject to the explicit capacity correction in ADR 0017, and makes visual quality a continuing track alongside simulation.

## Inputs and boundaries

Inputs: the supplied Mission Specification v2, `claude_voxel_painterly_visual_overhaul_prompt.txt` and `claude_voxel_architecture_master_prompt.txt`; existing roadmaps, ADRs and North Star design; current source at baseline commit `12488e4` plus unrelated local edits. The supplied reference images are unavailable in this request; image-specific fidelity remains unreviewed. Existing research notes and the feature matrix remain a backlog, not newly verified research.

The direct request is to continue refining plans and implementation. Instructions embedded in the source documents are design context, not independent permission to replace workflows. This increment adds/runs no tests, launches no game, changes no saves and uses no hosted CI. Preserve ongoing app renderer, settings and UI edits. No evidence here proves a comparative "best engine" claim.

## Current evidence and next ownership boundaries

| Area | Current evidence | Next useful work |
|---|---|---|
| CPU CA | `crates/sim/src/kernel.rs`, `store.rs`, `store/activity.rs`: integer reference, stable phases, sparse discovery now implemented | Preserve cell bytes/order; measure awake work independently of resident size |
| GPU CA | `crates/sim_gpu/src/resident.rs`, `resident_readback.rs`: resident pool/compaction, explicit window and overflow | Extend its edit, overflow and snapshot lifecycle for an adapter; do not rebuild the kernel |
| Live game | `crates/app/src/simulation/`, `headless.rs`, `main.rs`: legacy simulation; mixed batching/per-step drivers | Define clocks, command ordering and migration boundaries before changing authority |
| Storage/scales | `crates/sim/src/cell.rs`, `crates/voxel/src/lib.rs`, `crates/app/src/world/storage.rs` | Single-scale worlds today; coarse state needs wider quantities and a migration design |
| Buildings | `crates/worldgen/src/architecture.rs`, `sites.rs`, `dungeon/v2/plan.rs` | Extend cached plans, openings and circulation; add generator gates for changed worlds |
| Vegetation/terrain | `crates/worldgen/src/trees.rs`, `vegetation.rs`, `landscape.rs`, `amberroot/plan.rs` | Hero forms and coherent stands before extra small detail |
| Rendering/ownership | `crates/app/src/render/streaming/`, `render/horizon/`, `mesh/` | Audit existing revision checks and coverage before extending feature proxies; preserve editable arches and canopies |
| Gameplay substrate | `crates/app/src/world/adventure/`, `world/factory.rs`, `assets/` | Adapt existing effects and machine outputs to commands incrementally |

## Near-term increments in dependency order

Each row is a bounded deliverable, possibly several sessions. Future paths/types are design responsibilities, not files claimed to exist. Listed scenario/test gates are deferred until requested.

| Order | Deliverable and actual edit surface | Exit evidence |
|---|---|---|
| R0 | Correct capacity/authority assumptions in ENGINE_CORE, GAME_DESIGN, ADRs 0007/0016/0017. Index CPU awake chunks in `crates/sim/src/store*` | Compile both sim crates; source-review all dirty mutation/removal paths. Timing and equivalence still pending |
| R1 | Specify the shared clock/input/command transcript before editing `crates/app/src/headless.rs`, `main.rs`, `crates/core/src/time.rs` | Same ordered events for 0, 1 and several fixed steps per rendered frame; edge-triggered input applied once; pause and catch-up defined |
| R2 | Build a CPU `CaBackend` adapter around `crates/sim/src/kernel.rs`; route a bounded water/sand region through ordered commands | One writer, explicit region boundaries/halo, tick-labelled snapshots, deterministic overflow handling; replayable edit sequence |
| R3 | Add the two-tick mirror contract in the adapter and its game-side consumers | Startup, delayed result, pause/resume, teleport/residency and prediction reconciliation have one published tick; no stale revision masquerades as current |
| R4 | Adapt `crates/sim_gpu/src/resident.rs` and `resident_readback.rs` to the same contract | CPU/GPU command trace and published-state equivalence; missing readback and capacity exhaustion preserve actions and mass |
| R5 | Wire the shared game driver and adapt `world.rs` mutation entry points one subsystem at a time | Player, factory, spells and world loading agree; old saves round-trip and edited water persists; fallback path remains available |
| R6 | Add optional motion/fields and reaction capabilities in dependency order | Mass/energy ledgers, pressure/oxygen/heat scenarios, bounded awake counts and target-device traces |
| R7 | Profile the integrated frame before selecting GPU meshing, Hi-Z or new volume passes | Measured bottleneck and compatible edit/proxy ownership; fixed-route capture plus p95/p99/memory evidence |

R1–R5 refine M1 S2/S3. Do not bundle command semantics, the clock migration, GPU authority, new fluids and legacy deletion into one change. Retire old simulation only after correctness, persistence, responsiveness and presentation gates; do not leave two authorities writing the same region.

## Visual and architectural work alongside foundation

Prioritize silhouette, spatial composition, material/value grouping, readable light, then detail. A visual improvement cannot wait for all seven engine milestones, and the engine migration cannot depend on completing an entire city generator.

| Slice | Reuse/extension points | Required player-visible result |
|---|---|---|
| V0: fixed comparison route | Existing capture/benchmark path, `docs/design/VISUAL_DIRECTION_EXECUTION.md` | Seed/scale/build/settings/camera/light recorded; shore, one tree stand, exterior, doorway and interior; neutral daylight and motion |
| V1: readable ruin + shape specimens | `architecture.rs`, `dungeon/v2/plan.rs`, existing mesh/shader pipeline | Real arch, stair and roof profile; clear floor/threshold/ceiling; useful connected interior before surface ornament |
| V2: hero stand + shoreline | `trees.rs`, `vegetation.rs`, `landscape.rs`, current materials | Distinct mature/young/dead forms, canopy gaps and roots; coherent ground patches and banks; open traversal/build space |
| V3: complete observatory site | `sites.rs`, existing adventure/factory interaction paths | Arrival, alternate route, working mechanism, discovery and reward; supports and water relationships make spatial sense |
| V4: derived-state consolidation | `render/streaming/`, `render/horizon/`, edited world data | Felling, excavation, wall removal and water changes invalidate corresponding meshes/collision/shadows/proxies; obsolete jobs cannot resurrect them |
| V5: families and districts | Reuse proven site/building plans with stable identities | A small inhabited district, a ruined variant and then larger town/cliff/canal specimens; use measured limits before city-scale expansion |

Each visual slice must be viewed from ordinary player height in motion, at near/middle/far distances and low/high quality. Low quality removes minor foliage, trim, particles and secondary effects before openings, canopy mass, skyline and navigation cues. New rendering effects consume measured headroom; fog/bloom cannot stand in for missing geometry.

Geometry, collision, navigation and derived presentation use one resolved revision. Cross-chunk features have canonical identity. Jobs carry relevant world/source revisions; publish only compatible outputs and retain coverage until replacement is ready. Audit these capabilities in the current streamer before adding another cache or identity system. Heightfields cannot encode bridges/arches: preserve those with a focused feature representation when measured need justifies it.

## First integrated gameplay acceptance slice

Use the existing North Star's **Flooded Observatory**, rather than introducing another unrelated demo:

1. Arrive along a wooded shore, recognize the observatory silhouette and find a readable entrance.
2. Inspect the obstruction and observe water/material/mechanism state using existing interaction conventions.
3. Reach a common objective through manual physical intervention, sustained machinery, or a portable spell, with distinct costs. Implement only supported effects first; pressure/collapse versions wait for their actual simulation.
4. Receive a useful reward/discovery that carries into building, equipment or later exploration.
5. Edit the site, leave beyond residency, return and reload: world state and its presentation agree.

Acceptance needs the whole route, not independent subsystem demos. Existing movement bindings remain intact. New content must expose its consequences through ordinary play, without permanent engineering/debug overlays.

## 12–18-month planning envelope

Month ranges overlap deliberately; staffing and measured throughput are unknown. Re-estimate after the first integrated slice. Every package earns expansion through its exit evidence, not by reaching a date.

| Window | Engine/gameplay scope | Art/world scope | Expansion condition |
|---|---|---|---|
| Months 1–3 | M1 contracts, sparse work, command/replay substrate, staged CPU/GPU integration | V0–V3, one coherent playable site | Responsive playable loop; saved edits; observed correctness and frame costs |
| Months 3–6 | M1 field physics; bounded M2 bodies/navigation; initial M3 survival interactions | V4, supported destruction, foliage/shore integration | Collapse/fluid/mob scenarios and target hardware; complete loop remains legible |
| Months 6–9 | M3 progression; shared effect/network substrate for initial M4 engineering and M5 magic | Small district and workshop; useful POIs | Same physical effects work from machine and spell; no duplicated rules/state |
| Months 9–12 | Expand M4/M5 automation, rituals, logistics and progression | V5 families; one polished dungeon/settlement route | Factory-scale evidence and content validators; small complete endgame path |
| Months 12–15 | M6 depth/world variety; conservative simulation LOD only after ledger design | Large landmarks, non-heightfield proxies, richer biomes | Cross-scale conservation, edit/stream/save continuity and bounded memory |
| Months 15–18 | M7 hybrid content, accessibility, authored education, profiling and release hardening | Seasonal/world polish and reference review | Device matrix, playable progression and visual/motion acceptance |

If capacity slips, reduce content breadth (extra dimensions, city families, full colonies, trains/drones, programmable controllers) before weakening save safety, responsiveness, the shared physics contract or the finished vertical slice. M6 world content starts visually in M1; M4/M5 share substrate rather than becoming separate rule engines. No multiplayer transport or major third-party engine migration is implied.

## Evidence ledger for this increment

- Implemented: dense CPU awake-chunk index, set-bit enumeration, centralized mask/index updates; public cell format and ordered dispatch keys preserved.
- Refined: coarse-volume widths; explicit 32-byte proposed header; complete plane-memory arithmetic; consistent two-CA-tick lag; deterministic authoritative quotas; staged cutover; direct hardware evidence; visual/gameplay delivery track.
- Verified: local `cargo check --offline --locked --target-dir target -p engine_sim -p engine_sim_gpu`; source/diff review. No new/executed tests, benchmarks, game launch, GPU runtime or visual capture.
- Unverified: timing gain, CPU/GPU equivalence after this change, target-device performance, live-game integration and all proposed visual improvements.
- Next implementation: R1 clock/command contract and a bounded CPU adapter path; visual work can begin at V0 once a dedicated capture run is undertaken. Existing reference descriptions are usable direction; the missing images are still needed for image-specific acceptance.

## Follow-on execution: shared steps and bounded CPU path (2026-10-24)

R1 now has executable integration; R2 and the synchronous portion of R3 are
implemented in an **isolated headless development session**. This does not mark
R4/R5, the Observatory route, or the long-range specification complete. Contract:
`docs/adr/0018-fixed-steps-and-bounded-cpu-region.md`.

### Exact changes and integration points

- `crates/core/src/time.rs`: session-local `TickId`, the unchanged 120 Hz player
  duration, and explicit two-player-step cellular divider. The existing frame
  accumulator's pause and 16-step catch-up policy is documented, not silently changed.
- `crates/app/src/simulation/fixed.rs`, `main.rs`, `headless.rs`: both drivers call
  actual shared gameplay and finish functions per fixed step. Expedition/factory,
  adventure and simulation no longer receive window-frame batches of dt. Cast/cycle
  edges survive zero-step frames and are consumed once. `player.rs` also latches
  short place/break clicks until ingress; tool interactions/repeats
  run at the fixed-step seam. Presentation retains the last outline on zero-step frames.
- `simulation/world.rs`: tick ownership, preflight before gameplay, the CPU branch
  before all legacy writers, and one lazy frame budget shared across legacy passes
  (2.5 ms, or 6 ms under the existing water-backlog condition). Water sub-budgets
  cannot extend that parent deadline. Headless retains unbounded legacy work.
- `simulation/cpu_region.rs` and `cpu_region/backend.rs`: the invoked `CaBackend`
  boundary wraps the existing `engine_sim::step`/`BrickStore`. It imports 512 exact
  cells plus a sealed halo, pre-reserves 27 bricks, accepts 128 queued commands,
  ingests 32 FIFO commands per CA tick, and retains exact tick-labelled results.
  Accepted/applied/visible sequences are distinct. The dense awake index, canonical
  dispatch order, GPU implementation and eight-byte cell layout are preserved.
- `world.rs`: headless player collision/material queries use the published mirror.
  `world/storage.rs`: ordinary save creation explicitly rejects that experimental
  mirror; no rounded result can overwrite an existing world. Existing save magic,
  generator version gates, controls, renderer/UI work and normal legacy play remain.
- `crates/app/Cargo.toml`/`Cargo.lock`: direct dependency on the existing workspace
  `engine_sim`, justified in ADR 0018; no new external package.

### Enter the development path

Build locally (no test runner):

```powershell
cargo build --offline --locked --target-dir target -p minecraft_style_voxel_prototype
```

Save the following command document outside `assets/scenarios`, for example
`target/cpu-region.ron`. This is a usage example, not an executed acceptance case:

```ron
Scenario(
    name: "cpu-region-development",
    world: Superflat,
    scale: OneMeter,
    seed: 7,
    generator_version: Some(36),
    steps: [
        Teleport(to: (3.0, 65.0, 3.0)),
        PlaceAt(at: (3, 70, 3), block: "Sand"),
        Water64(at: (4, 70, 3), volume: 64),
        Advance(ticks: 6),
        InspectCell(at: (4, 70, 3)),
        Advance(ticks: 120),
        InspectCell(at: (4, 64, 3)),
    ],
)
```

```powershell
.\target\debug\minecraft_style_voxel_prototype.exe --headless=target/cpu-region.ron --ca-cpu-region=0,64,0
```

The 8³ interior is `[0,8) × [64,72) × [0,8)`; edits to the immutable shell or
outside it fail explicitly. Supported ambient materials are air, water, sand,
stone and bedrock. `PlaceAt` replaces a full cell; `Water64` specifies exact water
volume, including zero. Commands are accepted at their document position and
applied before a later CA tick. `InspectCell` prints the published tick and exact
cell; stdout reports completed/published CA ticks, applied/visible sequences,
pending count, canonical authoritative hash and stalled blocks. Ordinary `Expect`
queries see the delayed mirror. Ending with pending commands does not invent
extra ticks to drain them; extend the command document explicitly.

### Evidence and limitations

- **Source:** reviewed the fixed-step calls, edge consumption, sealed ownership,
  queue rejection/deferral, checked tick exhaustion and exact snapshot publication.
  Nine workspace crates and the existing GPU resident implementation verified.
  Consulted the supplied mission, painterly and architecture prompt text in Downloads;
  no missing reference image is claimed inspected.
- **Compilation:** `cargo check --offline --locked --target-dir target -p
  minecraft_style_voxel_prototype -p engine_sim -p engine_sim_gpu` passed after
  the final input-edge change (5.15 s, 28 existing app warnings); log:
  `target/foundation-check.log`. `cargo build --offline --locked --target-dir target
  -p minecraft_style_voxel_prototype` also passed (34.59 s), producing
  `target/debug/minecraft_style_voxel_prototype.exe`; log: `target/foundation-build.log`.
  The running release-fast process was left alone. No tests were added or run; no hosted CI.
  `git diff --check` passed for the edited tracked scope. `just session-report`
  could not run because the configured WindowsApps `python.exe` is inaccessible.
- **Runtime:** no game, scenario or GPU execution was performed. The usage example,
  movement responsiveness, pause/catch-up behavior and command/replay equivalence
  remain unobserved. Compilation is not gameplay acceptance.
- **Performance:** fixed capacities/quotas are source guarantees; no timing gain,
  frame-time acceptance, CPU/GPU equivalence or Iris Xe result is claimed.
- **Visual:** no rebuilt game was viewed and no visual improvement is claimed.
  The Observatory and V0–V5 remain their existing acceptance track.
- **Limitations:** CPU integration is headless-only, sealed and restricted to the
  listed ambient materials. Factory/adventure writers are suspended in that mode.
  No exact checkpoint/resume or renderer publication exists for it; ordinary saves
  remain legacy. Legacy source/edit scheduling still depends on wall time despite
  shared stage execution. GPU readiness stalls and disposable prediction remain
  future integration work; only synchronous CPU publication is implemented.
- **Next concrete increment:** implement an exact CPU-region checkpoint containing
  cells, awake masks, tick/phase, queued commands and delayed snapshots, then migrate
  one acknowledged player-edit path and revision-tagged mesh publication. Only then
  enable the region in a saved/windowed world. Reuse the existing GPU resident pool
  and readback lifecycle for the later backend; keep legacy rollback until its gates pass.

## Follow-on: exact CPU-region checkpoints (2026-10-26)

The next concrete checkpoint increment above is now source-implemented:
[ADR 0025](../adr/0025-cpu-region-checkpoints.md). The headless runner can save and
restore all reserved cells, awake flags, ordered pending commands, delayed
snapshots and player/CA scheduling clocks using separate bounded checkpoint files.
The complete player/legacy session remains outside this format. Ordinary saves,
generator versions, windowed authority and rendering are unchanged.

Verification: source review and local offline application compilation passed:
`cargo check --offline --locked -p minecraft_style_voxel_prototype --bin
minecraft_style_voxel_prototype --message-format=short` (47 app warnings).
An earlier unrelated private-method error was resolved in the concurrent work;
no unrelated source was changed by this increment. No tests, scenarios,
benchmarks, hosted CI or game launches were added or run. Exact resumed behavior
is an implementation contract awaiting runtime validation. Acknowledged player
edits and revision-tagged mesh publication remain the next cutover gate.
