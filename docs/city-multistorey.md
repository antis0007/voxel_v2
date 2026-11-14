# Paid two-storey construction

This is an opt-in local construction slice, not a general district simulator.

## Run

From the private checkout, with a newly built executable:

```powershell
$env:VOXEL_CITY_START_SITE='1'
$env:VOXEL_CITY_COLONY='0'
$env:VOXEL_CITY_MULTISTOREY='1'
$env:VOXEL_CITY_LAYOUT='Legacy'
$env:VOXEL_CITY_MARKET='0'
& ..\city-daily-district-v1.exe --city-quarter-check=..\city-multistorey-review
```

Use a new output directory. The ordinary scene still selects `city_start_site.ron` when the multistorey flag is absent. The new flag selects `city_two_storey_site.ron`. The existing automated `Play-CityStartSite.ps1` launcher can capture this scene with the flag set and `-PreviewTicks 150000`.

## Mechanics

- Three existing physical workers gather finite timber, craft planks and chests, and deliver and place every project cell through the existing construction jobs.
- The two-storey house has a ground entrance, an upper side entrance, a two-wide switchback and a bridge to its roof. Its shell, access steps and apron are all paid construction cells.
- `floors` defaults to one. Two-floor layouts require a depth of at least six cells. The complete project must remain within the existing 256-cell cap.
- `home_floor` chooses the existing home entry/interior contract. It does not create another population or inventory authority. This serial three-worker fixture does not run the colony household allocator or prove upstairs sleeping routines.
- `variant_seed` uses the engine hash for repeatable rear-window variation. It does not claim district-level procedural variety.
- Persisted project and state schemas are unchanged. Saved cell orders remain authoritative. Legacy site and colony assets are unchanged.

## Evidence, 7 November 2026

The combined private binary `city-daily-district-v1.exe` built successfully. Its opt-in checker exited zero:

- 441 paid cells completed: work shelter 118, two-storey home 215, storehouse 108.
- 124 finite logs supplied 438 placed planks and three crafted chests. Runtime checks conserved logs, planks, chests and meals.
- Initial reload and a reload while carrying construction materials preserved obligations and cargo.
- A 23.725 m grounded player route ascended the stairs, crossed the upper entrance and reached the roof.
- Completion took 123,360 simulation ticks and 1.666 seconds wall time in this three-worker check. This is not a large-city benchmark or a rendering performance claim.

`building.rs` contains five standalone tests, run with:

```powershell
rustc --edition=2024 --test crates\app\src\world\farming\jobs\quarter\frontier\building.rs -o ..\city-multistorey-geometry-tests.exe
& ..\city-multistorey-geometry-tests.exe
```

They cover 24 width/depth/stair-side route cases, headroom, per-layer construction support, mirrored materials, deterministic variants and invalid shallow landings. An additional planner-only harness compared all nine existing site/colony project outputs against baseline `ee00ec4`; cells, order and home metadata matched. It also verified the new site's nonoverlap, material sufficiency and 24 m service radius. All seven focused checks passed.

Native evidence is in the sibling `city-multistorey-v1-20261107` directory. Automated before/after captures are in `city-multistorey-before-view-20261107/bench-000.png` and `city-multistorey-after-view-20261107/bench-000.png`.

## Boundaries

The live paid scenario supports one-metre voxels only, matching the existing production installer. Finer-scale construction was not enabled or claimed. The new home is one destination with configured capacity, not separate households per floor. Wider streets, district economics, trading posts and generalized multistorey redevelopment are outside this patch. Existing separate market fixtures are not integrated here. The shared checkout and user saves were not changed by this work.
