# Resonance expeditions

Optional persistent-world roguelite loop, reached through **E → Forge**.
At a workbench, kindle with one crystal and one induction coil. Choose one of
three boons immediately, then earn another choice every three natural guardian
kills. Choices rotate deterministically and filter capped boons; each boon has
five stacks, with at most thirty earned choices in an expedition.

| Boon | Per stack |
|---|---|
| Copper edge | +8% melee damage |
| Prism catalyst | +20% direct spell-hit damage when an elemental reaction has a name |
| Cinder echo | On guardian kill, 4 damage to natural enemies within 3 m and line of sight |
| Moss covenant | Restore 2 health on guardian kill |
| Living dynamo | +1.5 mana per second |
| Reclaimer | +1 salvage per guardian kill |

Kills normally award one salvage. Pressure rises every eight kills and every
ninety seconds within 24 m of a living natural guardian, capped at five levels.
Each level adds 12% to damage through the adventure damage handler. There is no
new global spawn timer: building at home does not accumulate pressure.

Return to any recognized workbench to bank: each salvage yields one steel ingot
and half an induction coil; each eight salvage also yield one arcane core.
Banking releases all boons and pressure. Death loses unbanked salvage and boons
while leaving inventory, terrain, factories and discoveries alone. Existing
normal enemy loot is still awarded separately.

Between expeditions, calibrate the resonator up to five times. Level N costs
2N circuits, N coils and N cores; each level reduces expedition damage taken by
2%, multiplicatively after pressure. This provides a permanent use for automated
production without resetting the factory world. Banking a run with kills advances
the offer rotation; restarting empty runs does not reroll it.

Habitat creatures neither award resonance nor receive cinder echoes. Echoes use
the existing animated copper blast and voxel-burst effects; moss uses olive
motes. Echo victims stay in the enemy list until the next ordinary death pass,
preserving the normal loot and victory-record path without recursive callbacks.

Save version 32 writes a bounded fourteen-word ledger after environmental state.
Older saves default to an inactive, uncalibrated resonator. Pending choices,
boons, combat time and salvage survive reloads; the reader rejects invalid counts,
nonfinite timers and inconsistent reward ledgers. No terrain regeneration occurs.

Typography uses bundled PixelCode TrueType for pixel headings and numeric text,
through egui/epaint's ab_glyph font rasterizer and atlas. Body text keeps its
readable proportional family, with fallback glyphs on both named pixel families.

Validation: local cargo check only. No automated tests or game launch requested;
combat balance, save round trips and visual presentation still need runtime review.
