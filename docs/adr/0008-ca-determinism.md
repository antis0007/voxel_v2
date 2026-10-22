# 0008 Integer-only cellular state, pinned hash and phase order

Date: 2026-10-22. Status: accepted, implemented in Phase A (`docs/PLAN2.md`).

Every cellular field is an integer:
- material u8;
- volume u8/u16;
- temperature u16 in 0.1 K;
- pressure i16;
- charge i16.

Every random choice is a positional hash of (world seed, tick, cell); there is no RNG state and no floating-point atomic. A GPU kernel and its CPU reference therefore produce identical bytes on any adapter. Equivalence tests compare bytes, not tolerances, and the CPU reference is what agents debug against.

The hash function and the Margolus phase order are defined in one `const` block, the only definition. A test runs a canonical CA scenario and fails if its hash changes, so editing either one shows up as a test failure.

The tick is two Margolus phases, anchors offset by (0,0,0) then (1,1,1). A 2×2×2 block rule reads and writes only its own eight cells, so the blocks of a phase are disjoint and can run in any order or in parallel; this is what makes the GPU port race-free. The hash is `engine_core::hash::hash_u32` folded over the world seed, the phase counter `2·tick + phase`, the block anchor and a per-decision salt. A decision is rolled only when it is needed (water always moves, so it never rolls), and that is a CPU optimisation that cannot change a result. The rules are in `crates/sim/src/rules.rs`, the pinned definitions in `crates/sim/src/pinned.rs`, and `scenario_tests::the_canonical_scenario_hash_is_pinned` fails if either changes.

A brick sleeps only after a whole tick in which nothing in it changed and no move was suppressed by a random gate, because a state that is static in one phase may move in the other. A block that changes wakes every brick it spans.

Heat is exchanged in energy, not averaged as temperature. A cell stores a `u16` temperature, so most exchanges move a fraction of a step; each cell takes the whole steps plus one more with exactly the remainder's probability, decided by the positional hash. Energy is conserved in expectation and each exchange errs by less than one step of each cell, with zero mean; it is not conserved exactly, and tests bound the drift. Heat does not run every tick: it conducts every `HEAT_TICK_INTERVAL` ticks and each step carries the skipped ticks' energy, so the rate in simulated time is unchanged. A tick without heat keeps whatever was awake awake, because it says nothing about whether heat is still flowing. The arithmetic is `u32` plus two 64-bit-intermediate primitives, `mul_div_rem` and `mulhi`, which a GPU kernel builds from 32-bit halves because WGSL has no 64-bit integers.

Not covered by this guarantee:
- rendering, which uses floats;
- rigid bodies, which use floats and stay CPU-authoritative (ADR 0011).

These may differ between GPUs and drivers. Scenario hashes cover CA and gameplay state only.
