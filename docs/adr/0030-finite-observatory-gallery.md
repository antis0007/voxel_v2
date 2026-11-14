# ADR 0030: Finite observatory gallery

2026-10-26. Accepted for generator 60 new worlds.

## Decision

Reuse the existing Clifftop observatory, lens mechanism and archive reward for
O1. Add a bounded pool and lower receiving basin through a versioned sampler
overlay, preserving the dome and surrounding authored geometry. Historical
generator versions stay on their original paths; existing worlds are not edited.

The two routes use ordinary world authority. Excavating the stone plug exposes
the pool to the lower outlet. A powered pump can instead extract from its bottom
layer into finite machine storage, with pipes/outlets delivering to a destination.
The same stored water supplies both routes. Filling or blocking the destination
can obstruct flow; no site-specific code removes or replenishes water.

Four actual water observations beside the lens determine whether its mount is
ready. The threshold admits a shallow eighth-cell film rather than requiring
the legacy lateral solver to remove its final residual packet. The existing
restore action opens the archive and persists through normal mechanism records.
No saved route-completion switch, new loot table or save-format field is added.

Generation details: [generator 60](../../crates/worldgen/GENERATOR_60.md).
This is legacy-authority gameplay integration, separate from the isolated CA
adapter's window cutover. Portable transfer and the environmental guardian are
still separate O2/O3 increments.

## Evidence limits

Local offline compilation and build passed. A room-name camera initially landed inside the
old observatory wall; that frame provides no useful layout acceptance. A dedicated
camera in the dry aisle supplies a consistent view across versions. Both native
frames were inspected: generator 59 retains its dry chamber and generator 60
shows the contained pool and pedestal inside the same dome. The captures are
`target/place-review/axiom-observatory-dry-aisle-g59-001/camera.png` and
`target/place-review/axiom-observatory-dry-aisle-g60-001/camera.png`.
Both use seed 424242, one-metre cells and the frozen executable
`target/axiom-20261026/observatory-build/observatory.exe`, SHA-256
`54C9AD84645E83C4B666AFA24CF521B2C03C2049091F48BB1BF7C4E7580E285E`.
The subsequent contact correction samples the bottom cell at each voxel scale;
it changes interaction admission only, not captured geometry.
No automated tests or scenario runners were added or run. Runtime excavation,
powered transfer, complete volume accounting, streaming/reload and the reward
route remain unverified until observed in normal play.
