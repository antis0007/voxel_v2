# Large-world sandbox audit

This is a source-level audit of the current shared checkout. The target is a persistent, editable voxel world with local cellular physics and distant visual representations. Neither a finite voxel renderer nor a heightfield horizon is a full-world physics engine. There is no single optimal configuration independent of memory, simulation rate and fidelity.

## Implementation scope

All seven concrete improvements below are implemented in this pass. The near simulation lane also prioritizes proximity, and player-blocked granular motion retains a wakeup until the player moves away.

| Flaw | Consequence | Improvement |
| --- | --- | --- |
| Aerial perspective cache depends on inverse view projection | Turning or moving horizontally triggers an unnecessary 32x32x24 atmospheric solve | Key the direction-space table by physical altitude, effective distance domain and lighting |
| Cellular scheduler consumes at most one interval per render update | Low frame rates slow material flow even when CPU budget remains | Bounded catch-up ticks sharing one wall-time budget |
| Liquid recipients can execute later in the same tick | Flow speed depends on traversal order and chunk scheduling | Mark transferred-to cells visited for the tick |
| Failed liquid preparation consumes the active bit | Writeback pressure can leave water asleep indefinitely | Re-wake the source on unavailable data or admission failure |
| Sand/gravel only move vertically | Unsupported vertical piles never settle into slopes | Conservative diagonal descent with side clearance, player exclusion and water displacement |
| Wake and source-discovery stencils only cover vertical granular motion | New diagonal rules would stop at chunk or sleeping-region boundaries | Share the dependency wake stencil and update both discovery paths |
| Terrain discovery ignores water-level imbalance | Reloading or revealing partial water can miss active fronts | Seed partial-volume differences as well as empty neighbours |

## Architectural limits and next design decisions

- **Authority:** voxel edits and region records must remain authoritative. Distant terrain is a visual approximation, never collision or simulation truth. Coarse representations need versioned edit invalidation, including caves and elevated construction; a heightfield cannot represent all of these.
- **Scale:** integer chunk coordinates plus chunk-local floating positions are needed for genuinely large worlds. Current absolute floating GPU/player positions lose sub-voxel precision far from origin. A coordinated camera, mesh, shadow, picking, physics and save migration is required; changing only the shader would break their agreement.
- **Physics:** current cellular rules provide quantized water and granular motion, not incompressible fluid dynamics, heat, combustion, gas pressure, fracture or rigid-body islands. Each added conserved quantity needs storage, exchange rules, wake dependencies and checkpoint migration. Rendering fire alone would not provide Noita-like physics.
- **Time:** bounded active-cell scheduling preserves responsiveness but cannot guarantee all distant cells advance at the same rate under overload. Catch-up is bounded; arbitrary full-world real-time simulation is not claimed. Deterministic replay additionally requires deterministic ordering and an explicit overload policy.
- **Memory:** render, source, edited-region, simulation and queued-result budgets must include in-flight overlap. Dirty state stays pinned under I/O failure. Simulation admission must defer and retry, never discard mass to meet a budget.
- **Atmosphere:** layer compositing remains an approximation; separated decks avoid ordering cuts but do not model physically interacting weather volumes. LUT precision, optical-depth filtering and temporal rejection need camera-motion and weather-transition measurements.
- **Rendering:** per-chunk draw submission, exact-surface LOD and remeshing are still scaling costs. Sparse coarse terrain, draw batching and material-space detail should be evaluated with actual CPU/GPU timings rather than increasing distance alone.

## Acceptance boundary

Local `cargo check --locked --offline --target-dir target` passed, including composed WGSL validation. No tests or hosted CI requested. Source invariants are not proof of conservation across save/reload, long-running overload behaviour, visual quality or FPS. Concurrent distant-terrain and indirect-lighting work remains owned by its existing chats.

Catch-up permits four cellular ticks per update, carries at most eight intervals of debt, and shares one 3 ms wall-time budget. The aerial cache allows less than 0.5 metre altitude reuse, consistent with the existing approximate atmosphere; turns and horizontal movement no longer trigger regeneration. No save schema changes were introduced.

Release build succeeded using an explicit output path to avoid the running default executable. Launch with run-sandbox-improvements.cmd; candidate: target/sandbox-improvements/voxel-sandbox.exe. No runtime or physics acceptance claimed.
