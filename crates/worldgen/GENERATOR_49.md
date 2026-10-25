# Generator 49: coastal shoulders and courtyard gardens

New worlds use version 49. All changes below are gated at 49; versions 48 and
earlier retain their generation branches. No save-format change or migration.

- Coastal dry shoulders rise 1.2-4 m according to a continuous 38 m field,
  faded across the dry beach and back into inland terrain. The waterline stays
  fixed. This interrupts wide, low gravel terraces without small waterline pits.
- Beach surface limits follow broad sediment patches over a narrower elevation
  band. Gentle upper beach columns become grass over dirt; steep faces retain
  exposed rock. Rocky toe gravel is restricted to the lowest 0.65 m.
- Castle gatehouses become one bay deep while retaining their exterior threshold,
  freeing usable forecourt space. Side bays gain flush grass beds, limestone
  surrounds and a low outer bench. Existing route paving and a one-metre building clearance
  take precedence. Gatehouse and workshop sampling retain their own rules.

Implementation: `terrain_variety.rs`, `architecture/refinement.rs`,
`architecture.rs`, and the version constant in `voxel/src/config.rs`.
The location example accepts generator version and optional seed for reproducible
native review cameras. Render evidence is recorded in
`docs/design/WORLD_OVERHAUL_20261024.md`.
