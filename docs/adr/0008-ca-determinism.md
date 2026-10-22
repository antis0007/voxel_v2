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

Not covered by this guarantee:
- rendering, which uses floats;
- rigid bodies, which use floats and stay CPU-authoritative (ADR 0011).

These may differ between GPUs and drivers. Scenario hashes cover CA and gameplay state only.
