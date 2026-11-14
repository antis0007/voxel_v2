# Autopilot: the game improving itself in small steps

Three layers, all already in the tree. None of them needs a person in the loop; a person can steer any of them from the dev panel.

| Layer | What it does | Entry point |
| --- | --- | --- |
| World systems | Pillars (machines, magic, forging, space) ship in the game; a world switches them off at creation. Parity rows map to a switch. | `assets/systems.ron`, `crates/app/src/world/systems.rs`, new-world screen **Systems** |
| Live engine | Read and change the running world: systems, machines, spells, tuning, captures. | `node tools/axiom-dev/engine-cli.mjs help` (ENGINE_WORKBENCH.md) |
| Loops | Feature parity (one row per pass) and code chores (one rule hit per pass). | `.claude/skills/parity-step`, `improve-cli.mjs` (IMPROVE_LOOP.md) |

## Run it unattended

In Claude Code, one writer at a time:

```
/loop 45m /parity-step
/loop 30m run one improve-cli chore: scan --limit 1, brief, hand it to the tier's agent, done
```

Each parity pass reads one compact `parity-cli describe` brief (about 1 KB), touches one RON table or a few functions, runs one `cargo check` and writes one ledger line. What you find in the morning: `docs/research/mod_parity_status.json` rows moved to `building`/`playable` with evidence paths, plus an uncommitted diff per pass to review.

## Human customization before release

- **Systems:** edit `assets/systems.ron` (labels, hints, requirements, which tabs and parity families a system owns). `engine-cli systems magic=off` flips a switch in the running world and writes `<world>/systems.ron`.
- **Content:** every table in `assets/*.ron`; the dev panel editors (`server.mjs`) cover recipes, materials, tools, spells, machines and fabricator processes.
- **Feel:** `engine-cli describe ambience|materials|models|engine`, then `tune`/`material`/`model`, `undo` and `reset`.

## Add a system

1. One row in `assets/systems.ron` with the next unused `bit` (never renumber).
2. Gate its tick or entry point with `world.rules.systems.enabled("<id>")`.
3. List its inventory pages in `tabs` and its matrix prefixes in `parity`.

Worlds made before the row existed keep the system on: the sidecar stores only what a world switched off.
