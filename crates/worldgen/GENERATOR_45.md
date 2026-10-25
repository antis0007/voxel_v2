# Generator 45: woodland anatomy, shore sections and hall structure

New generation is gated at 45. Older worlds retain their generator identity;
there is no migration or save-format change. Rendering changes apply to older
worlds too. The shared workspace also contains independent generator 43/44 work.

- Classic broadleaf trees gain age-dependent proportions, bent connected
  scaffolds, unequal crown lobes and flared old-tree roots. Wood and leaves are
  ordinary editable voxels. Existing conifer, umbrella and weeping forms remain.
- Broad stand-density patches reserve glades through the existing vegetation
  decision. Resident and horizon generation share this decision.
- Sandy/shingle shores vary berm, swale and dune-toe strength together over
  broad sections. Upper sediment coverage varies in patches instead of a fixed
  contour band. No new terrain layer, fluid solver or independent scatter cache.
- The Weighing Hall gains a pale central route, supported lamps, pilasters and
  stepped ceiling ribs. Lamp additions are eight single source voxels across
  supported scales, avoiding clusters whose cost/brightness grows with scale.
- Dungeon masonry repairs occupy construction bays instead of per-voxel noise.

Local validation: `cargo test --offline --target-dir target -p engine_worldgen
painterly` checks woody connectivity/crown bounds, clear hall routes/supported
lights/source count, and bounded continuous shore sections. These focused tests
do not certify every seed, traversal, harvesting or saved-world reload.

`cargo run --offline --target-dir target -p engine_worldgen --example
painterly_locations` finds real generated coast/old-tree review cameras for seed
2738958700. It does not insert scenery or modify a save.

Native capture evidence, limitations and the unfinished larger visual brief are
tracked in `docs/design/PAINTERLY_20261024_CHECKPOINT.md`.
