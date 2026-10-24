# 0009 One CaBackend trait; gameplay reads the cellular world two ticks late

Date: 2026-10-22. Status: accepted; **not yet implemented** (corrected 2026-10-23). The code has no `CaBackend` trait and no 2-tick mirror, only the `step_with` closure in `crates/sim/src/kernel.rs`, and the game still runs `crates/app/src/simulation/`. Both land in M1 session 2 (`docs/ENGINE_CORE.md`).

The cellular world runs behind a `CaBackend` trait with two implementations that must produce identical bytes (ADR 0008):
- `Cpu`, the reference, used headless and in tests;
- `Gpu`, used in the window.

Gameplay reads a CPU mirror fixed exactly **2 ticks** behind the cellular state; this covers player collision, factory, adventure and meshing. The GPU backend blocks if a readback misses its tick, and the CPU backend applies the same lag, so headless and windowed runs see identical state on every tick. Gameplay writes are edit commands, applied at the start of the next CA tick in submission order.

The rejected alternative, CPU-authoritative with the GPU as an accelerator, uploads every dirty brick each tick and caps scale at the CPU.

When this lands, the lag becomes a CLAUDE.md invariant.
