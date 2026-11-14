# 0011 Rigid bodies stay on the CPU; GPU collider extraction deferred

Date: 2026-10-22. Status: accepted.

The mission asked for voxel-to-collider extraction on the GPU. Today's rigid bodies are capped at 64 bodies × 128 voxels: a flood fill from `RigidCore` over planks and logs, voxelised back into the grid each substep. That is at most 8,192 voxels, which the CPU handles inside its 0.6 ms budget. A GPU connected-components pass plus readback would add a kernel, a CPU reference and a sync point for no measured gain.

Rigid bodies read and write the cellular world through the 2-tick mirror and the edit-command queue (ADR 0009), like every other gameplay system. They use floats and are outside the cross-GPU determinism guarantee (ADR 0008).

Revisit when a scenario needs more than the 64 × 128 cap, with a bench showing the CPU pass over budget.

## Physics and destruction update, 2026-11-05

Ordinary assemblies still use the 64-body / 128-cell limits; felled trees have
their existing separate 4,096-cell limit. Metal storage blocks now join cores,
planks and logs as structural assembly materials. This remains a grid-backed
physics foundation, with no dedicated aircraft propulsion or control system.

Bodies use density-weighted mass centres and a full inertia tensor. Contact
impulses include angular velocity, friction and low-speed inelastic response.
Floor contacts use a support centroid. Supported, calm contraptions sleep while
retaining their identity, and recheck support and nearby water before staying
asleep. Radial pressure acts on individual cells and produces off-centre torque.
External edits retain surviving material and split disconnected components;
fragments inherit the parent's linear and angular point velocity. Excess pieces
remain ordinary terrain when the body cap is reached.

Integer-shear rotations avoid merging occupied cells. Rotation trigonometry is
shared across a substep. Sweep subdivision includes angular tip speed and caps
elapsed time rather than only capping the number of subdivisions. Forces are
integrated per subdivision; deferred moves retain their remaining time debt.

The motion loop reuses scratch buffers and occupancy sets, skips residency
preparation when occupancy is unchanged, and writes only changed material cells.
Thermal capacity admission has a constant-time fast path and a linear fallback.
TNT launches a deterministic sparse share of ordinary rubble, retains its mass
instead of also dropping loot, and leaves sixteen body slots for assemblies.
Crater debris gets spin and deterministic nearest-first admission.

Validation: isolated Rust compilation includes the actual four changed physics
and destruction modules with real content/core dependencies and stub world APIs
(`artifacts/rigid-compile`). This is type checking only. Full app compilation was
blocked by unrelated sky WGSL field-access errors and world-generation references
to absent Hellstone / HELL_PORTAL_GENERATOR definitions. No tests or hosted CI
were run. Runtime physics, save/load compatibility and performance gains remain
unverified. Large contraptions still need moving-body render meshes and collider
broadphase work to avoid terrain-remeshing cost; this update does not remove that
architectural limit.
