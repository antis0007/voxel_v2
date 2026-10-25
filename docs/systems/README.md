# System-design protocol

Status: protocol in force; no SDD has passed. The [tooltip slice](tooltips.md)
predates this protocol. All 140 IDs, their tiers and dependencies are in the
[master](../MASTER_PLAN.md). Each Tier A/B system gets `docs/systems/<ID>.md`;
shared documents may link contracts but may not merge away an inventory entry.
CPU/GPU kernels share semantics.

## Tiers ([decision 0022](../decisions/0022-master-plan-review.md))

- **Tier A** (36 systems, listed in the master): all fourteen sections below;
  independent critic, all twelve rubric rows 2/2, three-round cap.
- **Tier B** (other state owners): mini-SDD with sections 1, 3, 4, 6, 9, 13
  only; one critic pass, no rounds.
- **Tier C** (content-only): no SDD. A RON schema, validator rules and one
  scenario.

Promotion to a higher tier needs a one-line reason in the master inventory row.

## Required document sections (Tier A)

Use the bible's fourteen numbered sections in this exact order; justify every
N/A. Target 2–6 pages. A document blocked on an experiment remains a draft.

1. **Purpose:** one paragraph, pillars and exact C-* obligations.
2. **Scope:** in/out, upstream/downstream links and capability milestones.
3. **Data:** all structs/fields/types/bit widths/packed and aligned sizes,
   residency, allocation/lifetime/owner, RON schema/example, plane usage.
4. **Algorithms:** pseudocode for every operation, complexity, awake-count
   cost equation, ordering/hash/determinism, overflow and failure handling.
5. **API:** signatures for <=40 public crate items; consumed/emitted actions,
   events and versioned text/UI/VM query views. Budget any cross-owner writes.
6. **Tick integration:** stages, exact budgets/divisors, distance policy,
   sleep/wake conditions and named overlay metrics.
7. **Hardware:** computed memory and estimated/measured tick cost separately
   for H0/H1/H3 and every other applicable Addendum B class, owned ladder knobs,
   explicit GL/no-compute route. Defined targets are not measured support; N/A
   or an unrelated desktop result cannot substitute for hardware evidence.
8. **Content:** RON example, proposed scaffold command, validator conditions;
   extend registries and shared rules rather than adding matches on content IDs.
9. **Tests (planned until authorized):** correctness, properties, CPU/GPU
   equivalence, applicable matter/energy conservation, replay and class coverage;
   named scenarios/claim IDs and visual goldens where applicable.
10. **Bench:** workloads, each class's targets, awake counts, memory and a
    regression threshold. Cite spike receipts; do not invent benchmark numbers.
11. **Migration:** save/version effects; exact files/symbols replaced and
    estimated deleted LOC. Preserve legacy until compatibility evidence exists.
12. **Risks/open questions:** mitigation or experiment with session/time/memory
    cost; link an ADR for each decision with >1-week consequences.
13. **Sessions:** one system or spike each, ordered file scope, estimated
    production/deletion LOC and a concrete proposed `just verify` exit. State
    which checks/commands still need implementation or execution authorization.
14. **Alternatives rejected:** at least two with mechanisms and tradeoffs.

## Independent review

Freeze the SDD and its input hashes. Start a fresh critic without the author's
conversation; provide only the SDD, referenced source/spec/spike artifacts and
the B.4 rubric. The critic can inspect those sources and reports to the author.
Do not use the author as critic or describe this master-plan audit as an SDD pass.

Write `docs/systems/reviews/<ID>-r<N>.md` (at most one page) with critic identity,
document SHA-256, twelve 0–2 scores, line-specific findings, required corrections
and verdict.
Every item must be 2. The twelve checks are: data/memory; algorithms/determinism;
acyclic dependencies; named claims; fallback; degradation/sleep/metrics; RON and
scaffolds; correctness/conservation/replay/class coverage; verifiable session
exits; costed risks/ADRs; named deletions; mechanisms rather than wishes.

Revise and re-freeze for at most three rounds. On deadlock, state the disagreement
and alternatives to the human in <=10 lines. A pass permits only the reviewed
scope; hardware or interface changes reopen affected items. B.7 separately
requires human review of the first passed core SDD set.

## Implementation handoff

Load CLAUDE.md, the passed SDD and last exit report. Claim one scoped writer;
do not alter another session's files. End with a session report of at most 40
lines: changed, numbers, exit met y/n, next exit. No restating of gates. No
self-approved migration or visual acceptance.
