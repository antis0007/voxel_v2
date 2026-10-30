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
   waits. Sim slice landed: scenario, log replay and exact checkpoints
   (76d77a4f), GPU bit-exact test (097d30df). App wiring waits on the owner
   of the uncommitted `simulation/cpu_region` work; item 2 waits on the owner
   of the untracked `crates/app/src/multiplayer` module.
4. Measurement backlog, no code until a trace points at it: write_buffer
   staging accounting (item 2), DX12 compile stalls (item 5), queue age
   (item 9), checkerboard bench (item 10).

## 5. Owner answers, 2026-10-29

- R1 names are design references only. Decision 0039 item 3 stands: no name
  enters player text or RON rows without per-name approval.
- After the freeze fix, M1 `dam_break` comes next; T03 waits.
- Still open: whether state-sync client stepping of bodies (R1 D16 fallback)
  gets its own decision now or waits for fixed-point bodies (0039 item 2).

## 6. T03 first proof: forest slope outcrops (dormant gate 81)

Run 2026-10-29 at the owner's request to implement the update pack. It runs
ahead of the section 5 order (M1 `dam_break` next) because it is dormant:
`CURRENT_GENERATOR_VERSION` stays 80 and no live world changes.

Work card:

- Player result: grey ground on forest and meadow slopes says what it is.
  Steep slopes are the bed under them; on gentler slopes the bed breaks
  through only on convex shoulders and stands 0.5 m proud as a knob; hollows
  and soft beds keep soil and trees.
- Cause found: `column_from_landscape` greyed 0.35..0.54 slopes with gravel
  from a 19 m value-noise patch. Above 0.54 the slope rule's sediment test
  never fires in forest (a probe over three seeds found sediment about 0 on
  every slope above 0.2), so all of it was plain Stone.
- Change: `crates/worldgen/src/ground_exposure.rs`; one gated branch and a
  ledge term in `landscape.rs`; `sample_stratigraphic_bed` made crate-visible;
  `main.rs` lets `--benchmark-generator-version=81` render the dormant gate.
- Trial values: shoulder band starts at Stone 0.36, Basalt 0.38, Limestone
  0.42, Sandstone 0.48, other 0.56 (plus or minus 0.05 of 9 m wobble) and is
  0.18 wide; above it the whole slope is bare. Shoulders need 0.12 m (gentle
  end) to 0.04 m (steep end) above the 4 m stencil mean, about the 85th
  percentile of forest slopes on four seeds.
- Preserved: every gate up to 80 (golden digests before and after identical);
  trails grade over outcrops (`routes::block` carves to grade); households,
  sites and the structure chain still own their voxels; caves stay open.
- Not done: scree aprons below outcrops, ledges per bed boundary, litter,
  paths, construction disturbance (later T03 increments).

Evidence:

| Check | Result |
|---|---|
| `gen_golden` digests, versions 2..80, two seeds, two scales | identical before/after |
| `ground_exposure` tests (4), seeds 424242, 1, 7777, 1 m and 0.5 m | pass (run in a D: snapshot; the shared tree did not compile, see below) |
| Grey area (rock plus gravel) on forest slopes, gate 81 vs 80 | not larger (test `grey_ground_does_not_spread_over_forest_slopes`) |
| clippy on `engine_worldgen`, lines in this change | clean |
| Outside banks and fault scarps, slope gravel at gate 81 | 0 (gate 80: 109 columns on seed 424242) |
| Remaining gate-81 slope gravel | all drainage bank or fault scarp |
| Full `engine_worldgen` lib suite | 6 failures, all at versions 80 or lower, untouched by this gate: amberroot pools, valley roofs, undercrown pin and roof, two tree-form tests. Peer gen-80 work; not chased |
| Native captures | below; stills only, no motion |

Native review, seed 2738958700 at (2728, -3448), 1 m voxels, noon, HUD off,
one exe at `--benchmark-generator-version=80` and `=81`. Frames:
`target/visual-direction/t03-outcrops-20261029/`.

- First draft (`t03-g81-*`): rejected. Every 0.36+ hard-rock slope went bare,
  a whole forested hillside became a stone wall and the lift read as slabs.
- Revision (`t03r2-g81-*`): the stone bowl matches gate 80; gravel strips are
  gone; a few stone knobs sit on shoulders; walking view keeps its grass and
  trees with one knob in sight. Knobs are single-voxel steps: acceptable for
  this proof, weak as art.
- Outcrops on this seed lie about 4.4 km from spawn; near spawn the forest is
  too gentle to expose rock, so few players would see this change early.

Build context: the shared tree's app and worldgen failed to compile during
this session on a peer's in-progress `BlockId::Obsidian` reference
(untracked `hell_gate.rs`). Tests and captures ran in a copy at
`D:/scratch/t03-outcrop-snap` with that variant replaced by Basalt in the two
hell_gate files only; both gates rendered from the same exe.

Acceptance: REV-06 partial (bedrock class only; sediment, soil and paths
open); REV-29 partial (matched stills, no motion or reverse approach);
REN-10 not run. Mine/build at outcrop contacts: automated (surface and next
voxel are the bed), not played.

Not committed: gate 81 sits above uncommitted gates 54 to 80, and HEAD is at
generator 53 without `households`, `fernwake` or `meridian_terminal`, which
the tests use. Owned files: `crates/worldgen/src/ground_exposure.rs` (new),
the gate-81 hunks in `landscape.rs`, the `ground_exposure` line in `lib.rs`,
the allow-list term in `main.rs`, gate rows 80 and 81 in GENERATOR_GATES.md.

Next smallest step: scree aprons downhill of knobs (talus from the same bed),
then a motion capture along the walking route, then the owner decides the
gate-81 bump.
