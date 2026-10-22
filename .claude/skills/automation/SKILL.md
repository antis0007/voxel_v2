---
name: automation
description: Factory machines, item and power networks, inventory and recipes — data files, tick entry points, save-format rules.
---

# Automation

Where: `crates/app/src/world/factory.rs` (`Kind`, `Node`, `Factory::tick`, `step`), `world/factory/` (extraction, pile, power, induction, transit, assembler, overview, portable kits), `world/inventory.rs` (items, stacks, crafting), `world/recipe_index.rs`, `world/expedition.rs`, `world/habitats/`.

Data: machine rows in `assets/machines.ron` (block, name, workbench cost, accepted resource indices, flags, upgrade costs; one row per `Kind` in enum order) and recipes in `assets/recipes.ron` (hand, bench, crystal-tech). `all_recipes()` also derives a machine-kit recipe for every non-structural kind whose cost is in whole units of 64. Resource indices are the `Item::Resource(u8)` ids; `COKE`, `CREOSOTE`, `ETHANOL`, `pile::FUEL` name a few.

Tick: `factory::step(world)` returns the machine size; `Factory::tick(dt, size)` runs networks; extraction (drills, pumps) runs inside `WorldSimulation::update` under `FACTORY_BUDGET` with a `Budget`, so headless runs do identical work.

Save format: adding a machine kind or item changes the save layout. `Kind::count_in_format` gates kinds by save version; new kinds must append to the enum and bump `SAVE_VERSION` in `world/storage.rs`. Prefer a mode or field on an existing kind over a new kind.

Test: `just test factory`, `just test inventory`, `just test storage` (round trips of old versions). A new machine needs: an enum variant, a RON row (run the ignored `dump_machines_ron` after editing the struct, or add the row by hand), a `BlockId` for its block, and a storage test for the bumped version.

Pitfalls: `Kind::powered()` depends on `processor()`, which is still code; the RON row stores the resulting flag. Item kits (`PowerKit`) carry energy and upgrade level in the item itself.
