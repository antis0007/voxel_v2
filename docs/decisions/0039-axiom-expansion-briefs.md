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
