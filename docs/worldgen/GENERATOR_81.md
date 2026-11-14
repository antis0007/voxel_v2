# Generator 81: seal courts and folded passages

New Manyfold worlds add six inscribed courts and three pairs of overhead fold
arches to the existing 144-cell tube. Versions through 80 retain their sampler.
The brass-weight passage mechanism remains intact.

- Return seals send an approaching walker back toward the circumferential street.
- Misroute seals send them to another district's street.
- Reversal seals turn their body, aim and momentum back toward the approach.
- One, two or three strokes identify the effect and its matching arch. Chevrons
  show the triggering direction. Every court has a physical lane around the line.
- Use **F** on a crystal anchor to erase its inscription. Mining also works;
  rebuilding the crystal rearms it. These are ordinary persistent terrain edits,
  shared through existing multiplayer replication, with no separate puzzle save.
- Erasing both opposed anchors opens their paired arches. Copper keystones must
  remain intact. Crossings preserve body scale, transform velocity and the local
  up frame, and sweep the remaining movement against the destination capsule.
- Blocked or unloaded edited exits refuse traversal. A bounded upward step handles
  the change in voxel stair height. Walkers require support below their exit.
- Joiner reports are unfolded to a bounded, collision-checked source approach.
  Active lines cannot be ignored through the ordinary direct-movement admission.
- Luminous strokes, pulsing seals, arch threads and audio cues expose state.
  Arches are marked thresholds, not rendered windows into their destinations.

The runtime is in `crates/app/src/manyfold/weave.rs`; architecture and anchor
coordinates are in `crates/worldgen/src/manyfold/weave.rs`.

Validation: no tests run, as requested. The full offline application check passed
(`target/manyfold-weave-final-check.log`). The offline `release-fast` build passed
(`target/manyfold-weave-build.log`); the frozen executable is
`target/voxel-manyfold-weave.exe`. Solo traversal, save/reload, two-player corrections
and visual/audio quality remain unverified. A review-scene material typo was corrected from the
nonexistent `GlazedTealTiles` variant to the existing `GlazedTiles` material.
