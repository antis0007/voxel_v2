# Teardown: destruction, sleep, debris, voxel ray marching
Source: Tuxedo Labs / Dennis Gustafsson. No GDC 2022 or 2023 talk found, so unverified. Confirmed: every voxel takes part in physics (TechRadar interview, https://www.techradar.com/gaming/consoles-pc/teardown-devs-top-priority-is-always-physics-the-game-can-come-later). Sub-stepping engine via 80.lv article (unverified). Sleep, debris-budget and ray-marching posts: unverified.
Adopt: destruction as voxel-data state, so breakage follows from the data rather than scripts; a budgeted, wake-driven pass over changed voxels.
Reject: a float rigid-body solver with sub-stepping for debris inside the deterministic sim; full-world per-frame sweeps.
Why: Debris as float bodies would break integer determinism, so we keep destruction as cell state with a sleep-aware budget that fits the iGPU target.
