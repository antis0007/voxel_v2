# Factory lab

Headless machine tests: the engine bridge's operations on a private superflat
world. No window, GPU, server or save. Each call starts one process (about 1 s).

```powershell
node tools/axiom-dev/factory-lab-cli.mjs help
node tools/axiom-dev/factory-lab-cli.mjs recipes Crusher      # [recipe, inputs, block, output, ticks, kw]
'{"energy":5000,"input":{"Copper ore":64}}' | node tools/axiom-dev/factory-lab-cli.mjs test 0 40 0 Crusher 600
node tools/axiom-dev/factory-lab-cli.mjs run                  # every scenario; prints failures only
node tools/axiom-dev/factory-lab-cli.mjs run A09-08           # scenarios tagged with a parity row
node tools/axiom-dev/parity-cli.mjs verify A09-08             # run them and record evidence in the ledger
```

- 120 steps are one second; one process tick is 0.25 s (30 steps). At most 7200 steps per request.
- `assets/processes.ron` and `assets/fabrication.ron` are read from disk on every
  run, so table edits need no rebuild. A bad table is reported with every error.
  Machine kinds, `machines.ron` and Rust behaviour need `factory-lab-cli build`
  (debug profile; `run` warns when the executable is older than those tables).
- Machines sit in the air at y 40 by convention; the world below is flat ground.
  Machines one cell apart on an axis are adjacent at one-metre scale.
- `eval` takes request lines on stdin for multi-machine setups (`place_machine`,
  `set_machine`, `step_factory` with `probe` cells, `machines`, `fill`, `probe`, `command`).

## Scenarios

`tools/axiom-dev/scenarios/<name>.json`:

```json
{ "id": "crusher-copper", "parity": ["A09-08"], "steps": [
  { "op": "machine_test", "position": [0, 40, 0], "kind": "Crusher",
    "set": { "energy": 5000, "input": { "Copper ore": 64 } }, "ticks": 600,
    "expect": { "end.out.Copper dust": 128, "end.energy": { "lt": 5000 } } } ] }
```

Every scenario starts on a fresh world. `expect` paths index the reply data;
a value is a literal or `{eq,ne,lt,le,gt,ge,has,absent}`. Reports omit zero and
empty fields, so an absent number compares as 0. A `sum:a.b+c.d` key adds
several paths for conservation checks. `place_shell` builds a coke oven, blast
furnace or smeltery controller with its complete brick shell. `"ok": false` marks a step that
must be refused. A scenario is the evidence for a parity row: `parity-cli verify`
refuses rows without one.

## Conversion-cycle audit

`node tools/axiom-dev/alchemy-cli.mjs cycles` reads processes, fabrication and brewing from disk and fails (exit 1) when a loop gains items (parity rule C-MC1). Units are items, not mass. Reactions, Touch of gold and mana flowers are not modelled.
