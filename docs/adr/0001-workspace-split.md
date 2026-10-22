# 0001 Workspace split by separability, not by the target list

Date: 2026-10-22. Status: accepted.

The plan asked for `physics`, `automation` and `adventure` crates. In the code, `World` owns their state and they take `&mut World` (18, 1 and 88 functions), so peeling them is a logic rewrite, not a move. We split what moves cleanly: `engine_core`, `engine_content`, `engine_audio`, `engine_voxel`, `engine_worldgen` (15k lines, zero `World` coupling). The game binary at `crates/app` keeps the `World` hub, renderer and UI.

Consequence: the remaining crates (`sim`, `render`, `ui`, thin `app`) wait on decoupling `World`, tracked in `docs/PLAN.md`. Moves were done with named re-exports so call sites did not change.
