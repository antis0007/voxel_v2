# 0009 One CaBackend trait; gameplay reads the cellular world two ticks late

Date: 2026-10-22. Status: accepted, implemented in Phase A (`docs/PLAN2.md`).

The cellular world runs behind a `CaBackend` trait with two implementations that must produce identical bytes (ADR 0008):
- `Cpu`, the reference, used headless and in tests;
- `Gpu`, used in the window.

Gameplay reads a CPU mirror fixed exactly **2 ticks** behind the cellular state; this covers player collision, factory, adventure and meshing. The GPU backend blocks if a readback misses its tick, and the CPU backend applies the same lag, so headless and windowed runs see identical state on every tick. Gameplay writes are edit commands, applied at the start of the next CA tick in submission order.

The rejected alternative, CPU-authoritative with the GPU as an accelerator, uploads every dirty brick each tick and caps scale at the CPU.

When this lands, the lag becomes a CLAUDE.md invariant.
