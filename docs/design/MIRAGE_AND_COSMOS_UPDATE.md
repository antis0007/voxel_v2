# Mirage and Cosmos update (generator 21)

Everything here is gated. New worlds use generator 21. Saves at 20 or older generate exactly as before; the new blocks are
appended to the material table, so saved block ids keep their meaning.

## Illusions

Three blocks were appended after `MithrilBlock` in `src/material.rs`:

| Block | Looks like | Body | Notes |
|---|---|---|---|
| `MirageBrick` | `StoneBricks`, pixel for pixel (same colour, surface, texture kind 12) | passes through | Craftable: 2 stone bricks + 1 crystal -> 4 |
| `MirageVoid` | `VoidStone` | passes through | The false wall of a cosmos chamber. Not craftable |
| `VoidStone` | near-black violet marble, pinprick stars | solid | Cosmos chamber shell |
| `StarGlass` | dark glass tiles with a drifting sheen and twinkling stars | solid | Craftable: 1 crystal + 2 basalt -> 2 |

A mirage is an illusion for the body, not the pickaxe: `is_ray_pickable` includes it, so it can still be targeted and broken.
Texture kinds 52 and 53 live in `block_surface` and the emissive stage of `world_shader.wgsl` (`void_star_mask`).

## Puzzle rooms (`src/world/dungeon.rs`, generator 21 only)

About four rooms in ten become one of three new kinds. All are stateless: geometry comes from a hash of the room, so there is
nothing to save and nothing to desync.

- **Labyrinth of False Walls.** A binary-tree maze (perfect, so there is exactly one real route) around a cache in the middle
  cell. Every interior wall segment is a coin flip between real brick and `MirageBrick`; the outer wall is always real. Walk into
  a wall that is not there, or find the one route. Torches sit on some posts.
- **Hall of the Hollow Floor.** A cross of real floor joins all four doorways. The four quadrants are a one-metre skin of
  `MirageBrick` over a three-metre pit. Three pits are flooded, one holds a cache, and every pit has a hidden stair back up.
- **Hall of Wandering Stars.** A cosmos chamber (below). A trail of crystal dashes laid in the star-glass floor leads from the
  middle of the room to a false wall; behind it is a small annex with a cache and a torch. The seven Big Dipper pillars and a
  floating orb are set dressing and orientation.

The HUD location line names these rooms. `dungeon::cosmos_at` returns 0..1 for a point, easing in over four metres so the
doorway already glows.

## Fog that is the cosmos

`LightingState.mood.x` is the eased cosmos factor. `main.rs` samples `cosmos_at` at the eye each frame and smooths it. In
`mood_shared.wgsl`, `mood_cosmos_fog` replaces the in-scattered light with a procedural starfield (cube-face star cells in three
layers, a tilted galactic band, fbm nebula) plus world-space motes that slide past as you walk. The sky pass swaps its colour for the
same backdrop, so a shaft of sky inside a chamber agrees with the fog.

## Sky events and seasons (`src/world/mood.rs`)

Pure functions of (weather seed, day, time of day), so nothing is saved and two players on one seed see the same sky.
A day is six four-hour windows. Each window rolls once for an event suited to its hour and fades in and out inside the window.

| Hour | Events |
|---|---|
| Night | Meteor shower, aurora, blood moon |
| Dawn | Pearl fog, golden haze, rainbow |
| Day | Rainbow, golden haze |
| Dusk | Emberglow, violet hour, aurora |

Aurora, meteors and rainbow are painted in `sky_shader.wgsl`. Blood moon, golden haze, emberglow and violet hour also set
`mood_tint`, a whole-frame grade applied in `postprocess.wgsl` so terrain, water and cloud sit in the same coloured light.
A meteor shower also drops real stars: a crystal in a scorched dimple 30 to 80 m from the player, announced with a compass
bearing. A banner announces every event.

Seasons: five days each, a twenty-day year. They multiply the meadow and canopy material families through the existing
material tint array (`seasonal_tints` in `render.rs`) and nudge sky, fog and sun warmth. No mesh is touched.

## Clouds

`cloud_composite.wgsl` now ends in `stylise_cloud`: shade is lifted toward a sky-tinted lavender, sunlit crowns stay cream, the
light ramp is stepped softly (matching the terrain's cell shading), and edges are firmed. It fades out at night and twilight, where
the marched colours already carry the art direction. Cirrus fibres are shorter in `cloud_shader.wgsl` so they stop fanning out of
the zenith as a zoom-blur.

## Wands, the atelier and the living air

- **Held wand** (`src/mesh/models/wand.rs`). The wand is now always in the hand (the tool takes the hand on a swing), idle low
  and to the right, thrust forward with recoil when casting. It is built each frame from glowing micro-voxels, so it uses the
  emissive pass, bloom and night grade. Rune beads on the shaft fill with mana; the optional mana vein is a glowing line whose length
  is your mana. The head, trail and glow follow the player's style; the head throws a coloured dynamic light on the world
  (`wand_tip_light`), which flares on a cast.
- **Wand atelier** (`U`). Six shafts (oak, bone, frozen crystal, obsidian, verdigris copper, starwood), six crowns (cradled orb,
  resonant crystal, twin prisms, orbit ring, living flame, watching eye), six grips, seven glows (spell colour or a fixed hue),
  five shed effects (none, sparks, orbiting stars, rising embers, comet ribbon), three rune modes. A live schematic previews it;
  the hand changes at once. Choices are saved by `src/wand_style.rs` in `wand-style.txt` beside the saves (plain `key=value`, tolerant
  of bad data), not inside a world, so it can never break a save.
- **Living air** (`mood_ambience` in `mood_shared.wgsl`). Fireflies on warm nights over grass and hills, pollen on summer days over
  plains, diamond dust over snow, embers under a blood moon or emberglow dusk. They are world-space motes added as in-scatter, so they
  sit in front of surfaces and slide past with parallax. Biome and season come from `mood::ambience`; underground it is zero, as is the
  sky-event grade.
- **Gem glints** (`mood_glint`). Crystal, ore and metal blocks throw a sparse sweep of facet glints as the view angle changes.

## Looking at it

New fixed-pose review (no hand flying):

```powershell
$env:VOXEL_CAPTURE_DIR = "target\camera-review"
target\release\minecraft_style_voxel_prototype.exe --benchmark-world=hills --benchmark-capture-frames `
  --benchmark-terrain-camera=104.7,615.1,-513.6,0.0,0.0 --benchmark-warmup=4 --benchmark-duration=40 `
  --benchmark-review-time=0.5
```

- `--benchmark-terrain-camera=x,z,clearance,yaw,pitch`: a clearance at or below -500 is an absolute eye height in metres,
  `-(clearance + 500)`, for rooms that are not under the local surface.
- `--benchmark-review-day=N` jumps to a calendar day; `--benchmark-review-time=T` fixes the hour.
- `cargo test --bin minecraft_style_voxel_prototype print_review_rooms -- --ignored --nocapture` prints cosmos, maze and pit rooms
  for the default benchmark seed; `print_review_days` prints a day and hour for each sky event.

## Tests

`dungeon`: puzzle rooms exist at 21 and never at 20; the maze is solvable by real walls alone; the phantom floor is mirage over a
pit with a solid aisle; the annex has a false wall and a cache. `mood`: events are deterministic, fade at window edges, match
their hour; seasons blend without jumps; fallen stars land in range and are named by bearing.

Review overrides for the held wand (`VOXEL_REVIEW_WAND=1..3`, `VOXEL_REVIEW_STYLE=shaft,head,trail,grip,glow,runes`) apply to a
fixed-pose capture without saving anything.

## Not done

Illusion *spells* (a wand verb that makes mirage on demand) were left out: spell registration touches the wand, UI icon and growth
code that are mid-change on this branch. Mirage bricks are craftable so players can build fake walls today. The maze and pit rooms
are covered by geometry tests but were not captured; the two camera spots tried landed in corridors.
