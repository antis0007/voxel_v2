# Hell planet and player-built portals

Implementation: 2026-11-05. Extends `BURIED_HELL_GATES.md` and the portal-network
brief's implemented crystal gates. The owner requested the red Hell appearance,
ordinary planet access, basic player-built portals, player instructions and
seeded moons. This supersedes the cold palette for **new** player-linked Hell
worlds, not for historical Nether saves.

## Player construction

Open the inventory recipe catalogue and expand **Portals**. The same diagram
appears on the recipe page. It has separate Hell and Crystal Link tabs.

For Hell, build a vertical Basalt rectangle with an empty **2 m wide × 3 m tall**
opening. Corners are optional. Aim at its basalt frame from within 5 m and cast
**Spark** using an equipped wand. Normal spell payment and cast delay apply.
Accepted openings are 2–4 m wide and 3–5 m tall, measured in world metres rather
than a fixed block count. The frame is one cell thick; smaller cells require
more blocks to span the same opening. Spark into an already active frame behaves
as an ordinary cast.

The active frame has a warm red pearl membrane. Walk through it, or use
Interact (default **F**) near the opening. First arrival constructs a matching
return frame and a wide basalt landing above the local terrain. Walk through
that frame or use Interact to return. Breaking or blocking either frame prevents
travel; restoring the same frame restores its existing address.

For Crystal Link, build two vertical Crystal frames with matching rectangular
openings. Aim at the first ring and use Interact to attune it; use Interact on
the second ring to link them. Walk through their films. These retain the existing
same-world live destination views. Interact can release an attunement or close
a linked pair.

In a hosted save, gather every connected player at the gate and have the host
press F. The party crosses together on the same connection after the destination
checkpoint. This also opens the route for the first time. After a Hell portal
links both saves, guests can press F at either side to cross independently
while the other players stay in their current realm.

## Planet and presentation

New player-linked Hell realms use the current dedicated Hell recipe and the
parent world's seed. Existing destinations retain their recorded generator,
terrain and portal addresses.
They retain the Nether save identity but use the shared cube-sphere chart,
planet cover, tangent camera and face transitions. Terrain noise is evaluated
in sphere direction space so it continues across chart boundaries. Older Nether
generators keep their original column and material distribution.

Broad crimson Hellstone shelves overlook lava cuts with basalt banks, ash
patches and dark strata. Hellstone is an appended material identity: existing
block IDs stay unchanged. Hell's cover does not substitute water for low lava
columns. Existing caves, crystals, ore and encounters remain available.

New player-built portals open generator 159. Existing linked saves keep their
recorded generator. The region name appears on the HUD.

- **Ash Barrens** have exposed ash flats, basalt shelters and hot ash surges.
- **Fungal Hollows** have fungal soil, cap canopies and salamanders. Harvest
  Amber Cap seed and plant it in shaded Fungal Soil. It grows without irrigation
  and supplies Fire Resistance brewing for trips into the other regions.
- **Cinder Rifts** have wider lava cuts, basalt pillars and Kiln Crawlers.
  Radiant damage near lava banks bypasses shelter. Fire Resistance protects you.
- **Basalt Scarps** have higher shelves, exposed Crystal, broken ramparts and Pit Imps.

Kiln Crawlers at Hell camps alternate melee attacks with marked fire patches.
Step out of the ring or use Fire Resistance. A defeated crawler drops a Kiln
Scale once per encounter. Craft it between two Seared Bricks and place the
resulting Kiln Lining two metres above a formed Smeltery Controller, directly
over its anchor. It raises the controller's heat budget by 25% per burn tick.
The same upgrade works on the surface.

Use a bed in Hell to set your spawn. It needs clear space above it.
Portal travel keeps a separate spawn for each realm. If the bed is gone,
blocked, or flooded when you die, respawn near your last Hell arrival.

Each new ruin and its encounter use the same surveyed position. Earlier
generators retain their original placements.

Generator 146 adds broken crags, dark fungal stalks and harvestable Amber Caps.
Kiln Crawlers, Pit Imps and Corpse Lights occupy seeded camps; passive cave
salamanders also occur on dry Hellstone and basalt. Generator 147 gives camps
ruined ramparts or pillar aisles and a salvage crate. These are finite salvage
and harvesting sites, not automatic farms.

Hot ash surges last eight seconds every forty seconds, with a four-second warning.
Overhead cover within 18 m blocks exposure; Fire Resistance protects travellers
in the open. Amber Caps can replace the ordinary fire-resistance modifier in the
brewing stand. Covered paths, refuge roofs and a local brewing outpost make the
trip between resource sites safer.

New Hell lighting uses the planet's solar direction, amber direct light,
readable plum ambient shade and a restrained split-tone grade. Its atmosphere
has its own Rayleigh, aerosol scattering and selective absorption coefficients.
Changing between Hell and the surface rebuilds transmittance and multiple
scattering transport and invalidates the aerial-perspective cache. Sky-view and
aerial-perspective solves use the same selected transport. The sky combines
these LUTs with a burgundy ash dome and copper solar halo. Ordinary white cloud
composition is disabled. Ash falls and cinders rise using the existing bounded,
depth-aware living-air shader; no global particle population is spawned.

The parent seed determines **1–3 moons**, with independent sizes, mineral
palettes, inclinations, nodes, phases and periods. Craters retain the existing
pixel treatment; molten fractures emit on the night hemisphere. Their motion
uses saved planetary time, transformed into the observer's tangent frame.
Ash extinction fades them near the horizon; extinction falls away above the
atmosphere. The sky likewise fades toward space with altitude.
These are separate landable bodies in Hell. In Creative, enable flight and
fly toward a moon; use camera-directed flight for a straight approach.
Descend to land, walk with the moon's local gravity, and fly back toward
Hell to return. The surface is procedural landing terrain, without voxel
building or mining. Save 55 stores the local moon position, velocity, and
upright frame. The ordinary surface world's moons remain separate.

## Travel and persistence

`gates.ron` stores local/remote aperture pairs with ordinary crystal links.
Missing Hell records default to empty for older sidecars. Broken frames retain
their recorded address. Counts, cell extents, coordinates and aperture transforms
are bounded on load; each world accepts at most 32 Hell thresholds.
New frames must fit within both realms' build heights. First arrival checks
the actual destination chunks and admits the platform before registering its
return frame. Unavailable, flooded or obstructed arrivals refuse the crossing.

`hell-portals.ron` links the parent world and its new Hell world. It is separate
from the historical buried-gate `hell-gate.ron`, allowing both destinations to
coexist. Generator 142 remains readable after the new default planet version.
Inventory, player progression and status travel; each realm retains its edits,
encounters, drops and respawn point.

Swept eye/plane detection holds the actor on the departure side during a
checkpoint. Travel validates the far frame, actual edited body clearance and
solid footing before moving the actor. First arrival builds only in clear air;
subsequent arrivals never excavate or repair. Departure is saved first, then the
destination is saved before installation. Checkpoint/publication failure restores
the source actor and position. Blocked or damaged exits report refusal.

## Current rendering boundary

Cross-dimension portals reuse the existing clipped pearl membrane, warm rim
light and approach distortion. They do **not** display a live rendered second
world: the engine's current window stage draws one resident world's chunk cache.
The red membrane is deliberate unloaded-destination presentation, not a fabricated
view. Crystal links keep their existing live windows and bounded recursion.
The server ticks occupied realms separately and routes gameplay by player
residency. For guest crossings, the hosted world's `friends.vxp` holds one
record containing both the guest's current realm and player state. The server
saves the departure profile before loading the destination and replaces it
before moving the guest. A newly built return gate is checkpointed before that
replacement. Reopening the original hosted save can reopen the guest's recorded
realm on reconnect. An unavailable realm refuses the join and retains the
record. Secondary checkpoints leave the host's quicksave and autosave slots
alone. Older per-realm profiles can be adopted when their realm is already open.
The realm loader reads the landing area before the host installs the realm.
The relay counts packets by arrival time so a host pause does not put the
whole queue into one rate window.

Host travel remains party-wide. Connected guests must gather at the gate, and
guests with a saved position in another realm must return before party travel.
Cross-world live windows and entity or projectile passage are separate work.

## Evidence

The [local multiplayer review](../../tools/Review-MultiplayerRealms.ps1) passed
with three headless guests. One guest crossed into Hell and rejoined while two
guests stayed on the surface. After a forced host shutdown, hiding Hell's
checkpoint caused a refusal. Restoring the checkpoint recovered the guest and
their inventory in Hell. The guest returned and retained the surface position
and inventory across a second forced host shutdown. The run used a prebuilt
portal pair and a local relay. Host party travel, rendered multiplayer and
machine power loss remain outside this review.
