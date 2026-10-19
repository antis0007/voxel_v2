# Living factories and encounters

See [Arcane industry update](ARCANE_INDUSTRY_UPDATE.md) for the current combat, mining, fluids and interface changes.

This update extends existing generator-3/4 natural worlds without regenerating
their terrain. Flat worlds support machines and crafted magic, but do not gain
procedural guardians. The earlier factory guide remains useful for the first
ore-processing line; the recipes and behavior below supersede its limitations.

## Models and animation

Factory assemblies now render as separate lit geometry instead of terrain cubes:

- Coal generators have an engine housing, flywheel, exhaust and firebox.
- Crushers have exposed counter-rotating toothed rollers.
- Furnaces and alloy kilns have inset fireboxes and exhaust stacks.
- Fabricators use a moving press above a component bed.
- Bioreactors and cultivators have framed chambers and orbiting luminous cultures.
- Hoppers have open walls and a visible inventory fill level; output crates have
  a lid and metal framing. Status lamps distinguish running, idle and paused states.
- Item pipes and power cables have narrow octagonal tubes, collars and connected
  branches. Lit flow indicators appear when their network actually transfers
  material or power. Their collision and ray picking use the connected tube shape.

Animations advance from simulation state. Paused, unfueled and blocked machinery
does not continuously spin. Objects remain one physical metre across voxel scales.
Nearby machines use detailed parts; distant/dense factories use simpler housings.
The factory simulation remains capped at 1,024 assemblies; detailed model selection
is distance ordered and bounded. Machine chassis retain conservative collision
envelopes. Moving parts are visual, not rigid-body crushing hazards.

## Production beyond ingots

Use **F** to inspect a machine, then **E** for its inventory, recipe, upgrades and
filters. The workbench remains **F → E**. New recipes are:

| Station | Recipe | Base time / energy |
|---|---|---|
| Alloy kiln | 3 copper + 1 tin → 4 bronze | 6 s / 240 |
| Alloy kiln | 2 iron + 1 coal → 2 steel | 8 s / 384 |
| Component fabricator | 2 copper + 1 lumen fibre → 2 induction coils | 5 s / 200 |
| Component fabricator | 1 crystal + 1 ancient circuit + 1 wild essence → 1 arcane core | 7 s / 392 |
| Mycelial cultivator | 0.25 fibre → 2 biomass, with 1 biomass retained as a reusable culture | 8 s / 256 |
| Mycelial cultivator | 2 biomass → 1 lumen fibre | 8 s / 256 |

Build an alloy kiln with 4 iron and 2 copper ingots. Bronze unlocks the fabricator
(2 bronze, 2 iron, 1 circuit); coils unlock the cultivator (2 bronze, 1 coil,
2 biomass). The fungal bioreactor now requires 4 steel, 2 coils, 1 arcane core
and 4 biomass. Existing bioreactors continue working.

Two cultivators set to opposite recipes can maintain a powered fungal production
line. Seed the biomass machine with one biomass culture and load a little fibre
to begin. Use separate inventories or filters to retain useful excess output.

Drive upgrades cost 2 bronze + 1 coil for double speed, then 2 steel + 2 coils +
1 arcane core for triple speed. Power demand scales with speed; total recipe
energy stays the same. Dismantling returns the kit, inventory and installed drive
ingredients. Stored energy and partially burned fuel still dissipate.

Hoppers can now feed any carried resource. Hopper and output-crate filters select
one material or all accepted materials. Filters affect hopper loading/export and
crate intake. Consumers take priority over storage on a shared pipe bus, enabling
ore → dust → ingot → alloy/component production without manual transfers. Switching
machine recipes retains ingredients but resets unfinished progress and spent energy.

## Guardians and magic

Site-based encounters use the same ruin and cavern layout as terrain generation:

- **Sporelings** wander fungal chambers and rush the player with a visible wind-up.
- **Ruin sentinels** patrol, approach and fire slow bolts after a glowing telegraph.
- **Wisps** float, keep their distance and fire magical projectiles.

Guardians use terrain collision and line of sight, return toward their territory,
and cannot spawn within eight metres of the player. The inland clearing has no
encounter anchor; its introductory guardians are below the workshop in the fungal
chamber. Enemy movement is local steering with obstacle jumping, not full pathfinding.
Up to 24 guardians and 96 projectiles are active. Nearby fungal surfaces release
sparse, depth-occluded spores. Wounded enemies display a small health bar.

**Left-click** strikes a nearby aimed-at enemy instead of mining the block behind
it. Craft a lumen wand for 1 copper, 1 crystal and 2 fibre at the workbench.
**R casts; T cycles spells.** A wand appears during casting, with colored bolts,
trails and impact effects. Health, mana and spell selection appear above the hotbar.

| Spell | Unlock | Effect | Mana |
|---|---|---|---|
| Spark | Lumen wand | Projectile damage; can charge a processor with 48 energy | 12 |
| Mend | Lumen wand | Restore 30 health | 30 |
| Frost lance | Frost attunement: 2 steel, 2 essence, 1 coil | Damage and four-second slow | 18 |
| Resonant burst | Resonant attunement: 2 cores, 4 essence, 2 bronze | Impact damage and a three-metre burst respecting cover | 30 |

Mana recovers while playing. Enemy attacks have wind-up and brief player damage
immunity; defeat returns the player to a safe spawn with supplies retained.
Each guardian drops wild essence; defeated groups grant extra essence and an
ancient circuit. These materials connect exploration to core fabrication, advanced
power and spell upgrades. Completed groups remain cleared, including after reload.

## Saves and verification

VXWSAVE7 stores the extended resource inventory, machine modes, filters, upgrades,
health, mana, wand tier, selected spell and defeated-guardian records. V1–6 saves
remain readable. The world library recognizes V7. Living guardians and transient
projectiles/effects are rebuilt or cleared after load; damaged but undefeated
guardians recover when revisited. Cleared rewards cannot be claimed again.

Local `cargo build --offline --target-dir target` passed, including WGSL validation.
No tests or gameplay sessions were run. Gameplay balance, collision feel, visual
presentation and save/reload behavior still need hands-on validation.

Core files: `src/mesh/models.rs`, `src/world/adventure.rs`,
`src/world/factory/recipes.rs`, `src/world/factory.rs`, and their renderer,
collision, UI, input and save integrations. Build log:
`target/living-factory-build.log`.
