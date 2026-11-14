# Axiom visual overhaul: construction slice

## Scope and ownership

The two supplied critiques define the larger overhaul. This pass establishes
`ART_DIRECTION.md` and addresses settlement construction and paving. Existing
dirty renderer, worldgen, wildlife, factory, UI and Blender work was preserved.
This is not completion of the full overhaul or the 17-asset workshop milestone.

## Changes

- Generator 56 settlement houses use a metre-space construction kit in
  `crates/worldgen/src/valley/settlement/craft.rs`: roof overhangs, thin roof
  shells, ridge/verge timber, filled gables with trusses, wall plates, posts,
  stone bases and framed open windows. Old generator versions retain the
  previous settlement sampler.
- Main approaches use worn dirt, gravel shoulders and broad stone courses;
  the square has a paving border and service paths remain gravel.
- `place:workshop` and `place:workshop-side` are gameplay-height camera presets
  for the actual settlement. They do not place demonstration assets.
- Existing wildlife compilation was repaired with two hash module path fixes
  and visibility sufficient to re-export its model draw function. Behaviour
  was not redesigned.

## Pipeline audit

The current world shader already has a 20 texel/metre albedo lattice,
explicit sRGB-to-linear conversion and material roughness/specular functions.
The held-asset pipeline already contains editable Blender source and a pixel
atlas. These are existing implementations, not deliverables from this pass.
No speculative filtering or global saturation change was made.

There is no glass block. The new windows are open frames; using Ice as glass
would misrepresent material behaviour. Roofs use existing Planks, which still
need a dedicated authored roof material. Existing foliage placement already
uses substrate, local shade/water/disturbance and correlated species patches;
its current thin polygon silhouettes still fail the supplied artistic target.

## Verification

- Optimized `release-fast` application build succeeded. Its build script
  validated 19 composed WGSL programs. Existing warnings remain.
- Focused test `working_bay_and_door_remain_open_at_supported_scales` passed:
  entry/working-bay clearance, roof overhang, hollow loft and bounded footprint
  at 1, 0.5 and 0.25 metre scales. This is sampler verification, not traversal.
- Frozen native candidates: `target/axiom-art-20261026/craft-r1.exe` and
  `craft-r2.exe`. Revision 2 quiets metre-wide roof trim and removes coarse
  diagonal trusses after native inspection found an overly heavy dark gable.
- Matched seed 424242, daylight, 1 m, `place:workshop` captures:
  `target/place-review/axiom-craft-before/` (55) and
  `target/place-review/axiom-craft-after/` (56). Each manifest records arguments,
  environment and executable hash. An additional baseline with seed 2738958700
  is in `axiom-craft-before-valid`; despite its name the first baseline is valid.
- Revision 2 capture: `target/place-review/axiom-craft-after-r2/`. The focused
  sampler test was rerun after revising the roof pitch and passed again.
- Final comparison uses `axiom-craft-before-r2` and `axiom-craft-after-r2`,
  both from `craft-r2.exe`, SHA256
  `26D7AF2F122DB042193A172F86545F1EB00CFAA2D4690B4986EDE116FADED326`.
  Other workspace work changed vegetation between candidate builds; those
  changes are not attributed to this pass. Do not compare r1 foliage to r2.
- Capture timings include readback and startup/streaming. No isolated CPU/GPU
  performance claim or simulation benchmark is made.

## Native critique

Revision 1 overemphasized dark, metre-wide verge blocks and diagonal trusses.
Revision 2 removes that heavy trim at coarse scale, lowers single-storey roof
pitch and restores readable pale gable infill around a central post. The roof
still has large stepped edges and needs finer construction detail and a proper
roof material. The yard roof remains a flat plank slab. The workshop bay is
dark; this has not been accepted as finished lighting. These are visible
remaining defects, not a claim of reference-level artwork.

## Remaining work, in visual-impact order

1. Author roof tiles, stone and timber clusters; improve coarse-scale truss and
   window proportions from native captures. Review both side and interior views.
2. Replace the thin foliage shapes with a small authored species library;
   compose meadow/forest floor patches and tree crown/branch families.
3. Build the specified workshop integration scene. The referenced downloadable
   40 KB production pack and exact 17-item inventory were not attached. Eight
   material families, three real connected interactions, an actual condenser
   and shield emitter remain unimplemented here.
4. Add a neutral material review scene and compare daylight, torchlight and
   magical illumination; test proposed 32 px/m against the existing lattice.
5. Review/revise the existing held assets from near, side and moving native
   views. This pass did not remodel the hand or wand.
6. Validate multiscale native appearance, LOD motion, destruction, collision,
   and isolated performance. Identify the bright sky lines through a controlled
   scene comparison; their cause is not established by the screenshot.

No new shield shader, machine simulation, Blender asset family, ecological
simulation or complete terrain overhaul is claimed in this slice.
