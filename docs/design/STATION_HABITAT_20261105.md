# Voxel lift habitat

Implementation slice, 2026-11-05. Native acceptance pending.

Boarding a personnel lift registers one persistent habitat at that corridor's
crown. It is built incrementally during ascent using the ordinary edited chunk
store, mesher, collision, raycasts, mining and placement. The exterior and walked
hull are the same blocks. Existing procedural terrain is unchanged; this is a
runtime construction feature, not a rewrite of an old generator gate.

The initial habitat is a 12 m internal diameter, 16 m long cylinder with a 2 m
steel shell. A 3 m high vestibule connects its lower end to the cabin. Copper
surrounds identify two steel pressure doors. Parameters live in
`assets/station_habitat.ron`. These are a compact playable module, not the entire
population/district design in the civilization spec.

Movement reuses `player/radial.rs`: the cylinder's X axis is weightless and radial
acceleration grows toward the shell, reaching 9.81 m/s² at the inner floor.
The entrance passage retains downward gravity. The camera, capsule and controls
use the same local up. Leaving the cabin uses ordinary voxel collision; returning
through its real doorway restores cabin motion. A loaded station reconstructs its
docked return cabin from the persisted berth.

`engine_sim::atmosphere` owns conservative two-species pressure/volume transfers.
The world adapter uses fixed-temperature, one-metre control volumes and samples
the actual selected-scale voxels for free volume and open face area. Flow is a
bounded gameplay conductance, not a compressible fluid solver. Gas may be trapped
and compressed when construction removes free space; construction cannot erase it.
The bounded exterior is currently an explicit vacuum reservoir, not a planetary
atmosphere inferred from altitude. Lost gas is recorded in an exhaust ledger.

Pressure and oxygen partial pressure appear beside station play. Low pressure or
oxygen causes environmental suffocation damage. This slice has no suit/EVA reserve
or biological oxygen consumption. It does not provide general thermal simulation.
One-metre control volumes assume internally mixed gas; arbitrary thin partitions
inside a control volume need component refinement before exact sub-voxel sealing
can be claimed. The authored full-metre doors align with the control-volume faces.
The moving cabin carries a finite bulk atmosphere at its actual 7 by 7 by 3 m
volume. Docking couples that volume to the voxel port. The lock equalizes to cabin
pressure when docked and exterior vacuum otherwise; ordinary cabin boarding does
not require standing in a depressurized lock. Departure requires the outer panel
closed and retains the cabin's remaining gas rather than resetting its pressure.

Use `[F]` while facing either door to cycle. The lock closes both panels, rejects a
player obstruction, recovers gas into a finite tank or vents surplus, then admits
the requested side after equalization. Pressing use on an open side cycles toward
the opposite side. Damaged panels fault the mechanism and must be repaired with
blocks; use does not recreate a destroyed closed door. The pump is manually
operable in this slice and has no factory power connection yet.

Ordinary terrain spells, drills, replacement batches, blast harvesting and TNT
respect steel hull resistance. Direct player mining still follows the existing
material hardness and tool rules. A queued spell crater requires at least a 6 m
radius to cut steel; its finite hull-work budget scales with radius cubed, and
removal charges by physical voxel volume. A large visual radius alone does not
grant unlimited station removal. Luminous panels are weaker local fittings backed
by the outer steel shell. Player construction and repair use the normal inventory.

Save version 53 appends habitat identity, berth, construction cursor, gas, tank,
exhaust and airlock state after version 52's industrial state. Older saves have no
habitats until boarding. Cached geometry is rebuilt from persisted chunks; gas is
not refilled on load. First construction alone initializes the finite atmosphere.
At most 16 habitats are admitted. Co-op state replication and station inhabitants
are not implemented by this slice.

Validation: final local application compile check passed with 25 composed shader
programs validated (`target/habitat-final-check.log`). An intermediate release build
completed after a shared artifact lock (`target/habitat-build.log`); this is not a
build of every subsequent source adjustment. A native attempt with the available habitat executable
produced no log or capture and was no longer visible by its launch PID. It is not
runtime acceptance of the final source. Receipt:
`target/habitat-native-20261106-021604/receipt.json`. A subsequent native retry reached
GPU initialization and current-build shader validation; station gameplay acceptance
is still pending. No tests or hosted CI added or run.

## Generated wing, 2026-11-11

A habitat registered from this date carries a seeded station wing
(`engine_worldgen::station`) past the drum's far endcap. A 2 by 3 m doorway cut
through the endcap meets the wing hatch; the wing origin sits at local
x = 11 + length + shell. The drum, lock, cabin port and freight fixtures are
unchanged, so freight and security keep their drum-local positions.

Each habitat stores its own gas grid bounds (`Grid`): the drum box grown to the
plan bounds plus one vacuum cell. The wing uses downward gravity; the drum keeps
its spin. Rooms and doorways on the spine side start at station pressure. The
damaged section starts in vacuum behind a steel bulkhead in its doorway; removing
the bulkhead drains the station through the breach at the gas conductance rate
(about 20 % of spine pressure in 60 s in the test fixture).

Saves: `grid` and `wing` are defaulted RON fields in the version 53 footer, so
`SAVE_VERSION` is unchanged. A habitat saved earlier loads with the drum box and
no wing, then retrofits on its first tick: the grid grows, existing gas is kept
by local cell, and only the endcap doorway and cells past the drum are built,
never over existing blocks. `retrofit` marks a build in progress.

Inhabitants: `habitat/residents.rs` fills a roster once from the plan markers
(trader and residents as villagers, guards as security robots, hostiles from the
shell spawn kinds 10, 25, 29, 26, 23). Actors use enemy slots from 1000, appear
within 85 m of the player, keep position and health across eviction and saves
(`residents` and `populated`, defaulted fields), and are removed on death. They
never respawn.

Window check, 2026-11-13: `VOXEL_TORCH_REVIEW_MODE=station` with
`--benchmark-torch-review --benchmark-capture-frames` builds a habitat at
(0, 100, 0), then captures the spine, airlock, hull from outside and a second
spine pose (frames land in `VOXEL_CAPTURE_DIR`; the log prints inhabitant
positions). At 1 m scale the spine shows copper-trimmed doorways, steel plate
ceiling, lamp panels, rooms and a guard robot hovering 0.5 m up like the cargo
guards. Camera poses use feet height (eye is higher), so a feet y of 101.6 puts
the eye inside the 3 m ceiling.

Open: wing furniture is placed per voxel, so at 0.5 m and 0.25 m a bed or chest
becomes several blocks of the same physical size; the hull from outside is
checked only as flat distant boxes, not against the crown deck gallery; trader
stock is not wired to the freight ledger.

Tests: `orbital::habitat::tests` (8) and `station::tests` (6, 200 seeds).
