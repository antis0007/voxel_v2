# Workshop heart expedition

This pass gives the existing crafting, exploration and guardian systems a finishable objective: prepare at the workshop, recover three guardian relics, and bring their resonance back to the heart below the starter workshop. It develops one ruin in depth instead of adding another tier of machines, enemies or spells.

## Playable route

1. Follow the starter workshop's mine, clear its escorts and claim its reliquary. Its three sockets represent the foundry, archive and cathedral relics.
2. Press **F** while aiming at a claimed reliquary to trace the next guardian. The route recommends the foundry, archive, then crystal cathedral; players can recover them in any order. Directions, distance and metre coordinates remain available in **Gear** and the **Field journal** during the session. Consult a claimed reliquary again after reloading to refresh directions.
3. Claim Cinderbrand, Rift well, and Winterglass/Echo step from their defeated guardians. Existing physical claims count; older automatic inscription unlocks do not replace a guardian relic.
4. Return to the starter reliquary and press **F** to restore the workshop heart. Completion persists. The heart becomes a health/mana sanctuary when enemies are not nearby; building and exploration continue.

The survey is bounded and runs on interaction, never per frame. It searches a local district for the next required site and says when none is found. It does not guarantee a short route or a surface entrance to the cathedral; underground directions include depth and recommend a safe stair.

## Foundry encounter

New generator-12 worlds have an authored foundry court: an entrance arch, an inlaid process line, flank cover, a clear central combat lane, existing furnace stacks and a screened maintenance alcove. The metre layout and fixture orientation follow the site's rotation at every voxel scale. Other ruin layouts are unchanged in this pass.

Three modeled valves carry one, two or three illuminated pips. The entrance plate repeats their operating order, **III → I → II**, and can be read with F. The sequence persists between visits. A wrong turn resets progress and warns for 1.1 seconds before a local steam hazard deals 16 damage; move out of its 2.4 m ring. A short interaction cooldown prevents accidental repeated turns.

Exploring behind the furnace stacks reveals a maintenance bypass and a one-use tonic restoring up to 35 health and 45 mana. The bypass permanently disables this site's failure trap without solving its sequence. Balance the valves and defeat the escorts to awaken the Castellan. Previously awakened, defeated or claimed guardians remain accessible without repeating the new puzzle.

Fixtures are physical world models with aimed, range-limited, line-of-sight interactions. They require supported ground at their authored locations; mining away that ground requires rebuilding it. Older worlds gain the fixtures and puzzle while retaining their original terrain, so they do not receive the new screened alcove or cover layout.

## Guardian and relic feedback

- Castellan: rising hammer arms, weight shift, slam recovery, walking legs; ratcheting pressure windup and low resonant metal impact.
- Keeper: rising body, expanding and contracting book orbit; fluttering dissonant windup and pulsed release.
- Hart: lowered antlers, release recoil and walking legs; harmonic crystal warning and brittle release.
- Facing commits during windup rather than tracking the player continuously. Existing bounded projectile and hazard budgets remain in force.
- Cinderbrand's third contact releases a flame cleave: 20 damage to other visible enemies within 3.2 m of the impact and inside the forward cone. Its existing primary-target bonus and mana return remain. Fire-colored contact effects and a separate heavy ignition cue distinguish it from ordinary swords.
- Winterglass still alternates chill and shatter. Shattering now damages nearby visible enemies for 12 and chills them for 2.5 seconds, spreading the freeze/combo opportunity. Neither cleave nor shards damage through walls or re-hit the primary target.
- Rift well gains a distinct low pulse, crossing luminous rings and inward-moving particles. Echo step gains a separate rushing cue and a brief trail along its collision-checked path. Their costs, cooldowns and movement safeguards remain.

All added audio is locally synthesized original material. No downloaded assets or new audio dependencies.

## Compatibility and verification

Save version 15 retains the version-13 inventory byte layout and stores valve progress, bypass use and heart completion in previously unused encounter flag bits. Versions 1–14 remain readable; old binaries cannot load version 15. Terrain generator 12 applies the new court only to newly created worlds; existing generator versions retain their geometry.

Local build: `cargo build --offline --target-dir target`, log `target/ruin-expedition-build.log`. No tests, CI or game launch were run. Compilation does not establish arena accessibility, balance, audio quality, animation quality or save round-trip acceptance; those remain in-game verification work.
