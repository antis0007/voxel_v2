# Material-driven places, generator 55

The user supplied Addendum A again on 2026-10-26. Its SHA256 is identical to
`docs/specs/MOD_ADDENDUM.md`:
`416174BE001E0D7BB2DCCBD7502CD077888C8F0F8ACC591BD5A67B3270C56A4B`.
The existing `docs/research/MOD_FEATURE_MATRIX.md` already contains 422 planned
features, including the A.14 minimums and ten shared A.15 requirements. It remains
the adoption inventory, not a list of implemented features.

## This implementation

| Inspiration | Concrete change | Existing mechanism reused | Limit |
| --- | --- | --- | --- |
| Noita | Separate hot/cold forge basins | Lava, water, basalt and the existing contact reaction table | Native simulation of a deliberate breach still needs verification |
| RimWorld | Smith's yard groups workbench, fuel, ore and storage beside service circulation | Existing settlement reservations and material blocks | No pawn work assignment or hauling simulation added |
| Modded Minecraft | Distinct processing bays and recoverable feedstock | Existing workbench/smelter materials and ore blocks | No new recipe chain or automatically registered factory network |
| Obscuria direction | Flooded deep pit side bays with dry salvage approaches | Actual water cells, shaft, stone paths and existing cache types | No new pressure, cold, music or boss-phase system |

See `crates/worldgen/GENERATOR_55.md` for generation and compatibility details.
All changes are gated at 55; this pass does not migrate player saves.

## Validation

Focused whitespace checks passed. No tests or hosted CI were run, per the user.
Initial offline build compiled worldgen and validated 19 composed shaders, then
failed in concurrent chip-production code at `production.rs:475` because
`Node.lab_recipe` did not exist. That file was not changed by this pass.
The concurrent field was added by its owner and the full offline build then
passed. Final build log: `target/world-inspiration-20261026/build55-r7.log`.
Frozen binary: `target/world-inspiration-20261026/candidate55-r7.exe`, SHA256
`F5B3C5A407099DB81EA1A1341B03DA66310ADA5520334AF48F54B23EAC235E94`.
The binary includes unrelated concurrent work; its presence is not attributed
to this change.

Review evidence lives under `target/place-review/` with per-run manifests:

- `inspiration55-place-town`: R3 native view of the service yard, seed 2738958700.
  R6 retains the same settlement geometry. R3 SHA256:
  `C3E22921E0F6DED2C4BC5B4619E7E32E821C7BBC8D958E06F7EA8D7CB680C1CD`.
- `inspiration55-room-forge`: **rejected evidence**. The old review helper
  silently fell back to the demonstration hall when no forge was found.
  Named-room fallback is now disabled and room lookup cached once per process.
- `inspiration55-place-pitflood`: **failed**, exceeded the 180-second bound
  while looking for a two-landing site. Flooded bays now apply at the first
  landing too, and the reviewer uses the same bounded ordinary-pit selection.
- `inspiration55-pitflood-r6`: verified water bays and stone routes, seed
  2738958700. The view exposed poor route lighting; R7 adds four supported lamps.
- `inspiration55-forge-r6`: verified actual generated forge, seed 424242,
  eye `(681, 0.2, 2043.5)`, target `(681, -2.5, 2058.5)`. Both contained
  liquids and the central aisle are visible. The larger bounded search finds
  this forge; the earlier seed did not find one within the old search radius.
  R6 SHA256: `708E8A118D1960503953A36FDF7B10DE02D0D95F75F6E1F32FBA088A6B6EC248`.
  R7 retains the same forge geometry.
- Final pit lighting review: `inspiration55-pitflood-r7`.

Native review runs disable physics; screenshots establish rendering only.
Fluid release, reactive contact, walking the dry routes, machine registration,
and persistence are not established by these captures.

The wider visual overhaul remains unfinished. The town is still sparse, distant
tree canopies have LOD artifacts, and the forge's dark material palette needs
further art-direction work. This is an implemented world-content increment,
not completion of the mod adoption matrix.
