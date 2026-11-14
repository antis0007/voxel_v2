---
name: parity-step
description: One small modpack-parity feature per pass (Create, Thermal, Mekanism, IE, Thaumcraft, Botania...), data-first, behind its world system switch, prototyped in the running engine and recorded in the parity ledger. Use for the background improvement loop or "close the gap to the modpack".
---

# parity-step

The game should feel like a 1.12 modpack plus Create, as one design: every pillar ships in the game, and a world turns pillars off at creation (`assets/systems.ron`, `world/systems.rs`). One pass = one parity row moved forward by one honest state. Single writer in this tree.

1. **Pick.** `node tools/axiom-dev/parity-cli.mjs next --limit 5`. Take the first row whose owners show `code` and whose likely files no peer is editing (`git status --short <files>`). Skip a row a previous pass marked `blocked`. Then `parity-cli describe <ID>`: it names the `system` switch the feature belongs to.
2. **Scope it small.** Write one acceptance line before editing: the observable behaviour a scenario or `engine-cli test` will show. If the row is bigger than about 150 changed lines, ship its smallest playable slice and leave the row `building`.
3. **Data first.** Prefer new rows in `assets/*.ron` (recipes, machines modes, reactions, spells) over Rust. New machines or resources need a save bump: use modes/fields on existing kinds instead (`.claude/rules/saves.md`). Player text follows `crates/app/src/egui_ui/UI_RULES.md`; new proper nouns need owner approval, so use plain descriptive names.
4. **Gate it.** Behaviour owned by a switchable system must check `world.rules.systems.enabled("<system>")` (or the `automation()`/`magic()` helpers) where it ticks or is offered, unless it already sits under an existing gate (factory tick, casting, a hidden tab).
5. **Prototype live, then headless.** With a running workbench session (`engine-cli status`; start with `engine-cli start` only if none): `engine-cli systems`, `engine-cli test x y z Kind ticks` with a JSON `set`, `engine-cli command give ...`. Headless machine checks: `factory-lab-cli recipes|test|run` (FACTORY_LAB.md). Tagging a scenario with the row id lets `parity-cli verify <ID>` record it.
6. **Check.** `cargo check -p minecraft_style_voxel_prototype` only. No new tests, no `cargo fmt`, no commits unless the owner asked.
7. **Record.** `parity-cli set <ID> --status building|playable --evidence <file> --rev <rev>`. `verified` only with scenario evidence. Report: row, files, acceptance result, what is unverified. Keep the report under 10 lines.

Token budget: read only `describe` output, the listed files' relevant functions and the one RON table touched. Never load the full matrix or family notes; `describe` already distils them.
