# Courtyard roof egress

New courtyard districts keep a one-metre opening above the highest exterior stair. The opening removes one parapet cell from the authored paid bill, rather than placing a free block, teleporting a builder, weakening collision or forcing work-order priority. The remaining roof perimeter is unchanged.

The gate coordinate is `(WIDTH - 1, storeys * 4 + 1, DEPTH - storeys * 4)`. This keeps the final crossing at one metre of elevation change. The old continuous parapet created a two-metre crossing and could strand a builder before the final four blocks were placed.

## Save contract

`Lot` and `Parcel` have a serde-default `roof_access` field. False is omitted during serialization. Existing lots, parcels and design-version-1 saves keep their original geometry and bill; there is no automatic save rewrite or retrofit.

New district designs use design schema 2 and enable the opening only for Courtyard architecture. The enclosing layout remains version 2. This lets the previous executable reject a new gated design instead of silently ignoring the flag and restoring a different bill. Schema 1 rejects enabled gate flags; schema 2 requires the correct flags for its architectural tradition. The paid lot must match the saved parcel's flag.

## Checks

- Focused comparison against commit `941b4e0`: exact legacy cell sequences across three styles, three heights, two orientations, three variation seeds and three voxel scales. The opt-in courtyard delta is exactly one coarse material cell, or its equivalent scaled volume. Other styles are unchanged.
- Grounded-grid connectivity has a negative control: the old closed roof is unreachable from the ground, while the gated roof is reachable at one, two and three storeys.
- All 27 district combinations pass the new schema round trip and old-schema compatibility/rejection checks.
- Native acceptance adds 54 scaled bill checks, courtyard headroom checks at all three scales, and six actual grounded stair/roof/ground round trips at the currently supported one-metre fixture scale. The mover requires five-centimetre horizontal arrival, grounded contact and the expected elevation at each waypoint.
- After the existing full paid-construction gate passes, the check walks to the roof and back, then queries and physically replays the stockpile return route from each builder's actual completion position. These are grounded player-physics route replays, not a claim that an autonomous idle routine moved every builder home.
- An optional `VOXEL_CITY_ROOF_LEGACY_CHECKPOINT` loads an existing closed-roof checkpoint read-only and verifies that its original recipe flags are retained.

## Native result

The exact previously failing Mixed / seed 424242 / Courtyard / one-storey paid case passes on `city-growth-roof-v3.exe`:

- 441 blocks paid and placed in 58,200 simulation ticks; builder contributions 153 / 154 / 134, with zero unused material.
- Interrupted carried-material reload, exact accounting, final occupancy and sidecar reload all pass.
- Completed street-to-interior walk: 38.20 m. Completed roof ascent/descent: 26.30 m.
- Stockpile route replays from the three actual completion positions: 16.11 m, 37.64 m and 43.92 m. Two positions were on the roof. Every route reaches the stockpile while grounded.
- The six independent roof round trips pass for one, two and three storeys in both orientations. They cover the native one-metre fixture; the three-scale tests separately validate geometry, headroom and material volume.
- All 27 district plans, timber 530-block/eight-trade construction, terrace 450-block construction, original 561-block/eight-trade market and nine legacy layout walks pass on that same executable.
- The original closed-roof interrupted checkpoint is read and validated unchanged. No save header or world storage version is edited.

The combined build completed in 9m31s under competing compiler load, with 424 warnings. This is not a clean performance benchmark. The complete workspace test suite was not run. The executable contains the concurrently frozen housing-growth work as well as this roof change; source commits are scoped separately.

SHA256: `18C08FA59A0BED376994D14437F7EFA49B096AE04B9EE3AF9E26D9BC09F37839`.

## Reproduce and inspect

```powershell
./tools/Run-CityDistrict.ps1 -Check -Layout Mixed -Seed 424242 -Tradition 1 -Executable ../city-growth-roof-v3.exe
./tools/Run-CityDistrict.ps1 -Matrix -Executable ../city-growth-roof-v3.exe
```

Session task outputs:

- `city-roof-paid-v3-20261107/construction-check.json` and `paid-roof-access.json`.
- `city-roof-matrix-v3-20261107/roof-access-matrix.json` and `district-plans.json`.
- `city-roof-timber-v3-20261107`, `city-roof-terrace-v3-20261107`, `city-roof-legacy-market-v3-20261107` and `city-roof-legacy-layout-v3-20261107`.
- Before: `city-roof-before-20261107/bench-000.png` from the previous failed design.
- After: `city-roof-after-20261107/bench-000.png` from the completed gated design.

Both images were visually inspected. They share camera `28,35,18,-0.6,-0.5`, 1280x720, time 0.4 and clear weather. The after view shows the completed perimeter and permanent opening above the exterior stair.

## Integration boundary

This is an opt-in district change on top of `941b4e0`. The shared checkout is updated only by guarded integration and a fresh shared-source build. Do not copy the private repository wholesale or replace shared storage/version files. Existing saves are preserved; an old closed-roof design is not silently retrofitted.
