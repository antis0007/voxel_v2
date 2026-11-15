---
paths:
  - "crates/app/src/world/**"
  - "crates/voxel/**"
  - "crates/worldgen/**"
  - "assets/*.ron"
---
# Save and generator-version contracts

- `SAVE_VERSION` (magic `VXWSAV29`) and `CURRENT_GENERATOR_VERSION` change only with a migration. Old versions must keep generating and loading identically; never edit a historical version gate.
- A new-format round trip alone proves nothing about compatibility: also load an older save or fixture and check existing data survives (`just test migrate`, `just test save`).
- New machines or resources need a save version bump; prefer modes or fields on existing ones.
- Content tables live in `assets/*.ron`; `*_ron_round_trips` tests must keep passing, and a changed struct needs the ignored `dump_*_ron` test rerun.
- Gate numbers are shared across branches and worktrees: claim one in `docs/worldgen/GENERATOR_GATES.md` first and grep every branch for it.
- Optional presentation tables (descriptions, flavour, hints) load per entry at runtime: a whole-catalog fallback hid all 123 descriptions over 4 flawed lines. Check with `cargo run -p engine_content --example validate_descriptions`; it prints how many lines still load in game.
