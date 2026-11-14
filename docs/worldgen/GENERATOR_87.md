# Generator 87: Manyfold undercroft

This gate adds physical exploration below the six radial districts. Older
Manyfold samplers still dispatch through their existing version gates. Other
world types retain their previous terrain at this version.

Two weathered stone faces mark descending stairs at district-local arc -8.4,
in districts 0 and 3. From axial z=-9 to -19 the stairs descend five metres.
A continuous buried gallery circles the tube at z=-19, with a copper floor
line and two exits. The original bedrock boundary remains intact.

The lower room under district 0 contains a listening stone, four cover piers,
and mineable tin. Copper and iron remain in side niches along the circuit.
Under district 3, empty benches face a disused ticket window. That waiting
room has no hidden combat trigger. Salvage uses normal mining and inventory.

The listening stone has a nine-second cycle: its scored floor circles brighten
from 4.5 seconds, and it pulses at 6.5–7 seconds. The pulse deals four damage
before difficulty scaling, at most once per cycle per player, and only after
that player has received at least two seconds of warning. It reaches ten
metres only through unobstructed air. Solid cover and unloaded edited cells
stop both the warning and harm. Existing creative/death/spawn protections
apply. Solo and multiplayer authority own damage; clients do not predict it.

Aim at the actual crystal within four metres and use F to dampen it, or mine
it normally. Damping replaces that voxel with carved stone through the usual
edit transaction. Removing the source stops its inscriptions, cue and damage;
ordinary world edits persist the result. Replacing the crystal rearms it.

Design basis: Manyfold brief sections 23.3–23.5 (physical containment,
responsive machinery, abandoned transit), the monumental broken-statue
reference, and the existing radial arcades. This is a bounded chamber and
service circuit, not an implementation of the Repeater or Bellwarden AI.

Review cameras: `manyfold:descent` and `manyfold:crypt`, seed 424242, 0.5 m.
Build and native review results are recorded below after inspection. No tests
or hosted CI were requested or run for this change.

## Native review

Reviewed executable: `target/voxel-manyfold-undercroft.exe`, 41,232,384 bytes.
SHA256: `52DC687324B53C6BD0936F7FED5C74331870BF4F2D90778F8091400D0E803018`.
Final build: `target/manyfold-undercroft-final-r2-build.log`; release-fast
succeeded with 85 warnings. Runtime shader validation reported
`world_fnv1a64=fc62cabc25a333c2`, `build_match=true`.

Inspected these native captures using the same executable, generator 87,
seed 424242, 0.5 m voxels, clear midday, and fixed review cameras:

- `target/place-review/manyfold87-crypt-final-r2/camera.png`: source, suspended
  masks, vault ribs, cover piers, ore and side passage.
- `target/place-review/manyfold87-descent-final/camera.png`: broken crown,
  projecting nose and mouth entrance; earlier preview also exposes the stairs.
- `target/place-review/manyfold87-waiting-final/camera.png`: empty benches,
  closed ticket window and clear aisle under the opposite district.

The first final candidate failed startup on an embedded kinetic-vest
description validation error. That description was already corrected by its
owner; rebuilding picked up the corrected source. The three captures above
all launched and finished successfully. Earlier preview and failed-final
directories are retained as evidence, not presented as the final build.

Frame-pacer timeout retries remain in the native logs. These are camera
reviews, not proof of smooth movement, end-to-end stair traversal, audio feel,
save/reload interaction or multiplayer combat. Those play checks remain open.
No tests or hosted CI were run. `tools/Play-Manyfold.ps1` now launches this
frozen build. Create a fresh Manyfold world to receive the new terrain; saved
worlds retain their original generator. The live shared default has already
advanced to 88; this feature's compatibility gate remains 87.
