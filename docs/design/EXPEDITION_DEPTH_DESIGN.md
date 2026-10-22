# Expedition depth: hatch, threat, water, and spell reactions

Date: 2026-10-19

Superseded where it overlaps the Undercrown. The buried region, its expedition, and its six exploration verbs are specified in `2026-10-19-undercrown-region-design.md` and `docs/UNDERCROWN_SCHEMA.md`. That region does not use the generator-17 dungeon greeting, the 1.4 m choke, or spell ids 28–33.

Still in force outside that footprint, and inside it where the Undercrown design points here:

- Cave fill stays a silhouette floor. Passage fog stays analytic and short. A room with a authored sightline longer than 16 m uses the chasm haze in the Undercrown design instead.
- Resident water keeps the Gerstner surface and the celadon / sea / ink bands.
- The wand deck in `wands.rs` stays the combat caster. Undercrown verbs are a separate exploration selector.
- Backup `fluid.rs` stays unused. A borough reservoir is a bounded set of voxel edits.

## Intent

A descent should be a new adventure every time. Horace described the first seconds under the hatch as Zork's opening and Noita's runs at once: you do not know whether you will find a fight, a lethal room, or treasure. Combat has to be worth winning, and the creature in a fight has to be large enough and clear enough that walking past it is not an option.

Lighting, fog, water, and spells exist to serve that moment. They are not a separate art pass.

## What was decided, and by whom

Horace:

- The greeting changes every descent. New dangers, unknown opponents, unknown treasure.
- Prior art that matters: Noita, the Zork hatch, D&D spell schools, RimWorld and its mods, Minecraft mods. "Zord" in the first request is the same reference as Zork in the second.
- Enemies should look larger and scarier, and be a real combat threat.
- Winning a fight should feel more rewarding.
- Underground lighting, underground fog, performance, and Wind Waker-inspired water are in scope.
- Useful pieces may be taken from `C:\Users\antis0007\Downloads\voxel_reupload\backups\src_latest2`.
- The visual companion was accepted, then Horace asked for the design to be made as engineering work. The four greeting pictures were not clicked. The shuffle picture matches the words above, so the product is the shuffle.

Builder calls, not agreements:

- One design, five implementation slices, in the order at the end. Shipping the look without the encounter, or the spell table without a creature that can kill you, fails the request.
- Greetings are a pure function of seed, cell, and room index. There is no menu toggle and no greeting stored in the save.
- Room kind biases the greeting. A repair pass guarantees one unknown, one hazard, and one threat in every 9-room complex. Flat 40/30/30 weights are withdrawn; they ignored the themes the generator already builds.
- A threat creature fills one narrowed opening of its own room. It does not seal the 3×3. Corridors stay 3 m everywhere else.
- Enemy presence stays primitive cuboids and orbs in `Enemy::model`. Scale, silhouette, eyes, and windup color change. Skinned meshes and new texture sets are out.
- Water stays the existing Gerstner surface in `world_shader.wgsl`. The Wind Waker target is a readable three-band look on that surface, including lit cave pools.
- Cave fog is a new analytic term. It does not extend `local_mist`.
- Spell work extends `ShotMods` and the impact path in `adventure.rs`. It does not add a second caster.
- Famous works contribute mechanics and public-domain-style names. No copied spell text, bestiary entries, or level layouts.

## Goals

- Dropping through a hatch into a generated complex can yield a quiet relic, a room whose materials will kill you, or a creature that occupies the passage.
- In a threat room, the creature's eyes and windup read inside torch range before it hits.
- Killing it leaves something you can see and use in that room: a spell card, a material change, or an opened route.
- Underground space is dark except for authored lights (held torch, lumen lantern, spell lights, emissive voxels). Fog shortens view without hiding a creature that is already inside attack range.
- Open water and cave water read as bright shallows, a colored mid band, and a dark trough, with foam on sharp crests.
- Frame cost of the new fog and water terms stays in the same class as today's analytic mist: no sample loops, no shadow marches.

## Non-goals

- A new biome generator or quest line. The wand library's XMB arrangement and growth spells are specified in `2026-10-19-growth-and-xmb-design.md`. Health, the hotbar, and the factory screens stay as they are.
- GPU cellular fluid from the backup. Current water is a displaced voxel surface with a simulated bed depth, and that is the water this design changes.
- Replacing daytime atmosphere, clouds, or the horizon handoff.
- A difficulty menu. Menace already scales health.

## Failure, even if the code runs

- Every room is a boss, or every room is loot. The hatch told you what to expect.
- Cave ambient is raised until the torch no longer matters.
- Fog is a gray distance fade that also washes out noon hills, or a march that repeats the old 102 ms terrain-pass cost noted in `local_mist_shared.wgsl`.
- Water becomes a flat toon shader that ignores depth, shore foam, and the resident-to-horizon handoff.
- New spells are a longer `match spell` with no combination that an existing modifier could not already express.
- Backup shaders are copied over `world_shader.wgsl`.

## Current code this design builds on

- Complexes: `src/world/dungeon.rs` builds a cached 9-room graph per 144 m cell (`rooms.len() == 9` is a test). Rooms already have kinds: hub, hall, crypt, cistern, grotto, treasure, shrine, barracks, forge, library, ore, pedestal, atrium. Four themes pick which kinds appear: catacombs, undercroft, mycelial warren, cinder forge-halls. Corridors are `CORR_W` 3 m by `CORR_H` 3.6 m. A standing torch is written every 9 m along a corridor. Geometry is `Dungeon::structure` inside `sample_block`. Player digging is a later edit replayed onto the chunk.
- Encounters: `sites::encounters` asks `dungeon::anchors` for at most two side rooms of a nearby complex and spawns one lead there, kind `hash % 6`, on a ring around that point, never within 8 m of the player. `sanctum_fights` is a separate three-tier Holy Mountain and stays as it is. `adventure.rs` spawns `Enemy` values with `kind` and `menace`. Kinds in play include Ruin Skitterer (0), Spore Sentinel (1), Arcane Wisp (2), Burrow Beetle (3), Aether Moth (4), Sanctum Warden (5), bosses (6–8), Gravelback Colossus (9).
- Clears already persist. `Adventure::records` is a `IVec3 -> u32` mask. Bits 0–7 mark dead slots. Bit 8 (`256`) marks the site cleared and feeds mastery. `encode` / `decode` round-trip that mask. Live enemies are not saved; they respawn while their slot bit is clear.
- The colossus is the size reference: about two metres, four legs, collision reserved so it cannot use a doorway (`Enemy::model`, kind 9). The skitterer is a 0.15 m beam and a 0.76 m head. Most fights currently look like the skitterer.
- Combat already has windup (`tell`), stagger, knockback, and a hit flash. Windup tints the model orange. That channel is the "scary" read. It is easy to miss because the body is small and the cave is either black or flat-filled.
- Wands: `src/world/adventure/wands.rs` evaluates an ordered deck of at most 8 slots. `ShotMods` carries damage, speed, bounces, homing, orbit, bore, trail, volatile, and siphon. Multicast (16, 17) and trigger (18) already exist. `spell_hint` documents per-spell world effects: freeze water, boil water, electrify water, transmute stone, gravity wells, plasma crosses.
- Those world effects are special cases in the projectile update (`adventure.rs`, spell ids 2, 12, 13, 14, 15 and neighbors). There is no reaction table, so two modifiers do not invent a third behavior.
- Light: `world_shader.wgsl` applies a small `cave_fill` (`rgb 0.010, 0.012, 0.018`) scaled by `cave_amount`, cavity, and hemisphere, and skips water and lava. Up to 64 torches plus a held torch use `eval_torch_light`. Spell lights are collected separately (`adventure.rs` near the dynamic point-light comment).
- Surface mist: `local_mist` returns zero when sky access is ~0, when `cam.camera_pos.w > 0.5` (underwater), or inside the reference scene. It is an analytic exponential, capped at optical depth 0.22, with no shadow march. The comment records that a 12-step shadow march cost 102 ms.
- Water: kind 8 in `world_shader.wgsl` already has domain-warped Gerstner slopes, Jacobian whitecaps, Beer-Lambert absorption, dual-layer caustics, and shore swash. Far water fades those details across the horizon handoff. The look is photographic turquoise, not the high-contrast celadon / ink bands associated with Wind Waker.
- Performance counters: `src_latest2/perf.rs` is a CPU frame sampler (UI, gameplay, streaming, torch, render). The live tree's equivalent concern is keeping new work off the overdrawn terrain fragment path. This design does not revive that file as a subsystem.

## Salvage from `src_latest2`

| Path | Verdict |
| --- | --- |
| `fluid.rs`, `fluid_compute.wgsl` | Leave it. A 34³ GPU cellular sim with fill/drain thresholds. The live water path is the shader on voxel water. Porting the sim would add a residency and boundary-sync project and would not produce the Wind Waker read. |
| `shaders/world_shader.wgsl`, `lighting_shared.wgsl`, atmosphere and cloud shaders | Leave them. They are an earlier lighting stack. The live shader already has the cave fill, Gerstner water, and the mist cost fix. |
| `perf.rs` | Leave the types. If a slice needs a number, time the existing gameplay and render sections already measured around `main.rs`. Do not introduce a second profiler. |
| `weather.rs`, `render/clouds.rs`, `shadow.rs` | No expedition behavior. Do not merge. |

Nothing in that backup is a spell, enemy, or dungeon system.

## 1. The hatch

A greeting is decided in `Dungeon::structure`, not when the player enters and not in a second stored layout. The same function that writes the floor writes the choke, the hazard blocks, and the pedestal. Explored chunks keep their mesh until they regenerate. New geometry ships as generator version 17. Version 16 worlds keep today's corridors. Sanctum vaults and `sanctum_fights` do not take greetings.

`greeting(room)`:

| Room kind | First choice | Why |
| --- | --- | --- |
| Treasure, Pedestal, Shrine, Library, Atrium | Unknown | The room already has a pedestal, chest, or shrine furnish. |
| Cistern, Grotto | Hazard | Water and fungal soil are already the theme. |
| Barracks, Forge, Hall, Ore | Threat | These rooms are wide enough for a body. |
| Hub, Crypt | Whichever the complex still lacks | The hub is the shaft room and stays walkable. |

After that pass, if the 9 rooms are missing a greeting, flip the highest-index room of the majority greeting. Hub is never flipped into a threat. The result is a pure function of the plan, so a reload cannot change what the hatch was.

What each greeting writes:

- `Unknown`. No lead creature. The existing pedestal or shrine furnish is the relic. Its effect stays off the HUD until the player takes it, and the taken state is the existing reliquary claim. No new parser, door, or floodgate.
- `Hazard`. No lead creature. One skitterer at most, and only if the room is grotto. The furnish adds one pairing, using blocks that already exist:
  - Cistern: a water floor and one crystal node at the rim. Spark against that water is the kill, via the reaction table. Until slice 5, the water is only water; the crystal is the tell.
  - Grotto: a glowcap cluster and a fungal-soil patch. Walking it is safe. A life or fire shot is what wakes it, using the same cell writes as the spore spell.
  - Any other hazard room, after the repair flip: the cistern pairing, scaled down to a 2 m pool.
  - A falling ceiling is not in this design. Nothing in the simulation collapses a roof on enter.
- `Threat`. The corridor segment that joins this room to its lowest-index neighbor is air 1.4 m wide for the last 2 m before the room. The rest of that corridor stays 3 m. Torches are omitted on that 2 m segment and for 9 m back along it, so the 9 m torch rule does not light the creature's face. Other corridors keep their torches.

The player is not told the greeting by the HUD. The tell is the relic, the pool, or the eyes.

### Spawn, so the body exists in the choke

`sites::encounters` stops emitting the hash-picked lead for a version-17 complex anchor. It emits one record per room whose centre is within 24 m:

- Site id: `IVec3::new(cell_x + 4096, cell_z + 4096, 16 + room_index)`. `z` 16–24 does not overlap site `0`, cavern `1`, cairn `2`/`3`, sanctum `4`, or the second anchor `5`. The `+ 4096` matches `anchors`, so a room id cannot collide with a surface site at the same integer coordinate.
- Unknown: no enemy.
- Hazard: slot 0 is a skitterer in grotto rooms only.
- Threat: slot 0 is the occupant, placed at the choke centre, not on the ring. Slots 1–3 are skitterers on the existing ring, inside the room, capped at 3.
- The 8 m rejection stays for ring spawns, so an escort never appears inside the player. The choke spawn skips the ring and still skips a position whose capsule overlaps the player. If the player is standing in the choke, the occupant waits until the next spawn pass.
- `free()` half-extents grow with the mesh in the same change. Today's tall kinds use height 1.6 m and half-width 0.28 m, which would reject a 2.2 m sentinel or embed it in the ceiling. Hulk half-width is 0.62 m (1.24 m body) so the centre of a 1.4 m air gap passes `free()`.

`MAX_ENEMIES` is unchanged. The 24 m gate is what keeps nine rooms from spawning nine hulks. A complex you have not entered yet has props and no bodies.

## 2. Creatures that own the tunnel

Keep the kind ids. Change who is allowed to be the threat occupant, and how large they read.

Threat occupant, in order of preference for that room's theme: Sanctum Warden, Spore Sentinel, Gravelback Colossus, or a new kind 10, the Vault Hulk. Skitterers, beetles, wisps, and moths stay add-ons. A threat room whose occupant is only a skitterer is a failed roll; replace it.

Scale and read, authored in metres in `Enemy::model` and matched in the existing collision reservation:

| Kind | Role | Body | Read |
| --- | --- | --- | --- |
| 0 Skitterer | Add-on, swarm of 3 in threat rooms | Keep ~0.8 m | Low, fast, not the silhouette |
| 1 Sentinel | Threat | 2.2 m stalk, cap wider than the torso | Spore shed during windup |
| 3 Beetle | Hazard add-on | 1.1 m shell, stays low | Armour plate lights on hit, not on idle |
| 5 Warden | Threat | 2.4 m, shield silhouette that fills a 2 m door | Orange windup on the weapon, not the whole body |
| 9 Colossus | Threat in wide halls only | Keep ~2 m | Eyes already emissive; raise eye intensity when `tell > 0` |
| 10 Vault Hulk | Threat in corridors | 1.8 m hunched, 1.3 m wide | Fills the room's 1.4 m choke; eyes at player eye height |

Kind 10 uses the colossus pattern: cuboids, a windup pose that pulls a limb back, emissive eyes, and a `free()` box of height 1.9 m and half-width 0.62 m. Health starts at 160, menace multiplier unchanged (`1 + 0.4 * menace`). It does not fly. It fills the 1.4 m gap and still fits a 3 m corridor, so the loop around the room stays walkable. `tall()` includes kinds 1, 5, and 10 so the sentinel and warden use their new heights in `free()`.

Scary, in this game, means: you see the eyes before the body, the windup limb moves for at least 0.6 s, and the capsule fills the narrowed opening of this room. It does not mean a damage sponge, and it does not mean the only route through the 3×3. Warden and hulk gain one new attack each: a short lunge that stops at the player or a wall, using the existing `lunge` field. Damage stays inside the current melee band for that health pool. The threat is position, not a one-shot.

Flying wisps stay harass. They must not be the creature blocking the door.

## 3. A fight that pays

On threat-occupant death, roll one reward from the room seed. Expedition points already granted on kill stay as they are. The reward is the visible extra.

1. A spell card the wand tier can socket, biased toward a modifier the player does not have (19–27, and 28–33 once those cards exist) rather than another spark bolt.
2. A material stake in the room: ice bridge, crystal vein, or glowcap cluster, using the same cell writes as cryo, transmutation, and spore spells.
3. Breach. World edits, the same path as digging, widen the 1.4 m gap back out past 3 m and leave rubble blocks. The generator keeps describing the choke. Edits replay after `sample_block`, which is why a reload does not grow the wall back. A breach stored only in the mesh, or only by changing the pure greeting, comes back on the next chunk build.

The paid bit is recorded even if the inventory is full. A full inventory spills the card onto the existing dropped-item path at the choke. It does not retry the roll.

Hazard rooms set the paid bit when the player leaves the room alive after the reagent has fired (a spark in the cistern, a life or fire shot in the grotto). Leaving without firing pays nothing. Unknown rooms pay through the existing reliquary claim, once.

Record bits, on the new room site id only:

| Bits | Meaning |
| --- | --- |
| 0–7 | Slot cleared, same as today. Slot 0 is the occupant. |
| 8 | Site cleared, same as today. Mastery keeps counting this bit. |
| 16–17 | Reward kind: 0 none, 1 card, 2 material, 3 breach. |
| 18 | Paid. Death, claim, and hazard exit all check this before rolling. |

Old saves leave bits 16–18 clear. A cleared version-16 site does not pay a new reward. Live enemies stay unsaved.

RimWorld's useful lesson is the aftermath, not mood bars. The room looks different because you won. No relationship system, no pawn thoughts.

## 4. Underground light

`cave_fill` stays the silhouette floor so a creature-shaped hole is visible against black. Do not raise it.

Readable light is the lights that already exist:

- Held torch and lumen lantern go through `eval_torch_light` with radius long enough to cover the windup distance of the threat occupant (about 8 m). If the lantern is shorter than that today, extend the lantern, not the ambient.
- Spell point lights stay on the adventure light list.
- Emissive eyes and the orange windup tint are the creature's own light, so a hulk in the dark is a pair of eyes and then a moving limb.

Surfaces facing away from every local light stay near `cave_fill`. That is the scare. A uniform underground ambient is the failure mode called out in the shader comment ("overlit interior").

## 5. Underground fog

Add `cave_fog` beside `local_mist`, called from the terrain fragment where `cave_amount` is high and `local_mist` has already returned clear.

Contract:

- Zero when sky access is above 0.2, so mouths of caves blend into the existing mist instead of double-hazing.
- Zero underwater (`camera_pos.w > 0.5`); the underwater shared path keeps that look.
- Analytic extinction only. Distance cap 16 m. Optical depth cap 0.40. No loop, no shadow map, no extra texture.
- Color is a cold cave rgb near `(0.03, 0.04, 0.06)`, pulled toward the strongest local torch color in proportion to that light's intensity at the fragment. The torch carves a warm hole. It does not require a new light evaluation: reuse the torch diffuse already computed in the fragment, scaled down so fog does not become a second diffuse.
- Inside 4 m, transmission stays above 0.82, so a warden's windup at melee range is not smoked out. Between 8 m and 16 m the passage ahead becomes a silhouette. That matches "you do not know what is next" without hiding the fight you are in.

`MIST_STRENGTH`-style const of `1.0` lets the term be zeroed for an A/B. Default on.

## 6. Water, Wind Waker on the current surface

Keep vertex displacement, Gerstner slopes, Jacobian, absorption, refraction, shore foam, and the far-water handoff.

Art direction on the resident surface only (`local_water > 0`):

- Three bands a player can name at a glance, as albedo before the sun. Shallow: celadon `(0.42, 0.62, 0.48)` at depth under ~0.4 m. Mid: sea `(0.05, 0.38, 0.48)`. Trough and deep: ink `(0.02, 0.07, 0.14)`. The earlier `(0.55, 0.82, 0.62)` shallow is withdrawn; that value clips to white once the existing sun term hits it. `local_water` already fades across the horizon handoff. Lerp these bands toward today's sand / turquoise / navy by `1 - local_water`, so the resident ocean does not end in a hard cartoon edge.
- Crests: lower the whitecap threshold so Jacobian peaks read as thick white shapes, still using `water_foam`, capped so the whole ocean is not milk.
- Specular: a broader, softer sun glint on the crest normal, and a tight glint. Wind Waker water is readable in motion from the glint moving across flat color, not from micro-ripples. Capillary weight (`w3` and the cusp term) drops by about half on the resident surface.
- Cave pools: when `cave_amount` is high, sun caustics fade with sky access. Add caustic brightness from torch diffuse so a lantern over an underground pool still throws moving light on the bed. Same noise functions, no new samples beyond the torch term already in the fragment.
- Underwater fog and the underwater shared shader stay as they are in this slice. Surface read is the Wind Waker target.

This is a palette, threshold, and torch-caustic change inside the existing `kind == 8` block. It is not a new material.

## 7. Spell mixing

The deck evaluator stays the authority for order, mana, delay, multicast, and triggers. The new piece is a reaction resolved at impact and when two live player shots occupy the same cell.

### Tags

Every spell id 0–27 gets one or two tags. Tags are data, not a class hierarchy:

- Bolt, Field, Utility (already implied by `spell_kind_tag`)
- Element: Spark, Frost, Fire, Void, Life, Force, Metal
- School, used only as a mixing key: Evocation, Transmutation, Conjuration, Abjuration, Necromancy, Divination

D&D's contribution is the school key and the rule that some pairs are the spell. It is not slots, prepared casters, or spell levels.

### What is more than the current Noita deck

Today, modifiers multiply a shot and a few spell ids poke the world. Add three combination sites:

1. Cast-block catalyst. A utility spell marked catalyst does not fire. It rewrites `ShotMods` for the rest of the block. First catalysts: Conduct (Spark rides Frost or Fire and jumps once through water), Anneal (Fire then Frost, or Frost then Fire, writes metal or tempered stone instead of boiling or freezing), Echo (the trigger payload casts twice at half damage). Order still follows the deck. Anneal is the pair the current hints cannot express, because boil and freeze are separate ids.
2. World reagent. On impact, `(element, cell material)` chooses a cell write. The table starts from behavior the projectile loop already has, moved into one function:

   | Element | Water | Ice | Stone or dirt | Creature |
   | --- | --- | --- | --- | --- |
   | Frost | Ice | No change | No change | Slow, existing `slow` |
   | Fire | Replace water with the existing Steam block, one cell | Water | No change | A new `Enemy::burn` timer, 2 s, damage tick in the enemy update. `flash` stays the white hit blink and is not a damage-over-time channel |
   | Spark | Electrified water volume, damages creatures standing in it | No change | No change | Chain, existing spell 0 / 14 |
   | Life | Bloom caps on the shore cell | No change | Fungal soil, existing spore | Heals the caster a siphon tick. On a Sanctum Warden, heals the warden instead and tints the hit blue |
   | Metal / transmute | No change | No change | Crystal or ore, existing spell 15 | Armour: next hit on that creature is reduced |

   Conduct plus water is the Minecraft-mod lesson (a liquid becomes the circuit). Anneal plus stone is the Noita lesson (two spells make a material). Creature exceptions are only the warden Life heal and the hulk's immunity to Force knockback.
3. Shot meeting shot. If two non-hostile projectiles overlap and their elements are a known pair, both die and the pair's field is born at the midpoint: Fire+Frost = Anneal burst (stone temper, small radius), Life+Void = a short-lived well that pulls then pops (uses the existing well cap of 4). Unlisted pairs pass through each other, which is today's behavior. Spark does not meet a water projectile. Spark meeting a water cell, or meeting the pool from Bottle of the Great Sea, is the Conduct result in the world-reagent table.

RimWorld-mod lesson that fits a wand: the creature can be immune to one element. Life on a Sanctum Warden heals the warden. That is the same rule as the table, not a second rule. Hulks ignore Force knockback. The player learns by the fight failing, then sockets a different card. No trait UI in this slice; a resisted or inverted hit tints blue instead of the white damage flash.

### Names from other works, as mechanics

These are new rows or aliases on the table above, with original effects. They are references a player can recognize, not excerpts.

| Name | Comes from | Mechanic |
| --- | --- | --- |
| Hatch Glamour | Zork | Divination utility, spell id 31. Reveals the greeting of the connected room as a 2 s glint on the relic, the pool, or the occupant's eyes. Does not list loot in text. |
| Bottle of the Great Sea | Wind Waker | Conjuration, spell id 32. A thrown pool, 3 m, writes Water cells. Conduct and Frost treat those cells as water. |
| Wild Surge | D&D | Spell id 33, a catalyst. It does not roll on every mixed cast. When it is in the block, the reaction index is `hash(world seed, room site id, cast_count) % 8` into the eight reactions already in this section. `cast_count` increments on each cast and is written by `encode` after the existing loadout words. Old saves decode it as 0. |
| Fungal Shift | Noita | Life plus stone, already in the table, named as a card. |
| Raid Trophy | RimWorld | The threat reward bias: a modifier card, described in section 3. |
| Redstone Tide | Minecraft mods | Conduct plus water, named as the catalyst result. |
| Unnamed Relic | Zork | Unknown-room reward. The item description is a physical clue ("warm on the water side") resolved by the reaction table when used, not by a paragraph of instructions. |

Out of the first slice, noted so they are not invented ad hoc later: a full enchantment school that charms enemies, a divination that prints map text, and any reaction that deletes the wand or the player character outright.

### Spell ids

`MAX_SPELL_ID` is 27 and `wands::EMPTY` is 8. The loadout word stores the spell in the low 8 bits, so a new id has to stay under 256 and must not be 8. New ids:

| Id | Card | Fires a projectile? |
| --- | --- | --- |
| 28 | Conduct | No. Catalyst. |
| 29 | Anneal | No. Catalyst. |
| 30 | Echo | No. Catalyst. |
| 31 | Hatch Glamour | No. A glint. |
| 32 | Bottle of the Great Sea | Yes. Writes water at impact. |
| 33 | Wild Surge | No. Catalyst. |

`spell_available`, `spell_hint`, and the decode check that rejects `spell > MAX_SPELL_ID` all move to 33 together. A save from before that still decodes, because its ids are below 28.

### Limits

- Reactions do not allocate. They set fields on the existing shot, well, cross, and voxel-edit paths. `burn` is a field on `Enemy`, not a new entity.
- A cast block still respects mana, delay, and capacity. A catalyst spends a slot and its mana cost.
- Hostile shots do not participate in shot-meeting-shot. They can still be reflected by the existing crystal-rune spell.

## 8. Performance

Budget for the two shader edits:

- `cave_fog`: a handful of arithmetic ops and one `exp`, gated by `cave_amount`, in the terrain fragment. Forbidden: a ray loop, a shadow-map gather, an extra atmosphere evaluation.
- Water: no additional `surface_noise` calls. Torch caustics multiply the existing caustic rgb by a scalar from `torch_diffuse`. Palette changes are mixes already in that block.
- Kind 10 and the scaled models add a fixed handful of cuboids per enemy, the same path as the colossus. Cap add-on skitterers at 3 in a threat room.
- Reaction resolution runs once per impact, inside the projectile loop that already writes cells.

The tree has no GPU timestamp harness, so a 5% frame-time gate is not a CI check. The CI checks are the dungeon tests below. Fog cost is a manual A/B of the const in a torch-lit complex: if the render section moves by enough to notice, the term gets simpler. It does not grow a sample loop to "fix" the look.

## Tests the slices have to satisfy

These extend `dungeon.rs` tests and the adventure save tests. They are the acceptance bar for slices 1, 2, and 5.

- A version-17 complex still has 9 rooms, and every room's floor still meets a corridor.
- Across seeds, every complex has at least one of each greeting, and the hub is never a threat.
- The threat gap is 1.4 m of air for 2 m, and the same corridor is 3 m wide before that segment. A version-16 sample of the same seed does not grow that gap.
- No torch voxel sits in the dark segment.
- `free()` at the choke centre succeeds for the hulk half-extents and fails for a half-width of 0.9 m.
- Killing the occupant sets bit 18. A second death pass does not grant a second card. Encode then decode keeps bits 16–18.
- A decoded save that never wrote `cast_count` reads it as 0. A spell id 33 survives the loadout word.
- Wild Surge indexed twice with the same seed, room id, and cast count returns the same reaction.

## Implementation order

1. Generator 17 greeting carve, encounter ids, choke spawn, kind 10 model, `free()` heights, scale pass on warden and sentinel, lunge.
2. Record bits 16–18 and the threat-room reward, including breach edits. Hazard rooms start paying only in slice 5, because that is when a shot can fire their reagent. Until then a grotto or cistern is a readable room with no payout.
3. Lantern radius, then `cave_fog`.
4. Resident water palette, foam threshold, capillary cut, torch caustics.
5. Reaction function extracted from the spell-id special cases, plus Conduct, Anneal, Echo, shot-meeting-shot, warden Life inversion, hulk Force immunity, and three named cards that are part of this slice because the reagent rules need them: Hatch Glamour, Bottle of the Great Sea, and Wild Surge. Bottle is the only new source of a water volume; Water is not a projectile element.

Slices 3 and 4 can land independently of 5. Slice 5 without slice 1 ships a crafting system in a world that still does not scare you. Do slice 1 first.

## Left to the builder

- Exact cuboid coordinates for the hulk and the enlarged warden, inside the metre targets and the `free()` half-extents above.
- The numeric lantern radius, as long as a threat occupant's eyes are inside it at 8 m in a dark room.
- Hint-string wording for cards 28–33, as long as Conduct, Anneal, and Echo keep the behaviors in section 7.
- Which existing sound cues to reuse.
- The eight Wild Surge table rows, drawn only from reactions this document already defines.
