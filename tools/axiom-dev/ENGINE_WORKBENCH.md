# Engine Workbench

One native protocol powers the Playtest panel and the compact CLI. Inspect the
running game, change supported values immediately, compare frames or recovery,
then undo. Source files are not rewritten by live controls.

## Start and discover

1. Start `node tools/axiom-dev/server.mjs --open` and build once in **Playtest**.
2. **Engine workbench → Start fresh world** opens an isolated playable world.
3. **Inspect / refresh** loads controls from the running engine, including units,
   limits and current values. **Capabilities** explains live and rebuild support.

From the checkout, the same workflow in PowerShell is:

```powershell
node tools/axiom-dev/engine-cli.mjs help
'{"world":"hills","seed":74,"scale":1}' | node tools/axiom-dev/engine-cli.mjs start
node tools/axiom-dev/engine-cli.mjs status
node tools/axiom-dev/engine-cli.mjs wait
node tools/axiom-dev/engine-cli.mjs describe ambience
node tools/axiom-dev/engine-cli.mjs inspect player location
'{"sparkles":false,"mana_recovery_max":8}' | node tools/axiom-dev/engine-cli.mjs tune
node tools/axiom-dev/engine-cli.mjs capture quiet-woodland
node tools/axiom-dev/engine-cli.mjs undo
```

The CLI prints one JSON result, or an error and a nonzero exit code. It discovers
the local authenticated panel through `target/axiom-dev/engine-session.json`.
Use `--session target/axiom-dev/engine-session-<pid>.json` when multiple panels
are open. This file contains a local session credential; do not attach it to AI
context or copy its contents into reports. Neither client accepts a remote host.

`status.ready` means the command bridge is connected. `inspect` additionally
requires a loaded solo world. The native response distinguishes that condition
from an unsupported command or an old build. Hosted, joined and cellular worlds
are excluded from this workbench. The disposable session starts creative and
uses the existing static benchmark launcher; it remains manually playable.

`wait` makes bounded read-only observations until the world and its local sample
are stable across two observations (up to sixty seconds). **Close experiment** /
`close` waits for only this panel's disposable
world without requesting a save; captures and presets remain. Ordinary playtest
worlds still use the game's save/quit controls.

`survey 128` returns nine generated surface positions and biomes around the
player. These are navigation leads, not collision clearance or measured local
mana. Visit a position with the camera controls, then inspect the actual place.
For model work choose **Factory models**, or start with `{"scene":"factory"}`:
it reuses the existing factory visual fixture on superflat, with a fixed noon camera
and actual machines already in view. It is a model review scene, not a gameplay
or performance measurement.

## Compact inspection

`inspect` accepts up to six sections. Omit them for player and location only.
Transport rounds small floating-point observations to four decimals so both
interfaces avoid unreadable binary-float noise; native calculations are unchanged.

| Section | Evidence |
| --- | --- |
| `player` | Feet in metres, yaw/pitch in radians, flight/ground/water flags, actual vitals and capacities |
| `location` | Readiness, measured place, sample position, mana pressure, recovery multipliers, toxin damage, nearby flora/water/exposure, effect profile |
| `world` | Seed, generator, world kind, voxel size, mode, time and active object counts |
| `look` | Aimed block and cell within eight metres, or null |
| `tuning` | Current live ambience and regeneration settings |
| `models` | Factory model names actually loaded by the native renderer and active overrides |

Location effects are a profile, not a count of particles currently visible.
Daylight, weather, immersion, shielding, graphics rendering and live tuning also
affect presentation. A reading can be unavailable during travel; the engine
clears stale bonuses rather than carrying a forest through another dimension.

`describe ambience|materials|models|engine` returns bounds, units, source owners
and reload support. `status` includes a unique run ID and SHA-256 of the launched
binary. `sample` returns bounded observations with elapsed time and revisions:

```powershell
'{"count":6,"interval_ms":500,"sections":["player","location"]}' | node tools/axiom-dev/engine-cli.mjs sample
```

This measures observed state, not rendering performance. Frame captures report
their path and whether the file has been written; a queued/pending capture is
not visual evidence. The UI's **Gather Context** also includes an optional native
snapshot with build identity, source links, timestamp and content hash. It is
limited to sections relevant to the task instead of always listing every model,
and explicitly labelled one observation, not a complete playthrough. Known run or
preview changes require gathering fresh context before applying an AI proposal.

## Live edits and scope

| Control | Live scope | Persistence |
| --- | --- | --- |
| Ambience | Safe natural density ceiling, mana/health recovery caps, particle density, sparkles, mist, colour, transition speed | Session override; named presets save ambience values separately |
| Materials | Existing terrain-family tint, roughness, reflectance | Session overlay; original Material Studio save remains intact |
| Models | Existing loaded factory text-voxel asset | Session overlay; source/editor draft remains separate |
| Vitals, camera, time | Explicit experiment setup in the current world | Changes real world/player state; not covered by preview undo |

All preview writes require the current revision and validate the complete change
before publication. The CLI's `tune`, `material`, `model`, `undo`, `reset` and
`preset load` do their own compact revision preflight; `request` uses the exact
revision supplied by the caller. A conflict means inspect and reconsider, not
blindly retry. The native undo stack holds sixteen preview states. Leaving the
world clears these overrides. **Reset previews** clears all three preview types
and can itself be undone.

The safe density ceiling is constrained below the toxicity threshold even when
experimenting. Health recovery retains food, hurt and death conditions; this
control is not an unconditional heal. Sparkles off suppresses magical glints and
ice crystals while physical pollen, petals, dust, spores and spray keep their
place-specific roles. Set particle density to zero to fade all ambience motes.

Save/load a named ambience experiment without editing game data:

```powershell
node tools/axiom-dev/engine-cli.mjs preset save quiet-eightfold
node tools/axiom-dev/engine-cli.mjs preset list
node tools/axiom-dev/engine-cli.mjs preset load quiet-eightfold
node tools/axiom-dev/engine-cli.mjs reset
```

Presets include source-world and run provenance, live under
`target/axiom-dev/engine-presets`, and never overwrite an existing name. Loading
one applies ambience only; it does not teleport, replace a world or restore vitals.

## Models and materials

Paint a factory voxel model in **Models**, then choose **Apply to game**. Model
Study has the same action for an imported draft. The slot must already be loaded
by a visible machine in the game; `inspect models` gives exact names. Changes
replace render geometry on the next frame without rebuilding or relaunching.

```powershell
node tools/axiom-dev/engine-cli.mjs inspect models
node tools/axiom-dev/engine-cli.mjs model <loaded-asset> <draft.vox>
node tools/axiom-dev/engine-cli.mjs describe materials
'{"family":1,"tint":[1.1,1.0,0.9],"roughness":0.7}' | node tools/axiom-dev/engine-cli.mjs material
```

Native model validation requires `size 8 8 8`, eight rows per declared layer, defined
palette symbols, at least one occupied voxel, finite glow in 0..8 and at most
16 KiB. It rejects an invalid model without changing the previous preview.
This does not alter collision, animation rigs or block IDs. New native behavior,
embedded catalogs, new block IDs and rigged model topology still require a build.
Use the existing catalog, source, bundle and build tools for those edits.

## Native request reference

POST `/api/engine` with the existing `X-Axiom-Session` credential. The server
forwards `/engine <JSON>` over the private game bridge. Native replies contain
`ok`, `protocol:1`, `revision` and `data` (or `error`). HTTP clients receive a
descriptive error on rejection. Unknown operations/fields are rejected.

```json
{"op":"inspect","sections":["player","location"]}
{"op":"survey","radius_m":128}
{"op":"tune","revision":0,"values":{"particles":0.5,"sparkles":false}}
{"op":"material","revision":1,"family":1,"tint":[1,1,1],"roughness":0.8,"reflectance":0.5}
{"op":"model","revision":2,"asset":"a-loaded-name","text":"size 8 8 8\n..."}
{"op":"undo","revision":3}
{"op":"reset","revision":4}
{"op":"vitals","mana":0.1,"health":0.5,"hunger":1}
{"op":"camera","position_m":[0,100,0],"yaw":0,"pitch":0}
{"op":"time","fraction":0.25}
{"op":"capture","name":"woodland-day"}
{"op":"perf"}
```

`perf` returns the running session’s rolling frame-time percentiles (avg, p50, p95, p99, max), average FPS and 1% low, resolution, adapter and view distance, with the age of the sample. It also names where the time goes: `largest` lists the three biggest averaged costs across CPU stages and GPU passes, `cpu_ms_avg` and `gpu_ms_avg` give every stage (`cpu.render` includes waiting on the GPU; `gpu_ms_avg` is null without timestamp queries), `scene` gives drawn chunks, triangles, lights and shadow draws, and `streaming` gives the pending, generating and meshing queues with resident memory. `stages_ms` splits `cpu.gameplay` and `cpu.streaming` into their ten largest named sub-stages in ms per frame (`stream.*` are the streaming steps). Read `largest` first when a session is slow, then `stages_ms`, and `streaming.pending` to tell a world still loading from a steady frame. The game publishes them at 5 Hz only while the bridge is open. They describe this session on this hardware, not a controlled A/B, so compare builds with the interleaved frame bench. **Frame stats** in the Engine workbench shows them, and **Gather Context** adds them (`livePerf`) only for briefs that mention performance, frames, stutter or similar, to keep token use low.

Vitals are fractions of current maxima (health minimum 0.01; dead players must
respawn). Camera placement requires creative/spectator, uses world metres and
does not resolve collisions. Inspect the destination before moving. Captures
accept unique 1..48-character letters/digits/hyphen/underscore names and remain
inside the active playtest's capture folder. Ordinary commands remain available
through the existing command panel.

## Iteration discipline and owners

1. Gather the smallest useful source context and native sections.
2. Name the intended player experience and an observable comparison.
3. Inspect current revision, change a few controls, record the actual result.
4. Critique readability, balance and interaction with existing systems.
5. Refine or undo; use source review/save only when making a durable edit.

`crates/app/src/dev_panel/engine.rs` owns native commands and validation;
`world/ambience.rs` owns location rules; `material_studio.rs` and
`mesh/models/voxel_asset.rs` own their preview stores. `engine-service.mjs` owns
local transport, presets, captures, sampling and AI observations. The browser
and CLI consume those same capabilities rather than implementing parallel rules.

Future controls should extend this protocol with narrow inspect sections,
bounded values and explicit reload/persistence semantics. Do not advertise an
editable field as live before a native caller actually consumes the override.

## Fast iteration and world fixtures

Set `AXIOM_DEV_EXE` before starting the server to run any built executable, for
example `target/debug/minecraft_style_voxel_prototype.exe`; no release-fast build is
needed. A session accepts `"view": 4..64` (chunks): small values load quickly and fit
debug builds, e.g. `{"world":"hills","seed":74,"scale":1,"view":6}`.

```powershell
node tools/axiom-dev/engine-cli.mjs command gamemode creative   # any solo chat command; slash optional
node tools/axiom-dev/engine-cli.mjs command tp 35.5 38 70
node tools/axiom-dev/engine-cli.mjs fill 38 37 64 38 38 64 Ice  # inclusive voxel box, at most 4096 cells
node tools/axiom-dev/engine-cli.mjs probe 38 37 64              # block in one cell
node tools/axiom-dev/engine-cli.mjs hedrons 256                 # mana hedrons, capture state, pylon cells
node tools/axiom-dev/engine-cli.mjs capture-hedron              # build the pylon frame on the nearest one
```

`command` reports `needs_ui: true` when the game window, not the protocol, applies the
effect. `fill` edits loaded chunks only and reports how many cells now hold the block.
Edits land on the next frame, so read state back in a later call. Offline, `<exe>
--benchmark-world=hills --benchmark-seed=N --terrain-survey` prints hedron sites with a
ready `--benchmark-terrain-camera` pose.

## Machine tests

```powershell
'{"input":{"Copper ore":64},"energy":5000}' | node tools/axiom-dev/engine-cli.mjs test 40 37 64 Crusher 600
node tools/axiom-dev/engine-cli.mjs test 40 37 64 120          # existing machine: step only
node tools/axiom-dev/engine-cli.mjs machine                     # looked-at machine; machines [radius] [Kind]; machine-kinds
```

`test` places the kind when given, applies the stdin `set` object (`mode`, `target`,
`enabled`, `recipe`/`filter` with -1 to clear, `energy`, `input` {resource: count},
`clear_output`), steps up to 60 s of fixed ticks, and prints `end` (final state)
plus `changes` ({field: [before, after]} per probed cell). Reports omit zero, false
and empty fields; `off` marks a disabled machine. Machine commands print only the
native data, not the envelope. Steps run outside the frame budget; drills and
quarries edit resident chunks only. Single-step commands (`place-machine`,
`set-machine`, `step-factory`) remain for scripted sequences. F3 shows the same state.

## Enemy tests and scenarios

```powershell
'{"affixes":["Warded"],"elements":["Wet","Shock"],"health":50}' | node tools/axiom-dev/engine-cli.mjs spawn-enemy "Blade Rotor" 2
node tools/axiom-dev/engine-cli.mjs enemies 64                  # nearest first: kind, health, affixes, shield, statuses
node tools/axiom-dev/engine-cli.mjs clear-enemies               # all; pass a radius for nearby only
node tools/axiom-dev/engine-cli.mjs scenario tools/axiom-dev/scenarios-combat/field-foes.json
```

`spawn-enemy` takes any bestiary name; groups fan out sideways 3 m ahead unless
`position_m` is given, and blocked spots are skipped. Elements apply as one hit each.
`scenario` runs `{steps:[{op,..., expect}]}` (the factory-lab shape) or
`{steps:[{request, expect}]}`; `expect` maps dot paths in the reply data to a value or
`{min,max}`. It prints `pass` and failures and exits 1 on failure. Field foes
(`world/adventure/field_foes.rs`): Blade Rotor 60, Stone Colossus 61, Ruin Walker 62;
the Great Rock Worm is burrower strain 87.
