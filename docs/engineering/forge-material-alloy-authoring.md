# Forge material and alloy authoring: implementation and review

## Intended outcome

An author can adjust the existing bronze tool material and its copper/tin alloy
ratio through Forge's node/System Board editor or a JSON command. Both interfaces
publish through the same revision-checked native validation and backup/bundle
history. The existing native game consumes the definitions after rebuilding.
No new parallel material registry, item class or progression system is introduced.

The long-term direction is an intent-to-production compiler: requirements become
typed editable features, tested in the native sandbox and delivered as reversible
packages. This implementation establishes one real data path, not a universal
feature graph or automatic implementation of missing engine primitives.

## Current representations and sources of truth

* `assets/materials.ron` describes voxel appearance, physical properties and phase
  changes. `engine_content::BlockId` remains a Rust enum whose order is persisted.
  `MATERIAL_TABLE` loads the file and `SIM_TABLE` packs it for simulation. A new
  block ID still requires native registry integration and a rebuild.
* Factory resource IDs represent ore/dust/ingot inventory buffers. They remain
  native fixed IDs; bronze is resource 14. This work does not add resource IDs.
* `assets/tools/materials.ron` owns assembly balance: durability, mining tier and
  speed, attack, handle modifier, mass density and trait inputs. These are not a
  second mutable copy of each tool's computed stats. They are definition inputs;
  `ToolAssembly::stats` derives the result from schema weights and material parts.
  Some physically similar properties use different units/semantics from voxel
  materials; this work does not silently propagate between them.
* `assets/tools/smeltery.ron` maps solid/liquid blocks to ingot resource IDs and
  feeds. `assets/tools/workshop.ron` maps tool materials to payment/casting supply.
  Bronze now has an explicit `block: Some("BronzeBlock")` link. Its ID and table
  position are unchanged; existing supply and casting remain compatible.
* `assets/reactions.ron` owns material-contact reactions and exact molten batch
  ratios. Shared native parsing validates and compiles these against `SIM_TABLE`.
  Batch rows remain in the sparse connected-region smeltery solver; they are not
  flattened into inaccurate 1:1 GPU reactions.
* Bronze's kiln ratio and label previously repeated the smeltery recipe in Rust.
  Kiln mode 0 now projects the authored molten batch through the existing
  smeltery liquid/resource map. A ratio change updates costs, output and label.
  Its mode ID, 24-tick duration and 10-power cost remain unchanged. Steel mode 1
  remains a separate native recipe. Kiln temperature simulation is not added.
* `assets/recipes.ron` and `assets/fabrication.ron` cover other existing authorable
  crafting paths. Other factory routes still contain native recipes. There is no
  claim that all recipes or Minecraft mod mechanics are now generic nodes.

## Shared command and validation flow

The native content crate now owns the spell schema instead of the application.
`engine_content::spells::parse`, `engine_content::reactions::parse`,
`engine_tools::parse_parts`, and `engine_tools::parse_materials` are shared by
runtime loading and small read-only JSON checker executables. Tool materials
reject non-finite/invalid stats, unknown fields, duplicate IDs and unknown block
links. Reactions reject invalid batch ratios, field typos, and ambiguous duplicate
batch input pairs or products. Diagnostics retain field/record or source context.

`workspace.mjs::prepareSave` invokes those exact native checkers before staging
registered catalogs. This covers inspector saves, AI bundles and restores.
`commitSaves` retains existing hash checks, backups and transaction behavior.
Material/part/schema IDs cannot be renamed, removed or reordered through this
path. Appended IDs still need all existing engine integrations and capacity checks.

Checker receipts bind executable hashes to native source/manifests, the pinned
toolchain and the packed physical material table used by reaction validation.
Missing or stale checkers refuse catalog publication with a rebuild command.
Other project editing remains available. Native schema changes and dependent
catalog changes cannot be published together against an older checker: build the
new schema first, then validate its data. This is an explicit missing compiler
step rather than a fake guarantee about an unbuilt candidate.

The CLI uses the existing AST patch operations and `publishBundle`:

```powershell
node tools/axiom-dev/build-validators.mjs
node tools/axiom-dev/alloy-cli.mjs inspect bronze
'{"material":"bronze","changes":{"durability":480},"batchCounts":[2,1]}' | node tools/axiom-dev/alloy-cli.mjs preview
```

Preview writes nothing. Repeat the request with its returned `versions` and
`approval` to `apply`. Changed source revisions or a different candidate require
a fresh preview. Candidate bundle history can revert an applied edit while
refusing unrelated changes. The workbench uses that same publisher when reviewing
its two changed files; no separate AI mutation implementation bypasses validation.

## Open and review in Forge

Launch `tools/Open-AxiomDev.ps1`. Open **Reference Map → Materials and alloying**,
then **Bronze alloy workbench → Open controls**. The existing System Board exposes
durability, mining speed, attack, conductivity and the two volume counts. Use
**Review system changes** for a two-file publication. Follow dependency links from
the tool material to its block, molten phase and reaction. Rebuild and relaunch
Axiom to consume saved definitions. Catalog saves are not runtime hot reload.

No new art is invented for an existing material. Bronze retains its recognizable
ingot, block, casting and assembled-part art. New sprite/model and ore-generation
work remain explicit requirements when adding a genuinely new material family.

## Save and multiplayer behavior

Tool/weapon instances remain `ToolAssembly` records referenced by
`Item::Assembled { id, schema }` and the existing tool bank. Mining and blocking
read schema capabilities instead of subtype-name branches. Parts, modifiers,
XP, level and remaining durability keep their existing codec and IDs.

After an authored durability reduction, loading clamps remaining uses to the new
maximum. Increasing the definition does not repair an old tool. Unknown material,
part and level data retain validation. No save-version or packet-format change is
introduced. Multiplayer mining already uses the same authoritative tool resolver.
All participating builds still require matching content; content synchronization
and a multiplayer playtest are not supplied by this change.

## Verification and remaining limits

The slice is integrated in the shared checkout. Its 42-file patch and before/after
SHA-256 manifest are retained in task-5. Shared file revisions were compared before
publication; unrelated changes were preserved. Native validators were rebuilt and
installed after Git normalized text to the repository's LF policy.

The integrated solo app check passed with pinned Rust 1.99.1. Focused native tests
passed: 25 tool tests (including codec and durability reconciliation), 6 spell
schema tests and 9 reaction tests. The pre-existing reaction canonical-format
round-trip mismatch remains excluded from that focused command. The four checker
commands build and accept the actual 101 spells, 13 tool schemas, 19 tool materials
and 13 reaction rows. Earlier checker CLI verification passed 20 cases including
the 4 MiB boundaries.

All 13 native-save checks pass against the shared checkout using actual native
binaries and a disposable fixture. They cover publication/backups, rejection
without writes, stale revisions, whole-bundle rejection, schema freshness,
persisted IDs, restore, bronze preview/apply/revert and node-link/board resolution.
Batch pins now identify the unique alloy product; reversing recipe order keeps
both volume controls on bronze, while duplicate products make the link ambiguous
and unresolvable. The isolated Forge regression run passed all 18 tests.

The final combined shared Forge run passed 17 of 18 tests. The existing
sprite-usage test's native-composition precedence assertion fails after concurrent
art changes; the native-save checks all pass. A no-write CLI preview against the
integrated source validates the requested durability and ratio changes while
preserving both original source revisions.

The first integrated multiplayer check encountered two concurrent held-item errors.
After those definitions landed, a retry reached one remaining compile error in
crates/app/src/egui_ui/icons/tools.rs:96 (five arguments supplied to a six-argument
method). No file in this slice produced a compiler error. The full app test target
still reports 28 existing errors in other test modules (egui input/event/texture
changes and obsolete world/model APIs). The new kiln tests could not execute in
that blocked test binary. A newly linked playable executable and a multiplayer
playtest remain unverified. The earlier isolated snapshot passed solo and
multiplayer checks before those concurrent art changes.

Browser access recovered. The actual integrated Forge server was opened through
Graph, Reference Map, Materials and alloying, Bronze alloy workbench, Open controls.
Its live six-control board showed durability 430, speed 6.5, attack 4, conductivity
0.4, copper count 3 and tin count 1. UI navigation was checked without publishing a
balance edit. Publication and reversion are exercised by the disposable tests.

C: disk pressure was relieved by deleting only the completed task-local native
probe build output (225 MiB) and relocating this task's reusable compiler cache to
D:/axiom-forge-verification-task5/target. Probe source and runtime evidence remain.
No user saves or shared build outputs were removed.

Still native work: new resource IDs, unknown block/phase behavior, new machine
primitives, melee/ranged capability dispatch, world generation, network authority
for new effects, and real visual/playability review. This implementation edits one
supported existing alloy; it does not claim arbitrary new alloy/material creation
or perfect compatibility with every external mod.

## Native runtime evidence

A native probe linked the actual content and tool crates, constructed a bronze
pickaxe with stone binding and wood handle, then read the compiled reaction table.
The Forge JSON command changed bronze durability from 430 to 480 and the batch
ratio from 3:1 to 2:1. After rebuilding, computed tool durability changed from 460
to 510 and the native reaction reported 2:1. Mining speed stayed 6.5. The existing
bundle revert restored both source hashes. This demonstrates definition consumption
by the native libraries; it is not a graphical or multiplayer playtest.

The retained evidence is task-5/native-runtime-evidence.json. Architecture and
development-only dependency choices are recorded in
[ADR 0042](../adr/0042-shared-native-content-validation.md).
