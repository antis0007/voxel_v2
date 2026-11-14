# World places: generator 54

## Direction and implementation

Inspected Downloads references `sylvain-sarrailh-danghostisland.jpg`,
`tu-bui-dmz-overall.jpg`, `jung-park-boomtown.jpg` and
`Витаутас Лайсонас.jpg`. Applied their emphasis on dominant roof forms,
vertical layering, pale monumental walls and readable supporting structures.
These are composition references, not a claim of matching their image quality.

The implementation covers the six requested place families. See
`crates/worldgen/GENERATOR_54.md` for the versioned generation changes.
Existing stored generator versions are retained; these changes require a
generator-54 world. No migration or player-save overwrite was performed.

## Build and native evidence

Local offline `release-fast` build succeeded, including shader build validation.
No tests or hosted CI were run. Focused `git diff --check` passed.

Final frozen executable: `target/world-overhaul-20261026/candidate54-r7.exe`.
SHA256: `544F7069E6B812D109894D4463D40C4E633C9F76936C6D4C258B6F7A2B77332A`.
Build log: `target/world-overhaul-20261026/build54-r7.log`.
Earlier R5 exterior/library captures use SHA256
`438864DC754F8EEBB14041224D4E89CA95ADDDCDED387E60C0312C879CDDBFF3`.
R7 changes the mine beams, pit dimensions and pit camera; other place geometry
is the same as R5.
This is a shared working-tree build and also contains concurrent work outside
this change. Those changes are not attributed to this overhaul.

Native captures use `tools/Review-Places.ps1`: 1920x1080, one-metre voxels,
clear weather, four-chunk view, isolated generated review worlds. Each folder
contains the executable hash, arguments, camera receipt, logs and PNGs.

| Place | Capture folder under `target/place-review` | Seed |
| --- | --- | --- |
| Castle baseline, generator 53 | `overhaul53-startup-check` | 2738958700 |
| Castle, R5 | `overhaul54-castle-r5` | 2738958700 |
| Town, R5 | `overhaul54-town-r5` | 2738958700 |
| Tower, earlier candidate with same crown geometry | `overhaul54-tower` | 2738958700 |
| Mine, raised beam clearance | `overhaul54-mine-r7` | 2738958700 |
| Pit, widened chamber | `overhaul54-pit-r7` | 2738958700 |
| Dungeon library, revised nave camera | `overhaul54-library-r5` | 424242 |

Iteration removed the tree obscuring the castle gate, replaced grey-green
roof material with warm timber, steepened two tower caps and added two-storey
townhouses. Initial underground views were too close to walls; revised cameras
look through the mine workings, around the pit shaft and along the library nave.
Those views exposed legacy low mine beams crossing the raised workings, and
pit chambers that still read as narrow ring corridors. R7 removes the low beams
only in enlarged workings and adds three metres of chamber space on each side
of pit landings, moving their wall lights and structural piers outward.

## Acceptance limits

These are native renderer inspections, not physical traversal, save round trips,
or performance acceptance. Room dimensions and dressing need further seed and
scale coverage. Only the castle has a captured generator-53 baseline this pass.

The reference target is not reached: settlement parcels still look sparse,
one-metre roof steps remain coarse, background tree LODs show disconnected
canopies, and underground materials are strongly yellow under torch lighting.
The mine's old low cross-beam obstruction is removed in R7, but dense fungal
growth and the working-gallery/chamber intersection still clutter that view.
This pass changes architecture and composition; concurrent shader edits are
outside its ownership. Do not describe the overall visual overhaul as finished.
