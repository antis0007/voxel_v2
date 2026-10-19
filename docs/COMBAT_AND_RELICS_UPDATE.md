# Combat and relics update

This increment adds combat audio, committed melee strikes, two new spells, chain lightning, and site-specific exploration rewards. Existing uncleared encounters receive the new rewards; already-cleared encounters remain cleared.

## Playing

- **LMB** strikes a nearby aimed enemy. A short wind-up precedes damage, so range and aim still matter when the strike lands. The third strike is a heavier finisher. Frost followed by melee or Spark shatters the target.
- Hold **R** to cast; **T** cycles available spells. Equipment also lets you select any unlocked spell directly and explains its mana cost and effect.
- Craft the lumen wand for Spark and Mend, attune the frost wand for Frost lance, then the resonant wand for Burst, Mirror ward and Spore bloom. Recipes and workbench requirements remain visible in Equipment.

| Spell | Mana | Effect |
| --- | ---: | --- |
| Spark | 12 | Direct hit jumps to two additional guardians, each within 4.5 m of the previous target. Terrain blocks jumps. Frozen targets shatter; machinery can still receive a small charge. |
| Mend | 30 | Immediate healing, strengthened by mastery. Does not spend mana at full health. |
| Frost lance | 18 | Slows a guardian for four seconds, preparing a shatter combo. |
| Resonant burst | 30 | Heavy impact and a three-metre blast with knockback; terrain blocks splash damage. |
| Mirror ward | 26 | Six seconds and three charges. Reflects hostile projectiles into friendly Sparks; melee hits consume a charge and deal 35% damage. |
| Spore bloom | 38 | Arcing pod creates a five-second, 3.2 m spore field. Pulses hurt guardians and slowly heal you within sight of the field. At most six fields or incoming pods. |

Hammer wind-up, follow-through and the overhead finisher have distinct motion. Wand casts recoil and carry orbiting spell-coloured gems. Guardians wind up attacks and flinch; defeated bodies scatter short-lived fragments. Chain arcs, drifting spores and peripheral ward shards communicate the new effects without covering the aiming point.

## Sound

Fourteen original synthesized effects cover weapon swings, body impacts, metal blocks, each spell family, injury, attack warnings, defeat, treasure and shattering. Positional events use distance attenuation and stereo panning; small pitch variations reduce repetition. Sound effects volume is available in menu and pause settings and persists across sessions. Muting, pausing and loss of focus silence playback.

Windows audio runs on a bounded worker through WinMM, with no additional package or downloaded assets. The device is reopened after an output failure; gameplay does not depend on audio availability. Other platforms currently report audio unavailable. This is a synthesized effects pass, not recorded Foley, music or environmental ambience.

## Rewards and presentation

Every guardian still grants wild essence. Additional drops now differ: grubs and brutes yield fungal materials, sentinels supply circuitry and coil parts, wisps drop crystal, spitters yield luminous fibre, and wardens drop steel and core fragments. Fractions accumulate in the existing fixed-point inventory.

Clearing a site grants a one-time bundle with essence and circuits plus themed supplies. Workshops give bronze and coils, foundries give steel, archives give cores, and the four fungal sanctuary variants give different biological, mineral or industrial rewards. A brief square-framed card shows the recovered resources. Existing mastery advancement remains tied to cleared sites.

The HUD displays ward charges and remaining time, new spell icons and their correct cooldowns. Salvage cases gain timber slats, iron straps, studs and a brass keeper; moving machinery gains restrained patina and small surface pits. These extend the existing square frames and pixel illustration palette.

## Compatibility and verification

Spell IDs 0–3 remain unchanged; ward and bloom append IDs 4–5. World saving uses the current version-9 format alongside the separately developed simulation-state changes, retaining support for version 8 and older saves. Audio preferences use VXPRF002 and accept VXPRF001 with default sound volume. Existing cleared-site flags prevent repeated rewards after loading.

An intermediate local `cargo build --offline --target-dir target` passed. A later build could not replace the main executable because another instance was running (Windows access denied). The final source was therefore built with a separate binary name using a generated manifest under `target/combat-candidate-manifest`, preserving the normal project manifest and running instance.

`cargo build --offline --manifest-path target/combat-candidate-manifest/Cargo.toml --target-dir target` passed, including Rust compilation, linking and the existing composed-WGSL validation (65 warnings, no errors). The executable is `target/debug/combat_relics_candidate.exe`; the successful build log is `target/combat-candidate-build.log`. No tests or game launch were performed by this task. Audio balance, device playback, animation feel and final visual quality still need in-game acceptance.

See [Arcane industry update](ARCANE_INDUSTRY_UPDATE.md) for the existing automation chains and mining equipment.
