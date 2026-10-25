# Recovery and inventory descriptions — 2026-10-24

Read all three supplied specifications and reconciled them with current source,
roadmaps and ADRs. Four agent roles covered recovery, planning, implementation
and independent review. No test, benchmark, hosted CI or game launch ran.

## Delivered

- [Recovery inventory](../recovery/2026-10-24-inventory.md): 13 external
  worktrees, seven verified incremental bundles and a complete-history bundle
  for the old independent repository. Preserved 6,867 source paths as 1,673
  unique blobs, approximately 40.2 MB with provenance. Verified 1,758 archive
  checksums. Original worktrees and unique work remain intact.
- [Master plan](../MASTER_PLAN.md): 140 separate systems, all 24 bible claims,
  dependencies and milestone assignments. VM work moves to M1–M2; the Primer
  starts in M3. Full research and SDD/measurement requirements remain explicit.
- [Canonical inputs](../specs/README.md): byte-identical specifications with
  hashes, precedence and the latest 8/14/50-word description limits. Current
  orientation now records source save 32 / generator 46 without changing them.
- Recovered production atmosphere validation/fallback from `5259f997`:
  non-finite radius/ozone and negative ozone are rejected; empty manually
  constructed preset collections cannot panic in lookup. No historical tests
  or duplicate GPU CA implementation were activated.
- [Inventory descriptions](../systems/tooltips.md): sparse validated RON,
  empty standard prose, shared gameplay stats/costs, optional Alt details and
  `just lint-descriptions`. Combat and excavation supply weapon/bore numbers.
  Independent review corrected armor discoverability, bore differentiation,
  maul behavior and inactive material values before the final compile.

## Evidence and ownership

`cargo check -q --workspace --bins --lib --offline --locked
--message-format=short` passed, with 30 warnings. The final description lint
passed with eight entries and two keyword definitions. Catalog allocation was
48 inline bytes plus 1,454 bytes of owned capacity, excluding allocator overhead.
Source diff whitespace review passed. Compilation/lint does not establish UI
appearance, gameplay feel, save compatibility or hardware performance.

Logs, source snapshots and the task-file hash manifest are under
`downloads/recovery/2026-10-24/`. Task code/data delta against pre-edit snapshots:
**+378 / -91 lines**, excluding documentation and unrelated shared-tree work.
The initial main snapshot preserved 189 changed files; description-specific
snapshots and the later expedition snapshot disambiguate parallel edits.

Concurrent work advanced HEAD from `af9842ef` to `4837411a`; this task neither
created nor reverted that commit. Do not attribute the whole Git diff to this
session. Archive bundles record their actual baseline prerequisites.

| Evidence class | Status |
|---|---|
| Source/archive integrity | Recorded and verified; originals retained |
| Workspace compilation/content lint | Passed |
| H0/H1/H3 timings | Not measured; hardware addendum unavailable |
| Runtime/visual/save acceptance | Not performed |
| C-* claim suite | No claim marked passed |

## Remaining work and next exit

Chest/save-ID and player-edit-latency candidates remain preserved for deliberate
integration; already-present flame/orbit/multidraw work is identified. Cleanup
here means indexed, deduplicated recovery and corrected documentation, not
deletion of original directories or build evidence.

Spell/machine/relic descriptions still require an effect/data audit. Next
description exit: inspect the inventory at supported font scales and migrate
one spell family with its actual effect costs. Next engine exit: reviewed
exact-checkpoint/action/replay design for the isolated CPU backend, before
expanding authority. The full multi-month game specification remains unfinished.
