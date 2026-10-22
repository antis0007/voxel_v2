# 0011 Rigid bodies stay on the CPU; GPU collider extraction deferred

Date: 2026-10-22. Status: accepted.

The mission asked for voxel-to-collider extraction on the GPU. Today's rigid bodies are capped at 64 bodies × 128 voxels: a flood fill from `RigidCore` over planks and logs, voxelised back into the grid each substep. That is at most 8,192 voxels, which the CPU handles inside its 0.6 ms budget. A GPU connected-components pass plus readback would add a kernel, a CPU reference and a sync point for no measured gain.

Rigid bodies read and write the cellular world through the 2-tick mirror and the edit-command queue (ADR 0009), like every other gameplay system. They use floats and are outside the cross-GPU determinism guarantee (ADR 0008).

Revisit when a scenario needs more than the 64 × 128 cap, with a bench showing the CPU pass over budget.
