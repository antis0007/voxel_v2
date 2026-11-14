# Generator 145: preserve watershed relief at distance

New Planet worlds and customized planets use 145. Existing saves retain their
stored generator version. The change is confined to the spherical Planet sampler;
other world types retain their current terrain rules.

## Source finding

The active spherical pipeline is `planetary`, introduced in 142, rather than the
older generator-71 regional drainage pipeline. It already routes and erodes a
connected spherical mesh, then samples receiver segments to carve river valleys.

In `planetary::World::ground`, the valley cross-section was multiplied by
`1 - smooth(45, 400, footprint)`. At a 400 metre footprint the entire valley carve
was absent, although the same location retained it at ground resolution. This
is a source-level explanation for one possible loss of distant relief, not proof
that it caused the supplied screenshot critique (no original captures accompanied
that critique).

From 145 the valley carve remains at full strength at all footprints. Narrow
riverbed filtering and province-detail filtering remain unchanged. This preserves
the valley envelope, not every subpixel riverbed. Coarse tessellation can still
miss a valley between vertices; this change does not establish mesh adequacy.
Versions 142–144 retain their previous formula.

## Acceptance still required

Compilation cannot establish landscape quality. The requested regional milestone
remains open: select a connected catchment several kilometres across, record its
seed, recipe, generator, coordinates and executable hash, and inspect the same
ridge/tributary/main-valley system in authoritative grayscale terrain, fixed-detail
near geometry and normal distant geometry. Review from high altitude, 1 km,
200 m, 20 m and on foot, from multiple directions.

Verify contiguous forest cores and margins, open grassland, exposed rock and an
accessible settlement site against drainage and soils. Record generation duration,
memory, streaming and frame times at both 1 m and 0.25 m voxel sizes. Do not infer
traversability, ecosystem quality or performance from this sampler change.

No tests or hosted CI are part of this change, per workspace instructions.
