# Manyfold tube network and vines overhaul

Status: proposed 2026-11-03, waiting for owner approval. Audit evidence comes from the code read on 2026-11-03 (gaps listed at the end).

## Decisions taken

- Keep every existing Manyfold gate (80-120) generating identically. The new generator is one new gate, provisionally **130** (`manyfold::TUBES`). It must be claimed in `GENERATOR_GATES.md` before code lands.
- Shape: a network of twisting tubes with radial gravity. The Forest Temple twist is the reference for feel. The Maze of Ith labyrinth is the reference for walls on the inner surface.
- Order of work: this spec first. Then stage T1 of the Manyfold and stage V1 of vines can proceed in parallel.

## Manyfold: target experience

You walk the inside of a corridor-sized-to-district-sized tube. The floor curves up on both sides into walls of pale stone maze. Look up and, across the open bore, other streets hang upside down. The tube bends, corkscrews and branches. Walking straight along a twisting tube rolls the world around you without the player ever feeling a wrong "down". Tubes swell into districts and pinch into passages. Some places change, but only by announced mechanisms such as a counterweight street closing or a bell that turns an arch. Changes never happen because a player looked away.

## Manyfold: structure (all pure functions of `WorldSampleConfig`)

1. **Skeleton graph** (`manyfold/tubes/graph.rs`). A seeded graph of nodes (districts, junctions) joined by edges (tubes). It has at least two independent loops, so it is not a tree. Each node has a radius, a district kind and a landmark. Each edge is a centreline curve: a cubic or helical segment with radius along its length and twist (roll of local frame) along its length. The graph is cached once per seed.
2. **Field** (`tubes/field.rs`). For a voxel: nearest centreline point, then local frame (tangent, rotation-minimising normal plus authored twist), then radial distance `r` and angle `θ` in the tube's own frame. Gravity is `g_wall · r / R(s)` outward along the radial direction, as the shell does today. Twist only rotates which θ is "street" versus "wall". Gravity stays radial, so twist is never felt as tilt.
3. **Landform** (`tubes/landform.rs`). The inner surface undulates in two ways. The radius is modulated by low-frequency noise in (s, θ): ridges, troughs, ravines exposing older layers. A per-district swell changes the profile from passage to district.
4. **Maze walls** (`tubes/walls.rs`). An unwrapped (s, θ) route graph per district: corridors plus courts with loops, two route characters (fast/exposed and slow/sheltered). It wraps around θ. Walls are 6-14 m, pale plaster over stone, with wear and drainage. No ceilings by default, so the bore stays open and the sky is the far side. Roofs only in sheltered routes.
5. **Landmarks and light** (`tubes/landmarks.rs`). Each node has one visible identifying structure, made of existing materials only. Lamps are in every district kind. A faint emissive core strand runs along each tube axis, so every tube has a light source independent of the blue fill.
6. **Validation** (`tubes/validate.rs`, tests). A capsule walker over 20 seeds proves each node is reachable from spawn, every edge is walkable, and the spawn-to-weight route closes a loop.
7. **Mechanisms** (later stage). Announced shared events, stored in a sidecar: closing street, arch whose destination changes, fall that goes around. They apply as World edits through the change journal, never as regeneration.

## Manyfold: plug-in points

Gate 130 takes over each dispatcher in `crates/worldgen/src/manyfold.rs` at its top: `block`, `column`, `spawn`, `socket`, `shortcut_cells`, `acceleration_for`, `axis_y`, `column_shell_bands`, `vertical_extent`, `contains_weight`, `cradle`, `encounters`, `guide_lights`, `curve::shell_curve`. `axis_y` and `column_shell_bands` assume one straight axis. For a curved network they need a new contract: a per-column list of bands taken from the graph's bounding tubes. That contract change is the main renderer and streaming work. The player capsule controller (`player/radial.rs`) takes `acceleration_for` and needs no change beyond the up vector it already reads. `maze_portal` mouths move to graph nodes.

## Manyfold: stages

| Stage | Content | Acceptance |
|---|---|---|
| T1 | Graph, field, landform, a single district kind, walls, spawn, weight cradle; dormant behind 130 | Capsule validator passes on 20 seeds; capture from 4 poses shows open bore and far streets |
| T2 | Streaming bands and far view for curved tubes, core light, lamps per district | Frame bench within 10% of gate-120 Manyfold at the same pose; no black regions |
| T3 | Four district kinds, landmarks, two route characters | Owner review of captures |
| T4 | Illusions as physical blocks (painted abyss, false wall, veiled shaft) | Each illusion's physics matches its appearance once revealed |
| T5 | Stair fold beneath its own entrance via the portal pattern | Round trip with no gravity snap |
| T6 | Announced mechanisms (closing street, bell arch), sidecar, multiplayer events | Two-client test agrees on topology after each event |
| Bump | `CURRENT_GENERATOR_VERSION` to 130 | Owner sign-off |

Bug to fix in the current tree, independent of the stages: at gate 120, `inhabit::life` places stands on inhabit's 216×5 m grid while the walls use bend's 80×7 m grid. Fixing it needs its own small gate, or a guard under 130 only.

## Manyfold: enemies

Owner, 2026-11-03: Manyfold enemies "can't even do anything to the player". Root-cause trace in progress; the fix lands before T1 because it affects every gate.

Design for gate 130 (horror brief: no sanity meter, no random input loss; threats are physical):

| Threat | Behaviour | Uses the tube |
|---|---|---|
| Wall-walker | Follows the inner surface in local up and crosses walls instead of pathing around them | Drops onto the player from the far side of the bore along the radial field |
| Bore drifter | Flies along the tube axis where gravity is near zero; dives outward to strike | Low gravity on the axis |
| Mimic street | A false-wall section (illusion vocabulary) that closes on a player who lingers; its seam is visible | Ties to T4 |
| Keeper | Guardian per district landmark; melee plus a radial slam that throws the player toward the far wall | District objective |

Contract: every enemy reads local up from `acceleration_for`. Distance, reach, aim and line of sight use 3D vectors, never world-Y heights or flat XZ distance. A headless test spawns each kind beside the player on the shell at 4 angles around the tube and asserts the player takes damage within N ticks.

## Vines

| Stage | Content |
|---|---|
| V1 look | Mesh in `thin.rs`: tapered strands with a hashed sway offset that join across vertically stacked vine cells (top cell anchors, bottom cell tips). Leaf nodes as small offset boxes. Two-tone colour from position hash, with the colour pattern in `materials.ron`. Before/after captures. |
| V2 placement (new gate, shared with 130 or its own) | One helper `vine_for_wall(support_dir)` used by Manyfold, karst and ruins. Placement requires a solid support. Ruin strands become real `Vines`. Lengths follow anchor-to-tip with taper probability. |
| V3 hanging | `thin::attach` accepts a ceiling and a vine below a vine. Removing support pops the hanging run. |
| V4 growth | Budgeted random tick: a supported vine extends one cell down with low probability. Optional spread onto neighbouring wall faces. Positional hash, so deterministic. |
| V5 use | Shears drop vines. Vines become fibre, and fibre becomes rope (rows in `recipes.ron`). Climb on vines is slightly slower than on a ladder, and the player can hang still. |

Duplication: the four `materials.ron` rows stay (they are block ids), but a test asserts they match in every field except orientation.

## Open owner questions

1. Is gate 130 acceptable, or should it wait for a gate registry clean-up?
2. Should districts be named? Only Manyfold, Foldfields and Stillcourt are approved names.
3. Should vine growth run in every world (survival impact on builds) or only in the wild?
