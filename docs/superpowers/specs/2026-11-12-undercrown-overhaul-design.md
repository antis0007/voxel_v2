# Undercrown and dungeon overhaul — design

Status: proposed 2026-11-12. Awaiting owner approval. No code written.

## Why

The owner playtested the Undercrown and generated dungeons in November 2026. Two screenshots show what they found:

1. **Invisible interactables.** "[F] Claim" and "[F] Repair" prompts float over empty air.
   - No prop kind has a model.
   - Generators 19–31 stamp nothing.
   - Generator 32 and later stamp the same 1.2 m stone cube for every kind, and its top sits 0.6 m below the point the focus ray aims at.
   - Code: `v2/layout.rs:38-46`, `v2/feature.rs:236`, `app/src/world/undercrown/interact.rs:94`.
   - A claim grants an item that was never visible in the world (`rewards.rs`).
2. **Boring and repetitive.**
   - Rooms are boxes at fixed offsets.
   - Every tunnel is 3 m × 3.4 m.
   - Houses repeat on a 7 m bay.
   - Columns are always square and banded.
   - Caverns are smooth noise shells with stepped moiré banding.
   - One tower is a plain cylinder with punched square holes.
   - Torches sit on walls but the rooms still read as murky.
   - Every seed gets the same Undercrown.
3. **Bad framerate underground.** This is suspected, not measured. Candidates:
   - the torch occlusion grid restamping at half scale,
   - torch and shadow count in lit halls,
   - water wake after mechanisms.

The vision documents already describe something much better. The code has not caught up:
- `docs/design/WORLD_LORE.md`: the Anchor, the Ballast, the Surveyors, depth is age, every ruin had a job.
- `docs/specs/downloads-2026-10-24/Undercrown underground labyrinth design.md`: districts linked by physical dependencies, Xandering loops, reachable cavern layers, a construction language per culture.
- `docs/NEW/UNDERKINGDOM_COMBINED_SPEC.md` §12 (PlacePlan build order, "no dead unlit empty rooms") and its art section.
- The reference sheet `assets/art_direction/anchor_expansion_20261026/maintenance_hall.png`: braided cable and counterweight in a blue shaft, brass rails, ceramic casings, violet crystal, amber floor light, iron grating.

Goal: better than any Minecraft structure mod. The bar is When Dungeons Arise for silhouette, YUNG's mods for palette and decay, Trial Chambers for readable rewards, and the Ancient City for one landmark. On top of that bar we add what those mods lack: a world history, mechanisms that change the map, and loops.

## Goals

- **G1. Every interactable is a visible, specific object.** Exception: a hidden secret, which must still show a clue. The prompt only appears when you aim at that object's voxels.
- **G2. Rewards are visible before they are claimed.** The item model sits on an emissive plinth or in a cage, and claiming it removes it from the world.
- **G3. Each culture builds differently.** The four construction languages are below.
- **G4. Every complex has one reveal moment and one landmark that can be seen from most of it.**
- **G5. The layout is a set of loops, not a tree.** Each mechanism opens a shortcut or changes the map, not only a door.
- **G6. Variety at three scales:**
  - macro: a layout graph per seed,
  - meso: room templates with substitution, rotation and mirroring,
  - micro: palette, decay and history passes.

  Two seeds must produce visibly different Undercrowns.
- **G7. Lighting does wayfinding.** Warm light marks the main path; cool light or darkness marks optional areas. Light leaking through cracks hints at a secret. No dead, unlit, empty room.
- **G8. Underground frame time is no worse than the surface at the same view distance.** This is proven with frames.csv captures, not reasoning.

## Non-goals

- NPC dialogue system (inhabitants come later).
- New proper nouns beyond the canon in WORLD_LORE. New names need owner approval. Engineers, Choristers and Founders are not used.
- Combat or AI rework.
- Changing what existing worlds generate.

## Anti-goals (failure even if it works)

- Bigger empty boxes. Scale with nothing in it is the current failure.
- Posterised or overexposed lighting. Shading must stay smooth and balanced.
- Floating greebles or Mekanism-style machine casings. The direction is authored voxel shapes.
- Any prompt that appears over empty air.
- Shipping visuals without A/B captures.

## Constraints

- **Generator gates.**
  - All generation changes go behind a new generator gate.
  - Claim the gate number in `docs/worldgen/GENERATOR_GATES.md` first, and grep every branch for it.
  - Dormant work names its gate with a `pub const`.
  - Old gates (19, 32, 52, 60/63, 72, 128…) must keep generating identically.
- **Save compatibility.**
  - Prop state stays keyed by `ObjectId` (district 4 bits / room 10 / prop 8).
  - New mechanisms and prop kinds use existing fields or modes where possible. A new saved resource needs a save version bump.
- **Content in RON.** Room templates, palettes and prop models go in `assets/*.ron` or `.vox` files, not in Rust tables.
- **Generation stays pure.** Worldgen is a pure function of `WorldSampleConfig` and uses positional seeded hashes. The plan is cached per region and stamped chunk-locally and idempotently.
- **UI text.** Player-visible text follows `egui_ui/UI_RULES.md`.

## Design

### 1. Props become objects (fixes the invisible Claim and Repair)

- **Models.** Each `PropKind` gets an authored voxel model from the `.vox` loader or a RON shape:
  - Plinth: emissive cap plus the reward item rendered on top.
  - Cradle: a freight cage hanging from a chain.
  - Winch: drum and crank.
  - SluiceGate: a gate leaf in a frame.
  - BellClapper: the clapper under a bell.
  - ObservatoryLens: lens ring and mount.
  - Marker: a plaque, or Ballast graffiti over the plaque.
  - Counterweight: a weight block on a chain.
- **Placement.** Models are stamped through the feature system at the new gate. The prop's anchor voxel is recorded in the plan.
- **Focus targeting.**
  - Gate 32 and later: focus aims at the model's bounding box centre and needs the ray to hit a model voxel.
  - Older gates: keep the current behaviour but hide the prompt when the anchor cell is air. This is an app-side guard; it changes no generation.
- **State changes show in voxels.**
  - Repaired: the cradle is re-hung and the chain becomes intact.
  - Claimed: the item is removed and the plinth light dims.
  - Operated: the lever is thrown.
  - These are visible state diffs through the existing mechanism `edits` path.
- **Reward display.** The plinth reward is a small item model drawn at the plinth (held_art / icon voxel) with a slow spin and a light. This is the Trial Chambers vault idea, done in this game's own art style.

### 2. Construction languages (from the design doc, made concrete)

Each one is a RON "kit": palette ramp, wall profile, column type, opening type, trim and signature greebles.

- **Anchor (Stratum II).**
  - Steel ribs, ceramic panels, brass rails, iron grating, violet crystal cards.
  - Offices around a machine: every building has a ledger room and a maintenance stair, and tolerance plaques sit next to machines.
  - Silhouette: tiered setbacks, catwalks over shafts.
  - Signature: braid cable plus counterweight in a vertical shaft, as in maintenance_hall.png.
- **Ballast (Stratum I).**
  - Walnut, parchment, rope, patched brick, fungal glow.
  - They live inside Anchor machinery: a kitchen in a pump hall, a bedroom in a freight cradle. Graffiti corrects the Anchor plaques.
  - Silhouette: lean-tos, hanging platforms, patchwork.
- **Surveyors (Stratum III).**
  - Fused basalt.
  - Perfect lines: kilometre-flat floors, exact shafts, repeated marks, no doors, no writing.
  - These are rare and unsettling, and used as contrast.
- **Root keepers / canal builders / archive builders / cliff masons.** The design doc's four cultures map to sub-kits of the above. They replace the current palette-only `Style` swap.

Shared micro-rules:
- 1-voxel insets at openings.
- A pilaster every N voxels, with N varied per kit.
- Trim bands at floor and ceiling lines.
- Stepped corbel arches.
- Tiered silhouettes: base wider than shaft wider than cap.
- No raw cylinders. The screenshot's tower becomes a plinth, a shaft with buttresses and balconies, and a crown.

### 3. History passes (layered past)

Per complex, roll an event list from the seed. Each event is a geometry pass applied in order:

- built,
- reused (Ballast moves in: partitions, lanterns, beds in machinery),
- flooded (water line, silt, a tide stain band),
- collapsed (rubble cones, a broken bridge, a fallen counterweight),
- overgrown (roots, fungus),
- looted (emptied niches).

This replaces the single `Damage` value. Each pass is deterministic and chunk-local.

### 4. Layout: loops, reveal, landmark

- **Region graph.** Each region builds a mission graph first: start, then teach room, reveal, hub, then 2–3 spokes, each ending in a mechanism that opens a shortcut back to the hub, then the deep goal and the exit. The structure is cyclic (Dormans): at least two loops per complex, and one route that only opens from the far side.
- **Embedding.** The graph is placed into space by typed-connector jigsaw from RON room templates, with:
  - pool caps,
  - no identical template next to itself,
  - terminator set-pieces instead of bare dead ends,
  - DCSS-style glyph substitution plus rotation and mirroring.
- **Fixed story beats.** The authored Undercrown beats stay as unique-once set-pieces at fixed graph positions: well, counterweight teaching room, Crown Chasm bell reveal, bridge village, Drowned Borough, observatory exit. Seeds vary the connective tissue around them.
- **Reveal.**
  - One compression-then-release moment per complex: a narrow descent opening onto a huge cavern with a visible far goal. In the Undercrown this is the Crown Chasm with the house-sized bell.
  - The bell's top, its underside and its interior are all reachable.
- **Reachable cavern layers.** Floor, wall ledges, suspended structures and a roof route. Cavern shells get strata bands, ledges and outcrops instead of smooth noise surfaces, which also fixes the moiré banding.
- **Secrets.** One or two per complex, off the main path, each with a clue: a crack, light leaking, a draught sound, or an asymmetry in a symmetric room.

### 5. Lighting and palette

- Hue-shifted ramps of 3–6 values per kit. Fixes the uniformly yellow stone.
- Warm lantern pools on the main path, cool bounce light in caves, emissive crystal for landmarks.
- Light count per chunk is capped, and lamps are `is_light_source` blocks.
- Shading must stay smooth: no posterisation, no overexposure.

### 6. Performance

- Plan is cached per region. Stamping is chunk-local and idempotent, with positional hashes for decoration rolls.
- Light cap per chunk, plus a greeble budget per chunk so greedy meshing still merges.
- Before any optimisation, measure:
  - Capture `VOXEL_REVIEW_DUNGEON=undercrown:<room>` with frames.csv: draws, triangles, torch count, occlusion upload time.
  - Fix what the capture shows. Suspects: torch occlusion restamping at half scale, shadow work per light, water wake.

### 7. Room roles and pacing

Every room template carries one role:

| Role | Job | Rule |
|---|---|---|
| Teach | introduce one mechanism safely | early on the path, next to its first real use |
| Traverse | vertical climb, ledges, catwalks | at least one per spoke; never two flat corridors in a row |
| Puzzle | a mechanism with two solution categories | the result is visible from the hub |
| Arena | an encounter in elevated, readable space | exits sealed only by the mechanism, never permanently |
| Rest | lit Ballast camp, water, a bench | after each arena or deep descent |
| Vault | visible reward behind a lock | the key or mechanism sits elsewhere on a loop |
| Secret | optional reward | clue required; off the main path |
| Vista | reveal, or a window onto the goal | once per complex (reveal), plus 2–3 small windows |

Pacing:
- A reward beat every 2–3 rooms. Minor rewards are side niches, mid rewards sit at the end of a loop, major rewards come after a set-piece.
- No more than 25 m of path without a landmark, a light change or a role change.

### 8. District set-pieces (canon names only; new names need approval)

Each district gets: one signature mechanism that changes the map, one landmark, and its kit.

| District | Kit | Landmark | Signature mechanism (changes map) |
|---|---|---|---|
| Rootbound Halls | Ballast in Anchor | counterweight hall: weight on chain in a lit shaft | counterweight drops and raises the broken stair (teaches the loop) |
| Crown Chasm | cliff masons, Anchor | house-sized bell; colossal broken bridge | ringing the bell restores the Listening Stair; freight cradle repair makes a lift loop back to the village |
| Drowned Borough | canal builders | Street Beneath the Air Bell | sluice lowers the water level a storey at a time, exposing streets and the courthouse (Zelda water dungeon) |
| Lantern Gardens | root keepers, Ballast | orchard mouth glow canopy | irrigation reroute grows root bridges |
| Ashen Foundry | Anchor industrial | furnace mouth | cooling the lava channel opens a crossing; repaired machines become usable infrastructure |
| Glass Archive | archive builders | crystal stacks, light galleries | lens and shutters route light to open resonant bridges |
| Frozen Choir | Anchor rime | choir antechamber | resonance shatters rime walls |
| Hollow Sea | reserved | — | out of scope for this pass |

All eight districts already exist in the schema under these names. Room counts grow toward the design doc's 8 rooms per district, in slice order.

### 9. Acceptance criteria

These are checked per slice:

- **Props.** On the review seed, every prop in the plan has at least one non-air model voxel, and focus fails when you aim at air. Test: iterate `plan.props` and assert a model hit.
- **Variety.** Over 10 seeds, the Undercrown layout graph and room-template sequence differ, with fixed beats excepted. Test: hash the template sequence per seed and require them to be distinct.
- **Loops.** Every complex graph has at least 2 independent cycles, and each mechanism opens an edge on a cycle. Test on the graph.
- **No dead rooms.** Every room has at least one light source, and at least one prop or set-piece or a "traverse" role. Test.
- **Old gates.** Old generator versions still generate byte-identically. Existing fixtures plus `just test migrate` / `just test save`.
- **Visuals.** A/B captures at fixed `VOXEL_REVIEW_DUNGEON=undercrown:<room>` poses: well, counterweight hall, chasm reveal, bridge village, borough street, plus the screenshot tower room. The owner approves the looks.
- **Performance.** frames.csv in the same rooms: draws, triangles, torch count and p95 frame time no worse than baseline, with A/B passes interleaved and the best taken per side.

### 10. Gate

- Claim the next free generator number (289 is the highest seen in GENERATOR_GATES.md on 2026-11-12; recheck all branches) as `UNDERCROWN_V3_GENERATOR` before code.
- Slices 1, 3 and 4 can share one dormant gate until it is bumped. Slice 5 may need its own gate if it lands later.

### 11. Settled decisions (builder check, 2026-11-12; marked calls are Claude's, owner may override)

- **Gates (Claude's call).**
  - Slice 1's old-gate prompt guard is app-only. It ships immediately with no gate.
  - Slice 1's models and reward display, together with slices 3 and 4, share one dormant gate, `UNDERCROWN_V3_GENERATOR`. It stays dormant until slice 4 is approved, then gets bumped.
  - Slice 5 claims its own later gate.
  - Each slice is still "shippable" in the sense that it is merged dormant and reviewable through the generator-version override.
- **Uniqueness (from code).** The Undercrown remains one site per world (best hill 220–480 m from spawn). Its graph is built once per world and cached with the plan. Generated dungeons (`dungeon/v2`) remain per-region and are the repeating content.
- **Reward choice (Claude's call).** From the new gate onward, a plinth's item is chosen in the plan (pure, seeded by `ObjectId`) so it can be displayed. Old gates keep the current claim-time behaviour, with no display beyond the guard.
- **Culture to kit (Claude's call).**
  - Canal builders and cliff masons: Anchor sub-kits.
  - Archive builders: Anchor (glass, crystal).
  - Root keepers: Ballast.
  - Surveyor work appears only as side pockets or as a deep exit shaft, never as a loop hub.
- **Performance target (Claude's call).**
  - Hardware: the 4090 at the current default view distance.
  - Metric: p95 frame time and draw/triangle counts in the review rooms, held at or below the pre-change baseline, which is captured first in slice 2.
  - The iGPU is checked once in slice 2 and reported, but it does not gate.
- **Plan scope (Claude's call).** The implementation plan covers slices 1–2 in detail. Slices 3–6 are outlined and planned separately once 1–2 have landed.

- **Art (owner, 2026-11-12).** Claude authors the voxel models and kits from the reference sheets. The owner approves looks per slice from A/B captures.
- **Lore text (owner, 2026-11-12).** Graffiti, plaques and labels may be written freely using existing canon terms, with no approval step. A new proper noun still needs approval.

## Delivery slices (each shippable, each behind the gate)

1. **Props visible.** App-side guard for the prompt on old gates, prop models plus reward display at the new gate, and focus that requires a hit on the model. Fixes the reported bug.
2. **Framerate.** Profile one Undercrown room, fix the top cost, compare captures.
3. **Construction kits plus micro-rules.** Anchor and Ballast first, applied to existing rooms, plus the tower and cavern-shell rework.
4. **History passes.**
5. **Region graph plus jigsaw with loops.** The largest slice. The fixed beats stay.
6. **Random dungeons (`dungeon/v2`).** Adopt the kits, history passes and jigsaw.

Each visual slice ships with A/B captures from fixed poses.

## Left to the builder

- Exact template format (glyph grid vs. box list).
- Model dimensions.
- Greeble probabilities.
- Graph sizes.
- Which `.vox` assets to author first.
