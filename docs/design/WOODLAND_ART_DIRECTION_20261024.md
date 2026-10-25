# Woodland art direction, generator 47

Implemented a bounded world-composition pass after inspecting the rejected
`target/reviews/forest-repair-20261021/reference-shaders-after-r2/camera.png`.
Its thin, isolated crown shelves and uniform meadow were the primary targets.

## Source changes

- `trees/woodland/v47.rs`: younger, mature, broad old and slender proportions;
  deep connected crown cores, five unequal overlapping lobes, lower forks,
  tapered boughs and flared roots. Ordinary log/leaf voxels carry the geometry
  through the existing generation, meshing, collision and edit paths.
- `trees/woodland.rs`: two version-47 dispatches and module declaration. The
  version-45/46 profile and voxel algorithms remain unchanged.
- `landscape.rs`: version-47 stand envelopes increase occupied forest groups
  while preserving open glades. Dirt patches break continuous turf in closed
  woodland. All upstream density exclusions remain multiplicative, including
  the starting factory yard, roads, sites and water.

Weeping/conifer forms retain their existing generators; umbrella changes from
the subsequent image review are detailed below.
Horizontal reach stays inside `radius * 1.35`; vertical reach stays inside
`height + canopy_height`. Existing crown bounds and two-cell tree search still
cover the new shapes. No save format, terrain elevation or material ID changes.

Pre-edit source snapshots are under
`downloads/art-direction-20261024/world-before/crates/worldgen/src/`.
Both snapshots were hash-matched immediately before editing. Existing dirty
changes in those files were retained. No tests, CI, builds or GPU launches were
run by the woodland agent; the coordinating agent owns compilation/captures.

## Review locations

Seed: `2738958700`, Hills, one-metre voxels, new generator `47`.

- Production clearing: camera `44,35,1,-1.0,-0.12`, matching prior reviews.
- Prior broadleaf anchor: `(35,28,43)`, cell `(2,3)`. Candidate camera
  `35,69,1.7,0,0.12` backs away from the cropped prior close-up. Placement now
  uses a new stand envelope, so the tree's continued presence needs capture
  confirmation. This is a review suggestion, not a validated bookmark.
- For isolation, compare the new binary forced to `46` against the same binary
  forced to `47`; shared shader/UI changes otherwise confound attribution.

Visual quality, motion/LOD transitions, practical routes beneath crowns,
edit/reload behavior and performance remain acceptance work. Source changes
alone do not establish competitive visual quality.

## Workshop facade increment

The generator-47 workshop blueprint now uses a basalt roof over its timber hall.
`architecture/workshop.rs` adds an entrance cross-gable within the existing
2 m paved apron: two brick-footed timber posts, a continuous lintel and a closed
gable. Main hall end gables gain a timber tie and king post. This uses ordinary
solid blocks; no display-only facade or random prop placement was added.

The hook is restricted to kind-0 workshop buildings at version 47+. Doors,
stations, mine entrance, floor elevations, plot survey and path endpoints keep
their coordinates. The central passage has at least 3 m of clear headroom above
the one-metre floor; visual and traversal confirmation remain pending.
Architecture snapshots join the same `world-before` directory.

Starter workshop centre is world `(0,-42)` in XZ, facing positive Z. Candidate
front camera: `0,-13,1.7,0,0.06`; oblique: `18,-15,1.7,-0.55,0.06`. Ground height
is resolved by existing review controls. These are proposed capture locations,
not validated evidence.

## Native review and second revision

Inspected root's actual captures `art-20261024-before-forest/camera.png`,
`art-20261024-after-forest47/camera.png` and
`art-20261024-after-workshop47/camera.png` under
`target/reviews/forest-repair-20261021/`. The broadleaf canopy masses are visibly
deeper and more joined. Retained emergent umbrella forms still read as thin hats;
the meadow remains uniform, and the roof reads as a very dark mass.

The first porch roof consumed the front gable at one-metre resolution. R2 raises
its ridge from 6.26 m to 8.2 m, adds a timber fascia and widens the supported front
to expose its gable infill. The central approach keeps y=1..3 clear of new blocks.
The apparent timber behind the entrance in the capture is not proof of a blocked
door: source inspection shows the existing opening and new approach remain clear.
Physical traversal still needs verification.

R2 also adds explicit version-47 species branches in `trees/forms.rs`: rainforest
emergents keep umbrella identity with 14–20 m leaders and 5–8 m crown depth;
savanna umbrellas gain 2.6–4 m crown depth. Staggered lobe heights break the single
flat plane. Their proxy top uses the matching lobe heights; old species profiles
and samplers retain their numerical behavior. Before-R2 snapshots are in
`downloads/art-direction-20261024/world-before-r2/`.

Root subsequently inspected the R2 workshop capture and R4 pan/grazing/elevated
route frames. The entrance gable and deeper crowns are visible. Evidence and
remaining limits are recorded in `COHESIVE_ART_DIRECTION_20261024.md`; physical
traversal, all scales/seeds and performance acceptance remain open.
