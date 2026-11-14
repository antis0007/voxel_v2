# 0026: Acknowledged edits and CA publication

Date: 2026-10-26. Status: source implemented and locally compiled; runtime pending.

The user requested continuing through the remaining Axiom milestones. This
increment extends ADRs 0018 and 0025 without enabling windowed CA authority.

## Commands and results

The CPU adapter accepts conditional replacement against an exact observed cell.
At the command's turn, the backend compares that observation with current
authority before the kernel runs. It reports `Applied`, `NoChange`, `Conflict`
or `Protected`, including the original and resulting cells, sequence and tick.
Conditional commands cannot remove interior bedrock. Explicit fixture commands
retain their existing unconditional semantics; the sealed halo is immutable.

The existing 128-slot admission limit now covers queued commands and unconsumed
results together. Each accepted command resolves once in FIFO order. Consumers
release a slot by taking its outcome. A slow consumer cannot create an unbounded
result backlog. Conflict and no-op outcomes advance the resolved sequence without
claiming a terrain change. Published visibility remains separately delayed.

`EditAt(at: [x,y,z], block: "Stone")` in a headless command document submits
against the exact published cell observed by the player. It can conflict if
authority changed during the publication delay. The runner emits `ca-resolved`
receipts after a step and consumes them. It does not charge or award inventory
for these development commands. Full gameplay command ownership remains work.

Checkpoint format `VXCA0002` retains conditional observations and unconsumed
outcomes as well as the state from ADR 0025. Counts, FIFO order, clocks and
outcome consistency are checked during restore. Version 1 is rejected explicitly;
there is no development-format migration promise. Ordinary saves are unchanged.

## Publication

`World::publish_ca_view` swaps one immutable snapshot, marks every changed cell's
existing mesh dependency halo, invalidates derived world snapshots and increments
the world revision only when cell state changes. The snapshot's actual CA tick
advances even when geometry is unchanged. Initial attachment also invalidates
the sealed region's coverage.

`WorldSnapshot` retains that exact view. Queries and generated mesh sources read
it after legacy terrain overlays. The existing incremental mesh repair uses the
same query and water snapshot. Existing dirty revisions and final GPU publication
checks remain the stale-result gate. Collision and geometry therefore have one
published source contract; no renderer-owned material authority is introduced.

Partial water is projected into the renderer's existing eighths-plus-deficit
encoding; authoritative cells keep all 64 levels. Display conversion may round
by less than one microlitre at the quarter-metre scale. World water-fill queries
use the exact fraction directly. Legacy terrain/fluid setters reject writes
while the experimental mirror owns the session.

## Ordinary placement

The normal block-placement path now calls `effects::place`, which validates the
selected hotbar stack, conditionally commits terrain, publishes its receipt and
only then consumes the item. Failed admission, conflict and no-op writes charge
nothing and emit no placement geometry notification. Existing machine placement,
torch support, player-overlap checks and bulk-stroke scheduling remain in place.

## Evidence and limits

Local offline application `cargo check` passed after implementation. No tests,
scenarios, benchmarks or game launches were added or run. This is source and
compilation evidence, not acceptance of rendering, gameplay, checkpoint round
trips, cross-boundary edits or conservation. No graphics improvement is claimed.

The CPU region remains opt-in and headless. Full-session persistence, windowed
tool ingress, GPU command/readback handling and shared region boundaries remain
separate gates. The finite Observatory route and later architecture milestones
remain open in the execution ledger.
