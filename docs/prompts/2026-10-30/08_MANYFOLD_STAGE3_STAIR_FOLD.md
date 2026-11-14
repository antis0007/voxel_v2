# 08: Manyfold Stage 3, the stair that emerges beneath its own entrance

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. Start
after prompts 01 and 06. Design source: brief sections 4.4, 14 (First fold), 15.2,
15.3, 15.5, 16 (Stage 3), 17; the gap analysis
`docs/superpowers/specs/2026-10-29-portal-network-brief-gap-analysis.md`.

## Goal

One deliberately presented spatial fold in the Foldfields gatehouse: the stair
appears to rise but opens below a balcony the player visited earlier, with the
player's earlier wall mark still visible. Stable destination identity, actual
collision on both sides, safe loading, save/load continuity, supported entity
transfer. Local up is preserved (brief 15.5: translation and rotation about the
up axis only; no pitch or roll in the first slice).

## What exists (verified in code or docs; re-verify while reading)

- Portal engine, untracked until prompt 01 lands: `world/portals.rs`
  (`Portal { source, destination, entrance, exit }`, `Frame { center, rotation:
  DQuat, half_size }`, velocity mapping; it rejects transforms the
  fixed-gravity controller cannot traverse, around line 105), `portals/gates.rs`
  (player-built crystal-ring gates, `gates.ron` sidecar), `portals/traverse.rs`,
  `portals/views.rs` (bounded recursion: `MAX_TRAVERSALS = 4`, `MAX_VIEWS = 8`,
  `MAX_TARGET_PIXELS = 4_194_304`, clip plane about line 44, resolution halves
  per depth), `portals/realm.rs` (realm and hell-gate openings),
  `render/portals*`, `shaders/portal_composite.wgsl`.
- Crossing triggers on the camera so the view never sees behind the opening;
  speed and facing turn with the frame; a blocked exit stops the player at the
  opening (from the session summary; confirm in `traverse.rs`).
- Manyfold weave folds (`app/src/manyfold/weave.rs`): six districts, anchor and
  keystone cells, `linked()` pairs, frames from `layout::point(district, 0, 3.0,
  z)`. These are doorway folds, not stairs; this prompt adds a **static** fold
  authored by worldgen, not player-linked.
- Unit tests: `portals/gates_tests.rs` (9 tests passed on the session branch).

## Design

### A. Static fold record
Worldgen emits a fold record: `{ id, source_frame, destination_frame,
region_id }` with deterministic ids derived from seed and district id, placed by
the Foldfields graph (prompt 06) at the gatehouse stair. Put it in
`engine_worldgen::manyfold::foldfields::fold` and gate it with the Stage 1 gate
or its own (claim per the registry). The app converts records to `Portal`s
through a small adapter in `manyfold/fold.rs`, registered next to `gates`.
No player-linked state is needed, so no sidecar except the "discovered" flag
for the atlas.

### B. Geometry
Both ends are ordinary voxel geometry in ordinary chunks (no chunk-level
tricks). The stair top, at the gatehouse, has an aperture frame on the landing;
the destination aperture sits under the earlier balcony so that exiting places
the player below it. Both apertures are Euclidean-consistent individually; the
fold is the only impossible relation. Reserve swept volumes: no permanent
construction may occupy either aperture (brief 15.3); enforce through the
existing block-placement validation (find where placement checks reserved
volumes for gates, `world/portals/gates.rs`).

### C. Traversal and transfer
- Same transform used for rendering, collision, velocity, camera orientation
  and transferred entities (brief 15.5). Supported transfer set for the first
  slice: player, thrown items, the brass weight, projectiles that exist at the
  time; creatures excluded and blocked at the aperture (visible blocked state).
  Document the set in code comments at the fold adapter and in a short
  `docs/design/MANYFOLD_STAGE3_FOLD.md`.
- Atomic transfer ownership: a crossing is committed once on the host
  (`simulation::fixed`, host/solo authority per ADR 0011). No duplication or
  loss: the existing operation receipts (`netcode/src/operation.rs`) and
  `command_ack` already cover ordered requests; verify that crossing uses one of
  them, do not add a second mechanism.
- Occupied threshold rule: if an entity is inside either swept volume, the fold
  remains active; since this fold never changes destination in Stage 3, the
  conditional-change machinery (brief 15.3) is not needed yet.

### D. Rendering and loading
- Use the existing bounded recursion and a deliberate visual treatment at the
  depth limit (brief 15.5): beyond the limit, draw the veil membrane, never a
  hole or sky.
- Streaming: the destination chunks must be resident before the window renders
  or before crossing completes. Use the actual coverage state (render/coverage,
  `render/streaming/*`) and pre-request the destination chunk set when the
  player is within 12 m of the source aperture; block crossing with the veil
  until the destination reports resident geometry and collision. Residency
  budgets apply: do not pin more than the 192 MiB fine-ring cap noted in memory
  `visual-audit-vista-and-veil`; measure with the frame bench.
- Clip plane order trap: the side planes follow the exit plane in the frame
  data; the first implementation clipped the wrong plane and produced banding
  (memory `crystal-gate-portals`). Keep the regression capture.

### E. Persistence and multiplayer
- Save/load continuity: standing inside the fold's swept volume at save time
  restores a legal position (attach to `portals/sidecar.rs` semantics). Test it.
- Two players see the same destination; reconnection and delayed updates keep
  that. No obsolete destination crossings: destination is immutable in Stage 3.
- Map and atlas: add a discovered-connection entry only after the player has
  traversed it (brief section 6: knowledge boundary). Use the exploration map's
  data types in `egui_ui/map/*`; do not leak the destination before traversal.

## Tests

- `portals/gates_tests.rs`-style unit tests for the static fold: frame
  transforms, velocity mapping, yaw rotation only (reject pitch and roll),
  blocked exit, swept-volume placement refusal.
- Round trip: save and load while crossing; the player ends up legal.
- Determinism: fold ids and frames are a pure function of seed and district id.
- Rendering: capture test for the recursion limit with the veil treatment.
- Negative coordinates and large magnitudes: test the frame math with positions
  at -4096 and +65536 metres (world rule).

## Captures (acceptance evidence, brief 17)

Folded stair approach, mid-crossing, arrival under the balcony with the earlier
wall mark visible, a view through the aperture from both sides, and the
recursion-limit view. Same pose before/after for any shared shader change.

## Do not

- No pitch or roll transitions, no inverted walking, no Downward Shoes.
- No creature transfer. No fluid simulation through the fold. No new names.
