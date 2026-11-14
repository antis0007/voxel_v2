# Axiom revision 4: integrated implementation track

2026-10-24. User authorized incorporating the full revision and starting implementation.
This extends the master inventory without replacing system IDs, compatibility rules
or existing work. Axiom names the architecture input, not a product rename.

2026-10-26: user reopened implementation of the next missing milestone. The bounded
CPU-region checkpoint foundation is implemented in
[ADR 0025](../adr/0025-cpu-region-checkpoints.md); runtime acceptance is pending.
This does not reopen every parked package or complete the window/save cutover.

The subsequent request to continue through all milestones reopens the full Axiom
track. Follow the [current execution ledger](../plans/2026-10-26-axiom-execution.md);
the original records below remain historical evidence.

Inputs: [original](../specs/project-axiom-revision-4.txt),
[refined review](../design/PROJECT_AXIOM_REV4_REVIEW.md),
[master plan](../MASTER_PLAN.md). The review records the earlier design-only pass;
this document tracks subsequent implementation separately.

## Full scope routing

Every revision-4 section remains in scope. Planned work is not delivered by this increment.

| Revision-4 scope | Existing route | Status / next gate |
|---|---|---|
| 1. Interacting persistent-world experiences | North Star; game design; Observatory | Existing systems; integrated acceptance planned |
| 2. Canonical multiscale storage | C02/C03/C14/C16; ADR0017–0019 | Existing scales retained; mixed physical scale and moving-volume ownership planned |
| 3. Rendering, coverage and topology | Rendering inventory; streaming/horizon | Existing epochs, dirty revisions and proxies; dependency/coverage audit planned |
| 4. Causal islands and physical solvers | C04–C10/C17; B01–B08 | Existing bounded legacy solvers; conservative model exchange, pressure and structural contracts planned |
| 5. Shared effects, machines and magic | C15; M01–M21; G01–G09; VM | First synchronous effect boundary below; spells, circuits and fabrication staged |
| 6. Geology, ecology, dungeons and bosses | World/navigation/biology inventories; habitats; resonance | Existing content and optional expeditions; cross-system Observatory acceptance planned |
| 7. Change dependencies and scheduling | C15/C17/C20; publication jobs | Commit receipts for one edit path and water-buffer exchanges implemented; change stream and causal scheduler planned |
| 8. Rust modules, persistence, precision and mods | C01–C03/C15/C16; content/VM | Rust/WGPU baseline retained; stable mod IDs, migrations and sandboxed behavior planned |
| 9. Performance laboratory | C18/C19; profiling/capture tools | Workload and quality gates retained; no new measurements claimed |
| 10. Navigation and senses | N01–N07 | Existing bounded search reused; regional summaries and senses planned |
| 11. Drowned/Flooded Observatory | Same canonical Flooded Observatory; review O1–O4 | Finite reservoir and dug/pumped routes first; no location acceptance yet |
| 12. Approachability and progression | Player/UI/Primer inventories; North Star | Bindings and concise inspection preserved; advanced controls optional |
| 13. Research experiments | Research notes; review §9 | NanoVDB/meshing/Rapier candidates only; no dependency upgrade or unmeasured winner |
| 14. Development sequence | Review A–F; master packages | Gate B started; compilation does not complete the gate |
| 15. Acceptance | Master claim ledger; review §10 | Separate correctness, gameplay, visual, persistence and hardware evidence |

## First implementation design and evidence

### 1. Purpose

Centralize admission results, transferred quantities and immediate geometry
notification for drill excavation and pump/tank exchanges.

### 2. Scope and dependencies

`world/effects.rs` wraps existing world setters, material/environment ownership,
prepared source residency, revision counters and mesh-halo invalidation. It creates
no competing material store and does not migrate the CPU development region.

### 3. Data, residency and ownership

A stack-local, non-Clone commit receipt holds position, revision and result.
No new queues, persistent IDs, per-cell allocations or save fields. Water uses the
existing machine unit of 1,953,125 microlitres (1/512 m³). World and machine buffer
remain exclusively borrowed throughout synchronous execution.

### 4. Algorithm, cost and determinism

Admission rejects wrong authority, bounds and assembly ownership; unprepared cells
defer without I/O. Replacement checks expected material and confirms the actual
result. Exchange clamps to stock, capacity and requested limit; environmental
worlds preserve fractional water and legacy worlds move whole packets. No fallible
step remains after the world write succeeds. Added bookkeeping is constant-size;
existing setter/environment costs remain. Global legacy determinism is not claimed.

### 5. API, actions, events and queries

`replace` and `exchange_water` return `Result<Commit<T>, Failure>`, distinguishing
deferred admission, conflicting observation, rejected ownership and no transferable
quantity. `Commit::publish` consumes the result, debug-checks the immediate world
revision and marks the cell plus existing mesh dependency halo dirty. The returned
committed quantity controls downstream work. This is not a durable event stream.

### 6. Tick, budget and sleep

Existing fixed-step/machine cadence and traversal order remain. Transfer previews
precede promotion into editable residency. Deferred drill/pump work restores bounded
credit and reports waiting; energy is charged only after success. Global scheduler
fairness and background catch-up remain open.

### 7. Hardware and fallback

CPU bookkeeping with no new GPU requirement. Legacy packet rules remain. An active
experimental CA mirror rejects these effects instead of admitting competing writers.

### 8. Content and feedback

No new controls or recipes. Exact-volume tanks can drain a final amount below a
legacy packet; pumps can fill similarly small remaining tank capacity. Sediment
stays at intake under existing clogging rules. Ordinary mesh/power/production
feedback remains the presentation path.

### 9. Planned verification

At every world scale: mismatched excavation, assembly rejection, deferred admission,
full/empty tanks, one-unit exact remainders, legacy subpacket rejection, unchanged
quantities on failure, cross-chunk dirty halos and save/load after transfers.
These checks are not added or executed under the current no-tests instruction.

### 10. Performance target

No added I/O or full-world scans. Compare matched pump and drill routes before
claiming throughput gains; no timings measured in this increment.

### 11. Migration and deletions

No save/generator version, asset schema or dependency changes. Migrated consumers
no longer duplicate buffer debit/credit or mesh publication. Other writers remain
until individually migrated.

### 12. Risks and experiments

The borrowed buffer is not an asynchronous generational owner handle. A receipt is
not a replay ID, full dependency read set or durable journal. No general atomic
multi-edit batch is claimed. Snapshot consistency, bounded consumer recovery and
renderer final-publication auditing remain open. This is a source implementation,
not runtime or persistence acceptance.

### 13. Package and evidence

One module with narrow factory integrations. Local application compilation passed.
No tests, hosted CI, benchmarks or game launch. No independent rubric score is
claimed. Next: audit direct edits and final mesh upload against dependency halos,
then expose finite transfer through an Observatory interaction. Exact CPU checkpoint
and resume remain a separate master-plan I2 obligation.

### 14. Alternatives

Reuse the existing paired cellular transfer and authoritative setters. Defer a
global transaction engine or alternate fluid inventory until a consumer needs it;
neither should become a second engine beside the game.

## Second implementation: player and spell commit outcomes

Implemented subsequent to the first slice:

- Mining, salvage harvest, blast harvest, spell blasts and settled-log removal
  use conditional admitted effects. Rewards and resource remainders update only
  after a successful removal; repeat or rejected log removals cannot mint wood.
- Single-cell spell carving and fungal-soil conversion publish consumed receipts
  into the existing adventure dirty-cell consumer and its mesh dependency halo.
- `World::set_many_committed` reports successful edits in input order while
  retaining one revision increment per batch. It rechecks effect admission before
  every cell, including activity capacity after earlier edits. Existing ordinary
  and authored-mechanism setter behavior remains unchanged.
- Spell batches reserve room in their existing 2048-cell publication list before
  mutation. Failed/skipped cells generate no geometry notification. Tree spells
  reserve the bounded canopy result before starting.
- Cryogenic and ignition batches apply their associated thermal payload only to
  committed terrain cells. Partial-water freeze rejection therefore cannot attach
  an ice thermal payload to unchanged water. Canopy rewards count removed leaves.

Source audit confirmed diagonal one-cell mesh-halo invalidation, final pending-GPU
dirty-revision validation, and expansion of adventure dirty cells in `main.rs`.
Renderer files were inspected only; no renderer changes or visual claims.

Local offline application compilation passed after these changes (31 warnings).
No tests, game launch, benchmarks or save/load validation. Spell batches remain
intentionally partial; rejected work is skipped rather than persisted for replay.
Casting mana/cooldown remains the existing spell-level cost, not a per-cell charge.
Rigid-body multi-cell ownership/settlement and a playable finite Observatory route
remain open. This extends Gate B and does not complete revision-4 acceptance.
