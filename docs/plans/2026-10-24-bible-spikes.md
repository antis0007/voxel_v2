# Bible spike register

Status (2026-10-24): master review passed ([decision 0022](../decisions/0022-master-plan-review.md));
no spike has run. Each spike gets one session, numbers, raw CSV and an ADR before
any dependent SDD can pass. Results and the 5-run measurement protocol live in
[docs/spikes/](../spikes/README.md). Hardware classes come from
[ADDENDUM_B_HARDWARE.md](../specs/ADDENDUM_B_HARDWARE.md).

**Schedule per 0022:** S1 and S4 next (sessions 1 and 2), S2 third (in parallel
with session 4, on the M1 path through C06). S6 and S7 wait for M1.5. S3 is
deferred: the existing renderer serves the M1 slice; decide after M1 numbers.
S1 picks an alternative over eight-phase Margolus only if it is at least 25% better.

## Measurement contract

Record commit plus dirty-file hashes, compiler/build profile, CPU/GPU/driver,
backend, resolution if relevant, power state, seed, workload size, repeats,
warmup, p50/p95 time, peak memory, awake counts and output/conservation digest.
Compare alternatives against identical inputs; report unsupported paths and
failed runs. A bounded spike can return 'inconclusive' with its next experiment.
Do not rebaseline away regressions or extrapolate a component rate to game FPS.

| Spike | Alternatives/input | Required numeric outputs and decision | Dependent SDDs |
|---|---|---|---|
| S1 Brick CA | 8³/16³; workgroup 64/128; halo/global; equal total cells; sparse/dense active sets | Cells/s, ms/tick, bytes/cell, halo/read traffic, output parity; choose kernel shape within measured limits | C03/C04 then C05–C10, C14 and R01 consumers |
| S4 CPU reference | Existing scalar/SIMD alternatives at x86-64-v2; identical pinned integer rules and cell mix, one core first | Cells/s/core, p95 tick cost, bytes/awake brick, parity; derive CPU awake cap from measured per-cell cost plus fixed overhead | C04–C10, C18/T06 H0 limits |
| S6 VM | Interpreter dispatch alternatives; arithmetic, loops, queries, faults; fixed memory/instruction caps | Instructions/s, ns/instruction, sandbox overhead, worst bounded action/query cost, memory; online/offline glyph compile decision | V01–V05, B09, M09 and program consumers |
| S2 Liquid | Pressure-column / iterative relaxation / SPH-lite on u_tube and dam_break, same mass/bounds | Equalization ticks, overshoot, mass-error units, p95 cost, peak memory and artifact captures; pressure/compression model | C06 and pressure-dependent C07/M11/G06 contracts |
| S3 Rendering | Binary greedy + indirect / brick raymarch; solid, checkerboard, fragmented and transparent scenes | CPU/GPU p95, mesh latency, upload bytes, peak memory, image differences on H1/H3; choose path and no-compute alternative | R01/R02/R05/R10/R13 |
| S7 Resonance | Few-operator integer pressure/vibration plane versus bounded authored response | ms/tick/awake cell, memory, damping, modal peak/error versus analytic small chamber, energy gain/loss; disclose abstraction if needed | G01/G04/G05 and C14's later resonance adapter; not the generic C14 storage/allocation contract |
| S5 Nav summaries | Dirty single brick, contiguous damage, collapse burst; summary/portal alternatives | Rebuild microseconds/dirty brick, portal bytes, route error/failure rate, p95 backlog and mob service capacity | N03/N04/N07 and crew paths |
| S10 Civilizations | Ten seeds; linked names/architecture/temple mechanism/economy/downfall records | Generation time, bytes/civilization, rule contradictions, human coherence rubric counts with examples; choose generator approach | W02/W03/W05/W06; generated text/lore consumers |
| S9 Orbit | Integer patched-conic/element formulations; circular/eccentric orbit and transfer windows | Long-run position/phase drift, energy error, overflow bounds, step cost and memory; bound supported times/scales | W09/W10, C-S5 vessel/habitat scope |
| S8 Compiler information | Real small/large/repetitive/noisy build templates; canonical serialization + fixed compressors | Input/compressed bits/template, translation/rotation sensitivity, adversarial inflation, throughput and energy curves; choose honest information proxy | M24 Feed economy and C-S2 |

## Priority and dependency leverage

S1/S4 run first because they gate the CA/storage roots of most physical
systems. The counts below measure design leverage; the schedule above (milestone
demand) overrides them: S2 before the C06 liquid SDD, S6 before VM design (M1.5),
S3 only if M1 numbers call for a renderer change, S5 before nav and S10 before culture.
S7 precedes resonance-specific mana contracts, not generic plane allocation:
C14's core specifies typed byte storage, lifetime and admission independently
of which later plane operators are selected. C14's resonance adapter cannot
pass until S7 and G01's widths/costs exist. S9 precedes orbital scope and S8 Feed
balance. Counts measure gated design scope, never completed implementations.

| Priority | Spike | Direct system gates | Transitive systems, including gates |
|---|---|---:|---:|
| 1 | S1 | 10 | 88 |
| 2 | S4 | 9 | 61 |
| 3 | S2 | 4 | 28 |
| 4 | S6 | 7 | 20 |
| 5 | S3 | 5 | 13 |
| 6 | S5 | 3 | 12 |
| 7 | S10 | 4 | 11 |
| 8 | S7 | 3 | 9 |
| 9 | S9 | 2 | 2 |
| 10 | S8 | 1 | 1 |

Counts come from reverse reachability in the master adjacency table on this
planning snapshot. Shared downstream systems occur in multiple rows. C14's
resonance-specific adapter is a later capability of the same owner; it is not
counted as blocking the entire generic plane-manager SDD.

The fixed B.7 numbering conflicts with B.5/B.6. The proposed stage/sub-session
interpretation and S2 precursor are disclosed in master §6 and ADR0021 for human
review. No dependent SDD is silently finalized while its spike is outstanding.

## Decision boundaries

S1 may compare 16³ in an isolated experiment; the accepted 8³ production brick
contract remains until a justified superseding ADR. S2 must conserve integer
mass, including thin films/residuals; a visually quiet cutoff cannot delete it.
S7 cannot claim physical resonance when an authored lookup was measured instead.
S8's compression count is a gameplay proxy, not thermodynamic information cost.
S9 must address stress/spin/thermal/radiation adapters before claiming the full
orbital-habitat claim. S10 needs human review, not only validator counts.

If missing hardware prevents device work, report only locally measured host
numbers and estimates; the target-class decision stays open. A result that
invalidates a milestone triggers B.6 escalation. Every other inconclusive result
stays an explicit design blocker with a bounded next experiment.
