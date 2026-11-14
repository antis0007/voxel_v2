# Deep geological history — generator 287

New Planet worlds select 287. Saved generator identities remain separate.
The existing local world library was deleted at the user's explicit request;
preferences, multiplayer identity, screenshots and render caches were retained.

## Implemented processes

- 720 million years, with one-million-year plate steps and 250,000-year surface
  steps: 2,880 coupled surface intervals. Physical mesh resolution remains 84.
- Continental mass centres entrain plate motion toward a mantle downwelling.
  Measured concentration controls an insulating heat reservoir. Sufficient heat
  reverses the flow into dispersal; dispersal and reduced concentration allow a
  subsequent assembly at a different downwelling. These are reduced kinematic
  boundary conditions, not a mantle convection solver.
- Continental material still moves through the existing conservative spherical
  remap. Collisions thicken arriving crust; extension spreads the same material
  over more area. Existing crust ancestry survives those movements.
- Oceanic convergence adds juvenile arc crust with an explicit mantle-source
  term in the continental-volume budget; its carbonate fraction is diluted and
  its ancestry follows the parent plate.
- Inherited shelf blocks and failed rifts are initialized as crust thickness.
  They move with the crust. Thermal breakup can split continental plates as well
  as the existing strain criterion. Up to 40 plates are supported; the split
  exclusion list resets every 120 Myr so a daughter can split in a later episode.
- Twelve finite-duration mantle provinces each supply a broad basalt platform
  and seven irregularly distributed volcanic centres. Multiple centres spread
  across an area enable archipelagos in addition to the existing hotspot tracks.
  Permeability depends on continental thickness and extension. Supply feeds the
  existing magma reservoirs: eruption dates, basalt burial, erosion, transport
  and later exposure use the same history system as boundary volcanoes.
- A positive carbon reservoir relaxes analytically toward the ratio of volcanic
  supply to temperature-, moisture- and fresh-rock-dependent weathering. Its
  logarithmic radiative forcing feeds the existing ice-albedo energy model.
  Climate changes affect ice, runoff and erosion; the final temperature anomaly
  also reaches playable climate and reef generation. At handoff, the terrain
  is translated to the fixed runtime water datum, preserving the final coast.
- Twenty-four 30 Myr geological bins preserve the whole history, with two recent
  beds retaining fresh lava/soil ordering. Erosion removes younger deposits
  first. Lithology and weathering still affect erosion and mineable materials.
  Age-based rock classification now uses the actual history duration.
- Replay retains 241 endpoints at three-million-year spacing and up to four
  selected events per endpoint, drawn from all three intervening plate steps.
  The loading timeline takes its duration from the generated frames.

## Scope and limits

The model can assemble land and fragment it without repainting a final coastline,
but a particular seed is not guaranteed a supercontinent or a specified island
count. Shelf and volcanic provinces must still be inspected in the native game.
Long-term conservative remapping has numerical diffusion. Old sediment facies
within the same 30 Myr bin can mix; this is not bed-by-bed stratigraphic resolution.
No full rock metamorphism, mantle convection, atmospheric circulation, or calibrated
Earth carbon budget is claimed. Volcanic supply is bounded and dated but its
province locations and lifetimes are seeded boundary conditions.

The replay payload stays at the fine-history budget. Runtime increases with
the longer solve and the larger stratigraphic record; no loading-time result is
claimed. No tests or hosted CI were added or run, following the task instructions.

## Build status, 8 November 2026

The world-generation library compiled with the final changes. Two earlier full
release-fast builds also succeeded, before the final shoreline-datum correction.
The final full-game rebuild is blocked by concurrent settlement edits: private
`Enemy::stagger` access and borrow conflicts in community transit/civilization.
Those unrelated files were not changed to force this build through. The existing
executable must not be presented as containing the final shoreline correction.
Compiler output is in `artifacts/deep-time-287-build.log`. Native appearance,
generated continent/island counts and loading performance remain unverified.

## Research informing the approximations

- [Mitchell et al., The supercontinent cycle, 2021](https://www.nature.com/articles/s43017-021-00160-0.pdf):
  assembly and breakup can overlap; mantle coupling matters. The implementation
  uses a heat/concentration feedback approximation, not the paper's geodynamics.
- [The supercontinent cycle and Earth's long-term climate](https://pmc.ncbi.nlm.nih.gov/articles/PMC9796656/):
  motivates coupling geography, volcanic supply and weathering instead of a
  prescribed sequence of cold-only epochs. Coefficients here are game-scale.
- [Mortimer et al., Zealandia: Earth's Hidden Continent, 2017](https://www.geosociety.org/gsa-today/march-april-2017/zealandia-earths-hidden-continent):
  thinning and isostatic balance can submerge continental crust while leaving
  connected shelves beneath separated islands. This informs the thickness-based
  shelf fragments; no Zealandia reconstruction or copied island layout is used.

Relevant local context: `FINE_HISTORY_REVIEW.md`, `LANDSCAPE_CYCLES_286.md`,
and `docs/superpowers/specs/2026-11-03-planet-geology-placer-design.md`.
