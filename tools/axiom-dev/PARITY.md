# Mod parity workflow

[Feature matrix](../../docs/research/MOD_FEATURE_MATRIX.md): 422 stable IDs
(Thermal `A09-*`, Mekanism `A13-*`, Create `B-CREATE-*`, IE `B-IE-*`, plus `L*`).
The matrix is read-only planning; progress lives in
`docs/research/mod_parity_status.json`, written only by `parity-cli set`.

```powershell
node tools/axiom-dev/parity-cli.mjs help                 # commands, states, families, rules (printed once here)
node tools/axiom-dev/parity-cli.mjs next                 # {rev, cols, rows}: open rows whose owners have code
node tools/axiom-dev/parity-cli.mjs describe A09-08      # mapping, owners [code|design], likely files, absent keywords
'{"input":{"Copper ore":64},"energy":5000}' | node tools/axiom-dev/engine-cli.mjs test 0 70 0 Crusher 600
node tools/axiom-dev/parity-cli.mjs set A09-08 --status playable --evidence crates/app/src/world/factory/recipes.rs --rev <rev>
```

- `describe` lists `absent` words with zero hits in content or factory code: a
  strong sign the feature does not exist yet. Files rank factory code and
  machine/recipe tables first.
- Headless first: `factory-lab-cli recipes|test|run` needs no running game (FACTORY_LAB.md). Tag a scenario with the row ID, then `parity-cli verify <ID>` runs it and records it as evidence.
- `engine-cli test` places (optional kind), configures, steps and prints the
  end state plus `{field: [before, after]}` changes in one call.
- States: open, design, building, playable, verified (needs evidence), blocked, parked.
- A row is ready when every owner system has `L:` substrate in the master plan.
- An AI task that names an ID gets its brief and the rules once via Gather Context.
  The board's **Send to AI workbench** relies on that, so its task text is one line.
- Board: `/parity`. Machine looks follow the authored-voxel direction.
