# Generator 90: working ruins and compact geology

Fresh worlds default to this generator. Wildlands retains its existing planetary
drainage, tectonics, climate, glacial erosion, settlements, orbital structures and
buried Nether gates. The separate Planet preset is not substituted: its sampling
path does not include all that content. Manyfold is still a separate world preset;
its records now have explicit historical links to the buried freight works, but
this change does not implement travel from Wildlands into Manyfold.

The geological solve spans 24,576 m rather than 32,768 m, with 380 m rather than
440 m relief. Tectonic coordinates scale together, preserving plate shapes while
bringing ranges, coasts and basins closer. The hot solve cache includes the full
parameter fingerprint so historical and current worlds of one seed cannot alias.
Gate grid spacing is 640 m instead of 1,024 m. Dungeon candidate occupancy rises
from 48% to 66%; terrain safety and structure reservations still reject sites.

Ruins try all three district footprints before accepting a smaller clipped wing.
They can have four feasible return circuits, the first two open, with the sealed
core and antechamber excluded. The full job-specific programme supplies ordinary
rooms, rather than repeating two main room kinds. Reservoirs, root gardens,
infirmaries, barracks and foundries have larger footprints. Contained water beds,
fungal growing beds and paired hall supports occupy quadrants, preserving the
central circulation cross, authored doorways and branch caches.

Nine working records teach specific existing spells: catacomb light, sluice frost,
furnace lightning, warren spores, quarantine acid, orrery lodestones, Surveyor
gravity, pilgrim ricochets and workshop excavation. Each is a permanent discovery
with one-time XP, saved through the existing progression sidecar. Spell discovery
does not bypass wand tier requirements. The first working record also supplies
the lamp tally used by the existing gate quest. Records connect the offices'
practical magic to their architecture, the braid, freight tubes and return routes.

Earlier saves retain their generator and geometry. No test execution or native
visual/play acceptance is included. Local cargo check is the validation scope.
