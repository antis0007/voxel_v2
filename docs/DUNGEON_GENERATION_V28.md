# Dungeon generation, version 28

This increment develops the existing dungeon family against the revised technical-world brief. It adds a reliable exploration loop and a layered deepest-vault hall. It does not introduce another world representation, material catalogue or save object.

## Layout

The existing seeded 3x3 grid keeps its spanning tree and optional edges. Version 28 supplies one non-tree edge when the seed selected none. Loop endpoints count toward room degree before the leaf-elevation pass; they therefore stay at the shared level, so the later equal-height corridor filter cannot delete the exploration loop. The eight tree edges still connect all nine rooms.

The objective uses shortest travel distance in the generated room graph from the entry hub, including Z-shaped corridor legs, elevation and room-center approaches. It prefers the farthest ordinary terminal room, then an ordinary room on a loop. An all-puzzle layout falls back to its farthest terminal/loop room. This replaces straight-line objective placement without a runtime navigation solver. It does not prove voxel clearance or solve the intervening puzzle rooms. Planning remains bounded to nine rooms and cached with the complex.

## Deepest vault

The existing 30 by 28 metre hall becomes 16 metres tall, with a six-metre lower works and seven metres reserved below its original floor for the foundation. The central processional bridge and both entry/exit landings remain at the original level. Two supported side ramps lead down and back up, providing an alternate route through the lower works. Physical rails and piers frame the bridge. A seed-mirrored static diagonal supporting jib supplies a major asymmetric silhouette. Sparse pilasters and ceiling ribs replace the hall's generic furniture. Memorials flank the far landing while leaving its corridor clear.

Basalt carries the quiet mass; limestone carries pale load shoes and ribs; a few silver blocks mark the jib joint. Four upper and two lower torch positions use existing lighting. No shader effects or extra texture grids are added. The wider celadon/vermilion material direction remains future art work; existing limestone is not claimed to be a new ceramic material.

Every solid and opening comes from the normal authoritative block sampler, shared by collision, raycasts, edits and meshing. The bridge is permanently fixed architecture. The jib does not move. This is not an implemented counterweight/brake/bolt puzzle, protected tomb core or antimagic field. Those systems need their own authoritative mechanism contracts rather than decorative substitutes. Existing names, shrine/treasure rooms and encounter anchor coordinates remain intact.

Ramps voxelize at the selected world scale. At one-metre resolution they can require ordinary one-block jumps on ascent; half- and quarter-metre risers fit the existing 0.6-metre step allowance. No special invisible smooth collider is introduced.

## Compatibility and acceptance

All new generation is gated at version 28. Older generator versions retain their previous room geometry and selection rules. The generator version participates in existing dungeon/vault cache keys. No block IDs, saved machine/resource fields or storage schema are changed.

Current status: helper sources and minimal dungeon wiring are integrated, and match the reviewed candidate under `reviews/dungeon-generation-v28/candidate/`. After the habitat owner finished version 27, the coordinated global generator gate was advanced to 28. These additions are now the new-world source default. The renderer coordinator reports that the live integrated Cargo check passed in 7.07 seconds with these frozen dungeon sources and the gate still at 27. The final one-line 27-to-28 activation has not been rebuilt. No formal tests were added or run for this increment, and no dungeon native/visual review has run. Source hashes and the activation qualification are recorded in `reviews/dungeon-generation-v28/activation-source-receipt.json`.

Renderer benchmark/compiler ownership remains serialized through the renderer coordinator. Rebuild the activated game source and review the hall in motion at the supported voxel scales before claiming traversability, visual acceptance or performance. The earlier protocol executable `013FD81F...` does not include this dungeon increment; its failed world-switch run is separate evidence and leaves renderer acceptance open.

## Native review preparation

The current default generator is 29 for the subsequent habitat increment; the hall remains gated at 28 and must not downgrade that default. `src/dungeon_review.rs` provides opt-in cameras derived from the actual shaft surface height and the hall's original floor, including the seed-mirrored jib. Its `entry`, `reverse`, `lower`, `roof` and `motion` modes never alter blocks. Camera motion alone is not player collision/traversal proof. The helper awaits its minimal main camera hook and a coordinated rebuild.

`tools/Review-Dungeon.ps1` prepares isolated generated-world runs at 2560 by 1440, records executable hashes and process receipts, and requires a resident `camera.png`. Use separate labels at scales `1`, `0.5`, and `0.25`; `-Video` adds motion PNGs. These captures incur readback and are visual review evidence, not performance acceptance. Native execution is queued after the renderer coordinator's stale-job fix and reserved verification window. No dungeon run or visual acceptance is recorded yet.
