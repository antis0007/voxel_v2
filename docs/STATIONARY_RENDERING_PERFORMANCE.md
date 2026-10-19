# Stationary rendering follow-up

The user's report of low FPS while standing still shifts priority from streaming
CPU work to repeated rendering work. An existing capture at
`target/cloud-quality-review/colour-integrated/review.log` reports 8.949 ms cloud
raymarch, 3.067 ms shadow and 0.304 ms terrain GPU time on an RTX 4090 at 1600x900.
This is older evidence from a different review scene, not a measurement of this
change. The old shadow timing also omitted a fourth cascade (see below).

Changes in this follow-up:

- Shadow invalidation is tracked separately for each texture layer. Routine
  updates refresh the near cascade immediately and rotate fairly through the
  outer three cascades, selecting at most two per frame. Continuous mesh uploads
  cannot starve an outer cascade. Startup, large camera jumps, world-scale and
  split-distance changes refresh all four immediately. Each active projection
  is published only with its matching depth layer. Outer shadows may lag by up
  to three rendered frames during continuous change; map resolution and caster
  geometry are unchanged.
- Cascade culling and generation-buffer writes run only for selected layers.
  Shadow draw-call statistics report submitted work rather than cached lists.
- GPU timing previously reserved three shadow query pairs while submitting
  four shadow passes. The fourth pair overlapped the terrain pair. Capacity and
  pass indexing now derive from all four cascades, and queries are allocated only
  for layers actually rendered.
- Aerial-perspective generation reuses its froxel volume when camera projection,
  position, world scale, extent and lighting are unchanged within the existing
  atmosphere tolerance. Camera movement invalidates immediately. Lighting is
  compared against the last solve, so small changes accumulate rather than being
  ignored indefinitely. Transmittance-only mode now uploads current lighting
  parameters before AP generation as well.

Concurrent cloud-filtering work in the shared sources is preserved in the
integrated executable: adapters supporting FLOAT32_FILTERABLE use hardware
weather filtering; other adapters retain the explicit four-tap path. Both
composed shader variants are validated by the existing build compiler.

Verification: local compilation and optimized executable build only. No tests,
CI, game launch or fresh FPS capture. The pass-count reduction is established by
the scheduler; its measured GPU benefit and shadow-transition appearance still
need runtime acceptance.

Optimized candidate: `target/stationary-render-review/voxel-workshop-fps-fix.exe`.
SHA256: `1219B3FF9902A58BD1F9F479F49E60615391AF730E6C07510B2056B2F601C30A`.
Build succeeded in 1m 06s; log: `target/stationary-render-release.log`.
