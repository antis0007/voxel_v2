# Seeded district design fixture

This opt-in increment uses Axiom's existing architecture, terrain survey, grounded resident navigation, paid construction, finite timber salvage and barter owners. It is a six-parcel design/construction fixture, not a full autonomous city simulation. Default colony, legacy quarter and one-floor start-site behavior remain separate.

## What is implemented

- A connected 9 m boulevard, 5 m frontage streets (tapered to 3 m near door approaches), two 3 m cross-link alleys and a level 15 x 7 m public plaza.
- Ordered, Mixed and Organic route costs and seeded parcel setbacks. Direction-state A* surveys the complete street width, rejects exclusions and slopes greater than one metre, and keeps paving clear of structures, construction frontage, stockpile and salvage yard.
- One coherent architectural tradition per district, with deterministic one-to-three-storey variation, existing windows/balconies and the original material palettes. Available traditions are Timber Framing, Courtyard Masonry and Terraced Stone.
- Authored land-use reservations: three Housing parcels, a Market Hall, a Workshop and Storage. Only the timber Market Hall has a staffed functional supply/trade service. Workshop and Storage are design reservations, not remote production facilities.
- The paid home uses the existing three-builder job pipeline and exact material bill. Roads and the other five buildings are explicitly supplied review scenery. The optional fourth actor salvages eight finite log blocks, hauls them to the market chest and sells them for eight physical iron units.
- Saved version-2 plans retain their seed, parcels, road paths, widths, uses and service positions. Legacy version-1 plans omit the new optional design field.
- District UI counts its physical assigned worker roster instead of inventing four new people when a house completes.

## Run

Build the native app in the usual way. From the repository root in PowerShell:

```powershell
./tools/Run-CityDistrict.ps1 -Layout Ordered -Seed 7 -Tradition 0 -Market
./tools/Run-CityDistrict.ps1 -Check -Layout Ordered -Seed 7 -Tradition 0 -Market
./tools/Run-CityDistrict.ps1 -Matrix
./tools/Run-CityDistrict.ps1 -Capture -Layout Mixed -Seed 424242 -Tradition 1
./tools/Run-CityDistrict.ps1 -Capture -Layout Organic -Seed 8675309 -Tradition 2
```

`-Executable` selects a specific built binary. `-OutputRoot` must be a new disposable directory. The runner records executable SHA256 and arguments, sets a separate save directory, and restores its environment afterwards. It never opens a user save. Capture mode is automated; no manual desktop control is required. Live timber barter requires Tradition 0 and is rejected for other material palettes before fixture mutation.

## Validation and evidence

- 27 deterministic plans: three profiles x seeds 7/424242/8675309 x three architectural traditions. RON round trips, repeated generation, all six road-connected frontages, valid building footprints and the paid project's 24 m service bound are checked.
- Negative cases reject overlapping parcels, reversed frontage endpoints, invalid street widths, extreme coordinates, timber reserved on a building and a cliff inside the plaza. A blocked cell within the widened boulevard causes a detour instead of being paved.
- Existing geometry checks cover 432 work-clearance samples, nine architectural routes and 27 volume cases at 1 m, 0.5 m and 0.25 m voxel sizes. The full district fixture itself is currently supported at 1 m only.
- Three native grounded street/interior matrices traversed 547.11 m in total. This proves the reviewed access routes, not every possible route or city scale.
- Ordered seed 7 timber: three builders completed 530 paid blocks in 191,340 simulation ticks, with contributions 182/171/177. All eight salvaged logs were traded for eight iron. Construction-cargo and carried-payment checkpoints, exact material accounting, final save reload and a 38.59 m grounded completed-building walk passed.
- Organic seed 8675309 terraced-stone: three builders completed 450 paid blocks in 59,340 ticks, with three actual roster entries and all cargo/conservation/reload gates passing.
- Legacy layout checks passed nine plans, 27 scale cases, 27 service-radius cases and nine native walks. The final executable also passed the original 561-block market fixture, eight trades, carried checkpoints and completed-building/covered-market walks.
- Saved operational projects are required to match their validated district parcel, including origin, style, height, orientation, variation and frontage. Mutated saved projects are rejected while valid in-progress layers remain loadable.
- A matrix run with spaces in its output path verified launcher quoting.
- The 27 plans used 1,771-9,065 A* expansions and 1,009-1,205 paved square metres each. Route queries retain a 20,000-expansion cap; total v2 provenance is bounded at 380,000. These are bounded fixture results, not a city-scale frame-rate claim. Native build/capture timing was contended with other authorized work.

## Courtyard roof access

The initial Mixed seed 424242 courtyard check exposed a closed-parapet defect: four paid blocks remained, and the stair-to-roof crossing became two metres high. The original failed logs and screenshots are retained as baseline evidence.

New courtyard districts now keep a permanent one-metre opening above the highest exterior stair. The formerly failing case completes all 441 paid blocks in 58,200 ticks and passes grounded roof-return and all completed-worker stockpile-route checks. See [Courtyard roof egress](city-roof-egress.md) for the exact save contract, tests, current executable and before/after captures.

Existing design-version-1 saves keep their original geometry and bill. They are not automatically repaired or rewritten; the original warning remains for those closed-roof designs. New gated districts use design schema 2 so older executables cannot silently restore a different bill.

## Further limits

This is not an expanding multi-district city, a traffic simulation, zoning demand, functioning remote industry, unlimited timber production or a regional trade network. Terrain-aware routes use the fixture's surveyed heights; building parcels and plaza require level ground. Generation validates terrain, while structural validation of a modified saved plan does not re-survey an arbitrary changed world. The architecture meshes and building primitives predate this increment; the new behavior is coherent selection, compatible variation, district reservations, full-width routing and functional market integration.

## Review outputs

The initial layout-review binary is `city-district-design-v3.exe` in the session task directory, built from scoped private sources based on `940bc5b`. Its build completed in 4m31s with 423 warnings; the entire workspace test suite was not run. `just session-report 940bc5b` completed, but its aggregate untracked-source counts include the recovered repository and are not this change's size.

Each output folder contains a manifest and native logs. Captures use the same 1280x720 camera (`50,52,48,-0.72,-0.62`), time 0.4 and clear weather:

- `city-district-before-v3-20261107/bench-000.png`: original Ordered seed7 layout.
- `city-district-timber-v3-20261107/bench-000.png`: coherent Timber, Ordered seed7, completed paid home and eight trades.
- `city-district-courtyard-v3-20261107/bench-000.png`: Courtyard, Mixed seed424242, visibly marked roof-access limitation and incomplete paid home.
- `city-district-terrace-v3-20261107/bench-000.png`: Terraced Stone, Organic seed8675309, completed paid home.

The separate native result folders are `city-district-matrix-v3-20261107`, `city-district-market-v3-20261107`, `city-district-terrace-check-v3-20261107`, `city-district-legacy-market-v3-20261107` and `city-district-legacy-layout-v3-20261107`. The original failing courtyard gate is preserved in `city-district-courtyard-known-failure-v3-20261107`; its failure is not counted as a pass. The renderer's background ground cover can change during later frames, so the matched first frames are design comparisons, not a claim of visual determinism or render performance.
