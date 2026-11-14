# River and terrain consistency

The distant water appender previously generated only a sea-level sheet. It now
also reads the same river/lake surface sampler as voxel generation and clips
elevated water to the existing bed triangles. Explicitly dry samples bound the
shoreline. Inland surfaces use fine-ring triangles with edges at most 64 m;
kilometre-scale triangles cannot infer a narrow river. This is visual geometry,
not a source of new simulated water.

Wet chunks use their canonical solid mesh at every terrain LOD. Coarse terrain
tops can otherwise round up above a shallow river's exact water surface and
hide it. The existing mesh planner admits and charges the actual output before
building. Transparent water also no longer occludes coarse ground faces.

Dry coarse terrain keeps unsupported thin ledges rather than discarding their
only occupied cell during median filtering. Coarse trees use a separate,
constant two-voxel grid, so an eight-voxel terrain LOD does not inflate trunks
and crowns into large boxes. The ordinary source, GPU and horizon budgets stay
in force; runtime frame time and visual transitions still need review.

Streamed water activation compares exact fill, including saved fractional
deficits. A real gradient inside one legacy eighth can resume after reload.
Flat equal-level basins can still rest, and the finite fluid owner remains
responsible for movement and drainage. Continuous catchment/runoff supply is
outside this slice; rendering a surface does not prove sustained river flow.

No tests or CI were run. Compile evidence is in
`target/river-consistency-check.log` and its `.exit` receipt. Candidate build
evidence is in `target/river-consistency-build.log`. The candidate executable is
`target/voxel-river-consistency.exe`; the shared launcher routing is preserved.
No live visual acceptance of the supplied screenshot's world is claimed.
