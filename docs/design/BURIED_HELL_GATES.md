# Buried hell gates

For the new player-built route from default planets, see
[Hell planet and player-built portals](HELL_PLANET_AND_PORTALS.md). Its saved
destination is separate from the historical sanctuary route described here.

Generator 83 adds rare sanctuaries to new Hills worlds. Existing generator versions
keep their terrain. Sites repeat every 1,024 metres with seed offsets; the first
is near (240, -64), offset up to 32 metres on each horizontal axis.

A weathered tower opens onto a lit spiral stair. The sanctuary sits below both
realms' terrain, above bedrock. A side crypt contains a persistent salvage cache.
The basalt arch bears `ABANDON ALL HOPE / YE WHO ENTER HERE` on both faces and a
dark red veil. A spatial echo accompanies the approach. Aim at the veil and use
Interact (default F) within four metres to enter; use the same control to return.

The linked Nether uses the same seed and threshold coordinates. The arrival room
is dry, has a matching staircase to the surface and a crystal deposit behind the
altar. Each realm retains its terrain edits, caches, drops, encounters and respawn
location. Inventory, wand/tool records, progression and player status travel with
the player. First arrival establishes a Nether respawn at the threshold.

Crossing freezes local gameplay, checkpoints the departure, checks the actual
edited destination for body clearance and footing, then checkpoints the arrival
before installing it. Broken supports or blocked arrivals refuse passage. Failed
destination saves restore the source actor in memory. `hell-gate.ron` links the
two saved worlds. Loading/renderer state is reset on arrival.

This slice supports ordinary local Hills play. Multiplayer and CA sessions do not
cross realms. It is an explicit saved-world transition, not a live view into a
second simulated dimension. Separate dark-dimension biomes and custom bosses are
outside this increment.

Validation: local compilation only; no tests requested or run. Native stair
traversal, lettering legibility, sound audition and save/reload round trips still
need in-game review. Build evidence is recorded in the task response.

## Generator 89: approach, tower and sanctuary

Gate 89 (`hell_gate::approach`, `hell_gate::sanctuary`) adds surface ruins
around each sanctuary. Versions 83-88 keep the bare tower and room.

- A paved road runs 44 m out from one tower doorway along a seeded cardinal
  axis. Paving wears away toward the outer end; kerbs appear only at 0.5 m and
  finer, where they stay low.
- Six column stations flank the road. Some stand whole with capitals, most are
  broken stumps with moss caps and vines, and some lie toppled outward.
- Two arches cross the road: the first whole, with wall torches on its posts;
  the second missing one side, its lintel on the ground.
- A roofless toll house beside the road, with a counter facing the door. This
  follows the existing toll and spark lore; it adds no new names.
- A failed crystal ring of basalt with crystal shards stands where the road
  begins, its top quarter fallen. It echoes the player-built crystal gates.
- A broken curtain wall at 21.5 m, gated with piers at the road.
- The tower ring rises to a ragged 2 to 10 m with arched doorways, a string
  course and arrow slits. Torches on plinths flank the road doorway.
- The sanctuary gets grey pilasters, a cornice, a framed arch, a dark floor
  runner and ring, and four wall torches.

The arch opening is drawn by the portal compositor, never by stand-in
geometry: the surface side shows the underworld vista, the underworld side a
dark mist membrane (`world/portals/realm.rs`). The red thread veil and its
backing plate are removed. The inscription is cut into the lintel as dark
grooves with lit lower bevels in seven-by-nine serif capitals, interpuncts and
ruled lines, sized to the lintel cells at every scale.

Validation: `engine_worldgen` tests `hell_gate::tests` (road walkable, every
torch supported, ruins absent before 89, threshold clear and intact). Captures
from fixed poses, before and after: `target/gate-approach-20261030/`. Poses come
from `cargo test -p engine_worldgen review_poses -- --ignored --nocapture`.
