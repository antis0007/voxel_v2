# 0039: Axiom expansion briefs as backlog, not inputs of record

Status: accepted, 2026-10-29. The owner delegated the choice to the agent
("analyze and take the best decision"); the owner may reverse any item.

## Context

Three briefs were staged in `docs/specs/`:
`AXIOM_Science_Spaceflight_And_Planetary_Expansion.txt`,
`AXIOM_Planetary_Simulation_And_Coop_Architecture.txt` and
`AXIOM_Living_World_Expansion_Prompt.txt` (about 44k words). They were written
without repository access. The audit is
`docs/superpowers/specs/2026-10-29-axiom-expansion-briefs-gap-analysis.md`.

The audit found:
- the briefs reopen the full Axiom scope that 0022 §1 froze and 0036 §2 narrowed;
- they ask for client-local float physics, against the lockstep plan and ADR 0011;
- they suggest transport crates the owner already declined;
- they introduce about 70 proper nouns, peoples and creatures;
- each area adds save sections or generator gates while gates collide across branches.

## Options

1. Adopt the briefs as inputs of record and replan around their milestones.
2. Keep them parked entirely.
3. Keep them as a backlog that feeds MASTER_PLAN rows, and take only slices
   that fit current decisions.

## Decisions

1. **Scope: option 3.** The briefs are backlog, not inputs of record. 0022 and
   0036 stand. MASTER_PLAN M1 (`dam_break`) keeps priority. Brief items enter
   the plan only as rows under existing tracks (W09 space, W10 skyhook
   substrate, P10 vehicles, multiplayer) at their current milestones.
   Agents handed a brief verbatim follow this record where they differ.
2. **Authority.** The lockstep plan stands: every peer runs `engine_sim` for
   integer matter. Rigid bodies, players, projectiles and vessels stay
   host-authoritative (ADR 0011) until fixed-point bodies (0022 §5, B03) land
   and a new decision moves them.
3. **Names.** None of the new proper nouns, peoples, factions or encounter
   names is approved. Work uses descriptive placeholders in code identifiers
   (`airless_moon`, `ocean_world`) and no new names in player text or RON
   rows. Existing names (Meridian, Talon, Needle, Ark) are unchanged. Encounter
   designs that follow Fablehaven or Percy Jackson are not used.
4. **Transport.** No iroh, Renet or Quinn. Own keys and UDP relay first, then
   Steam, as already decided.
5. **First slices.** Three slices fit these rules and need no new names:
   - operation ids with a bounded dedup table for the last-shared-item transfer
     (`crates/netcode`), which later shipment and docking work depends on;
   - the Talon consignment on the ground: radio report, receiver repair, the
     same consignment landing with a usable controller;
   - sheep shearing with fleece regrowth and one wool trade.
   They are taken in that order, each by one writer, after M1 work in flight.
6. **Prerequisites.** Commit `crates/app/src/world/orbital/dynamics.rs`, which
   tracked code depends on, and remove the `Instant` use in
   `world/orbital/passes.rs:47`.

   Status 2026-10-29: `passes::forecast` only feeds `orbital::view()` display
   data, so its wall-clock cap is allowed and stays. `dynamics.rs` is a peer's
   untracked file. Slice 1 is `crates/netcode/src/operation.rs` (e83d7188,
   0d6e529e). The live command path does not use it: `multiplayer::actions::
   admit_sequence` already applies each ordered request once, and the reply is
   the replicated `command_ack`. Receipts are for unordered or post-restart
   retries (shipments, docking), wired with those features.

## Consequences

- No save bump or generator gate is taken for brief work without a row in
  MASTER_PLAN and a check of the cross-branch gate registry.
- Vessels, reference frames, cruise drive and cohort simulation each need
  their own decision before code.
- The CLAUDE.md save and generator constants are stale (source: save 44,
  generator 79) and are corrected separately.

## Reversal

Revisit 1 when M1 is accepted or the owner names a brief milestone as the next
target. Revisit 2 when fixed-point bodies pass the determinism tests. Revisit 3
item by item as the owner approves names.

## Addendum, 2026-10-29: unified engine prompt

`docs/specs/AXIOM_Unified_Engine_Implementation_Prompt.txt` consolidates the
networking/performance and worldgen/physics briefs into stages 0 to 7. The
rules above apply to it. Stage by stage against the tree at 67c2fcdf:

- **Stage 0 (audit, freeze repro).** Prior audits: `docs/perf/audit-08e11ae3.md`
  and the 2026-10-27 ten-rules pass. The known default-budget freeze is the
  streaming source-cache stall at view 28 (published sources never evicted,
  admission fails, the last ring regenerates forever). Fix 3f6d4eec sits on
  branch `idle-stream`; HEAD still has the bug; the same change is applied,
  uncommitted, in the shared tree with other residency edits. Fits.
- **Stage 1 (lifetimes, ordinary edits).** `render/streaming/gpu_budget.rs`
  (HEAD) reads the adapter budget. Uncommitted shared-tree work adds a GPU
  growth reserve, pending-GPU publication tests and edit proxies. Fits; one
  writer, landing that work before new streaming changes.
- **Stages 2 and 3 (authority, prediction, joining).** `crates/netcode` has
  edit prediction with sequence acks, acked delta baselines and budgeted cell
  deltas. Section 7.1 local stepping of bodies is allowed only as a labelled
  state-sync mode until fixed-point bodies land (decision 2). The transport
  references in 7.5 (Quinn, Iroh) are reading only (decision 4). The
  changing-cover PvP rule in 8.4 needs its own row and owner approval.
- **Stage 4 (critical path).** Fits existing perf work and `just frame-bench`.
- **Stage 5 (coupled materials).** The Salva and staggered-grid liquid pilot
  is not taken: liquids are integer `engine_sim` (ADR 0018, M1), Salva is a
  float dependency without an ADR. Coupling rules in 10.1 and 10.4 are
  backlog for the M1.5 rigid/sim interface.
- **Stage 6 (world).** Drainage and erosion fit gate 71 (`worldgen/src/geo`,
  dormant). The four named locations in 11.2 are new proper nouns and are not
  approved (decision 3); a first district uses a descriptive placeholder.
- **Stage 7.** The acceptance matrix in 14.2 is backlog for MASTER_PLAN rows.
  The Minecraft comparison runs are not scheduled.

## Addendum, 2026-10-29: update pack R1

The owner named `docs/UPDATE_PACK/` R1 (DESIGN, TASKS, REVIEW, ACCEPTANCE) as
the key design reference. Reconciliation with this record and the tree:
`docs/superpowers/specs/2026-10-29-update-pack-r1-reconciliation.md`. Items 2
(body authority) and 3 (names) stay in force until the owner answers the
questions listed there.

## Addendum, 2026-10-30: Manyfold, Webway and Crimson names approved

The owner approved these names for code, player text and RON rows. This revises
decision 3 for the items below only; every other proper noun in the briefs stays
unapproved.

- **Manyfold**: the dimension, the Manyfold Tube world type, and its seven regions
  (Foldfields, Bellworks, Cistern of Borrowed Rain, House of Contrary Stairs,
  Unfinished Borough, Guest Wing, Room with an Outside), plus Stillcourt.
- **Webway**: the portal network (code keeps `portal_network` as the module name).
  The term originates in Warhammer 40,000; the owner approved it knowingly. Keep
  designs, art and text original to this game.
- **Crimson Nexus** and **Hall of the Crimson King**: the multiversal hub. Its role
  stays separate from the Manyfold's discovered routes, as the Manyfold brief states.

The briefs remain backlog (decision 1). Approval of a name does not schedule the
work: M1 keeps priority, and any save bump or generator gate still needs a row in
MASTER_PLAN and a check of the cross-branch gate registry.
