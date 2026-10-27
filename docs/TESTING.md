# Testing

One command gates everything: `just verify` (check, clippy, tests, the perf gate, clone completeness, LOC report).

The task gate (`scripts/workflow.py`, run from the Stop hook) derives its required checks from the
diff via `.claude/verify/policy.json`. It runs the perf gate (`scripts/bench.py check`) on Rust and
policy changes. It blocks at most once per user turn (`stop_continuations`); after that it reports
the checks still UNVERIFIED and lets the session end, so run `just verify` before committing.
Manual checks are self-attested by evidence file and must be reviewed by a person or independent agent.

## Unit tests

`just test` runs every `#[test]` in the workspace with nextest. `just test <substring>`
runs a subset. No test in `just test` needs a GPU or a window; visual review is a
separate, manual capture process (`docs/design/VISUAL_DIRECTION_EXECUTION.md`).

## GPU tests

Tests that need a GPU are named `gpu_*`, carry `#[ignore = "..."]`, and get their
adapter from `engine_gpu::test_adapter()`. `just test-gpu [substring]` runs them one
at a time and fails (never skips) when no adapter exists. `WGPU_BACKEND=dx12`
picks another backend (default Vulkan); `VOXEL_TEST_ADAPTER_NAME=4090` requires a
specific adapter. A layout test rejects any other code that requests an adapter.

## Benches and the perf gate

Criterion benches live in `crates/<crate>/benches/`, with their baselines in
`benches/baselines.json` beside them (median nanoseconds). `just bench [name]` runs
once and prints the delta; `just bench-check` (part of `just verify`) takes the
median of 3 runs and fails above +8% or on a bench with no baseline; `just bench-save`
records new baselines, which are committed with the reason. A new bench is a
`[[bench]]` entry with `harness = false` in its crate's `Cargo.toml` plus a
`just bench-save`. Example: `crates/worldgen/benches/terrain.rs`.

## Frame-time scenes

`scripts/frame_bench.py` drives the windowed game through fixed `--benchmark*` command
lines (seed 74, generator 53, 1600x900, clear weather, 6 s warmup, 15 s timed): Hills static,
orbit and clouds-off, Superflat, and quarter-metre Hills. Benchmark mode ignores saved
preferences and holds the auto-quality governor. `just frame-bench [scene]` prints one pass
against `crates/app/benches/frame_baselines.json`; `just frame-bench-check` runs 3 interleaved
passes and fails when the best pass's p95 or p99 is more than 5 % over the baseline or the
per-frame median draw calls (terrain plus shadow, from `frames.csv`) are more than 10 % over;
`just frame-bench-save` records the best pass with the adapter, commit, tree state and exe hash.
Each pass first waits up to 15 minutes for `rustc`/`cargo`/linker processes and other game
instances to exit, since a peer's build or capture halves the frame rate; a pass that still
overlapped one is marked contended. When every pass is contended, a frame-time excess is
reported UNVERIFIED (exit 3), not failed; draw-call excess always fails. On this shared
machine frame times varied up to 3x between passes while per-frame median draws repeated
within 3 % on static Hills (11 % on orbit, 21 % on superflat); see `docs/perf/audit-08e11ae3.md`. It needs a GPU, so it is not part of `just verify`; run it
for render changes. A baseline from another adapter is refused, not compared.
Script tests: `python -I scripts/test_frame_bench.py`.

## Clone completeness

A clean clone holds only committed files. `just deps [rev]` (part of `just verify`, default
`HEAD`) reads the commit's tree and fails when a `mod`, `include_bytes!`/`include_str!`,
Cargo `path =` dependency or `crates/*` workspace member names a file outside it.
`just deps-inventory` groups the working tree's untracked files into commit sets: each set
lands in one commit with its tracked referrers, or that commit does not build. Sets follow
`mod`, `include_*!` and `crate::` paths only; build each set before committing it.
Script tests: `python -I scripts/test_tracked_deps.py`.

## Seam route

The `seam_route` gate check (policy route `world_meshing`: mesh, horizon, streaming, voxel, worldgen, core) has two halves, both owned by whoever changes those paths.

1. Automated: `just seam-route` runs `crates/worldgen/src/seam_route.rs`. It walks a fixed route of chunk coordinates (deep negative, across the origin, about 96 000 voxels out), at two voxel scales and at generator 33 and current. It checks that each padded halo equals direct `sample_block`, that padded interiors equal `generate`, and that neighbouring padded chunks agree on their overlap.
2. Captures: the gate demands at least two images, before and after, from the same pose. Run `tools/Review-HorizonContinuity.ps1 -Executable <exe> -Case route -Label <unique> -GeneratorVersion <n>` once per side (default version 25) and attest with `python -I scripts/workflow.py attest seam_route --evidence <capture dir> --note ...`.

## Task gate: ownership and its limits

A session's required checks come from the files that session wrote and that still differ from the current HEAD, so committed work leaves every task. The PreToolUse and PostToolUse hooks snapshot the dirty set around each writing tool call and record what changed in between; `claim PATH... --note TEXT` adds files written another way, and `adopt-ownership` switches a legacy session over once. Owned files stay in the task for its life, so a peer's later edit to one of them still counts. Gaps: a peer write that lands during one of your tool calls is attributed to you (conservative), and a background process's writes after its tool call returns are not attributed, so `claim` them.

The post handler also runs on `PostToolUseFailure`, so a shell command that writes files and then exits non-zero is still attributed. A tool call whose post hook never ran (hook timeout, crash, or a hook config older than the session) leaves its pre snapshot behind. At Stop, a snapshot older than 15 minutes is diffed against the tree, and the files changed or created inside that call's possible window (11 minutes) are reported, not attributed: any window in a shared checkout also holds peers' writes. Claim the ones you wrote. The reverse case, a peer's write that landed inside one of your calls, is removed with `disown PATH... --note TEXT`; it is logged, announced at the next Stop, and undone automatically if your session writes or claims the file again. The Stop hook also announces once which files `adopt-ownership` dropped from the task.

Evidence binding: automatic checks (build, clippy, test runs) are bound to the fingerprint of the whole task. A manual check is bound to the files whose routes require it plus every non-doc file in the task. A capture or review stays verified while you edit inert files (the policy's `inert_paths`: docs, `.claude/`, the gate scripts), but any code, shader or content edit makes it stale, even one no capture route names. A test fails if the game ever embeds an inert-pattern file. `status` prints the binding when it differs from the task fingerprint. `verify CHECK...` runs only the named required automatic checks, so an unrelated failure in the full test run does not stop an independent check from being recorded; the failing ones stay required. `owners PATH...` lists which sessions own a path and when each was last active, to find who else is editing it; a path changed only by legacy sessions shows no owner. `python -I scripts/test_workflow.py` runs the gate's own tests in a throwaway git repo, two simulated sessions each. The `policy` route requires it (`tests_workflow`) for any change to the gate scripts, the policy or the hooks config.

## Scenarios: how an agent plays the game

`just run assets/scenarios/<name>.ron` builds a world from a seed, applies the
scenario's steps without a window or GPU, and prints one JSON line:

```
{"scenario":"smoke","ticks":180,"hash":"976b0d34dce94255","failed":0,"failures":[]}
```

Exit status is 0 when every `Expect` passed, 1 when one failed, 2 when the file
did not parse. The runner is `crates/app/src/headless.rs`; the format is RON:

```
Scenario(
    name: "sand_falls", world: Superflat, scale: OneMeter, seed: 1,
    steps: [
        Advance(ticks: 30),                                   // 1/120 s each
        Place(offset: (2, 4, 0), block: "Sand"),              // offset from the player's feet, in voxels
        Advance(ticks: 480),
        Expect(Block(offset: (2, 0, 0), is: "Sand")),
    ],
)
```

Steps: `Advance`, `Hold` (inputs for N ticks), `Place` / `PlaceAt` (block name
as in `BlockId`; `"Water"` fills the cell's volume), `Teleport`, `Expect`.
Checks: `Block`, `BlockAt`, `Water { offset, min }` (volume 0–8), `PlayerY`,
`Finite`, `Hash`. `generator_version` pins the world generator.

## Determinism

Every simulation pass takes an `engine_core::Budget`. The windowed game passes
wall-clock budgets so frames keep pacing; scenarios pass `Budget::UNBOUNDED`
and wait for chunk generation after each tick (`WorldSimulation::settle`), so
the same file always does the same work and ends with the same `hash` (resident
chunk blocks, player pose, factory nodes, inventory). Terrain and loot use
positional hashes seeded from the world seed; there is no global RNG and no
`rand` in game code (proptest, a dev-dependency, pulls it into test builds only).

Known gap: nothing is meshed headlessly, so water that only the mesher would
have seeded in generated terrain stays asleep until an edit wakes it.

## Snapshots

The headless tests run every file in `assets/scenarios/` and compare its JSON
summary with `crates/app/src/snapshots/*.snap` through `insta`. A changed hash
means gameplay changed: fix the regression or, if the change is intended, run
`INSTA_UPDATE=always just test headless` and commit the new snapshot with the
reason.

## Content tables

`assets/{materials,spells,recipes,machines,bestiary}.ron` each have a
`*_ron_round_trips` test: serialising the loaded table must reproduce the file.
Edit values in the file; if you change a struct, run the ignored
`dump_*_ron` test to rewrite the file in the new shape.

## Adding a scenario

1. Write `assets/scenarios/<name>.ron`.
2. `just run assets/scenarios/<name>.ron` until the checks pass.
3. `INSTA_UPDATE=always just test headless` to record the snapshot.
4. Commit both files.
