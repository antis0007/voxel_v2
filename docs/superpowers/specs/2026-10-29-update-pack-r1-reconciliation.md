# Update pack R1: reconciliation with the tree and decision 0039

Date: 2026-10-29. Tree: `7457bb90` plus the shared dirty tree.
Inputs: `docs/UPDATE_PACK/` (owner staging, untracked). The owner named the
R1 package (`START_HERE.md`, `DESIGN.md`, `TASKS.md`, `REVIEW.md`,
`ACCEPTANCE.json`) as the key reference. `ROUGH_READ_THIS_AND_REFINE.txt` is
the chat transcript that produced the performance, latency and physics
sections of `docs/specs/AXIOM_Unified_Engine_Implementation_Prompt.txt`.

This file refines the rough transcript against R1 and the source. It is a
gap analysis, not an implementation record. Every R1 acceptance case stays
NOT_RUN.

## 1. Precedence

R1 D00 orders authority as: latest owner decision, verified compatibility
contracts, R1 corrections, the consolidated directive, older briefs. Applied
here:

| Topic | R1 | Decision 0039 | Resolution |
|---|---|---|---|
| Execution unit | one bounded proof per session (T00..T21) | brief items become MASTER_PLAN rows | Compatible. A T-packet proof is the row. |
| Next task | T00, then correctness blockers, else T03 ground surfaces | M1 `dam_break` keeps priority | Correctness first under both. The known freeze (section 3, item 1) is a confirmed blocker, so it goes ahead of T03 and M1 work alike. T03 vs M1 after that is an owner choice. |
| Body authority | relevant client physics; exact replication only for proven domains; state-sync fallback | rigid bodies host-authoritative until fixed-point bodies | Not yet compatible. `engine_sim` lockstep is the exact-domain pilot (T13). Client-stepped bodies in state-sync mode need a new decision before code. |
| Transport | keep existing transport, no rewrite | no iroh, Renet or Quinn | Agree. Quinn and iroh notes in the rough file are reading only. |
| Names | preserves Crimson Mirror, Hall, Null Choir, Kestrel and others as canon | no new proper nouns approved | Open. R1 itself says names do not license placeholder registries. Treat R1 names as approved for design discussion only; player text and RON rows still need explicit owner sign-off per name. |
| Liquids | one selected bounded solver; candidates are not all built | Salva/APIC pilot not taken; liquids are integer `engine_sim` | Agree. R1 D15 matches 0039 stage 5. |
| Numbers | 256 stacks, 16 m scanner, 120 FPS, 4 MiB/frame uploads, 70/80 % memory are trial values | none | Trial values stay in RON or config, never constants. |

## 2. What the rough transcript adds over R1

R1 compresses the transcript's engine material into D14, D16 and D17. Items
worth keeping, with R1 cross-references:

- Interaction latency split into seven measured spans (input to local
  feedback, authored mining time, eligible break to predicted hole, request to
  host apply, host apply to client install, remote observer state age, apply
  to durable). R1 D16 asks for state age; this is the finer breakdown.
- Edit invalidation follows the reverse of the read stencil: an output at x
  that reads x+1 is dirtied by input y at output y-1. Symmetric neighbour tests
  hide this. Belongs to T01 / WLD-08.
- Edit amplification as separate counters (bytes inspected, copied, generated,
  uploaded per accepted edit), not one score. Belongs to T01 measurement.
- Conditional refinement: record refined meshes discarded before draw. R1 D17
  "useful versus wasted rebuilds" is the same metric.
- Snapshot capture and suffix retention must be one atomic step, or an edit
  between them is lost (NET-08).
- Break-then-shot exemption limited to the exact removed revision; the host
  records one adjudication event (NET-13). Needs an owner row; no PvP work is
  scheduled.
- Rapier gravity changes do not wake sleepers (PHY-09). Relevant only after a
  body backend decision.

Dropped as not applicable or superseded: StagingBelt recall advice (no
`StagingBelt` in the tree), Quinn datagram eviction semantics and iroh relay
paths (decision 0039 item 4), Salva, APIC and Nexus MPM pilots (0039 stage 5),
clustered-forward lighting (R1 D14: only after measuring local-light cost),
the 30 % p99 and 50 % time-to-interaction goals (R1 D17: absolute budgets
matter more than relative ones).

## 3. Claims checked against source

| # | Rough claim | Tree | Status |
|---|---|---|---|
| 1 | Reproduce the default freeze first (REN-01) | Streaming source-cache stall at view 28. Fix landed in HEAD as `cd114bce` (same diff as `3f6d4eec` on `idle-stream`). | Fixed in HEAD. Native view-28 convergence rerun not yet recorded. |
| 2 | Cancelled StagingBelt encoders leak chunks (REN-03) | No `StagingBelt` use. 41 `Queue::write_buffer` call sites in `crates/app/src`; `render/streaming/gpu_upload.rs` has fallible staging. | N/A as written. Open: whether pending `write_buffer` staging counts against the budget in `gpu_budget.rs`. |
| 3 | Memory thresholds are ceilings, not targets (REN-05) | `render/streaming/gpu_budget.rs`: "A budget is a ceiling, not an allocation"; native query off the frame loop. | Matches. Allocator backing vs live bytes not reported yet. |
| 4 | No blocking `Device::poll(Wait)` in play | `render/frame_pacer.rs:52` waits with a timeout by design (device-lost fix 58f4b367); other `Wait` uses are tests and menu capture. | Refine the rule to: bounded waits only, reported as `acquire_ms`. |
| 5 | PipelineCache is Vulkan-only; plan DX12 compile stalls | `render/pipeline_cache.rs` gates on `Features::PIPELINE_CACHE` (wgpu 27.0.1). | Handled for Vulkan. Open: measure first-encounter compile stalls on DX12. |
| 6 | Predicted block break with matching collision (NET-05, NET-07) | `crates/netcode/src/edits.rs` has `EditPredictor` with sequence acks and held server writes. No caller in `crates/app`. Joiner mining (`multiplayer/mining.rs`) only animates progress; the hole appears after a host round trip. | Main latency gap. First T13 proof: wire `EditPredictor` into joiner dig/place, collision and mesh patch together. |
| 7 | Target generation, not just coordinates (NET-06) | `EditPredictor` keys by cell; no target generation in the request. | Gap. Add expected-before material or generation to the edit request when item 6 is wired. |
| 8 | Operation identities stop duplicate rewards (WLD-06) | `crates/netcode/src/operation.rs` receipts, landed in e83d7188; expired sequences force full sync. | Partial. Covers transfers, crafts, trades; edits not yet routed through it. |
| 9 | Urgent traffic ahead of bulk (NET-10) | `multiplayer/deltas.rs` test `player_edits_go_ahead_of_a_backlog`; `netcode` has reliable ordered and unreliable lanes, chunk pacing. | Partial. Queue age (not only bytes) is not tracked. |
| 10 | Checkerboard worst case (REN-12) | Not benchmarked. | Open: add solid, checkerboard and perforated chunks to `just bench`. |

## 4. Proposed order

1. Done: source-cache eviction fix (item 1) is in HEAD as `cd114bce`. T01
   correctness, R1 "correctness before cosmetics".
2. T13 first proof, narrowed: joiner dig/place through `EditPredictor` with
   expected-before validation (items 6, 7). Measure spans from section 2 on
   localhost; label cross-machine proof pending.
3. M1 `dam_break` is the next content-side proof (owner, 2026-10-29); T03
   waits.
4. Measurement backlog, no code until a trace points at it: write_buffer
   staging accounting (item 2), DX12 compile stalls (item 5), queue age
   (item 9), checkerboard bench (item 10).

## 5. Owner answers, 2026-10-29

- R1 names are design references only. Decision 0039 item 3 stands: no name
  enters player text or RON rows without per-name approval.
- After the freeze fix, M1 `dam_break` comes next; T03 waits.
- Still open: whether state-sync client stepping of bodies (R1 D16 fallback)
  gets its own decision now or waits for fixed-point bodies (0039 item 2).
