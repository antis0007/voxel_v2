# Generator 46: inhabited dungeon rooms

Four room types extend the existing graph: scriptorium, refectory, infirmary
and armory. Job-specific programmes rotate ordinary room uses before repeating;
entrances, reveal halls, guard puzzles, sealed cores and dead-end rewards keep
their existing roles. Each new room has a distinct furniture layout, floor
finish and location name. Libraries use bookcase banks and reading desks, with
seeded variants adding interior stacks to sufficiently wide rooms.

Bookcase, CarvedStone and GlazedTiles are appended to the material registry;
previous numeric block IDs remain unchanged. They use ordinary editable solid
voxels, collision, inventory selection and persistence. Bookcases inherit wood
combustion. They are building materials, not readable-book inventories.

Furniture is specified in metres. At 1 m, a case retains its book material;
at 0.5/0.25 m the uprights resolve into timber voxels. Shelf divisions remain
in the material to avoid cropping book spines with coarse timber slabs. Shader family
45 draws metre-aligned rows, coloured bindings and spine bands continuously
across voxel faces, with filtered detail at distance. It does not stretch an
entire bookcase model into each tiny voxel or render outside its solid volume.

The construction kit clears door throats before furnishing and keeps the room
centre cross clear. Existing galleries and stairs retain sampling precedence.
Historical generators below 46 do not select the new rooms or furnishings.

Native review modes: `room:library`, `room:refectory`, `room:infirmary`,
`room:armory`. They find actual generated rooms and put the camera on the central
aisle, with no injected demonstration geometry. Camera review is not traversal
or inventory/persistence acceptance. No automated tests were added or run.

Validation on 2026-10-24: offline worldgen check and full game build passed;
the game build validated 18 WGSL programs and reported 31 warnings. Native
library captures used seed 424242 and the same physical camera at 1/0.5/0.25 m.
The initial fine-scale views exposed cropped book spines and duplicated pier
torches. Both were corrected and the fine-scale views recaptured and inspected.

Evidence under `target/place-review/`:
- `rooms46-library-1m/camera.png` (initial candidate; coarse shelf unchanged)
- `rooms46-library-half-final/camera.png`
- `rooms46-library-quarter-final/camera.png`
- `rooms46-infirmary-half/camera.png` (inspected tiled ward and side furniture)

Each directory includes arguments and executable SHA-256 in its manifest.
These establish rendered furniture at the three scales, not exhaustive seeded
navigation, mining, inventory round trips or persistence acceptance.
