# 0025: Space track from the Seed of Andromeda review

Date: 2026-10-24. Status: decided by the human (answers to the open questions
of the [space track design](../plans/2026-10-24-soa-space-port-design.md)).
Amends one clause of [0024](0024-cube-sphere-planets.md).
Updated 2026-10-26: the human authorized local integration of reviewed phases
0, 1 and 5a, in that order, with a test run after each merge.

## Context

The human asked for the voxel-planet, space-travel and physics ideas of Seed of
Andromeda (MIT, `RegrowthStudios/SoACode-Public`) to be reimplemented in Rust.
The review and port design are in the space track design. The master plan
froze its inputs and schedules planets and vessels at M3 to M7; 0024 chose
cube-sphere planets but left edge geometry ambiguous between a folded cube
and an unfolded column grid.

## Options

Edge geometry:

1. **Folded cube (F):** faces drawn flat in their own planes with a 90 degree
   crease. A wedge gap above each edge needs special handling in collision,
   fluids, meshing, light and pathing. Rejected.
2. **Radial columns (R):** unfolded column topology, vertices placed by the
   planet projection, gravity along the column. Grid skew near the 8 corners,
   placed under ocean or ice. **Chosen.**

Schedule:

1. Run phases 0, 1 and 5 now beside the M1 slice. Rejected: three new threads
   in a shared tree.
2. Keep every phase at its master-plan milestone. Rejected: phase 0 fixes an
   existing flat-world precision defect.
3. **Start phase 0 (camera-relative rendering) now; hold phases 1 and 5.**
   Chosen.

## Decision

- Planets use model R. 0024's "within a face, play, meshing and simulation
  stay flat and axis-aligned" holds in the simulation grid, not in rendered
  space. Corner cells treat the missing Margolus cell as solid.
- The initial phase-1/5 hold is superseded for phases 1 and 5a by the
  2026-10-26 integration authorization. Phase 5b (system map), the integer
  simulation sun, and the later planet phases remain pending.
  The design document is supporting context, not an input of record.
- Home planet face edge 32 km, provisional until the system-scale spike.
- System scale (year length, transfer times, warp) is set by a small playable
  spike before numbers are committed; KSP-like ratios are the reference.
- Proper nouns for the system, bodies and vessel classes stay with the human.

## Consequences

- Phase 0's per-draw transform carries an offset and a rotation from the
  start, so model R's bent near chunks need no rework.
- The C02 SDD specifies radial-column neighbours and corner valence 3; W01
  places corners under ocean or ice.

## Reversal

If phase 2 shows corner skew is visible in normal play despite placement, or
the bent-chunk transform breaks the pixel-grid invariants in
`.claude/skills/render/SKILL.md`, reopen the edge choice with captures.
