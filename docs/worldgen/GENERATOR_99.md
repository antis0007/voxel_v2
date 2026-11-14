# Generator 99: irregular landmark geology

New Hills and Ridged worlds retain compact landmark spacing. This pass changes
their physical shape, not the global palette, shaders or scale. Historical
generator versions keep their existing rules.

- Arches retain the flared leg profile introduced in generator 98. Broad spatial
  variation changes span thickness, crown height and cross-section together.
  The opening still stops at the valley floor and ascending road.
- Canyon shoulders have broad variations in crest height rather than repeating
  only sine-wave ridges.
- Buttes have different widths, warped outlines, uneven caps and broader talus
  skirts. The same field forms their full physical mass.
- Lookout platforms retain their flat foundation but blend into an irregular
  apron rather than ending as a uniform rounded mound.
- Cliff shelves use a lens-shaped eroded recess which pinches out at its ends,
  replacing the rectangular alcove. The ridge itself bends with the same broad
  variation field.
- Sedimentary contacts are farther apart, slightly tilted and undulating.

These are geological shape heuristics, not a simulated erosion history. New
landmarks own their voxel field before cavern lining is applied, preventing
rooms from refilling their openings. Column heights, horizon sampling and
physical blocks share the same height field. Cave and arch openings remain
volumetric cuts beneath that height field.

Review with `tools/Review-Vistas.ps1 -Generator 99` and an explicitly supplied
new executable. Manifests record the executable hash and scene conditions.
Native camera review does not establish survival traversal or performance.
No tests or hosted CI are required for this pass.

The application build passed. Native arch, butte and shelf captures were
inspected at seed 424242, one-metre voxels, 1600 by 900, clear midday weather and
eight-chunk view distance, with the normal horizon and clouds enabled. All three
produced settled `camera.png` captures in `natural99-review`. The rock shading
still has strong repeated face contrast; this is a shape improvement rather
than a claim of photorealistic rendering. No tests or hosted CI were run for
this work.
