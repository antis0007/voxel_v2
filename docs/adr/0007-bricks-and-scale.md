# 0007 32³ chunks for storage, 8³ bricks on the GPU, scale reserved

Date: 2026-10-22. Status: accepted, implemented in Phase A (`docs/PLAN2.md`).

The 32³ `ChunkData` stays the unit of streaming, region files, saves and gameplay authority. The GPU cellular world allocates, flags and dispatches **8³ bricks**: 64 per chunk, 512 cells each, one workgroup tile. A 64³ brick was rejected because one falling grain would keep 262,144 cells awake, and because it does not align with 32³ chunks or 8×8×8-chunk region files.

Each chunk carries a `u64` dirty mask, one bit per brick. A brick whose bit is clear is not dispatched at all, and the debug overlay reports the active-brick count.

The cell is 8 bytes, all integers (ADR 0008): `material u8 | volume u8 | temperature u16 (0.1 K) | pressure i16 | charge i16`. `volume` is how much of the cell the material fills, 0..=8 (water already used 0..8); a brick is 4 KiB. (Corrected 2026-10-23: the code's scale is 0..=`VOLUME_FULL` = 64; see ADR 0015.) A chunk's entry holds the dirty `u64`, a 64-slot pointer table into a fixed-capacity pool (the GPU buffer's size) and the scale. A brick of only empty cells is not stored, and a write equal to the current value does not set a dirty bit, so idle regions cost nothing. The store is `engine_sim::BrickStore`; it depends on `engine_core` only, and a layout test keeps it so, which is why chunk conversion lives in `engine_voxel`.

Variable scale is level of detail first: a single-scale world whose distant regions are held at coarser octree depths. `Scale` is reserved, unused, in the brick header and in the brick coordinate type from the first version, so mixed authored scales can come later without a save migration.

Scale-boundary rules, the only place they are written down:
1. A depth-d cell covers 2^d finest cells per axis.
2. Neighbouring bricks differ by at most one depth (2:1 balance).
3. Mass is counted in finest-cell units. Volume fields widen with depth: u8 for depths 0–1, u16 for depths 2–4.
4. A depth-d brick ticks every 2^d fine ticks. This is the existing `0.08·m` interval rule.
5. Matter crosses a boundary only in a dedicated boundary pass. Fine faces aggregate into the coarse neighbour; coarse outflow is split by a positional hash with a remainder carry, so nothing is created or lost.
6. Heat is conserved as energy, not averaged as temperature. A cell's thermal energy is `capacity × volume × temperature`, with `capacity` taken from `SimMaterial`. Merging sums energy and derives the coarse temperature as `ΣE / Σ(capacity × volume)`. Splitting gives each child the parent's temperature. The integer-division remainder is carried the same way as mass in rule 5. Averaging temperatures would let a small lava cell beside a large volume of air lose heat that still exists. (Amended 2026-10-22.)

Proptests cover the round-trip, coordinate and transition rules. A scenario checks mass and energy totals across a boundary.
