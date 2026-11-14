# Station voxel interiors, pressure and access

Status: proposed implementation design, 2026-11-05. Source inspection only; no
runtime, performance or pressure-system acceptance. Extends civilization section
16. Numbers below are proposed gameplay dimensions unless identified as existing.

The subsequent implementation slice is recorded in
[Voxel lift habitat](../../design/STATION_HABITAT_20261105.md). It supersedes this
document's baseline survey for the compact voxel habitat, cylinder controller,
finite gas, airlock and hull resistance. Broader settlement and volume architecture
below remains design work; native acceptance is recorded separately.

Bounded implementation follow-up: `express::deck_floor_spans` now supplies the
analytic deck collision and rendered floor/roof footprint, including the cabin
threshold. Door clearance includes the passenger margin and airborne passengers no
longer report grounded. Local application `cargo check --offline` passed; 25
composed shaders validated. In-game crossing/capture remains unverified. This does
not implement the proposed station voxel volume or pressure system below.

## 1. Direction and document authority

Freight/security increment: `world/orbital/cargo.rs` now transfers ordinary item
stacks from a persistent station locker into a saved shipment. Destinations are
the boarding elevator's base or the registered ground receiver. Loading lasts
45 seconds, followed by 120 seconds of descent; collection retains any overflow.
The source locker is cleared and its open UI copy refreshed during dispatch.

Two protected constructors behind a voxel glass field deploy security robots
through external hatches. The field blocks ordinary edits and projectiles.
Production stops at launch, with six live guards and twelve produced per dispatch.
The local control pauses production for 12 seconds with a 45-second recharge;
surviving guards keep their health and positions. Robots award no encounter loot
or clear records. Save v54 retains manifests, clocks and security identities;
older habitat saves default to an empty freight ledger. The optimized local build
passed; combat, delivery and save/reopen behavior have not been checked in-game.

Owner requirements: more usable personnel elevators; inhabited stations with loot,
enemies, friends, neutrals and traders; human character models; properly scaled
voxel interiors with breaches and airlocks. One broken elevator city coexists with
working elevators. This does not authorize replacing the game's voxel art style.

| Source | Authority and requirement |
|---|---|
| `docs/NEW/AXIOM_Science_Spaceflight_And_Planetary_Expansion.txt:405–415` | Expansion proposal: editable assemblies, usable interiors, shared construction, conserved salvage and coherent topology revisions |
| Same brief, lines 443–455 | Proposal: persistent frames and epochs; coarse travel advances reserves and consequential hazards |
| Same brief, lines 471–485 | Proposal: actual interior/exterior correspondence, explicit gravity, connectivity/volume airlocks, EVA reserves and rescue, habitat checks for ordinary creatures |
| Same brief, lines 670–684 | Proposal: storage, seating, maintenance, galley/garden and communal space; real dimensions and conserved material across visual resolution |
| `docs/NEW/AXIOM_Reference_Worldgen_And_Bounded_Physics_Prompt.txt:103,133–149,411,591` | Controlling art/physics addendum: human routes before ornament, repairable breaches, shared cargo identities, coherent physical boundary and topology |
| `docs/MASTER_PLAN.md:214,343` | Roadmap: vessel body subsystems pending; `vessel_hull_breach` includes crew sealing, repair and evacuation |
| `docs/adr/0007-bricks-and-scale.md`; `0017-roadmap-contracts-and-sparse-activity.md:30–46` | Recorded architecture and corrections: sparse bricks are reusable; mixed-resolution transfer is not implemented |

`ENGINE_CORE.md` and `GAME_DESIGN.md` are proposals, as stated in `CLAUDE.md`.
Neither a roadmap row nor an integer cell pressure field proves a working station
atmosphere. The briefs specify no binding station cell size, leak coefficient,
door pressure tolerance or suit reserve. Those remain content/tuning decisions.

## 2. Current code and gaps

| Existing support, inspected 2026-11-05 | Limit for this work |
|---|---|
| `crates/voxel/src/config.rs:9–10,75–85`: ordinary column reaches 512 m; Nether has its own minimum | Extending the terrain column to the crown is unnecessary and would not supply local frames |
| `crates/core/src/scale.rs:1–18`: 1, 0.5 and 0.25 m cells, one preset per world | No assumption that one ordinary `World` mixes scales |
| `crates/app/src/world/orbital.rs:15–19,252–275,549`: sparse candidates in 4096 m cells, guaranteed initial corridor, crown offset 18,000 m | Not one elevator in every cell; do not silently increase density or change old generation |
| `world/orbital/express.rs:1–8,18–78,122–158`: local cabin movement, 7 m floor, 3 m headroom, analytic concourse/gallery and boarding at generated corridors | Cabin ride is transient; deck is not editable station voxel geometry |
| `mesh/models/orbital.rs:12,61`; `mesh/models/orbital/deck.rs` and `cabin.rs` | Rendered structures are not a separate authoritative station voxel store |
| `player.rs:197,478–506`: precise position and local-origin movement | Existing positions include voxel units; explicit metre conversion is required |
| `world/hell_gate.rs::exchange_actor`; `world/portals/realm.rs` | Realm travel/presentation precedent, not simultaneous exterior and station grids |
| `player/radial.rs`; `simulation/rigid.rs:1–65` | Gravity-relative collision and bounded assemblies are reusable ideas, not a crewed vessel subsystem |
| `crates/sim/src/cell.rs:30`; `rules.rs`; `fine.rs` | Cell pressure and gas movement do not provide compartment oxygen, seals or life support |
| `world/fittings.rs:1–45`; `crates/content/src/fittings.rs` | Metre-sized door assemblies and obstruction checks; no airtightness contract |
| `world/adventure/wildlife/vehicle.rs:292–311`; `mesh/models/network_player.rs:1–41` | Current robed villager model differs from the jointed human presentation available for reuse |

The three main missing facilities are a station volume/frame adapter, revisioned
pressure topology, and inhabitants/navigation that operate in that frame.

## 3. Station-local voxel volumes

Introduce a bounded `StationVolume` rather than enlarging the surface column or
teleporting to an unrelated room. Proposed record fields: station/settlement ID,
frame ID and epoch, double-precision origin/orientation, voxel preset, local bounds,
sparse chunks, topology revision and persistent deltas. Chunk coordinates remain
32-cubed cells with floor division. Reuse chunk data and material IDs; adapt the
terrain-specific generation, residency, edit, collision and mesh consumers.

Every location/query carries a volume/frame ID. Convert local cells to local metres
using that volume's scale, then transform to the parent frame in f64. Subtract the
camera origin before float GPU conversion. Never pass station cells directly to
surface `World::get`, loot placement, raycasts, navigation or actor spawning.

Start with a tether-fixed station and the save's existing voxel preset. All three
presets must preserve metre dimensions. A later independent quarter-metre station
grid needs an explicit adapter and inventory/mass conversion contract; it is not
enabled by the reserved simulation `Scale`. Material accounting uses occupied
physical volume and density, never a fixed item per newly subdivided cell.

Generate a few inhabited modules inside the crown's existing envelope. Detailed
voxels and the exterior proxy derive from the same station plan and damage revision;
no decorative solid hull across a playable breach. Unvisited modules remain plan
records. Unknown/unloaded neighbors are unresolved boundaries, never free vacuum
or automatically sealed walls. Admit an edit only when its affected domain can be
resolved within supported limits; keep the previous revision until then.

Use explicit gravity policy: first slice retains the tether station's downward
local gravity. Rotation habitats and free flight require later frame dynamics.
This is a gameplay rule, not a claim that altitude creates zero gravity.

## 4. Metre dimensions and usable construction

| Space | Initial clear dimension |
|---|---|
| Main corridor | 3 m wide, 3 m high |
| Secondary passage | 2 m wide, 3 m high |
| Personnel pressure door | 2 m wide, 3 m high |
| Airlock chamber | 4 m wide, 6 m long, 3 m high |
| Crew room | At least 4 by 4 m clear floor |
| Freight room | At least 8 by 12 m clear floor, 4 m high |
| Pressure shell | At least one full cell; default 1 m in the first fixture |

These are clear spaces after wall thickness, equipment and door travel. At 1 m
scale a main corridor is 3 cells wide; at 0.5 m it is 6; at 0.25 m it is 12.
Do not scale player, NPC, stair rise or furniture with the cell size. Shaped stairs
and fittings can provide smaller features without pretending a solid voxel is air.
Validate the actual capsule, crouch, carried equipment and two-way passing space.

Lay out lift concourse, trade counter, quarters, freight and an isolated damaged
service section before decorative piping. Give the damaged area both an internal
bulkhead route and an exterior repair route. Place lockers beside bunks, cargo at
handling access, lamps at intersections, and controls where they can be reached.
No decorative geometry may contradict collision, projectile cover or the seal map.

## 5. Pressure compartments and breaches

Use a compartment graph over connected breathable space, not simulated gas in every
empty voxel. Store free volume in m³, gas species amounts, thermal energy, derived
temperature and pressure, and last simulation tick. Oxygen partial pressure drives
breathability alongside pressure, temperature and supported contaminants.

Build connectivity from cell faces with explicit permeability/seal properties.
Collision solidity alone does not prove airtightness. Glass, closed rated doors,
vents, damaged seals and open doors have declared behavior. Fittings need oriented
face coverage; prevent diagonal/corner leaks from a naive neighbor rule.

Hull edits invalidate affected regions. Use resumable local connectivity work and
chunk boundary summaries; a worst-case whole-module split still needs an admission
limit. On merge, sum quantities/energy. On split, distribute by resolved free volume
for the well-mixed approximation, retaining division remainders. Pressure is derived
from conserved gas, volume and temperature; never copy a full gas inventory to each
new component. Filling space must displace/compress gas or reject an unsupported
operation rather than deleting it.

Stage a topology revision containing hull, collision, seals, free volumes, doors and
compartment membership. Publish it as one authoritative change. Derived meshes can
lag only through a controlled transition that does not conceal an accepted hole.
Until a revision is ready, damage remains pending; stale connectivity cannot provide
an invisible temporary seal after collision already opened.

Connections transfer gas using opening area, upstream/downstream pressure and a
documented bounded flow approximation. Clamp transfer to available species/energy
with deterministic ordering or simultaneous edge accumulation. A breach exhausts
to the local external environment reservoir, recording lost gas and energy. An
18 km crown is not automatically perfect vacuum: obtain outside conditions from
the world's environment model. Use explicit low-pressure exterior fixtures for EVA.

Decompression cues, bounded character/item impulses, sound attenuation and exposure
read that same transfer. No room-wide scripted instant death or cosmetic breach.
Patch construction consumes material and changes the same seal faces. Closing a
bulkhead isolates damage; it does not restore lost gas. Refilling requires tanks or
production with accounted inputs, power and finite capacity.

## 6. Airlock and lift interface

Two rated doors enclose a real gas compartment. Controller states: inner access,
both sealed, recovering/venting, outer access, repressurizing and fault. Opening a
door requires the opposite door to report physically closed, acceptable pressure
difference and a clear swept doorway. Door damage overrides controller intent.

Normal exit: close inner door, recover gas into a finite tank when equipment/power
allow, vent remaining gas to the actual exterior, then unlock outer door at its
configured tolerance. Entry reverses this with supplied gas. Stop on blocked doors,
bad seals, insufficient reserves or power loss. Recovery never makes free oxygen.
Manual operation remains possible under declared mechanism rules; deliberate
override can open both doors and becomes an ordinary breach. Do not silently seal
destroyed doors. Show door state, pressure, reserve and concrete faults.

The personnel lift docks at a specific station port. Preload destination collision,
atmosphere and navigation before enabling the door. Match frames at one tick;
transfer occupant identity and point velocity once when crossing the threshold.
The dock seal and cabin atmosphere participate in the boundary contract. Failed
docking or pressure mismatch leaves the cabin closed with a return option.

Keep a real station residence state independent of `express::Ride`: leaving the
cabin cannot freeze all inhabitants or make the station disappear on reload.
Persist station identity, port, actor-local position and gas/damage state. Mid-ride
save recovery must choose a documented safe berth without duplicating passengers
or cargo; current transient ride behavior is not that persistence implementation.

## 7. Residents, trade, loot and human presentation

Use civilization settlement/resident/container IDs with frame-local positions.
Crew, friendly visitors, neutrals, traders and hostile occupants have explicit
dispositions. Rendering never determines hostility. Safe occupied districts and
contested service areas follow ownership/history, not random enemies among shops.

Navigation uses the same voxel collision and doorway revisions as the player.
Actors assess pressure, breathable reserve and protective equipment before routing.
Expose seal/repair/evacuate tasks for crew. Distant cohorts retain counts and reserves;
materialization refines those records rather than spawning replacements.

Extract the jointed human presentation from `network_player` into a reusable actor
model. Supply appearance, clothing, equipment and pose separately from network ID.
Give crew, guards, traders and human enemies readable silhouettes, walking legs,
hands and job poses. Retain authored voxel forms and palette; keep nonhuman species
on their own models. A colour-only reskin of the robed villager is insufficient.

Freight determines trader stock through actual shipments and capacities. Containers
hold persistent cargo/crew property or abandoned loot; purchase, theft, salvage and
death drops transfer the same items once. Unloading/reloading cannot refill crates
or regenerate traders. First content needs a repair/supply trader, inhabited quarters,
useful cargo and a hostile salvage area, without inventing a new faction.

## 8. Bounded implementation order and acceptance

1. Add frame/volume IDs and one static station module using the current voxel preset.
   Route render, collision, edit, raycast and container access through it; connect
   the lift port with a walkable return path. Preserve old saves/generator output.
2. Add topology revisions, rated seals and compartment accounting; support one
   breach, isolation, a consumed-material patch and finite gas replenishment.
3. Add the two-door airlock, environmental capabilities and recoverable EVA route.
4. Add human actors, dispositions, navigation, persistent loot and a working trader.
5. Connect a second independent elevator/station; then expand class-based districts
   and history. Additional density requires a separately claimed generator gate.

Future acceptance captures must show the full lift ride, threshold crossing and
return; readable rooms at all three voxel presets; a patched breach retaining gas;
airlock interruption/override; a resident refusing unsafe passage; trading and loot
remaining correct after reload; and two destinations with independent state.
Also verify a breach straddling chunks, pending topology work, unload/reload during
a leak, blocked doors and gas exhaustion. Profile occupied-station frame/tick tails,
memory and edit/mesh backlog before expanding the active domain.

Before implementation, assign concrete limits for resident chunks, topology work,
active compartments and entities; reject unsupported construction explicitly.
Use fixed ticks, deterministic work quotas and resumable jobs. Distant leaks and
life support advance to the same epoch through bounded events; camera visibility
cannot pause a hazard. Live state needs a reviewed save migration, generated plans
need a newly claimed gate, and co-op needs host-authoritative edits/transfers.
No dependency, test or hosted CI was added or run. The follow-up received a local
compile check only.
