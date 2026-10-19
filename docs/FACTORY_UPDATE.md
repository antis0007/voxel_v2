# Machines, automation and expedition rewards

For current models, advanced recipes, encounters, magic and VXWSAVE7 persistence,
see [Living factories and encounters](LIVING_FACTORY_UPDATE.md). This document
records the original factory increment and its first-line layout.

New natural worlds use generator 4. Existing worlds keep their saved terrain
version and can use the new machines; create a new world for the new ruin and
cave variants. The protected inland start and workshop mine remain intact.

## First production line

1. Open the workshop salvage cache with **F**, smelt starting ore at its manual
   processor, and harvest luminous mushrooms below for fibre and the lantern.
   Manual smelting now produces **one ingot per ore**, using 0.25 coal.
2. Use **F** on the workbench, then **E** nearby. Crafting buttons show their
   ingredients and disable when the bench is out of reach or supplies are missing.
   Bore upgrades are now explicit crafting buttons, rather than automatic F actions.
3. Craft a coal generator, crusher, electric furnace, feed hopper, output crate,
   item pipes and power cables. A kit is selected when crafted; close E and
   right-click clear space to place it. Kit buttons in E select remaining stock.
4. Run an item-pipe bus along one side of your machines and a power-cable bus
   along another. Connect the hopper, crusher, furnace, output crate and generator
   to the item bus. Connect the generator, crusher and furnace to the power bus.
   Connections must touch **faces**; machines are endpoints and do not relay buses.
5. **F** inspects a machine. **E → Load matching supplies** loads its accepted
   materials; **Collect / unload** returns both its input and output inventory.
   Load ore and coal into the hopper. Pipes route coal to the generator, ore to
   the crusher, dust to the furnace and ingots to the output crate automatically.

Example top-down layout (each position is one metre; `.` is empty space):

```text
P P P P P P P P P     P = item pipe
H . C . F . O . G     H = hopper; C = crusher; F = furnace
. . W W W W W W W     O = output crate; G = generator; W = power cable
```

This example needs nine pipes and seven cables: craft pipes twice and cables
once. Machines can share a bus in any order; material filters determine routing.
One crusher supplies two furnaces at full utilization, so a second furnace is
a useful first expansion. With one furnace, dust accumulates safely in the crusher.

## Recipes and operation

| Assembly | Crafting cost | Operation |
|---|---|---|
| Coal generator | 2 copper, 2 iron ingots | 64 energy/s; 0.25 coal provides 400 energy |
| Crusher | 2 copper, 1 tin, 2 iron ingots | 1 ore → 2 dust in 4 s; 128 energy |
| Electric furnace | 2 copper, 2 iron ingots, 1 fibre | 1 dust → 1 ingot in 3 s; 72 energy |
| Feed hopper | 1 tin ingot | Accepts raw ore, coal, fibre, crystals and biomass; exports to matching consumers |
| Output crate | 1 iron ingot | Receives finished ingots; never exports them back into the line |
| 8 item pipes | 1 tin ingot | Routes up to one unit per source per quarter-second |
| 8 power cables | 1 copper ingot | Connects generator buffers to processors |
| Fungal bioreactor | 4 iron ingots, 2 crystals, 2 ancient circuits, 4 biomass | 128 energy/s; 0.5 biomass provides 800 energy |

Inventories hold 64 units of each accepted material. Output backpressure stops
processing without consuming the input. Power shortages pause work; consumer
priority rotates. Pause machines to reconfigure them. Mining any part recovers
the complete assembly, its kit and unconsumed input/output. Stored energy and
partly burned fuel dissipate when dismantled; unprocessed material is returned.

All assemblies occupy a **one-metre cube**, including at half/quarter-metre voxel
scales. Placement checks the whole body against terrain and the player. Pipes
and cables are full-block conduit junctions with distinct port patterns; machines
have metre-sized panels, vents, warning stripes and reactor windows. These are
not animated multiblock models. Builder edits cannot overwrite a registered
assembly and strand its inventory.

Machines run while playing, including while exploring elsewhere; pause and
offline time do not advance them. Graphs rebuild only when assemblies change.
There is a 1,024-assembly world limit. This is a bounded factory simulation, not
a general fluid-pipe system. The creative block palette remains available, so
the surrounding game is still a sandbox rather than a strict survival economy.

## Reasons to explore

- **Foundries** have twin furnace stacks, trusses, ore/coal bunkers and engineer
  caches. **Conservatories** have broken colonnades and harvestable mushroom beds.
  **Archives** have branching aisles and crystal-lit alcoves. These join workshop
  mines, watchtowers and waterworks, with seeded dimensions, rotations and damage.
- Watchtower balconies hold an additional engineer cache, rewarding the climb.
- **Lanternroot groves** expose iron; **Embercap hollows** concentrate red biomass
  and coal among basalt; **Prismatic cathedrals** have taller chambers, mineral
  columns and crystal geodes; **Drowned gardens** contain shallow pools, dry
  causeways and tin. Curved connecting galleries remain.
- Chamber side niches contain mycologist or engineer caches. Engineer caches
  provide refined metal and ancient circuits; mycologist caches provide fibre,
  crystals, biomass and circuits. Circuits are needed for the stronger bioreactor.
- Red caps and mushroom stems supply biomass. All harvested materials and cache
  rewards remain volume-weighted, so finer voxels do not multiply the yield.

## Persistence and verification

VXWSAVE6 includes factory kits, inventories, power buffers, fuel remainder,
enabled state, active recipe and progress. V1–5 saves remain readable. Factories
share the terrain checkpoint; loads validate bounded quantities, unique positions
and the assembly's blocks before accepting the session. Networks rebuild after
load; workbench proximity and inspection focus are deliberately transient.

Local `cargo build --offline --target-dir target` passed, including build-time
WGSL validation. No tests or game launch were performed. Runtime interaction,
visual presentation, throughput under load and save/reload still require hands-on
validation. Build evidence is in `target/factory-build.log`.

Implementation: `src/world/factory.rs`, `src/egui_ui/factory.rs`, and the expedition,
generation, material, save, shader and main-loop integration points.
