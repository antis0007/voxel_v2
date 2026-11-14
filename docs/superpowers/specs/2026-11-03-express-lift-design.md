# Express lift and moving frames

Date 2026-11-03. Source briefs: the two-minute express elevator note and the
curved-cells / local-frames / Manyfold note (owner pastes, same day).

## Decisions taken in this slice

- Travel time is the gameplay rule: 120 s activation to doors, 110 s in motion,
  5 s of doors at each end. The drive is rated for the route:
  `a_max = 5D/T²`, `v_max = 2D/T`. Distance, speed and g always agree.
- Bounded-jerk S curve with five constant-jerk phases, ramps one tenth of the
  motion time; evaluated from precomputed phase knots, never integrated per
  client. `engine_space::express`.
- Drive acceleration and cabin gravity are separate readings. The compensating
  field holds 1.1 g on departure, 1.0 g through the midpoint, 0.9 g under
  braking, 1.0 g docked; it is a blend on the signed drive share, so there is
  no step at a phase edge.
- The cabin is a moving frame. Passenger physics run in cabin-local metres on
  a flat floor with four walls under the compensated field; the cabin's world
  position comes from the trajectory each fixed step. Panel, model, audio and
  physics read the same state (`world/orbital/express.rs`).
- Route: the local boarding floor to that corridor's crown anchorage
  (roughly 18 km on current worlds). At that length the two-minute
  rule rates the drive at 0.76 g and 327 m/s; the same code rates a 200 km
  route at 8.4 g and 3.64 km/s (tested).
- Every generated corridor has passenger boarding within 64 m of its footing,
  while grounded within 24 m of the base height. No freight commissioning or
  receiver deployment is required. Existing registered receiver berths remain
  available. The use key boards at the base and starts the return trip at the
  crown; the registered receiver console still opens its service ledger.
- The ride is transient and never saved. A world loaded mid-ride starts docked
  at the terminal. No save version change.
- Chunk residency plans for the terminal once the return trip is within 40 s
  of arrival, so the doors open onto loaded ground.
- Panel refreshes numbers at 10 Hz, normal colours only; precision is one
  decimal for ordinary g, whole g past 10, km/s past 1,000 m/s.
- Drive sound is a seventh ambience bed: hum climbs two octaves with speed,
  wash follows load, braking adds a thin whine; other beds drop to a trace in
  the sealed cabin.

## Crown station deck (added 2026-11-05)

- The docked cabin's +x knee panel opens into a door (2.2 m) while the cabin
  sits at the crown. Walking through puts the passenger on the station deck,
  which shares the cabin-local frame: the docked cabin centre is the deck origin.
- Deck plan: a concourse square around the shaft, half width 8.5 % of the
  anchorage ring radius clamped to 7.5..18 m, and a gallery arm along +x,
  26 m long and 8 m wide. The gallery end is a rail only, so the planet limb is
  in view below and the sky above. Walls are knee panels to 1.05 m with open
  glazing above.
- Collision is axis-separated against the plan polygon, so walls slide.
- Lamps are emissive strips on the ceiling (cabin and deck): the keel shadows
  the interior and the sun never reaches it.
- The panel hides on the deck; the descend action needs the passenger aboard.
- Not saved: a world loaded with a passenger on the deck starts at the terminal.
- Review: `lift-walk x z` places the passenger in the frame; the driver adds
  door, hub, gallery space/planet/up shots.

## Station experience pass (2026-11-06)

References weighed: Elite Dangerous (station services board, docking
announcements), Star Citizen (walkable arrival, windows with traffic),
Seveneves (tether, spun gravity, sunrise cadence), Neuromancer (sodium light,
signage, grime). A peer's voxel habitat (`world/orbital/habitat`) now owns the
crown interior: cylinder, finite air, airlocks, cargo, spun drum beyond 11 m.
The separate deck model from 2026-11-05 was removed as superseded.

- Arrival line on docking: "Crown anchorage · Docked · 18.4 km · Door open";
  "Terminal · Docked · Door open" on return. Six seconds, HUD message.
- Anchorage board on the deck (replaces the bare pressure readout): class,
  altitude, local clock, countdown to sunrise or sunset, rim spin in rpm and g,
  braids in sight, pressure and O2 from the habitat, cabin door state.
- Idle hum: a docked cabin feeds the drive bed at 6 % speed, no load.
- Cabin: amber chevron threshold and a lit lintel at the open door, sodium
  skirting strips along the floor edges, ceiling lamp.
- Cabin alignment (2026-11-06): the berth sits `CABLE_CLEARANCE_M` (6.7 m)
  +z of the site centre so the braid runs up the cabin's -z face through a
  shoe; the cabin adds the braid's drawn bow (`sway_m`, sine of the span
  share, scaled by the span squared over 18 km) each step, so it stays on the
  cable when wind bows it. Cabin pieces use plain shaded cuboids: family 62
  mottle read as soil at arm's length.
- Not done: deck mood lighting inside the voxel habitat (peer's material
  table), traffic through the windows beyond the existing carriages and
  meteors, PA audio.

## Not in this slice

- Multiplayer replication of the ride (route, departure tick, profile). The
  state is already compact; it lands with the lockstep wiring.
- Leaving the cabin at the crown: there is no walkable station there yet. The
  cabin stays sealed at the top until the return trip is activated.
- Curved voxel cells, rotating habitats and Manyfold junction gravity from the
  second brief are separate projects. The frame-transfer rule from that brief
  (`v_world = V + Q(v + Ω×r)`) is the contract the cabin frame already keeps:
  the passenger's world velocity is the cabin's plus the local walk.
- Shader-side changes for the ascent view. The existing altitude tiers,
  f64 eye with camera-relative residual, planet-curved cloud deck and globe
  renderer carry the view. Research pointers for the next pass:
  Hillaire 2020 sky/aerial-perspective LUTs for ground-to-space in one model;
  CDLOD-style geomorphing or stochastic cross-fade for terrain LOD edges
  during the fast middle of the ride.
