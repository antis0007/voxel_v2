# Shoreline and water flow correction

The annotated shoreline reference shows coarse distant land, repeated trees,
reported flashing, isolated water marks, a dark shore band and repetitive water.
A still image does not prove that the flashing is depth fighting.

## Implemented

- `render/horizon_water.rs` clips water triangles against the sampled bed rather
  than emitting a full triangle whenever one corner is submerged. A 2.5 cm
  physical underlap keeps distant water behind resident water.
- `GeneratedChunk::water_depth_meters` estimates near-water depth from available
  authoritative columns. Unknown deeper beds retain a conservative depth;
  missing chunks are not treated as an empty bed. Depth and roof visibility are
  unpacked before interpolation so cave water retains occlusion.
- Water normals use six filtered wave scales with independent phase envelopes.
  This breaks coherent long ripple stripes without screen-space animation.
- Direct sunlight is shadowed by CSM rather than also multiplied by the
  approximate overhead sky mask. Ambient and bounced fill retain that mask.
  Atmospheric scattering no longer disappears on a dark receiver just because
  its overhead sky visibility is low.
- A final one-eighth water cell can drain toward an open lower neighbour.
  Flat single-quantum puddles do not wander. Neighbour flow order varies by cell.
- A bounded 48-cell hydraulic connection search permits a submerged outlet to
  rise below the head of a connected water column. It traverses filled water,
  stops at solids, suspends on missing source data, and transfers existing mass.
  The search radius and head scan are eight cells: this is a local pressure
  approximation, not an incompressible Navier-Stokes solver or unlimited pipes.

## Evidence and limits

`cargo test --offline --target-dir target water_tests -- --nocapture` passes all
three focused checks: last-quantum ledge drainage, pressure-driven outlet rise
with conserved mass, and no transfer through a solid wall. Log:
`target/shoreline-integration-review/water-tests.log`.

Frozen production source and hashes are in
`target/shoreline-integration-review/{source,source-hashes.json}`. Its optimized
offline build includes composed WGSL validation. Executable SHA-256:
`55E43AC02C633277B4DDB6AB040518251CC58AC62152A295A4805525569BD5AA`.

The native moving daylight run completed its measured interval with physics
enabled and zero missing visible chunks after settling. Captures and logs are
in `target/shoreline-integration-review/day`. Requested window dimensions were
overridden by the native window state: captures are 2560 by 1346. Observed
67.94 FPS is not an isolated performance comparison. The PowerShell wrapper
then treated an unavailable ExitCode as failure, so it did NOT launch sunset
or storm. Runtime stderr contains the analytic-mist status line, not a shader
validation error. The executable's numeric exit code was not recovered.

Daylight inspection still shows coarse distant land and repetitive, overly
textured clouds. No claim of final visual acceptance, exact reproduction of
the user's save, or elimination of all moving-shadow/LOD artifacts is made.
An unintegrated cloud morphology proposal exists at
`target/shoreline-integration-review/cloud-morphology-candidate.wgsl`; its isolated
development build passed composed WGSL validation, but has not been visually
accepted. Later shared rendering changes are not
represented by this frozen executable.
