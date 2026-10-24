# 0016 The low-end profile is an Intel Iris Xe laptop

Date: 2026-10-23. Status: accepted by the user 2026-10-23 (the target device); the numbers below are a proposal.

Every measurement so far was taken on an RTX 4090 with a hybrid desktop CPU pinned to its performance cores (ADR 0006). The v2 spec makes "low-end" the bench default and gates 60 tps and 60 fps on it, but no low-end profile existed.

Decision: `lowend` means an Intel Iris Xe laptop:
- GPU: 96 EU (Tiger Lake / Alder Lake class), shared memory;
- CPU: about 4 performance cores and 8 threads;
- memory: 16 GB LPDDR4x, with 1.5–2 GB usable as GPU memory;
- screen: 1920×1080 output, rendered at 1280×720.

Profile caps (first proposal, tuned by measurement):
- view distance 12 chunks;
- at most 20k allocated bricks, about 120 MB including optional planes (ADR 0015);
- CA target of 500k awake cells at 60 tps;
- 60 fps p95 frame time;
- simulation budget at most 6 ms per tick on 4 threads.

Until a real Iris Xe device is benched, the dev machine emulates `lowend`:
- CPU benches pinned to 4 performance cores (the existing `bench.py` pinning);
- 1280×720 at 12 chunks;
- GPU timings scaled by a measured 4090-to-Iris-Xe ratio from one calibration kernel (the A5 resident CA step). This is recorded once in the bench table, and the gates use it until hardware numbers replace it.

The profile is data: a budgets table in RON (per profile: lowend, desktop, steamdeck), loaded with a round-trip test when it is introduced in M1. The perf gate (median of 3, 8%) keeps running on the dev machine. A lowend regression is judged on the scaled numbers until a device exists.

Risk: emulation can hide iGPU memory-bandwidth limits. The first real-device bench is an M1 exit item.
