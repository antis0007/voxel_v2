# Fluid and gas logistics (proposal)

Status: proposed 2026-11-05, names pending owner approval. Every new item or machine below needs a save version bump (`.claude/rules/saves.md`); where a mode or field on an existing row fits, prefer it.

## Goal

Moving liquids and gases is a progression line of its own, with the same shape as items and power: carry by hand first, then store in place, then automate. Each tier removes one chore from the tier before and adds one decision.

## What exists

- `Item::Bucket(u32)`: water in microlitres, 1 m³ capacity. Lava rides on payload bit 31 as one whole block (`water_tools::LAVA_BUCKET`), so no save change was needed.
- `FluidPipe` and `Tank`/`WaterTank` rows in `assets/machines.ron`, water only, network built in `world/factory.rs`.
- Steam and Smoke live in the thermal sandbox only. Nothing carries them.

## Tiers

| Tier | Hand | Placed | Network | Unlock |
|---|---|---|---|---|
| 0 Primitive | Bucket: 1 m³ water or 1 lava block | none | none | wood + iron |
| 1 Copper | Fluid cell: 4 m³, one fluid id | Copper tank: 16 m³, keeps contents when broken | Copper pipe: 1 m³/s, any liquid below 600 K | bronze |
| 2 Steel | Pressure cell: 16 m³ liquid or 64 m³ gas | Steel tank: 64 m³, gas or liquid | Steel pipe: 4 m³/s, liquids to 1500 K (lava); gas duct: 8 m³/s | steel, first steam engine |
| 3 Alloy | Reservoir pack: 64 m³, auto-fills from a tagged tank | Multiblock tank: 1 000 m³ per 3×3×3 shell | Alloy pipe: 16 m³/s, per-side filters | chip slice |

Rules that keep it cohesive:

- One fluid id per container, shown as name and volume with units (UI_RULES).
- A pipe rated below a fluid's temperature does not carry it. Lava in copper is refused at the port, never silently lost.
- Gas ducts carry Steam and Smoke from the thermal sandbox. A boiler's steam goes to turbines through a duct. A furnace's smoke can be vented or scrubbed, so pollution becomes a choice.
- Pumps are the only source that pulls from the world. A pump on lava is the tier-2 route to geothermal power, tying the new magma cave zone (gate 126) into industry.

## Order of work

1. Fluid id in the bucket payload for every liquid, not only lava (no save change).
2. Copper tier: the cell item, a fluid id on `Tank`, a temperature limit on `FluidPipe` (save bump, one migration).
3. Gas duct plus steam from the boiler to the turbine.
4. Lava pump plus geothermal generator.
5. Multiblock tank, filters.
