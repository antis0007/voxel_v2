# Generator 52

`CURRENT_GENERATOR_VERSION = 52`. Every change below is gated on
`cfg.generator_version >= 52`; versions up to 51 generate bit-identically
(checked with the extended `gen_golden` list 2, 9, 12, 15, 16, 20, 31, 32, 33,
34, 36, 41, 47, 49, 50, 51).

## Trees

Code: `src/trees/skeleton.rs`, `src/trees/giant.rs`, `src/trees/woodland/v52.rs`.

- Broadleaf woodland (the generator 47 `woodland` trees) keeps its generator 47
  proportions but is grown as a skeleton: a leaning, bowed bole with a root
  flare of 1.5-1.8x trunk radius, surface roots on older trees, 2-4 limbs that
  leave the bole at 40-50 % of trunk height (some lower), each limb ending in
  an elbow lobe plus two twig lobes. Crowns are 7-13 clumped lobes with sky
  between them instead of one central mass.
- Landmark giants: one lottery cell per 10x10 block of 12 m tree cells
  (120 m), chance 0.16, kept two cells from the block edge. A giant needs
  grass or woodland soil, a temperate climate (no desert, badlands, tundra,
  alpine, taiga, ocean, coast, marsh or fen), at most 1.6 m of relief within
  4 m, 3 m above water, 150 m from spawn, no trail within 6 m of the trunk and
  no site, homestead, valley, Amberroot or Undercrown reservation. About 1.3
  giants per km² of mixed land and sea for seed 2738958700.
- Giant habits: spreading (29-39 m, reach 0.62x height), tall (34-44 m,
  steep limbs) and ancient (strong lean, drooping limbs, heavy buttresses).
  5-6 limbs with three twigs each give 21-25 lobes.
- Leaf and wood reach stays under 22.5 m, inside the +-2 cell (24 m) tree
  search, so a giant needs no change to the per-voxel search. Ordinary tree
  cells whose anchor lies inside a giant's reach + 2 m grow nothing; bushes
  remain as undergrowth. Trails clear giant voxels only up to 4.5 m above the
  tree's ground, so a crown can arch over a path.
- Geometry is in metres and sampled at voxel centres; wood is widened to 0.84
  voxels so it stays 6-connected at 1, 0.5 and 0.25 m. Skeletons are cached per
  worker in a 64-slot table keyed by cell. Giant decisions are always made
  from exact columns (LOD callers included, so proxies and voxels agree) and
  cached per worker, because one site query costs milliseconds.
- `giant_tree_top_near` lets streaming residency keep a whole giant crown
  resident (the ordinary 26 m tree margin is too low for 30-44 m giants).
- Autumn-gold crowns are a shading variant (a rare regional field on leaves
  in `world_shader.wgsl`), not a new block, so the save format is unchanged.
  Pale bark is not done: a regional bark field also whitened log houses, and
  a distinct pale wood needs a new block id and material row.
