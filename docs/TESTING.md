# Testing

One command gates everything: `just verify` (check, clippy, tests, LOC report).

## Unit tests

`just test` runs every `#[test]` in the workspace with nextest. `just test <substring>`
runs a subset. No test needs a GPU or a window; visual review is a separate,
manual capture process (`docs/design/VISUAL_DIRECTION_EXECUTION.md`).

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
`rand` crate.

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
