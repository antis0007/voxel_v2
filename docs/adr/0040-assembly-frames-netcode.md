# ADR 0040: Assembly frames in netcode

2026-11-03. Proposed. Owner decisions open: physics solver, assembly authority.

Moving machines (airships, cranes, crawlers, mobile workshops) need voxel
volumes that move as one body and stay editable, with players aboard. Every
replication path in `engine_netcode` assumes one fixed world grid:

- `BodyState` (`body.rs`) is a world position only, so a passenger and its
  carrier interpolate independently and the passenger jitters or slides.
- `CellStream` (`cells.rs`) keys bricks by world cell, so a block on a moving
  hull changes address every tick.
- `movement::Validator` tests world solid cells, so a player carried by a hull
  reads as an over-speed move and gets corrected.
- Chunk interest (`chunk_stream.rs`) knows terrain chunks only.
- `simulation::sandbox::RigidBody` is a fixed voxel list with no identity, no
  edits and `f32` state.

This ADR adds a frame concept to the netcode crate. It does not choose a physics
solver or add a dependency.

## Assembly

An assembly is a voxel volume with its own frame:

- `AssemblyId(u32)`, allocated by the host, never reused within a world.
- A local grid in bricks of 8 cells, addressed by local `IVec3`. Local
  coordinates never move; mass changes (cargo, fuel) update a separate
  centre-of-mass offset and never recentre the grid.
- A pose: world position in the existing fixed-point form (chunk carries
  magnitude, 1/256 voxel steps) and the packed rotation from `rotation.rs`.
- A `revision: u32`, bumped by every committed cell edit on the assembly.

Terrain is assembly 0: world frame, identity pose, no revision. Existing world
keys are the special case, so terrain code paths keep their meaning.

## Wire changes

All are additive behind a protocol version bump; peers with different versions
refuse to connect, as today.

1. `BodyState` gains `frame: u32`. Mask bit `FRAME = 32` writes it only when it
   differs from the baseline. With `frame != 0`, `position`, `velocity` and
   `angular` are in that assembly's local frame. Ordinary bodies pay no bytes.
2. Cell flushes gain a frame header per run of bricks: `frame` varint, then for
   an assembly the `base` revision the run applies on and the `advance` to the
   next one; the base lets a client detect a skipped flush. Brick offsets stay
   relative to the run's first brick, so terrain flushes cost one extra byte.
   The server advances an assembly's revision once per flush, so a client's
   revision counts flushes received; budget-deferred cells follow later.
3. New reliable message `AssemblyBaseline { id, revision, bounds, bricks }`:
   sent when an assembly enters a client's interest, through the same budgeted,
   nearest-first sender as chunks. A client drops cell flushes for an assembly
   whose baseline it lacks or whose revision skips ahead, and asks for a resend.
4. Assembly poses ride the existing snapshot as bodies with `frame = 0` and a
   reserved state bit, so they get delta coding and priority for free.

## Passengers

- The client reports its pose in the frame it stands in (`frame`, local
  position). The server validates the move inside that frame: the validator
  receives a `Solid` view of the assembly grid and the carrier's motion between
  the two reported ticks is removed before the speed and ascent budgets run.
- Leaving a frame (jump off, fall, assembly despawn) converts once to world
  space and adds the carrier point velocity `v_com + omega x (x - c)` once.
- Snapshot playback draws the carrier and everything parented to it at the same
  playback tick. A passenger never interpolates in world space while parented.
- Gravity stays in world space; a banked hull does not redefine down.
- The inherited carrier velocity is drift the validator removes from world
  moves; support bleeds it (half-life 0.15 s) into burst allowance, so the
  take-off step of a jump keeps it. The body box stays on the local grid axes.
- The server samples each assembly from a bounded tick-indexed pose history
  (`carrier::CarrierHistory`) at the client's reported tick; a report in a
  frame with no pose there is corrected (`Reason::Frame`).

## Interest

An assembly enters a client's interest by its world bounding box against the
chunk radius, not by chunk ownership. Out of interest it is dropped like a
chunk; on re-entry the client keeps its cached baseline if the revision matches,
as cache-aware chunk delivery does now.

## Authority

Assembly physics stays out of lockstep: `f32` rigid bodies are not guaranteed
identical across machines. Default: the host owns every assembly and sends
poses; a pilot predicts the craft it controls from its own inputs and corrects
to host snapshots. In co-op the owner chose (2026-11-05) to let a pilot own the
assembly group it flies (carrier, crane load, docked craft): the pilot simulates
and sends poses, the host checks them. A handover names the accepted tick, state
and an authority epoch; a missed handover or a disconnected pilot leaves the host
as owner.

Item moves between an assembly and the world (dock, unload, mining output) use
`operation.rs` receipts, so a retry cannot duplicate items.

## Tests (engine_netcode, `link.rs` simulator, no window)

- `BodyState` round trip with and without frame; terrain bodies cost the same
  bytes as before.
- Cell flush round trip across two frames; a flush for a missing baseline or a
  skipped revision is refused and requests a resend.
- Passenger on a hull turning 30 deg/s and moving 20 voxels/s, client walking
  aboard at 150 ms RTT, 5 % loss: zero corrections over 60 s.
- Jump off a moving hull: world velocity after leaving equals the carrier point
  velocity plus the jump within one velocity step.
- Assembly leaves and re-enters interest with an unchanged revision: no
  baseline resend.
- Negative and large chunk coordinates for assembly poses (far from origin,
  across chunk edges).
- Bytes: a 20,000-voxel hull costs one baseline, then under 40 bytes per
  snapshot while only its pose changes.

## Open

- Physics solver: Rapier (new dependency, needs a line here) or an in-house
  rigid solver extending `sandbox.rs`.
- Joints and articulated sub-bodies: each moving part becomes its own assembly
  with a parent frame; joint state replication is deferred to its own ADR.
