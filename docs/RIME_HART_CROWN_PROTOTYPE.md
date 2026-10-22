# Rime Hart: editable external crown prototype

21 October 2026. This is the first interaction prototype, not the finished boss
roster or a completed version of the concept-sheet asset. The separate design
brief informs the direction; the pasted request establishes the Hart-first scope.

The existing Rime variant of enemy kind 8 now commits to a visible nearby heat
source, plants at a local full-water cell, grows real editable ice, warns, sends
a travelling pulse through connected ice, vents, and resumes physical combat.
Amethyst and Verdant keep their earlier attacks. Normal damage remains effective.

## Shared systems and limits

- Existing torch indices and sparse thermal cells supply heat cues. Terrain can
  occlude a source. Targets stay fixed for a planting action. Each decision tries
  at most eight wet contacts separated by 80 cm, so its own old ice can force a
  different contact instead of repeatedly rejecting the nearest one.
- One full eight-unit water cell pays for each new ice voxel. Partial destination
  water is refused; failed destination writes refund the withdrawn full cell.
  Factory-owned cells are excluded. No inventory, material ID or save field added.
- Water candidates come from one initially connected local pool, within 3.5 m.
  Water and ice use existing world edits, persistence and mesh invalidation.
- Each action admits at most 384 planned cells. Growth advances at 1.5 m/s with
  at most four voxel transfers per work step. Ice blocks the player and normal
  projectiles, and uses the existing cutting and thermal melting operations.
- A new pulse requires the Hart to remain within 35 cm of its planted position
  and have real ice at its visible contact. Six-neighbour propagation checks live
  terrain every 180 ms, visits at most 768 cells, and admits at most 64 next-front
  cells. A cut ahead stops that route. Already-emitted pulses do not need a remote
  tether to the moving creature. One pulse can damage the player only once.
- Planting stops if its supporting floor is removed. Disengagement stops attacks
  without teleporting home, refilling health, or undoing consumed water and ice.

Freezing and the pulse are explicitly magical rules. Water, work time, and growth
admission are costs; this prototype does **not** implement an energy-conserving
heat pump. Mantle venting is presentation, not measured heat rejection.

## Model

The Rime uses peat hide, an aged pale face and mantle, warm narrow vents, heavier
chest/neck roots, a lean belly, broader crown beams and comb tines. Named rigid
parts retain gait, bow, sweep and recovery poses. Surface family 57 is a renderer
tag, not a saved block. Its 5 cm pixel grain uses coordinates baked before rigid
part animation and world rotation, with distance filtering and matte response.
Other crystal Hart variants retain family 56. This remains a procedural authored
mesh: a production-quality asset, complete planting rig and polished foot contact
are still outstanding.

## Native review

Build locally with the existing target directory and no CI or tests:

```powershell
cargo build --profile release-fast --locked --offline --target-dir target
tools/Review-HartCrown.ps1 -Case Intact -Executable target/release-fast/minecraft_style_voxel_prototype.exe
tools/Review-HartCrown.ps1 -Case Cut -Executable target/release-fast/minecraft_style_voxel_prototype.exe
tools/Review-HartCrown.ps1 -Case Combat -Executable target/release-fast/minecraft_style_voxel_prototype.exe
tools/Review-HartCrown.ps1 -Case Manual -Executable target/release-fast/minecraft_style_voxel_prototype.exe
```

The disposable fixture supplies a finite pool, a bank, a trench and a torch beyond it.
Intact runs real AI and world edits. Cut performs one explicitly scripted junction
cut through the existing utility carving function. Both move one existing torch
after the first vent to demonstrate a changed lure; no extra torch is created.
Combat automatically aims and
casts the real wand with normal costs. Cameras are scripted first-person views;
these recordings do not establish manual traversal or combat feel. Manifests
record executable hashes, arguments and capture cadence. The independent review
binary uses a frozen source snapshot to avoid racing other sessions' builds.

Manual opens a visible disposable scene for ten minutes with normal mouse and
keyboard input, real fluid physics, the starter torches/building blocks and a
bronze bore in slot 6. Click the window to capture the mouse; existing controls
apply (the bore uses Alt+LMB). No automatic camera, casting, cutting, retargeting
or repeated PNG capture runs in this mode. This launch path is provided for
human review; it has not been manually played by the agent.

## Remaining production gates

Routes currently grow on one horizontal level; ledge-climbing growth is not
implemented. Manual crossing and cutting, different successful player approaches, slopes and
all voxel scales, reduced effects, persistence/reload and interruption exploit
review remain acceptance work. World material persistence is inherited, not yet
verified by a reload recording. The live graph and pulse restart safely as idle
runtime state, rather than reconstructing a saved attack. Unfinished boss health
is not newly persisted. Existing victory flags and rewards are unchanged.

The natural site's seed-archive history, restoration conditions, heat/work
accounting, rime construction reward and useful persistent custodian need a
further integrated slice. Existing worlds are not excavated or regenerated.
The Unfinished Engine and the remaining roster are not implemented by this pass.
