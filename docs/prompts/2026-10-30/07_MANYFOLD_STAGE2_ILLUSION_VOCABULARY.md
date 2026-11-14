# 07: Manyfold Stage 2, the illusion vocabulary

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. Start
after prompt 06. Design source: brief sections 7 (read all of it), 15.4, 16
(Stage 2), 17. This prompt plans the engineering.

## Goal

Three illusion families, each one consistent relationship between appearance and
physics, with recoverable introductory examples: the painted abyss (looks like
a drop, is a floor), the veiled shaft (looks like floor, is a drop into a shallow
maintenance room), and the false wall (looks solid, is passable). Plus one
terrain-aware enemy and a meaningful use for a revealed passage.

Brief rules to keep (section 15.4 and the handoff): separate the apparent
surface from the physical geometry; give each phenomenon a family, a
maintenance or source rule, a detectable signature, and consistent interactions
with contact, light, sound, water, particles and creatures where those systems
exist. A false floor is not a player-only trigger: thrown objects and mobs obey
it too. Teach new dangers where recovery is possible. Never add an exception
solely because the player learned a good test.

## Engine facts to check first

- Block model: `crates/content` property table (`assets/materials.ron`),
  `BlockId`, render kinds (`RenderKind::Opaque|Cutout|...`), collision in
  `crates/app/src/world/collision.rs`, raycast in `world::raycast_block`
  (voxel units; use `aim_cell`, memory `spell-system-constraints`).
- Rendering of appearance: meshing in `crates/app/src/mesh/*` and
  `shaders/world_shader.wgsl` with material ids; debug decode in memory
  `material-id-debug-decode`.
- Light and sound: guide lighting checks edited crystal cells
  (`docs/design/MANYFOLD_TUBE_STAGE_A.md`); audio cues in `crates/audio` and
  `world/adventure` sound events (`Event { cue, pos, gain, pitch }`).
- Interaction prompts: `world/interaction::Prompt`.
- Thrown objects: the brass weight toss (`manyfold.rs`), projectiles
  (`world/adventure.rs`), items. Mobs: `world/adventure/navigation.rs`;
  `mob_spawning = false` is the Manyfold default (`world.rs:1407`), so the
  terrain-aware enemy needs a scoped spawn rule.
- Save: no new block ids if avoidable (`SAVE_VERSION` in `world/storage.rs`;
  materials are id-keyed). If a new `BlockId` is needed, that is a save
  consequence: check `assets/materials.ron` id limits and the rules in
  `docs/` before adding, and prefer a **mask** approach below.

## Design: an illusion layer, not special blocks

Add `crates/app/src/world/illusion.rs` (split by family if over 300 lines):
- Data: per-district list of illusion records `{ family, cells | region,
  appearance_block, physical_block, source, signature, revealed: bool }`,
  deterministically derived from the world seed and graph by worldgen
  (`engine_worldgen::manyfold::foldfields::illusions`), stored as a sidecar
  `manyfold-illusions.ron` for the runtime-revealed flag only. Positions are
  derived, so old saves without the sidecar work.
- Appearance versus physics: **physical** voxels stay the truth in the chunk
  data (collision, water, sim, mining all read it). **Appearance** is a per-cell
  override consumed only by meshing: for a painted abyss the mesher draws a
  dark pit texture on the floor top face; for a veiled shaft it draws floor
  texture over air. The override is a small bitmap keyed by cell, applied during
  meshing, with the chunk remesh triggered on reveal. This keeps one source of
  physical truth, so thrown items, mobs and water obey the same rule.
- Reveal and detection: any of these set `revealed` for a record and flips the
  appearance override off: a thrown item landing on it, dust, a tap with a
  held tool (the Use action on the cell within reach), light of a specific
  colour, or the dragon's breath later. Reveal is local and persistent, and
  replicates as a sidecar event; do not rely on client-only state.
- Signatures (brief: each family has a detectable signature): a faint fringe
  of dust motes at the edge (use the existing mote system, one native pixel,
  palette colour; memory in render skill), a hollow sound on footfall for the
  false floor, a draught that moves flame pixels near a veiled shaft. Each
  signature is a pure function of cell position and the stepped clock.

### Families
1. **Painted abyss.** Floor looks like a pit. Walk, pebble and probing all find
   floor. Teaches detection safely: introductory example has no lethal
   alternative nearby, and a small creature refuses to follow (the brief's
   "first lesson"). Mobs: pathing already follows physical voxels; the tactical
   use is a mob AI flag that treats the appearance as hazardous.
2. **Veiled shaft.** Looks like floor, drops 3 to 4 m into a shallow
   maintenance room with a ladder and a note (use the lore note mechanism in
   `world/adventure/inscriptions.rs`, no new names). Fall damage uses the
   standard rule (server-measured falls, `netcode/src/movement.rs`); the
   introductory instance is shallow enough to be non-lethal at all voxel
   scales (compute the drop in metres, not voxels).
3. **False wall.** Looks solid; the cells behind are air in the physical
   layer and a thin opaque appearance covers them. Probing by Use reveals it;
   a thrown item passes through; the revealed passage leads to a hollow lane
   or to a shortcut that shortens an expedition route (meaningful use).

### Terrain-aware enemy
One creature kind added through `assets/bestiary.ron` and the bestiary loader
(round-trip test required). Behaviour: refuses to cross a painted abyss
appearance until revealed; attracted to noise; slow and readable. No new
proper noun in player text: descriptive name only. Spawn only in the district
via a scoped rule while the world default stays off. Combat balance values are
read from existing bestiary tiers; do not invent a new damage scale.

## Gate and save

Illusion layout is worldgen: claim a gate (or extend the Stage 1 gate if both
land together; one commit per gate bump). Sidecar for runtime state, no save
version bump. Write sidecars with the atomic helper from prompt 03.

## Tests

- Unit: illusion derivation is a pure function of seed; at least 20 seeds and 3
  voxel scales; the introductory instances are within reach of the route graph
  and recoverable (validator walks into the shaft and climbs back out by the
  ladder).
- Physics consistency: a thrown item, a mob path query and a water cell all see
  the physical layer; the player sees the appearance only. Assert both layers
  in one test per family.
- Reveal persistence: reveal, save, reload, remesh; appearance stays off.
- Multiplayer: reveal event is replicated and a late joiner sees the revealed
  state (see memory `netcode-crate-and-multiplayer-ownership` for the local
  lobby E2E recipe).
- Meshing seam test: an illusion cell on a chunk edge (negative coordinates, use
  `floor_div`) remeshes both chunks.

## Captures

Before/after per family at eye level in the introductory instance, plus the
revealed state, labels under `target/`. Include a capture showing the signature
(dust fringe) at night and in daylight.

## Do not

- No player-only trigger volumes. No exceptions to the rule after the player
  learns the test. No new block ids unless the save impact is documented.
- No new proper nouns, no sounds that imply lore not in the brief.
