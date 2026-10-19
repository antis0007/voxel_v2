# Camera-linked ground shading and surface detail

This earlier isolated review is superseded by the integrated release documented
in [INTEGRATED_LIGHTING_REVIEW.md](INTEGRATED_LIGHTING_REVIEW.md). The latest
release reproduces the reported save, fixes indirect-lighting stripes, and
integrates the horizon-buffer repair. The details below record the earlier
fallback build and its original acceptance limits.

The reviewed game is `run-visual-glitch-fixes.cmd`. Its frozen source and build
receipt live in `target/visual-glitch-review/accepted-source` and
`target/visual-glitch-review/build-receipt.json`.

## Changes

- Sun-shadow PCF now evaluates the receiver plane at each filter tap, including
  the neighboring cascade during blending. Screen derivatives are evaluated
  before divergent material branches. This addresses slope-dependent false
  shadowing without disabling real shadows or increasing filter samples.
- Sand retains its authored material identity inland and in player builds;
  climate no longer replaces it with green turf.
- Stones use irregular, rotated, flattened rings and caps instead of identical
  four-sided pyramids. Sand has one quarter of the previous stone density.
  Geometry remains inside the existing per-detail 20-vertex/60-index memory
  reservation and retains physical sizing across voxel scales.
- The reviewed bundle includes the shared continuous analytic mist update,
  replacing the older 12-sample surface shadow march. Automatic VRAM budgeting
  remains enabled.

## Actual rendering verification

No tests or CI were added or run. Local offline release builds and actual native
renderer captures were used. Six low-sun sand views include the previous PCF,
indirect-lighting disabled, receiver-plane PCF, cloud-shadow isolation, a
translated camera, and a rotated camera. Images are in
`target/visual-glitch-review/shadow-ab`. No rectangular patch appeared in these
controlled views; the exact original save/pose was not reproduced, and the PCF
comparison alone does not prove it caused the reported patch.

The normal game executable also completed an edited reference-courtyard run at
1200x700, with its native capture in `final-capture/OneMeter-noon.png`. Final
streaming reached all 756 requested chunks with no missing chunks. No clean FPS
comparison is claimed because concurrent build/review activity affected this
session. Runtime stderr contains the existing preferred-save-directory fallback
notice; the accepted runs have no renderer validation failure or device loss.

## Shared-checkout integration

The initial latest-root candidate exposed an unrelated oversized horizon-buffer
crash after courtyard edits (343,953,568 bytes versus a 268,435,456-byte device
limit). Its owners are repairing edited-horizon expansion and vegetation. Their
live source changes remain intact. This task's reviewed executable uses the
previously running frozen renderer baseline plus these visual fixes and matching
mist shaders, rather than publishing the crashing intermediate integration.
The separate `final-source` snapshot records that unsuccessful integration;
`accepted-source` is the source for the delivered executable.
