# Guardians and reliquaries

Discoveries now live on modeled reliquaries in the world. Each has a stone plinth, luminous seals, and a floating open book or weapon. Clear the local escorts, approach within 3.8 metres, aim at the relic and press **F**. A nearby prompt describes whether it is sealed, ready to awaken, or ready to claim. Walls block interaction.

Ordinary sites grant their inscription when you physically claim it. Foundries, archives and crystal cathedrals instead offer an optional guardian encounter. Press F to awaken the guardian, defeat it, then return to the reliquary and press F to claim its relic. Crafting supplies still arrive when the escorts are cleared; they are separate from the rare discovery.

## Signature encounters

| Location | Guardian | Fight and rare reward |
| --- | --- | --- |
| Foundry | The Cinder Castellan, 360 health | Armoured furnace colossus; spreading volleys alternate with staggered lines of ground eruptions. Guards **Cinderbrand**. |
| Archive | The Unwritten Keeper, 280 health | Robed keeper with orbiting books; radial bolt wheels alternate with marked arcane eruptions. Guards **Rift well**. |
| Crystal cathedral | The Prism Hart, 320 health | Crystal guardian with branching antlers; bolt fans alternate with a ring of eruptions around its committed target. Guards **Winterglass** and **Echo step**. |

At half health each guardian enrages: attacks recover faster, volleys grow denser and area patterns gain additional strikes. Ground attacks mark their positions before damage, allowing movement, jumps or Echo step to escape. Ordinary hits no longer indefinitely cancel boss wind-ups. Frost shatters can interrupt, followed by three seconds of resistance to another interruption. Knockback and Rift well have reduced displacement against bosses.

Bosses use body-sized collision checks and require open supported ground to awaken. If the local ruins have been built over, the interaction explains that space must be cleared. Leaving the encounter or dying allows a surviving guardian to return at full health; boss health is not persisted mid-fight. Awakening, defeat and claimed rewards are persisted. Defeated bosses and claimed relics do not respawn.

## Hunting rewards

- **Cinderbrand:** a broad ember blade with 38 base damage. Finishers add 12 damage and return 8 mana, connecting melee combos to spell casting.
- **Winterglass:** a slender crystal blade with 28 base damage. A hit chills the target; the next hit can shatter it. This provides a melee route into frost combos.
- **Rift well:** 42 base mana, 2.5-second cooldown. Launch a four-second field that pulls nearby visible guardians together and pulses damage. Frozen targets take extra pulse damage. Up to four active or incoming wells.
- **Echo step:** 24 base mana, two-second cooldown. Teleport up to six metres along your aim. The player's full body is checked along the path; walls stop it and grounded players need supported destination terrain. A blocked attempt spends no mana. Successful steps provide a brief 0.25-second grace period.

Rare weapons are actual inventory items with individual names, descriptions, icons and held models. They can be equipped from Gear or selected from the hotbar. A full pack leaves the weapon at the reliquary until a slot is available. Rare spells are recovered from their world objects, require a crafted wand, and join the **T** cycle and Gear spell selection; cast with **R**. Existing mana discounts apply.

## Saves and presentation

Version 14 saves retain the version-13 slot inventory layout and expand validated encounter flags and weapon/spell IDs. Versions 1–13 remain readable. Older binaries cannot read the new version. Existing inscription benefits are retained for previously cleared sites, including while an optional guardian is awakened there. Those sites also gain a reliquary and can offer the new encounter and rare reward. New sites require physical claims for inscription benefits.

World objects are reconstructed from site coordinates and supported terrain; rituals, victories and claims use the existing bounded encounter record table. There are at most 24 active enemies, 16 hostile fields and 96 projectiles. Relic claims are one-time and survive reloads. Boss health and phase appear only nearby; the relic prompt appears only while aiming within interaction range.

## Validation

Local `cargo build --offline --target-dir target` passed, including Rust compilation and the existing build-time WGSL validation. Output: `target/reliquaries-build.log`. No tests or game launch were performed. Boss balance, generated arena accessibility, save migration and final visual feel still require in-game acceptance.
