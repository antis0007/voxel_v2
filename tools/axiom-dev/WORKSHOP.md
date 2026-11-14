# Forge workshop: first content slice

This slice adds typed machine-process edits, revision-pinned comparison snapshots and a bounded zero-model campaign. It uses the shared native Fabricator schema and generated browser metadata. It does not implement the playable separator or mill, install content, add new machine behavior, migrate saves or copy a live world.

## Production planning and human-unit edits

`inspect` now lists every supported process and the native resource names, with
quantities in items, time in seconds and power in kW. No ID or subunit lookup is
needed to compare the existing processes.

```powershell
node tools/axiom-dev/workshop-cli.mjs plan Coils 120
'{"process":"Coils","changes":{"secondsPerBatch":4,"powerKw":12}}' | node tools/axiom-dev/workshop-cli.mjs design
```

`plan` accepts a process ID or quoted full name and the requested output items per
minute. It returns the required whole machine count, capacity, utilization, input
rates and average/peak power. At the current baseline, 120 coils/minute needs five
base-drive Fabricators, 120 copper ingots/minute, 60 lumen fibre/minute and 50 kW.
These are ideal catalog estimates, not measurements: continuous supply and free
output capacity are assumed. Transport, upgrades, interruptions and network load
are excluded. The command reads source, not the currently running game's catalog.

`design` is read-only. It resolves a named process, pins current dependencies and
returns its exact typed `request`, the preview `approval` and before/after output
rates and energy per item. Add `copyName` to append a new process instead of editing
the original. `changes` accepts `name`, `secondsPerBatch`, `powerKw`, `inputs` and
`output`. Input/output pairs use `[resourceId, items]`, so `[6, 2]` means two copper
ingots. Time must be an exact multiple of 0.25 s and item counts of 1/64. Invalid
values are rejected, never rounded.

Review the preview, then stage that exact request:

```powershell
$draft = '{"process":"Coils","copyName":"Coils trial","changes":{"secondsPerBatch":4,"powerKw":12}}' | node tools/axiom-dev/workshop-cli.mjs design | ConvertFrom-Json
# Review $draft.production, $draft.changes and $draft.candidate first.
$request = $draft.request
$request | Add-Member -NotePropertyName approval -NotePropertyValue $draft.approval
$request | ConvertTo-Json -Depth 12 | node tools/axiom-dev/workshop-cli.mjs stage
```

The lower-level equivalent is `tune-process {id, changes}` in an ordinary preview
request. The CLI and shared service use the same conversion and planner. Staged
manifests retain the production comparison and its assumptions. Review hashes also
cover the planner, CLI and resource-name registry. Source changes invalidate prior
reviews; native execution and playtesting remain pending. There is no new browser
panel in this slice and no automatic live installation.

## Current workflow

From the repository root:

```powershell
node tools/axiom-dev/workshop-cli.mjs inspect
```

Use the returned version in one request. The inspector, future viewport bridge and CLI can all call `workshopService().preview()` with the same typed operations:

```json
{
  "version": "COPY_VERSION_FROM_INSPECT",
  "requirements": ["fabricator-process", "electric-power"],
  "operations": [
    {"op": "derive-process", "sourceId": 0, "name": "Workshop coil prototype"}
  ]
}
```

Pipe that JSON to `node tools/axiom-dev/workshop-cli.mjs preview`. A preview returns the exact baseline and candidate, changed rows, dependency revisions and an approval digest. Add that digest as `approval` and pipe the same request to `stage`.

`derive-process` appends a stable process ID and preserves every existing row. It does not replace the source process. The new ID appears in the preview. A later `set-process` operation in the same batch can tune that ID's `name`, `inputs`, `output`, `ticks` or `power`. Quantities are 64 subunits per item; each tick is 0.25 seconds; power is kW. The authoring adapter admits all 25 existing native resources, including feathers (ID 24); ID 25 is rejected. Resource identities are unchanged. New resource mappings remain outside this slice.

Stage writes only a fresh `target/axiom-dev/workshop/content-*` folder containing:

- `baseline/assets/fabrication.ron`
- `candidate/assets/fabrication.ron`
- `manifest.json`, with exact source/runtime/tool revisions and pending evidence

Every stage rechecks dependencies and the precise preview digest. A source or tool change invalidates the preview. A change detected during staging discards that partial snapshot. Redirected source or staging paths are refused. Source catalogs and saves stay untouched. There is no publish or live-install command. Discarding this authoring snapshot does not rewind inventory or another player's actions.

The evidence is deliberately limited: **schema checked**, **native checks pending**, **performance not measured**, **needs playtest**. These files are not a complete game pack or native correctness receipt. Their comparison is data comparison, not side-by-side gameplay.

## Bounded campaigns

Pipe a request to `campaign` with `version`, `budget: {"maxCandidates": 3, "maxStages": 1}` and an ordered `candidates` array. Each candidate has a unique `id`, `requirements` and `operations`.

The caller's order is the priority order. Each candidate starts from the same baseline, not the preceding candidate. The controller previews and stages supported content automatically within the budget. It stops at the first missing-authority requirement or at the budget. No-change and duplicate content within the run consume no stage. A failed candidate stops the run and retains every earlier stage receipt in the report; the CLI exits nonzero without hiding completed work. An empty list returns `no-remaining-candidate`. All operations are deterministic and local: zero model calls and zero tokens. Cross-run history, reference research, repair loops, native execution and cost benchmarking are not implemented by this slice.

The inspect response includes the separator and mill requirements. Submitting either stops with exact gaps rather than silently substituting an electrical box. Requirements are conservative adapter capabilities, not a universal engine graph.

## Runtime changes needing approval

The audited implementation is `world/factory/recipes.rs`: both `Process` and `Fabrication` have one resource output; the Fabricator catalog is embedded with `include_str!`. `world/factory.rs` checks that output's capacity before spending power, then consumes inputs at completion. `world/factory/power.rs` is an electrical kW network; its waterwheel is an electrical generator, not a shaft-drive system. The fixed resource mapping in `world/expedition.rs` has no blight feed/product or wheat/flour pair. Existing scrubber water handling is specialized native behavior.

Proposed next approval: permit an **isolated runtime prototype**, retaining normal saves and live deployment unchanged, for:

1. A typed multi-output process and atomic item/fluid capacity reservation, with blocked-output, interrupted-power and unload/reload conservation tests.
2. A mechanical drive input with torque/speed/stall state, plus a shared state-to-animation/fault-cue adapter. Reuse it for the separator and a different grain mill before treating it as an approved pattern.
3. A disposable native factory fixture and revisioned selection/port bridge, so authored candidates can be played and compared without touching the original world.

Keep this prototype behind an opt-in development path. Do not add persisted machine/resource identities or change production save encoding under that approval. If existing identities cannot represent the prototype, return a separate save-migration proposal first. Multiplayer authority, reconnect, dense-factory workload and human craft review remain promotion gates. No automatic policy can mark those as passed from this tool's schema checks.

## Verification

```powershell
node --test tools/axiom-dev/workshop-process.test.mjs tools/axiom-dev/workshop-planning.test.mjs
```

The tests cover original-row preservation, typed field bounds, unsupported primitives, exact-preview approval, stale source/runtime/tool revisions, interrupted staging cleanup, redirected paths, saved-world preservation, finite budgets, authority stops, duplicate suppression and CLI routing. Native gameplay, multiplayer, save/load semantics, frame-time performance and visuals have not been tested by this suite.


## Native schema and generated metadata

Run `node tools/axiom-dev/build-validators.mjs` after changing shared native schemas.
`engine_content::fabrication` owns process validation and limits; the runtime uses
that parser. `factory_resources` owns the existing resource names/count without
changing their saved IDs. The native checker's `--describe` output generates
`fabrication-contract.mjs`. Do not hand-edit this projection. The browser parser,
recipe resource picker, planner and Workshop consume it. Native checks remain the
publication authority; the editor additionally requires unique display names for
unambiguous selection, preserving the prior authoring rule.

Checker receipts bind generated bytes and semantic values. Stale loaded server
metadata fails closed; clients send their metadata digest and mismatches request
reload before actions. Restart Forge and reload its browser after regeneration.
Workshop preview now runs native draft validation, but is not an execution or
multiplayer test. Stage reviews pin native sources, the checker and its receipt.
Ordinary Forge saves and restores protect appended recipe IDs. To undo tuning,
restore values while retaining any recipe rows already used by saved machines.
