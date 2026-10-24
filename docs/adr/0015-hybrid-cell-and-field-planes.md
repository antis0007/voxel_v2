# 0015 Keep the 8-byte cell; add the spec's extra state as optional per-brick planes

Date: 2026-10-23. Status: accepted by the user 2026-10-23 (the choice of approach); the plane list below is a proposal.

The v2 spec (III.2) asks for a cell of two packed u32 planes: `material:12 | state:2 | volume:4 | flags:6 | temp_lo:8` and `vel_x, vel_y, vel_z: i8 | aux: u8`. The built cell (ADR 0007, `crates/sim/src/cell.rs`) is `material u8 | volume u8 | temperature u16 (0.1 K) | pressure i16 | charge i16`. The CPU reference and the GPU kernel already agree on it byte for byte (Phase A5). Repacking would rewrite the A1–A5 kernels and their tests for no measured gain.

Decision:
- The 8-byte cell stays the base layer of every allocated brick, unchanged.
- The spec's extra state goes into **optional planes**, allocated per brick only when a material in that brick needs them:
  - `motion`: `vel_x, vel_y, vel_z: i8` (1/16 cell per tick) and `aux: u8` (sub-cell accumulator, fuel, growth or item progress, by material class);
  - `u8` field planes: oxygen, mana, aspect, moisture, scent, sky light, block light;
  - a `u16` item plane.
  A brick header bitset records which planes exist. A plane that is absent reads as its default value (zero, or full oxygen for open air).
- Temperature stays u16 in 0.1 K (the spec says K×4), which covers 0–6553.5 K.
- `material` stays u8. It widens to u16 only when the material table passes 255 rows. That needs a save bump and is decided by its own ADR then.
- `VOLUME_FULL = 64` in code is authoritative. ADR 0007's "0..=8" text is a documentation error; ADR 0007 now carries a dated correction pointing here.

Consequences: the CPU-equals-GPU tests cover each plane as it is added. A brick with no optional planes costs exactly what it does today (4 KiB). The spec's 500k-awake target is measured with planes present (powder needs `motion`).

Rejected: adopting the packed layout now, which costs about 2–3 sessions of kernel rewrite before any gameplay progress; and dropping velocity entirely, which loses powder momentum and spread bias.
