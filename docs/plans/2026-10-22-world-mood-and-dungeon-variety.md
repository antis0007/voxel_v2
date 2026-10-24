# World mood and dungeon variety: findings and plan

Date: 2026-10-22. Status: findings from measured captures and a read-only code audit; nothing below the "Done" list is built.
Target feel: Elden Ring and Breath of the Wild. The sky and light set the mood of a place; underground spaces are distinct, not one room kit repeated.

## Done so far (this branch, uncommitted)

- Atmosphere coefficients are data: `assets/atmosphere.ron`, `render/atmosphere_config.rs`. `earthlike` renders identically to the old constants; `alien_default` is untuned.
- `SkyArt::fog_seam` (default 1.0, env `VOXEL_FOG_SEAM=0..1` for review only). It weights the authored fog colour in the sky horizon seam and the heavy-cover veil in the aerial-perspective table.
- `tools/Review-VisualDirection.ps1`: stale `src/` path fixed; new `-TimeOfDay` (0..1, drives `--benchmark-review-time`; the old hard-coded 0.5 meant every earlier capture was noon) and `-Camera` parameters.

## Measured: the seam is not the mood problem

A/B at the same pose, `fog_seam` 1 vs 0, at time of day 0.70, 0.7475 and 0.78: no perceptible difference.
What the captures do show, at the default pose looking away from the sun:

- Golden hour (0.70) is a flat grey-mauve overcast.
- Sunset (0.7475) is a murky maroon-brown with almost no luminous horizon.
- No rim-lit cloud edges, no warm-to-cool colour shift with distance, no readable depth layers.

Caveat: `-Camera` did not change yaw (four yaws gave identical images), so no sun-facing frame exists yet. Find out what the five fields of `--benchmark-terrain-camera` mean in `crates/app/src/main.rs` before the next capture round.

## Sky and light plan (BOTW / Elden Ring)

Order matters; each step needs same-pose captures at 0.25, 0.50, 0.70, 0.7475, 0.78 and one sun-facing pose.

1. Sun-facing review pose and a time-of-day sweep script (blocked on the camera fields above).
2. Horizon glow: warm, bright band near the sun's azimuth that falls off with elevation. The physical LUT carries it; the `photographic_sky_radiance` grade (`sky_grade`, `sunset_saturation`) is flattening it. Try lowering `sky_grade` at twilight first.
3. Cloud rim lighting: silver-lining lobe (HG g about 0.8) strong near the sun; shadow side picks up cool sky ambient, not grey.
4. Distance layering: aerial perspective that shifts hue with distance (warm in-scatter toward the sun, cool away), driven by the froxel table only. Then delete the authored fog-colour paths for good (`fog_color_density`, `fog_params` seam use) once captures show the LUT alone reads at least as well.
5. Ambient from the sky: low-order SH from the sky-view table so shadowed terrain takes the sky colour (teal shadows at golden hour).
6. Tune `alien_default` against the same sweep: amber horizon, violet zenith; check dusk and night for NaNs and banding.

## Dungeons: what makes them repetitive (code audit, `crates/worldgen/src/dungeon.rs`)

- 17 room `Kind`s but only 4 `THEMES`; a theme is a wall/alt/floor block triple plus a 6-slot kind list.
- Every complex is the same skeleton: 3x3 grid of 28 m slots, 9 rooms, the same 14x14x10 hub with a spiral shaft, floor at `ground - 26` for every room (one depth), one corridor type (3.0 x 3.6 m, torch every 9 m), flat ceilings.
- Props are fixed per-kind stamps in fixed relative positions (crypt sarcophagus grid, hall pillar grid, forge corner pile, library shelf strips).
- Doors sit on every wall midpoint, so approach and entrance shape never vary.
- No environmental state in random dungeons: `Damage` (Intact, Collapsed, Flooded, Overgrown) and `Style` exist only for the single authored Undercrown region. No cave breakthrough (caverns are excluded by `reserved`). No stairs or galleries beyond one 8 m dead-end sink.
- Lore has the hooks and the code does not use them: `WORLD_LORE.md` "Dungeons: every ruin had a job" lists stratum, job and "hazard = the job gone wrong" for eight complexes; code carries only the theme name. Depth-is-age is not applied (`ground` is stored but unused).

## Dungeon plan: built in generator 32 (`crates/worldgen/src/dungeon/v2/`)

Status 2026-10-22: steps 1–4 below are implemented behind `dungeon::v2::VERSION` (32); generator 31 and older generate byte-identically (golden digests checked).
- Identity: eight jobs from the lore table pick depth (stratum), palette, room kinds, hazard odds and the reveal-hall landmark (`job.rs`, `landmark.rs`).
- Layout: six footprints on a 34 m lattice (5–12 rooms), up to three levels joined by ramps with flat landings, a reveal hall with a balcony ring and stair linking the top two levels, loops, hashed off-centre doors (`plan.rs`).
- Construction kit: piers, sconces, ribbed/corbel/coffered/beamed ceilings, door frames, corridor variants (`kit.rs`); props with seeded variants per kind and damage states (collapsed, flooded, overgrown, breach) (`dress.rs`).
- The objective is a sealed tombstone core entered only through a puzzle room (labyrinth, hollow floor or orrery); deep complexes seal the puzzle room too.
- The gatehouse over the shaft now appears above ground (it never did: `sites::cavern` returned before sampling it).
- Tests flood-fill real voxels on foot (96 complexes at 1 m, 6 at 0.5 m) and assert spread across jobs, sizes, depths and levels.

Original plan, kept for reference:

Per `.claude/rules/saves.md` and `world.md`: new hash keys at new offsets so older rolls are untouched; test an old-version fixture generates identically; cover negative and large chunk coordinates.

1. **Dungeon identity.** Per-cell job and hazard roll from the lore table. It biases the kind list, picks a depth offset (replaces constant `ground - 26`), and picks a footprint shape (2x3, 3x4, L-shape, ring) instead of always 3x3. Names in `sites.rs`.
2. **Room state.** Port `Damage` to rooms: collapsed rubble, flooded (water voxels through the existing cellular sim), overgrown, and a cave breach as the one allowed exception to `reserved`. Hazard follows the job (sluice floods, quarantine seals).
3. **Variety inside rooms and links.** 3 to 4 seeded prop layouts per kind, junction chambers, stairs and galleries, corridor width and ceiling variants, door placement off the midpoint, per-theme lighting (not only torches). Put props in a new `dungeon/props.rs`; `dungeon.rs` is already 2544 lines against the 300-line rule.
4. **Landmarks and secrets** per dungeon, so each has one thing worth remembering. Fill the five-part lore slot first.

Acceptance for the set: a headless scenario that generates N dungeons from different seeds and asserts measurable spread (distinct footprints, depth offsets, kind multisets, hazard states), plus before/after captures from a fixed route.

## Decisions needed from the owner

- Approve a generator bump to 31 for dungeon identity (step 1).
- Whether the Undercrown's `Damage`/`Style` enums should move to a shared module (touches `undercrown/plan.rs`).
- Mood reference priority: golden-hour warmth (BOTW) or brooding overcast (Elden Ring) as the default look.
