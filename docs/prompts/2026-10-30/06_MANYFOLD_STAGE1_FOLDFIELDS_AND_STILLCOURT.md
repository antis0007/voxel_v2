# 06: Manyfold Stage 1, the Foldfields district and Stillcourt

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. Start
after prompts 01 and 02 land. Source of truth for design:
`docs/NEW/The_Manyfold_Design_and_Implementation_Brief.txt` sections 1, 3, 4.1,
5, 6, 14, 15.1, 15.2, 15.6, 16 (Stage 1), 17 and 18. Read them in full; this
prompt only plans the engineering.

## Goal (brief section 16, Stage 1)

One beautiful, persistent, inhabited Foldfields district plus the Stillcourt
refuge, as ordinary Euclidean space with ordinary gravity. Provide:
elevation changes and a real vista; useful ordinary excavation; two materially
different routes to one objective; one working shortcut home; persistent
markers and construction; a satisfying return. The first expedition (brief
section 14) is the acceptance script: roughly 30 to 45 minutes, not wandering
through equivalent corridors.

Approved names: Manyfold, Foldfields, Stillcourt (decision 0039 addendum
2026-10-30). Everything else in the brief (Clerk of Returning Things, Lantern
of Stolen Dawn, Bellwarden, Long Guest, Last Surveyor) stays unapproved:
use descriptive placeholders in code identifiers and no new names in player
text or RON rows.

## What already exists to build on (verify each, do not assume)

- `WorldType::Manyfold` with save tag 5 and a header extension (upright
  quaternion, weight position and velocity) in `world/storage.rs`. The tube is
  the current content: `engine_worldgen::manyfold` (`CENTER (0,36,0)`,
  `RADIUS 24`, `HALF_LENGTH 24`), districts in `manyfold/districts.rs`
  ("a cylindrical maze: angular neighbors wrap", `maze_wall(seed, arc, z)`),
  spawn via `engine_worldgen::manyfold::spawn(cfg)` called from `world.rs`
  (about line 1198).
- The brass weight (`app/src/manyfold.rs`, `Weight::default() = (1.5, 13, 0)`)
  with a socket at `engine_worldgen::manyfold::socket`, toss through the Use
  action, `opened` flag, chronicle entry 14. This is the brief's "route weight".
  Reuse it as the gatehouse objective instead of inventing a second object.
- Dungeon maze generator for style reference: `worldgen/src/dungeon/v2/maze.rs`
  (generator 72, "labyrinth of false walls").
- Terrain systems: heightfield columns, sites and homesteads
  (`worldgen/src/sites.rs`, shared footprint caches, memory
  `worldgen-plan-cache-reentrancy`: cached plans must not query site, homestead,
  dungeon or undercrown caches while probing), geo planet solve (gate 71/73,
  dormant), biome weights (gate 66 dormant on `biome-p1`).
- Persistence patterns without a save bump: sidecars (`world/portals/sidecar.rs`,
  `world/farming/sidecar.rs`, `world/block_data/sidecar.rs`) and voxel edits.
- Map and markers: `egui_ui/map/*` (exploration map, memory
  `elevator-tracking-and-orbital-captures`, docs/design/EXPLORATION_MAP_IMPLEMENTATION_20261029.md)
  and the update-pack map prompt `Axiom_Underground_Exploration_Map_Implementation_Prompt.txt`.
- Distant terrain: horizon is a heightfield (`render/horizon.rs` only; cube band
  needs `VOXEL_HORIZON_CUBES=1`). The brief requires near and far to agree
  (15.6); a maze-ridge landform that is a heightfield plus wall voxels can use
  the existing horizon proxy for the landform and an aggregate proxy for walls.

## Decision to confirm before coding

Where does Stage 1 live? Recommendation recorded with the owner: extend
`WorldType::Manyfold` behind a new generator gate; the existing tube becomes the
House of Contrary Stairs prototype area (a later region). If the owner chose a
new world type instead, adapt the world-type plumbing and save tag. Do not
start until the answer is in `docs/TRACKING.md` or the task text.

## Design

### A. Gate and versioning
Claim the next free gate in `docs/worldgen/GENERATOR_GATES.md` before writing
code (after prompt 02 reconciles it; do not assume 90). Dormant first: the new
layout lives behind `pub const FOLDFIELDS_GENERATOR: u32 = N;` with
`CURRENT_GENERATOR_VERSION` unchanged until acceptance passes; then bump and
re-save baselines in one commit (`just bench-save`, golden hashes). Old Manyfold
worlds (generator below the gate) keep the tube exactly.

### B. Generate relationships before walls (brief 15.1)
Module `crates/worldgen/src/manyfold/foldfields/` (split files under 300 lines):
1. `graph.rs`: district route graph as a pure function of `WorldSampleConfig`:
   nodes (entrance balcony, Stillcourt, gatehouse, wall-top belvedere, lower
   gallery, diggable thin wall, veiled-shaft room placeholder), edges with a
   character tag (exposed-fast, sheltered-inhabited, flooded-profitable), at
   least one loop, one shortcut slot that starts closed. Seeded by positional
   hashing from the world seed (`engine_core::hash`); no `rand`.
2. `landform.rs`: height of the serpentine ridges: low-frequency ridge field
   plus the graph's walkable corridors carved as constraints (corridors
   reserve capsule clearance, gradient limit, stair space, bridge landings).
   Walkable routes first, walls fill around them.
3. `walls.rs`: pale serpentine masonry as voxels with courses, drainage slots,
   repairs and plants as palette-bound detail. Hollow wall interior lane for
   the signature discovery (homes, oven, shop) as an authored site embedded
   into the wall record (use existing site-embedding patterns).
4. `stillcourt.rs`: planted courtyard, communal water source, repair bench,
   warm inn, rooms in thick walls, visible maintained anchor monument. All
   placed from the graph node, no randomness that can place it inside rock.
5. `validate.rs`: test-time and debug-assert validator that walks the actual
   voxelised world with the player capsule (see `player/radial.rs` or the
   standard collider in `world/collision.rs`, `player_aabb`) from the entrance
   to the gatehouse by both routes and back via the shortcut, and fails if any
   edge is blocked. Validate voxels, not only the graph (brief 15.1).
- Worldgen stays pure: no runtime state, no `Instant`. Plans that are cached
  must not call other plan caches while probing (memory
  `worldgen-plan-cache-reentrancy`). Time-test with in-app timing (memory
  `thermal-climate-and-river-ice`: per-voxel worldgen needs in-app timing) and
  respect the load budget (memory `horizon-build-cost-hotspot`).

### C. Gameplay objects and persistence
- Gatehouse holds the brass weight; "route weight" objective: carry or toss it
  home to the Stillcourt socket (reuse `use_weight`, `opened`).
- Installing the weight sets a shortcut open. Persistence without a save bump:
  an edit to a designated socket voxel and a sidecar `manyfold-district.ron`
  with route states and marker ids, written with the temp-then-rename helper
  (see prompt 03 for failure handling). Old saves without the sidecar load as
  "closed".
- Player markers: use the existing map marker data (verify the type in
  `egui_ui/map`); they survive reload and are never rewritten by in-world
  deceivers (brief section 6).
- Recovery and return (brief 5.1): an always-available labelled emergency
  return to the last validated refuge, with consequence shown before confirm;
  recovery caches keep one persistent inventory identity. Keep this minimal in
  Stage 1: the refuge permanent connection outside the conditional graph.
- Multiplayer: positions in region-local coordinates; chunk delivery is
  generator-versioned and pristine chunks regenerate (`netcode/src/chunk_stream.rs`
  `delivery(version, pristine_hash, cached)`). Do not add per-district network
  state beyond sidecar replication; run the local lobby E2E recipe in memory
  `netcode-crate-and-multiplayer-ownership` if you touch replication.

### D. Look (brief section 3) and captures
Pale walls, blue ravines, one conspicuous green belfry, a bronze aqueduct,
a warm window. Colours come from `assets/pixel_palette.txt`; materials from
`assets/materials.ron` (add rows; run the `dump_*_ron` test if the format
changes). Distant structure: the belfry and aqueduct must be visible from the
entrance balcony (vista acceptance). Smooth world shading, no posterisation
(memory `feedback-no-posterization`). Review captures use the existing recipes
(`worldgen-ab-same-exe`, `biome-capture-poses`): define named poses for
entrance balcony, eye-level corridor, Stillcourt, wall-top route, lower
gallery, diggable wall, gatehouse. Before/after labels under `target/`.

### E. Performance
Large scale map: use `Budget` for any bounded pass; no unbounded loops in
generation. Wall detail instances aggregated for the horizon (brief 15.6); do
not push every wall as separate far geometry. Run `just bench` and the frame
bench; compare with `docs/perf` baselines; report p95 load and frame time.

## Acceptance (brief section 17, adapted)

- A new player reaches the gatehouse by two materially different routes, can dig
  the thin wall, installs the weight, and the shortcut home works; all state
  survives save, quit and reload.
- Near and far agree on walls, bridges and elevations in a captured route.
- The validator passes for 20 seeds and 3 voxel scales (1 m, 0.5 m, 0.25 m).
- Old Manyfold worlds still generate the tube byte-identically (golden hashes).
- Not claimed without evidence: enjoyment. A continuous captured playthrough
  (outward, discovery, return) is required; unit tests do not substitute.

## Do not

- No new proper nouns. No dragon, relic, illusions or fold stair here (prompts
  07, 08 and later stages). No `rand`. No `Instant` in simulation.
