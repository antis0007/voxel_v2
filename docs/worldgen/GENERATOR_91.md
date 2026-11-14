# Generator 91: exploration distance and a working surface map

The ordinary map's `note_chunk` and `absorb` streaming hooks were empty. No live
terrain entered the tile store, while the UI defaulted to a strict local
observation chart. The replacement queues resident mesh sources and scans their
surface columns within the existing time budget. Unavailable sources retry;
terrain uploads and persistence retain their existing bounded paths. Ordinary
worlds use explored surface tiles and normal waypoint distances. Manyfold keeps
its local chart for radial gravity. The ordinary map sidebar no longer exposes
unlocalized levels, report provenance and route-analysis controls.

Fresh worlds use a 10,240 m geology solve instead of generator 90's 24,576 m
solve, with 360 m relief. A seeded finite uplift spur is centred about 600 m
from the home craton, then carved by the same erosion/drainage simulation.
This is a terrain-generation input, not a measured guarantee of a passable
600 m route to a mountain summit.

Surface sites formerly rejected ground at 105 m and buried complexes rejected
ground at 130 m. Generator 91 admits suitable ground up to 460 m; existing
cover, clearance, slope and reservation rules still govern dungeon placement.
The denser generator 90 dungeon candidates and 640 m gate grid remain enabled.

Saved worlds retain their generator. They receive the map repair, but their
terrain is not regenerated or silently migrated. A fresh world is required for
the compressed geography and new altitude admissions. No existing saves are
deleted by this change.

Validation: local app compile check passed. Native map and exploration review
must be recorded separately; a compile is not proof of discovery pacing.

Native review: `D:/codex-build/voxel-review-20261030/bench-001.png` renders the
restored surface tiles on the minimap, alongside actual starting-area terrain.
Generator 91 surveys place the nearest sampled alpine point at (-480,-192) for
seed 424242, (-384,384) for seed 7 and (-192,-480) for seed 1337: 517–543 m
from the origin on a 96 m sampling grid. These are biome samples, not measured
walking routes or proof that every seed has identical mountain distances.
