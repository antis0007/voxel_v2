# 0023: W-GEO/HYDRO/BIO becomes the worldgen input of record

Date: 2026-10-24. Status: human supplied the spec after [0022](0022-master-plan-review.md)
froze inputs; recorded as the seventh input of record. Milestone placement
below is proposed for the session-2 review.

## Context

[W_GEO_HYDRO_BIO.md](../specs/W_GEO_HYDRO_BIO.md) specifies terrain, climate,
hydrology, soils, biomes and set-piece generation as a staged pipeline
(P0–P12 on grids G0–G3). Until now the plan had no worldgen document: the
master's §8 listed WORLDGEN.md as absent. On 2026-10-24 rivers were judged
"horrible": straight grid channels from `crates/worldgen/src/drainage.rs`, made
to meander at generator 50 as an interim fix.

## Decision

1. The spec is an input of record and stands in for the missing WORLDGEN.md.
   It does not reopen general intake (0022 §1 still applies to everything else).
2. Its stages become inventory rows W11–W19 in the master. W01 keeps the
   realization (P12) and grid/version policy; P11 stays with W04/W05.
   W12 (climate) and W13 (erosion/drainage/hydrology) are Tier A: every other
   stage consumes them.
3. The M1 slice is unchanged; worldgen is not on its path.
4. W13 is placed in M3 because the M3 waterwheel needs a real discharge Q.
   The generator-50 drainage meander is interim and is not extended further.
5. The spec's §13 scenarios are acceptance targets of their rows, not bible
   claims. `hydro_basin` is the first (M3).

## Conflicts to resolve in the W01/W12/W13 SDDs

- **Planet topology.** Decided: bounded cube-sphere planets, see
  [0024](0024-cube-sphere-planets.md).
- **Fixed-point.** §12 requires fixed-point stage outputs (cm, L/s); current
  worldgen is `f32` throughout. Planning outputs adopt fixed-point; per-brick
  realization may stay float if it is deterministic per platform.
- **Budget.** §0.7 asks ≤ 4 ms per 32³ chunk on H1. The committed terrain bench
  baseline is about 10 ms per chunk on the desktop. Needs measurement on H1.
- **Versions.** The pipeline is a new generator version; every older version
  must keep generating identically, so the current landscape code stays.

## Reversal

The human can demote the spec to reference or park it; the W11–W19 rows then
move to `docs/parked/` with it.
