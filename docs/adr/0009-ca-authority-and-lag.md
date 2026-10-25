# 0009 One CaBackend trait; gameplay reads the cellular world two ticks late

Date: 2026-10-22. Status: accepted; **partially implemented in an isolated CPU development path** (2026-10-24, ADR 0018). The app now has a small `CaBackend` boundary and an exact two-CA-tick mirror invoked by `--headless` with `--ca-cpu-region`. Ordinary window gameplay remains on the legacy simulation. GPU authority, general writer migration and exact checkpoint/resume are still pending.

The cellular world runs behind a `CaBackend` trait with two implementations that must produce identical bytes (ADR 0008):
- `Cpu`, the reference, used headless and in tests;
- `Gpu`, used in the window.

Gameplay reads a CPU mirror fixed exactly **2 ticks** behind the cellular state; this covers player collision, factory, adventure and meshing. The GPU backend blocks if a readback misses its tick, and the CPU backend applies the same lag, so headless and windowed runs see identical state on every tick. Gameplay writes are edit commands, applied at the start of the next CA tick in submission order.

The rejected alternative, CPU-authoritative with the GPU as an accelerator, uploads every dirty brick each tick and caps scale at the CPU.

The lag is an invariant of the opt-in CPU region only. ADR 0018 pins startup,
command quotas, sealed boundaries and publication; it does not declare the whole
game migrated or asynchronous GPU deadlines validated.
