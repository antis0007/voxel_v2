# Power, multiblocks and the recipe browser

Current writer: **save 22**. The v20/v21 design notes below are retained for
history; current pickup, matrix ownership and accounting behavior is in the
v22 section at the end and `INTEGRATED_GAME_RELEASE_GATES.md`.

Save version 20. Earlier saves (1–19) still load; nothing in an existing world
changes until you build the new parts.

## Power in SI units

The factory still simulates in 0.25 s ticks and integer energy units. One unit
is now defined as **250 J**, so the numbers read directly in SI:

- a flow of 1 unit per tick is exactly **1 kW**;
- 14.4 units are **1 Wh**; 14 400 units are 1 kWh.

Nothing about the old balance moved; only the labels did. Every panel, the
look-at readout and the recipe browser now say W, kW, kJ, Wh and kWh.
Helpers are in `src/world/factory/power.rs` (`fmt_power`, `fmt_energy`,
`fmt_joules`).

| Source | Output | Fuel value |
|---|---|---|
| Combustion generator: coke | 16 kW | 800 kJ per item |
| Combustion generator: coal | 16 kW | 400 kJ per item |
| Combustion generator: bio-ethanol | 16 kW | 600 kJ per item |
| Combustion generator: creosote | 16 kW | 300 kJ per item |
| Fungal bioreactor | 32 kW | 400 kJ per item of biomass |
| Arcane pile | 64 kW | 3.6 MJ per arcane core |

The generator burns the best fuel it holds first (coke, coal, ethanol,
creosote). Machine draw: crusher 8 kW, furnace 6 kW, washer 12 kW, and so on;
drive upgrades multiply the draw, not the energy per batch.

## Battery banks

A new assembly (2 copper, 2 tin, 1 iron). It sits on a cable network like any
machine.

| Cells | Capacity | Rate | Upgrade |
|---|---|---|---|
| Lead-acid | 1 kWh | 4 kW | — |
| Nickel-iron | 4 kWh | 16 kW | 2 bronze, 1 coil, 1 iron |
| Resonant cell | 16 kWh | 64 kW | 2 steel, 2 coils, 1 arcane core |

Each tick, generators feed machines first; batteries cover what the generators
cannot, up to their rate; any generator surplus then charges batteries.
Modes: **Balanced**, **Charge only** (a reserve that never drains), **Discharge
only**, and **Mana font** (see magic below). Upgrading keeps the stored charge;
dismantling kept only parts before v22. Current packing retains charge and upgrades.

Each cable network is measured every tick (`Factory::grid`): generation, load,
net battery flow, stored and capacity. Machine panels and the look-at panel
show it, including how long the reserve lasts at the present deficit.

## Railcraft-style multiblocks

Both structures are built from one-metre brick assemblies, so a shell costs the
same at every voxel scale, and both run on heat instead of electricity. The
temperature is simulated: it climbs while fuelled and falls when not.

**Coke oven.** A 3 × 3 × 3 shell of 26 coke oven bricks with the **coke oven
door** in the middle of one wall, middle course. Charged with coal, it heats to
about 1000 °C and bakes 1 coal → 1 coke + 0.25 creosote oil every 10 s.

**Blast furnace.** 3 wide × 4 tall × 3 deep, 35 blast bricks, with the
**tuyere** in the middle of one wall on the second course. Coke both heats it
(a little burns every 2 s) and reduces the ore. Modes:

- Iron: 2 iron ore + 0.5 coke → 3 iron ingots at 1500 °C (16 s)
- Steel: 2 iron ingots + 0.5 coke → 2 steel at 1400 °C (12 s)

Pipes attach to the controller's exposed face, so both are fully automatable:
a hopper or extractor feeds coal into the oven, a pipe takes coke on to the
blast furnace and the generators, and iron ingots go to a crate. A brick may
belong to only one shell. Formation is recomputed whenever the factory
topology changes. Tearing down a controller mid-batch returns its charge.

Bricks: 2 clay + 2 sand → 4 coke oven bricks; 2 stone bricks + 1 clay +
1 coke → 4 blast bricks (both at a workbench). The workbench kit list also
offers a coal or coke fallback for players without clay.

Code: `src/world/factory/multiblock.rs`.

## Smelting, Forestry-style fuel, and new recipes

- Electric furnace has a **feed** mode: Dust (as before) or Ore (raw ore → ingot
  1:1, slower, no yield bonus). Existing furnaces stay in Dust mode, so
  established lines keep routing ore through crushers and washers.
- Mycelial cultivator, **Fermenter** mode: 2 biomass + 0.25 fibre → 1
  bio-ethanol. Two cultivators and a generator make a renewable fuel loop.
- Component fabricator, **Essence** mode: 1 crystal + 2 biomass → 1 wild
  essence at 6 kW for 12 s, a tech route into the magic economy.

## Crafting: Minecraft-style grids

The crafting grid is 3 × 3. Away from a workbench only the top-left 2 × 2 is
usable, as in Minecraft's inventory; press F at a workbench, then E, for the
full grid. Machine kits are now ordinary grid recipes generated from each
machine's cost (`inventory::all_recipes`), so the grid and the workbench
buttons agree. Leaving the bench folds the outer slots back into the pack.

## Recipe browser (JEI-style)

A **Recipes** tab in the inventory. Left: every item, searchable by name or
category, `@station` to filter by where it is made. Right: the selected item's
**Recipes**, **Uses**, and **Tree**. Click any item in a card to follow its
recipes, right-click for its uses, ◀ to go back. Over any inventory slot, **R**
opens recipes and **U** uses (Sort keeps R when nothing is hovered).

Recipe cards show inputs → station → outputs, time, draw (kW) and energy per
batch (kJ). Grid recipes have a **+** that stages the ingredients into the
grid. The index covers grid recipes, every machine process, both multiblocks,
generator fuels and the field smelter (`src/world/recipe_index.rs`).

**Tree** expands a target ×1/4/16/64 into every intermediate step, drawing on
what you carry first (like an autocrafting calculator), and lists the raw
materials still to gather with total station time and energy. ⟳ on a step
switches to another recipe for that item (crusher vs. washer dust, furnace vs.
blast furnace iron...).

## Look-at panel

The HWYLA/Jade panel is smaller (196 px) and now reads the assembly under the
crosshair, not only the inspected one. It shows only what matters for the
kind: live power flow, battery charge and rate, generator fuel and burn time,
batch progress with seconds left, hearth temperature, multiblock shell
completeness, buffers as tiny item stacks, and the grid balance with reserve
time. Bricks in a formed shell say which structure they belong to.

## Magic links

- **Mana font** battery mode: within 6 m, the nearest font converts stored
  charge into mana at 1 kJ per point, up to 20 mana/s. A violet wisp marks it.
- A **Spark** that hits a battery banks 60 kJ (thaumic induction).
- The fabricator's Essence mode turns crystal and biomass into wild essence.

## Verification

`cargo test`: 274 passing, including new tests for power units, battery
charge/discharge and reserve mode, multiblock formation in every orientation,
a broken shell, coke and iron throughput, recipe index coverage, the crafting
tree planner, bench-only kits, 2 × 2 staging, and v19 → v20 inventory and
factory decoding. Visual review fixture: `VOXEL_TORCH_REVIEW_MODE=factory` with
`--benchmark-torch-review` now includes a battery bank, a coke oven and a blast
furnace.

# Save version 21: induction, transformers, cable tiers, assemblers

## Thermal / Mekanism power

- **Energy-cell I/O limit.** Each battery bank has an I/O slider (1 kW up to
  its tier rate), like a Thermal Expansion energy cell's input/output setting.
- **Induction matrix** (Mekanism). A hollow box of induction casing, 3 to 9
  metres on every side, with one or more induction ports in its walls. Inside:
  induction cells (64 kWh each) and providers (256 kW each); empty space is
  allowed. A 9 × 9 × 9 box holds up to 343 cells, about 22 MWh. The matrix has
  one energy pool shared by all ports. A port in Input mode charges it from
  its cable; Output mode discharges into its cable. The panel names the
  problem when a box does not form (size, a gap in a wall, a foreign part
  inside, no port). Charge survives re-forming, clamped to the new capacity.
  Code: `src/world/factory/induction.rs`.
- **Cable tiers.** Copper 256 kW, HV armoured 2 MW, resonant superconductor
  16 MW. A network carries at most its tier's capacity per tick (the cable
  panel and the look-at panel show the load against the limit). "Upgrade line"
  upgrades every connected piece of that tier while materials last.
  Different tiers do not join.
- **Transformer.** Bridges two networks or two cable tiers: 64 kW, then 512 kW,
  then 4 MW. It buffers two ticks of energy and moves surplus on one side to
  the deficit on the other. Any storage touching two networks bridges them;
  every storage pool now has one rate allowance per tick shared across all
  the networks it touches.

## Biomass burning

- Generator **Biomass burner** mode: burns biomass at 12 kW (200 kJ per item).
  It is opt-in, so existing combustion generators do not start eating the
  cultivators' culture.
- Grid recipes: 4 leaves → 1 biomass, 4 pine needles → 1, 2 planks → 1
  (logs → planks → biomass).

## Factorio-style automation

- **Assembling machine.** Choose any grid recipe made only of factory
  resources (every machine kit, among others). It crafts from piped
  ingredients at 5 kW, 2 s per craft, faster with drive upgrades. Resource
  products leave through the pipes; machine kits collect inside (up to 256)
  until you take them. An assembler line can build the parts for the next
  line. The recipe browser lists every automatable recipe under the
  assembling machine as well as the workbench.
- **Production statistics** (inventory → Production): items made and used
  per minute over the last minute or hour, with net rates and a 60-second
  generation-against-load graph. Counts are measured from machine buffers, so
  every recipe, fuel and multiblock is included. Code:
  `src/world/factory/stats.rs`, `src/egui_ui/production.rs`.

Save version 21 stores two more words per assembly (assembler recipe, battery
I/O limit, assembler stock). Saves from versions 1–20 still load.

## Save version 22: ownership and accounting repair

Packed power assemblies retain exact charge, upgrades, mode and battery I/O
settings in a non-stackable item. Dismantling requires room for all contents;
assembler kits are recovered too. Collect assembler outputs before changing
the recipe so existing stock cannot change identity. The loader now accepts
valid partial assembler crafts, inputs/outputs and biomass-generator buffers.

Matrix pools belong to cells rather than a selected port. Before topology edits,
charge is distributed into physical cells; cell pickup takes that charge and
port pickup leaves the healthy pool behind. Splits and merges preserve actual
cell ownership. Incomplete and over-capacity legacy structures retain charge
offline instead of clamping it away.

Production separates source generation, committed consumption, consumer-buffer
deliveries, storage charge/discharge and converter intake. The hour window is
exactly the last 3,600 seconds. These are ideal energy links, not simulated AC
transformers or resistive cables. See `INTEGRATED_GAME_RELEASE_GATES.md` for
verification, timing failures and unfinished release gates.
