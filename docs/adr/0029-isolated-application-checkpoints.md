# ADR 0029: Isolated application checkpoints

2026-10-26. Accepted for the opt-in headless cellular development path.

## Decision

`--ca-session-out=<directory>` writes a coherent application cut alongside its
region store. `--ca-session-in=<directory>` restores that cut before executing
the continuation document. Initial activation still requires
`--ca-cpu-region=x,y,z`; restored sessions carry their own origin. The document
must match the saved seed, world type, scale and generator version. The optional
`--ca-backend=gpu` reconstructs residency from the committed cellular cache.
Backend choice remains explicit on each invocation.

The VXCASE01 envelope contains the existing versioned world manifest, an exact
VXCA0002 region checkpoint, and bounded RON actor bytes. The actor section keeps
all Player, Input and DayNightCycle fields, including precise coordinates,
movement cooldowns, input edges and viewmodel springs. Serde support for glam
is enabled in the application dependency; no new package is introduced.
Actor decoding has a recursion limit and rejects unknown fields, invalid
numbers and unsupported ranges. Source/model fingerprints reject incompatible
continuations. This development format has no cross-build migration promise.

The world section retains ordinary inventory, systems, activity and paged edit
records under the existing save contract. Unmigrated gameplay and legacy physics
remain suspended while cellular authority is active. This does not establish
exact continuation for an arbitrary live legacy game or its renderer/workers.
Only the committed cellular cut is saved; speculative GPU work is reconstructed
from the retained commands. No GPU handles or asynchronous callbacks are stored.

## Publication and retention

All sections share a commit ID and outer checksum. The embedded world and region
sections also retain their checksums. Limits are 64 MiB for the envelope,
160 KiB for cellular state and 64 KiB for actor state. A mismatched world/mirror
cut or failed authority cannot be saved. World edit capture may defer when its
existing memory admission fails; the command returns an error rather than
publishing an incomplete checkpoint.

Region segments are synced before the immutable commit. Publication uses the
existing synced temporary-file/rename writer and a directory checkpoint lock.
The fixed slots are `simulation.vxs` and `simulation.prev.vxs`; failed primary
loads try the previous slot with a recovery notice. There is no procedural
fallback on failed restoration. Three immutable commit manifests are retained.
Region reclamation recognizes the new envelope and both discovery slots, so
their segment references remain live even when old immutable manifests expire.

The directory and its `regions/` contents form the save. Moving only the `.vxs`
file is insufficient. Continuing and saving must use the same directory as the
loaded region store. Ordinary window saves keep their existing format/version
and continue refusing an active experimental mirror. No generator change is made.

Import now rejects legacy fractional water that cannot be represented exactly
in the cellular 0..64 volume field; coarse eighths cannot silently erase it.

## Evidence and remaining gates

Local offline application compilation and the debug application build passed.
No tests, checkpoint scenarios or
runtime round-trip comparisons ran under the standing no-tests instruction.
Runtime continuation, interruption/recovery and CPU/GPU equivalence remain open.
The native renderer baseline predates this persistence increment. Windowed
activation, complete effect ingress, boundaries shared with legacy authority and
the finite Observatory route remain separate milestones.
