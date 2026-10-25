# 0022: Master-plan review decisions

Date: 2026-10-24. Status: decided by the human review of the session-2
[master plan](../MASTER_PLAN.md) draft. Supersedes the session-grouping
proposal and the "every system gets fourteen sections" rule of
[0021](0021-bible-planning-gates.md); 0021's planning gate otherwise stands.

## Context

The session-2 draft routed 140 systems, 24 claims and the full Downloads intake
into one foundation-first M1, applied the full SDD process to every system and
left several architecture decisions pending. The review approved it with the
amendments below.

## Decisions

1. **Frozen inputs.** Inputs of record: ENGINE_CORE_PROMPT_v2, MOD_ADDENDUM,
   ADDENDUM_B_HARDWARE, GAME_BIBLE, the tooltip contract (master §7) and this
   review. Downloads intake, coverage registers and reconciliation passes stop;
   `plans/download-coverage/` and `specs/downloads-2026-10-24/` are historical.
   Project Axiom revision 4 is out of scope for M1–M3; its track moves to
   [docs/parked/](../parked/README.md).
2. **Tiered SDDs.** Tier A (36 listed systems): full SDD, independent critic,
   12 rubric rows 2/2. Tier B (other state owners): sections 1, 3, 4, 6, 9, 13,
   one critic pass. Tier C (content-only): RON schema, validator rules, one
   scenario. Tier is a column in the inventory; promotion needs a one-line reason.
3. **M1 is a vertical slice:** `dam_break_200k` on the integer backend,
   authoritative in an ADR 0018 region of radius 3 bricks around the player, GPU
   bit-exact with CPU, replayable from the action log, checkpoint save/reload,
   rendered with liquid volumes through the existing renderer, 60 tps and p95
   ≤ 16.7 ms on Iris Xe. Off-path M1 rows move to M1.5, including light (C11),
   audio (C21), non-H1 profiles, upscaling, far LOD and the VM (V01/V02).
4. **Spikes now, with a protocol.** S1 and S4 first, S2 third; S6/S7 at M1.5;
   S3 deferred until M1 numbers. Protocol in [docs/spikes/](../spikes/README.md).
5. **Bodies use fixed-point.** D3 applies to everything in `sim::state`. The
   B03 float-to-fixed migration is part of B01's SDD.
6. **Budgets.** Deterministic quotas (items per tick) are authoritative for
   simulation. Wall-time caps only limit render and mesh jobs, never sim.
7. **Kernel shape.** Eight-phase Margolus, unless S1 shows an alternative at
   least 25% better.
8. **Authority cutover.** The new backend becomes global authority when the
   `tour` scenario's dual-run world hash (legacy vs new, same actions) differs
   only in bricks the new rules intentionally change, and p95 holds target on
   Iris Xe for 10 minutes. Until then region authority grows by one brick of
   radius per passing milestone.
9. **C-N2 denominator:** unordered material pairs, context-free; any
   contextual row makes that pair nontrivial.
10. **Save version** is chosen at migration time against then-current source.
11. **Claims.** Active in M1–M3: C-MC3, C-MC5, C-Z3 (slice), C-Z4 (review),
    C-S1, C-N5 (review). `just claims` is created in M3 with those six.
12. **Process.** Session reports ≤ 40 lines (changed, numbers, exit met y/n,
    next exit). One Status line per document instead of per-paragraph
    disclaimers. Critic reviews ≤ 1 page; three-round cap stays.
13. **Next five sessions** are fixed in master §6. The human reviews after
    session 2 (spike numbers) and session 5 (first passed SDDs).

## Consequences

Design effort concentrates on ~36 Tier A systems; the rest carry lighter
artifacts. M1 produces a measurable playable result before the broader
foundation. Legacy stays authoritative outside the slice region.

## Reversal criteria

S1 beating eight-phase Margolus by ≥ 25% reopens decision 7 through its own
ADR. Slice numbers missing the Iris Xe target reopen S3 and the M1 scope.
Any change to inputs of record needs the human.
