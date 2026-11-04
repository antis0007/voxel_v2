# Ambience that fits the place, a larger planet, a soft cloud handoff

Owner request, 2026-11-03: ambient audio often does not fit the location (wind in
space, birds that sound fake and come from nowhere), biome and colour blending is
coarse, the planet reads too small, and the cloud picture jumps when the view
rises from the in-world march to the orbital globe.

Part A (audio air profiles, exposure, void realms) is a small change landed
alongside this document. This document covers Parts B and C.

## Intent

The listener should be able to close their eyes and know roughly where they are:
under trees, by a pond, on a bare ridge, in a cave, above the air. Rising from
the ground to orbit should look like one continuous planet with one cloud deck.

## Goals

- Birds come from canopy, frogs from still water, nothing from bare ground.
- Animal voices cross-fade in space across biome borders, not only in time.
- Bird calls carry a distinct voice per biome family (whistle, trill, chatter, caw).
- The planet is larger, with every face and address still deterministic.
- From ground to orbit the clouds hand off over a band, never a frame.

## Non-goals

- Music, footsteps, contact sounds: unchanged.
- Visual biome floor colour blending in `world_shader.wgsl`: separate visual pass
  with its own A/B captures, scheduled after Part C.
- Recorded samples: every sound stays synthesized in `engine_audio`.
- Save format: no new bytes. `BED_COUNT` stays 6.

## Anti-goals

- Birds that follow the camera, or a chorus that pans randomly with no source.
- A planet size change that alters any existing world's terrain without a
  generator gate.
- A cloud handoff that doubles the deck (march and shell both opaque at once).

## Constraints

- Audio render cost: `layers_render_far_faster_than_real_time` keeps the layers
  under 25% of one core in debug. Positioned bird emitters stay within that.
- Probes are bounded: the ambience probe is ~360 cells every 0.5 s. The canopy
  probe adds at most the same order (see budget below).
- No `Instant`, no `rand`; positional hashing from `engine_core::hash`.
- Text rules: no second person, no exclamation, banned words list in CLAUDE.md.
- Globe shader constants mirror host constants and are checked by tests
  (`band.rs` checks `GLOBE_HALF_M`); any radius change updates both.

## Part B: localized fauna

### Canopy probe (`crates/app/src/soundscape/ambience.rs`)

Extend `Surroundings` with `canopy: [f32; 8]` (per `DIRECTIONS` sector, 0..1)
and `still_water: f32`. The probe walks each of the 8 directions at rings
6 m, 14 m and 24 m, and at each ring column scans from 2 m below the eye to
12 m above for `Leaves`, `PineLeaves`, `Bush`. Weight per hit: 1.0, 0.6, 0.35
by ring. `canopy[d]` is the clamped weighted sum / 2. Budget: 8 × 3 × 15 = 360
cells at 1 m spacing, same order as the existing probe. At fine voxel scales the
stride logic already in `probe_flow` applies.

`still_water` reuses the existing water ring walk: water found with
`water_motion` below 0.2 counts as still water.

### Fauna voice becomes a field (`crates/app/src/soundscape/fauna.rs`)

`voice()` takes the blended biome voice (below) and the surroundings, and returns
`Fauna` plus a `Placement`:

```rust
pub struct Placement {
    /// Bird emitter weight per sector; all zero means no birds.
    pub canopy: [f32; 8],
    /// Frog chorus weight, from still water in the rings.
    pub water: f32,
}
```

Birds level = biome birds × daylight × rain × open × max canopy weight.
Frogs level = biome frogs × night curve × open × still_water.

### Biome border blend

Sample `landscape::sample` at the eye and at four points 16 m out along ±x, ±z.
Blend the five voices with `Fauna::mix` using weights 0.4 centre, 0.15 each
side. Five landscape samples per 0.5 s is within budget (the probe already does
one). Builder may cache the four outer samples when the eye moves under 4 m.

### Positioned calls (`crates/audio/src/fauna.rs`)

`Chorus::set` takes `Fauna` and `Placement`. When a call starts it picks a
sector by weighted draw from `canopy`, then a distance class: near (8 m) with
probability `canopy[d] * 0.3`, else far (20 m). Pan from the sector's bearing
relative to the listener's forward vector, which the game passes along with
`Placement` as a yaw in turns. Gain by distance class and a per-call random
spread as now. Frog croaks pan toward the still-water sector(s) found by the
probe; if the water weights are flat, croaks spread across the stereo field.

### Voice families (`assets/biome_sounds.ron`)

Add `family: Whistle | Trill | Chatter | Caw` per biome row. Synth per family:

- Whistle: current falling sine with slower sweep, longer notes.
- Trill: existing AM warble at 40 to 60 Hz, shorter notes, more notes per phrase.
- Chatter: two-formant chirp (carrier + ring-modulated second partial at 1.5×),
  rapid short notes with random pitch steps.
- Caw: noise-excited band-pass at 1.2 to 1.8 kHz, a single drawn-out note.

Every call keeps a breath-noise onset of 8 ms at 10% level. Cost per call stays
under 4 oscillators.

### Tests

- `probe` on a forest fixture reports canopy in the tree sectors only, zero on
  open field.
- A "Forest" biome over open ground yields `birds == 0`.
- `Fauna::mix` border test: halfway between Desert and Rainforest the call rate
  is near the mean.
- `Chorus` RMS stays within the existing band; each family renders finite and
  non-silent.
- `dump_audio_wav` gains a row per family for listening review.

## Part C: planet size and cloud handoff

### Planet size

Today: `HALF_M = 24_000`, `RADIUS_M = 30_558`, face pitch `PITCH_M = 65_536`.
Faces use 48 km of a 65.5 km address pitch, so the sphere can grow without
touching face addressing up to `HALF_M = 32_768` (radius 41.7 km).

Decision needed from the owner (see questions). Default call: `HALF_M = 32_000`,
`RADIUS_M = 40_744` (HALF_M × 4 / π rounded), which is a 33% larger planet,
keeps `PITCH_M`, and keeps every face address.

Gating: `HALF_M`/`RADIUS_M` become functions of generator version
(`atlas::half_m(version)`), with the old constants kept for versions below the
new gate. Planet-face worldgen (`planet.rs`, `basin.rs`) already takes a
`WorldSampleConfig`, so it reads the versioned value. The renderer, globe chart
and orbital rails read the world's version through the config they already hold.
The new gate joins the dormant planet gates 122 to 124: it bumps together with
them, not alone.

Consequences the builder must carry through:

- `altitude::ORBIT_M` (30 km) sits below the new radius; the orbit tier
  threshold scales with radius (`radius × 1.0` today). Keep the ratio.
- `cloud_shader.wgsl` planet curve uses `cam.planet[0].x` = radius from the
  host, so it follows automatically.
- `globe_shader.wgsl` `GLOBE_HALF_M` is a shader constant checked by a test;
  it becomes a uniform or a compose-time constant per version.
- Tile cache keys include radius (`cache.rs`); old caches invalidate by key.

### Cloud handoff

Today the march stops at `top_slab + 500 m` (about 7.7 km) and cloud history is
dropped that frame. The globe's cloud shell, a tile mesh at a fixed 1.6 km, is
drawn whenever the globe draws, from the Transition tier at 1.5 km upward, so
between 1.5 km and 7.7 km both the march and the shell draw: the double deck is
the current state, and the before captures will show it. The shell coverage maps
through `smoothstep(0.58, 0.86)`, which shows roughly half of the march's deck.

New behaviour: a handoff band of 2 km centred on the top slab.

- Host computes `handoff = smoothstep(top_slab - 1000, top_slab + 1000, altitude)`
  and passes it in the cloud and globe uniforms (one spare float each; builder
  finds the slot, both structs are shader contracts).
- March: `dist_fade` and the deck extinction scale by `(1 - handoff)`. History
  stays valid through the band (`march_handoff` holds true to the band top); it
  is only dropped above `handoff == 1`.
- Shell: draws whenever the globe draws (Transition tier and up), with alpha
  scaled by `handoff`. Below the band it contributes nothing, so no double deck.
- Coverage match: the shell's `globe_cover` ramp is fitted so that a full-sky
  average of shell alpha equals the march's average coverage for the same
  weather. Builder fits the two smoothstep ends against a 16-sample table of
  `CloudControls.coverage` and records the fit in the test.
- Shell altitude: stays at `SHELL_HEIGHT_M` 1.6 km. Terrain reaches 512 m on
  the home world and more on planet faces, so a shell at the deck base (about
  100 m) would cut through hills on the globe. From the band (7 km and up) the
  1.5 km offset between the march deck and the shell is below the parallax the
  cross-fade hides. Writer's call.

### Captures

Same pose, before and after, per the render rules: ground pose under the deck,
pose at top_slab − 800 m, pose at top_slab + 800 m, pose at 15 km. Planet size
change: globe pose from orbit plus a horizon pose at 1.5 km. Evidence goes under
`target/review/`.

### Tests

- `volumetric_clouds_enabled` replaced by `march_weight(altitude, top_slab)`
  returning 0..1, with the band endpoints asserted; a shell weight test asserts
  zero below the band so the globe never double-draws the deck.
- Coverage fit test: shell alpha mean vs march coverage within 0.05 across the
  table.
- Atlas round trip (`to_body`/`from_body`) passes for both old and new radius.
- Existing globe GPU tests pass with the version switch at both values.

## Left to the builder

- Exact sector selection draw and the per-family oscillator layouts.
- Where the handoff float lives in the uniform structs.
- Whether the canopy probe shares the ring walk with the water probe or runs its
  own loop.
- Order of work: B first (no visual gate), then C clouds, then C size.

## Owner decisions (2026-11-03)

1. Planet size: `HALF_M = 32_000`, radius 40.7 km, no address change.
2. The size gate lands on top of the tree as it stands (gate 127, awake); the
   owner of gates 122 to 124 carries both to commit. Old planets keep 30.6 km.
3. Document approved; build order B, C clouds, C size.

## Writer's calls after the builder check

- Shell altitude stays at 1.6 km (above).
- Forward yaw for bird panning goes per frame through the existing atomic slot
  pattern in `layers.rs`, not through the 0.5 s fauna command.
- Order: B, then C clouds, then C size.
