# Encounter recipes and progression — 2026-10-21

This pass adds authored combinations to existing creature identities and save records. It extends the earlier geometric feedback/navigation pass. No new resource, machine, item kind, or serialized field is introduced.

## Reusable enemy encounters

`src/world/adventure/encounters.rs` owns a recipe registry and transient encounter state. Selection is deterministic from the existing site, creature kind and slot. The starting encounter is excluded. Recipes supplement the existing species, affixes, elemental reactions, equipment, navigation and group alerts.

| Recipe | Behavior | Counterplay |
| --- | --- | --- |
| Scree Lurker | Beetle waits under a rubble disguise; proximity or passing with your back turned triggers emergence | Observe the seam/glint, disturb it early, exploit its exposed window, dodge the committed charge |
| Patient Cache Mimic | Waits as a banded wooden cache before revealing its body and biting | Inspect suspicious caches, attack before approach, retreat during the audible emergence |
| Aether Harrier | Alternates wide slow volleys and paired shots while maintaining ranged spacing | Use cover, move between bolts, punish committed windup |
| Cistern Stalker | Moves toward a bounded interception point and commits a short charge | Change direction after the tell; it does not steer the charge toward your new position |
| Kiln Trapper | Places a warned field ahead of observed movement, then seeks ranged spacing | Reverse direction, break sight, or bait its field into other enemies |

Disguises are model geometry resting on terrain, not voxel replacement or destructive burrowing. Their underlying enemies remain damageable. An audible 0.85-second emergence prevents immediate surprise damage; normal attack windup follows. Disturbance grants a longer exposed interval. Exposure increases direct melee and `Enemy::hurt` damage by 25%; specialized direct-damage spell paths retain their own rules. Existing finite site/slot records prevent repeated kill rewards.

## Boss patterns

`boss_patterns.rs` defines the Castellan and Archive Keeper attack registry, planning, shared warning geometry and release. The Hart and Tollkeeper retain their dedicated controllers.

- Fault line: staggered ground ruptures; sidestep the line.
- Closing script: five traps around the committed player position, with a missing sixth segment for escape.
- Cinder fan: spaced aimed bolts, with additional bolts in the second phase.
- Crossed pages: two lateral launch positions converge on a committed predicted point. Solid terrain prevents a lateral origin from appearing through a wall.

Selection responds to proximity, observed player movement and stationary behavior; immediate repetition is avoided. Aim is committed at windup. The second phase starts with a sound and a short opening, preserves the duration and layout of an attack already in progress, and uses maximum health for its threshold. Recovery grants a 0.7-second exposed window. The compact boss panel names the pending pattern and shows the exposed opening.

Ground-pattern warnings and release use the same marks. Hazard/projectile caps remain 16/96; navigation shares the existing one-search-per-frame budget. The two bosses now use cached routes and can attempt a one-metre hop, with ordinary collision and gravity still resolving movement.

## Experimentation and progression

Hazards now damage enemies as well as the player, at 60% base damage before enemy defenses. Spore fields apply the existing toxic reaction rules, enabling existing cleanse, poison, immunity and affix interactions. Other fields deal direct damage without inventing an elemental type. Pack members avoid pending and active fields, making baiting a tactical opportunity rather than a guaranteed clear.

Recipe variants add themed existing materials. Bosses add larger crafting-resource bundles and a guaranteed spell-discovery attempt; discovering a new spell still depends on the spell log having eligible undiscovered entries. Existing reliquary claims continue to own rare-magic unlocks.

Site completion now uses the same slot-count function as spawning. Previously the completion mask expected three defeats even in single-enemy rooms. Clear awards are centralized and checked against the existing persisted completion bit. Nearby old saves with a finished but unawarded room are reconciled during spawning, once per site. Clear rewards, mastery and spell discovery can therefore advance from those rooms.

## Adding content

Add a recipe to `encounters::RECIPES` and implement its mechanic hooks in tactics/attack release as needed. Add a pattern ID and its marks/release behavior in `boss_patterns`, preserving committed geometry, a reachable escape and recovery. Resource payouts use `rewards::guardian` and recipe loot; whole-site awards use `Adventure::complete_site`. These are Rust extension points, not an external mod loader or data-pack format.

## Validation

The initial `cargo check --offline --target-dir target` succeeded; output is recorded in `target/encounter-progression-check.log`. The subsequent native gameplay pass exercised boss fights, ambushes, obstacle routing, pack hazards and actual save/reload progression, and fixed casting responsiveness and visual obstruction. See [the gameplay review](ENCOUNTER_GAMEPLAY_REVIEW_2026_10_21.md) for exact builds, outcomes, evidence and remaining coverage. No hosted CI was used. Local bounded routes do not guarantee success through every generated dungeon or modified terrain arrangement.
