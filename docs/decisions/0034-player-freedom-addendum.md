# 0034: Player-freedom addendum (07) as an acceptance addendum

Date: 2026-10-26. Status: reversed the same day; the human demoted the brief
to reference (see [Owner ruling](#owner-ruling-2026-10-26)). The human supplied
`07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md` after [0022](0022-master-plan-review.md)
froze inputs; its own §12 asks to be an acceptance addendum first. Routing below
follows the [0023](0023-worldgen-spec-input.md) precedent and waits for the
human's sign-off. Numbering continues above ADR 0033 because `docs/adr/` and
`docs/decisions/` share one sequence.

## Context

The brief is preserved byte-for-byte at
[docs/NEW/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md](../NEW/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md)
(SHA-256 `a58a43ad11a8a3402c557b6bf3df8311a20dec72478cbe534c3ca406cce1e958`,
copied from the Downloads attachment). It is the seventh document of the U1
set (`docs/NEW/01`–`06`, untracked, dropped by the owner on 2026-10-26 at 14:01).
It adds twelve player promises C-PF01..12, seventeen named scenarios, and
revisions to C-MC5, C-N3, C-N4, C-Z2, C-S1, C-S4 and C-F3.

The repository audit behind this record is in
[the design note](../superpowers/specs/2026-10-26-player-freedom-economy-design.md).
Source facts at this pass: HEAD `56bde2b7` has save version 34 and generator
version 53 (`world/storage.rs`, `voxel/src/config.rs`, as `CLAUDE.md` says);
peers' uncommitted edits in the shared tree carry them to 39 and 65, so any
slice that adds a record picks its version against then-current source.

## Decision (proposed)

1. The brief is an acceptance addendum, not a new system list. It does not
   reopen intake (0022 §1 stands). Its promises become claim rows C-PF01..12 in
   the master plan §4 under existing owners; no new inventory row is created.
2. Its scenarios are registered in `assets/scenarios/claims/README.md` as
   proposed rows. One is automated now: `afk_vs_adventure_factory`
   (C-PF12) runs as the Rust test module `world/factory/authority_tests.rs`
   because the headless RON runner has no machine step or ledger check yet.
   The rest wait for their milestone like every other claim.
3. The revisions to existing claims are recorded against those rows as
   "addendum 07" notes; the original obligations are not deleted.
4. The M4 slice definition gains the brief's §12 M4 list (portable storage,
   cooking experiment, three material routes, reliable first machine, salvage
   cave, one-hour mystery, safe-home mechanism) as acceptance targets, not as
   authorization to build them before M1–M3 close.

## Conflicts to resolve with the human

- **Hunger.** C-PF08 asks for no punitive sprint-linked hunger by default. The
  game has one: `Adventure.hunger` drains at 0.04/s, 2.2× while sprinting, and
  passive regeneration stops below 75 (`world/adventure.rs`). Changing the
  default is a product decision (B.6). Options: keep as a world mode, make the
  default "food heals, no drain", or prototype both as the brief asks.
- **Ordinary materials from machines.** Drills delete Stone, Dirt, Sand,
  Gravel and Limestone without yield (`factory/extraction.rs`), and no recipe
  produces sand or gravel. Sand and gravel are `Item::Block`, not resources;
  a crusher route needs either block-item outputs through `Node::stock` or
  new resource ids, which bumps the save version.
- **Blast recovery.** The creative explosive wand recovers every block; spell
  craters and dig spells keep only ore, caches and logs (`world/expedition.rs`).
  The brief wants declared recovery rules for both.
- **Which ruin is the first functioning site.** Candidates: the Sunken
  waterworks (site kind 2, has a pool, no mechanism), the Abandoned foundry
  (three ordered valves, a heal cache, opens the reliquary) or an Undercrown
  sluice (writes voxels on restore, no cost). The brief asks for a recoverable
  mechanism plus a grounded clue; none of the three has a clue graph.
- **Respawn persistence.** Death keeps items and max health (C-PF09 holds)
  but `respawn_point` is not saved, so after a reload death sends the player
  to the world spawn. Small fix; owner P13.

## Reversal

The human demotes the brief to reference: the C-PF rows and the proposed
scenario rows move to `docs/parked/` with it. The `authority_tests` module
stays as an ordinary factory determinism test.

## Owner ruling, 2026-10-26

Reversal applied. The brief is at
[docs/parked/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md](../parked/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md)
(same SHA-256); the C-PF and scenario rows are in
[docs/parked/2026-10-26-player-freedom-addendum.md](../parked/2026-10-26-player-freedom-addendum.md).

Two of the conflicts above were product questions in their own right and
were answered independently of the brief:

- **Hunger.** Default is no drain; food heals. The current sprint-linked
  drain stays available as an opt-in world mode. Not yet implemented.
- **Ordinary materials from machines.** Yes. Drills and a crusher route yield
  sand and gravel as `Item::Block` through `Node::stock`; no new resource ids,
  so no save bump for this alone. Not yet implemented.

Blast recovery, the first functioning ruin and respawn persistence stay open
and unowned by any claim row.

Sequencing ruled at the same time
([direction review](../superpowers/specs/2026-10-26-direction-and-technology-review.md)
§6): the S06 machine-network spike runs first after M1, ahead of the
far-field, Hi-Z and variable-scale step 2 work.
