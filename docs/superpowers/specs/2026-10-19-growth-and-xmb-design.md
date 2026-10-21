# Growth, the lanterncap, and the XMB wand

Date: 2026-10-19

## Intent

Spells, creatures, caves, and the workshop barely change one another. Horace wants three interactions that share one operation: cultivate a creature, conjure a living mushroom, and enlarge a target with a growth projectile. A growth hit on a small rooted mushroom turns it into a surface you can stand on. The same hit on a mobile creature makes that individual larger, so it carries more, needs more clearance, and fills more of a fight.

The wand has to show that operation the way an XMB shows a focused icon: one large spell in the center, its neighbors receding, and particles on the focus that are the same particles the spell sheds in the world. A player arranging "bolt, then on impact, conjure a lanterncap, then enlarge that lanterncap" should see that sentence in the deck before they cast it.

This sits beside `2026-10-19-expedition-depth-design.md`. That design owns procedural complex greetings, the vault hulk, cave fog, and the reaction table. This design owns one placed cistern, one creature, one growth operation, and the wand surface that arranges it. It does not replace those other slices.

## What was decided, and by whom

Horace:

- Cultivate, conjure a living mushroom, and enlarge a target are all supported.
- One growth effect does both jobs. A rooted mushroom becomes a climbable platform. A mobile creature changes what it can carry, where it fits, and how it fights.
- The valuable creature is one that connects exploration, magic, physical contact, and production. A larger ore drop is not that connection.
- A flat mana number is fine. The test is whether order, delivery, and target change the outcome.
- The same effect must stay recognizable when a wand, a machine, a plant, a trap, or a creature applies it.
- Enlarging a body does not mature it, and does not change who owns it.
- Calling a companion brings the same individual back.
- A growth spell that conjures and then enlarges must affect the creature that cast created.
- The first proof is one underground place where the creature is found, used to climb, used as weight, used in a fight, and taken home.
- The wand arrangement takes inspiration from the PlayStation XMB, and spells are read through particle effects.

Builder calls, not agreements:

- The first slice is the lanterncap, growth, the Hanging Cistern, and the XMB wand. Echo familiars, greek-letter copies, glass curses, and path modifiers wait until delivery, target, effect, and timing exist for growth.
- Owned lanterncaps are not `Enemy` records. Expedition enemies are unsaved slot masks. A companion has to survive leaving the chunk.
- Three size classes, not a continuous scale.
- The XMB replaces the spell palette and the deck row inside the existing gear screen. It does not restyle the hotbar, health, or the world.
- There is no full-screen XMB wave. The cave stays visible. Focus is a ribbon, scale, and particles.
- UI particles are a separate pool from the 480 world `VoxelParticle`s, and they use the same recipe numbers.
- Biological age and magical size are separate fields. The cistern climb needs one magical enlarge of a juvenile, not a wait for maturity.
- Call companion is a gear action, so a stray bolt cannot duplicate the individual.
- Spell ids for this design start at 34. Expedition cards 28–33 stay theirs.
- A rooted juvenile starts small. One Growth raises it to normal, and the cistern landing is built for that normal cap. A second Growth raises it to large, which is the plate and the blocked doorway. Shrink is its own card and steps back down one class.
- The cultivator emits a spore item and, when a planted spore or lanterncap occupies an adjacent cell, calls `apply_growth` +1 once per cycle. Biomass is spent only when that call succeeds.
- A successful carry moves one ground stack from a marked crate cell to a marked pad cell. The gear column sets those two cells while the player looks at the ground.

## Goals

- A player can watch a wild lanterncap root, take a spore, raise a juvenile during the same visit, root it under a broken stair, enlarge it once, and walk up the cap.
- A second enlarge, where there is clearance, weighs down one plate and blocks one doorway. Shrink steps it back to a class that fits the maintenance tunnel.
- A bolt, a touch, an impact payload, and a cultivator emitter call one `apply_growth`.
- The gear screen reads as an XMB: the deck is a horizontal strip with one slot large and centered, and a vertical column offers spells for that slot. The focused spell sheds its particle recipe.
- A cast in the world sheds that same recipe, so the icon and the shot match.
- The owned lanterncap is still that individual after a save, a recall, and a failed enlarge.

## Non-goals

- A second particle simulator, GPU particles, or a raise of the 480 world cap.
- Skinned meshes. The lanterncap is cuboids, a cap, and emissive gills, same path as `Enemy::model`.
- Echo, Alpha, Omega, Mu, Divide, glass curse, vampirism, antiheal, and path wiggle. The XMB has empty room for them. They are not in this slice.
- Retheming inventory, factory, or the pause menu.
- A growth effect that writes a special "this mushroom solved this room" flag. The cap is ordinary supporting geometry.
- Mature breeding, extra harvest biomass from a magical enlarge, or charming an enemy by enlarging it.

## Failure, even if the code runs

- Enlarge projectile and enlarge target are the same button, and the player cannot tell which one fired.
- Growth picks the nearest creature in the room instead of the one this cast created.
- The cap looks climbable and the player falls through it.
- The body grows through the ceiling, or shoves the player into solid.
- Recall spawns a fresh lanterncap and leaves the old one in the cave.
- Opening the wand spends the world particle budget, or a growth cast is invisible because the budget was already full of UI motes.
- The XMB is a wallpaper that hides the cistern.
- The cultivator grows a creature only inside the machine, and the spore item cannot be planted in the cave.

## Current code this design builds on

- Deck: `src/world/adventure/wands.rs` walks at most 8 slots. A trigger payload is attached only when the next spell is a projectile or spell 9. That gate is why an impact cannot yet carry "conjure, then enlarge."
- `ShotMods` has no projectile radius. Orbit widens a hit by 0.24 m and does not move the bolt in a circle.
- Palette: `src/egui_ui/wands.rs` draws spells as a wrapped row of 38 px icons, drag onto slots, with `paint_cast_block_brackets` under the deck. Icons are pixel strings in `src/egui_ui/icons.rs`. Spells 3, 24, 25, 26, and 27 share one purple gem.
- Particles: `VoxelParticle` in `src/world/adventure/spells.rs`. `spawn_voxel_particle` drops the oldest when `particles.len() >= 480`. `tick_particles` integrates gravity, curls rising motes, and bounces off solid cells.
- Cultivator: `src/world/factory.rs` keeps a biomass seed in the machine and emits biomass items. It does not emit a creature.
- Collision and digging already decide what the player can stand on. A rooted cap should be blocks or a solid the existing walk uses, not a special-case stair.
- Expedition design owns generator-17 room greetings. This cistern is one placed site, version-gated, and it does not add a greeting to every complex.
- Lumen helm already casts a lantern beam from the head. The lanterncap's light is a body you can leave in the room.

## 1. One growth operation

`apply_growth(world, target, steps)` is the only enlarge. Callers are touch, projectile impact, and a cultivator emitter. `steps` is +1 or −1 in this slice.

Targets that accept it:

| Target | What changes |
| --- | --- |
| Owned or wild lanterncap, mobile | Size class steps up or down. Carry class, collision half-extents, and model scale change with the class. Age does not. Allegiance does not. |
| Lanterncap that is rooted | The cap's block cluster grows or shrinks by one class. The blocks are the platform. |
| A planted viable spore on fungal soil or glowcap | Becomes a juvenile lanterncap at small size. This is the cultivate shortcut used inside the cistern. |
| Anything else | Reject. No block writes, no second creature. |

Clearance is tested before any visual change. If the destination class intersects solid or the player capsule, growth rejects. The cast's mana is spent. A cultivation nutrient is not spent. The fail recipe plays.

Size classes, authored in metres and matched by collision:

| Class | Body | Rooted cap | Carry | Weight on a plate | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Small | 0.7 m | Below the broken stair | 1 small item | 0 | Fits the maintenance tunnel. The juvenile's starting class. |
| Normal | 1.3 m | Cap top meets the gallery landing | 1 crate-sized item | 1 | One Growth from small. The form you lead home. |
| Large | 2.4 m | Taller than the landing, where the ceiling allows | 1 heavy item | 2 | A second Growth. Needs headroom. Easier to hit. Blocks a 1.4 m gap. The plate accepts this class. |

Magical size is a class plus an optional timer. Permanent biological size is the class with no timer. This slice uses permanent class changes from growth, because the cistern has to keep the platform after the fight. A timer field exists on the companion so a later aura can rent a size without a second code path. This slice does not grant a timer.

Health, damage, and speed do not scale with volume. Large is easier to hit because the capsule is wider, slower by a fixed 15%, and its rooted cap blocks movement. Small keeps today's skitterer-adjacent speed.

## 2. The lanterncap

One species. Wild, cultivated, and conjured copies share `lanterncap` behavior. Origin, allegiance, and whether the record is saved are the differences.

A wild one in the cistern is rooted at normal size beside the garden. It is scenery plus a demonstration until the player takes a spore from it. Taking a spore does not kill the wild one. The wild one is part of the placed site, not an owned id.

An owned companion stores: id, age (spore, juvenile, or mature), size class, rooted flag, health, position, and home. This slice only creates juveniles. Mature has no extra behavior yet and no recipe reaches it. Recall moves this record to the player. It does not allocate a new id.

Root and unroot are an interact on the companion. Rooted, it does not pathfind. It writes a cluster of existing glowcap and fungal-soil blocks tagged with its id. Unroot deletes only blocks still tagged with that id. Dug blocks are already gone. The player stands on those blocks with the ordinary walk.

Conjure creates a juvenile with a new id at the impact point if the cell is open. The payload that follows in the same cast receives that id as its target. The conjure is the parent of the enlarge. Nearest-creature search is not used for that link.

Limits: one owned lanterncap. A second conjure rejects while one is alive. A conjured juvenile is the owned one. Temporary summons that expire are out of this slice, so conjure and cultivate produce the same persistent juvenile. What stops duplication is the one-owner cap and the rule that recall never clones.

The cultivator, when set to a spore culture, emits one viable spore item per cycle. Biomass stays a nutrient. If a planted spore or a lanterncap occupies a cell adjacent to the machine, the same cycle calls `apply_growth` +1 on that target and spends one biomass only when the call succeeds. A rejected growth leaves the biomass in the machine. The machine does not search beyond that adjacent cell and does not choose among two targets: if both a spore and a lanterncap are adjacent, the lanterncap wins. The cistern path does not need the machine: spore on soil, then Growth from the wand.

Light: a point light on the cap, shorter than the lumen helm beam, warm, using the existing adventure light list. Rooted or mobile, the light is on the body.

## 3. Wand grammar

The deck evaluator stays the authority for order, mana, delay, and capacity. This slice extends what a trigger may carry and adds a target pointer on the cast.

A cast action is delivery, then timing, then effect. Modifiers in front of a delivery change the delivery. Effects after a trigger change the target the delivery names.

| Arrangement | Result |
| --- | --- |
| Touch, Growth | Enlarge or cultivate the compatible body or spore under the crosshair. |
| Bolt, Trigger, Growth | The bolt's impact is the growth target: a creature it hit, else a rooted cap or spore in the impact cell. |
| Widen, Bolt, Trigger, Growth | The bolt's collision radius grows. The effect is still growth of the target, not a bigger creature because the shot was wide. |
| Bolt, Trigger, Conjure, Growth | Impact conjures a juvenile, and Growth's target is that id. |
| Seek ally, Bolt, Trigger, Healing | Existing heal spells (1 and the siphon mod) prefer the owned lanterncap when it is in line. Healing numbers stay as they are. |

New ids, all under 256 and not 8:

| Id | Card | Role |
| --- | --- | --- |
| 34 | Touch | Delivery. No projectile. The next effect reads the crosshair. |
| 35 | Growth | Effect. Calls `apply_growth` with +1. A second copy in the same target chain is another step. |
| 36 | Conjure lanterncap | Effect. Creates the owned juvenile if the owner slot is empty. |
| 37 | Widen | Modifier. Sets `ShotMods` projectile radius to 1.8×. Does not call `apply_growth`. |
| 38 | Seek ally | Modifier. Steering prefers the owned lanterncap inside 18 m, then enemies. Uses the existing homing steer. |
| 39 | Shrink | Effect. Calls `apply_growth` with −1. Own glyph: a cap lowering, distinct from Growth's rising cap. |

`spell_available` for 34–36 and 39 requires wand tier ≥ 1. Widen and Seek ally require tier ≥ 2. `MAX_SPELL_ID` becomes 39 in the same change as the decode check. Old saves only contain ids below 28 or, once expedition lands, below 34.

Trigger attachment in `evaluate_cast_block` accepts an effect spell (35, 36) as a payload, not only a projectile or spell 9. Two effect payloads in a row are a chain: Conjure's created id is Growth's target. The chain is at most two effects, so an 8-slot deck cannot fork into a tree.

Icons: 34–39 get their own pixel glyphs. In the same pass, spells 3, 24, 25, 26, and 27 leave the shared purple gem and get their own glyphs. A growth icon reads as a rising cap. A widen icon reads as a fatter bolt. Those two silhouettes stay distinct at 32 px.

## 4. The XMB

The gear screen's spell palette and deck row become one focus surface. The world view behind the panel dims to about 55% and stays visible. No animated wallpaper, no wave that covers the screen.

Anchor: the focused deck slot sits at the horizontal center of the panel.

Horizontal axis, the deck:

- Left and right move the selected slot. The strip slides so the new slot lands on the anchor. Slide time is 180 ms, ease-out.
- The focused slot's icon is 72 px. The next slot each side is 44 px at 72% opacity. Slots two away are 32 px at 40% opacity. Empty slots are a dim frame, still focusable.
- `paint_cast_block_brackets` stays under the strip, in the focused slot's coordinates after the slide.
- The focused slot's spell name and one-line `spell_hint` sit under the icon. Other slots show no paragraph.

Vertical axis, opened with Down while a slot is focused, closed with Up or confirm:

- A thin category ribbon above the column: Bolts, Modifiers, Effects, Summons. Left and right move the category when the column is open. This is the second XMB row, drawn smaller than the deck.
- The column lists that category's spells the wand tier can socket, focused spell at 72 px, neighbors scaled like the deck.
- Confirm writes that spell into the focused deck slot through the existing `SetWandSlot` action.
- Drag and right-click clear stay, so a mouse can still do what the palette does today.

Categories for this slice:

| Category | Spells |
| --- | --- |
| Bolts | The current projectile ids, plus Touch (34) |
| Modifiers | The current modifier ids, plus Widen (37) and Seek ally (38) |
| Effects | Growth (35), Shrink (39), and the utility ids that are not modifiers (vigour, teleport, timberfall, gale, glamour once expedition adds it) |
| Summons | Conjure lanterncap (36). Later summons occupy this column. |

Gear, the fifth category, holds Call companion, Root toggle, Mark crate, and Mark pad when a companion exists. Those are actions, not deck spells. Mark crate and Mark pad store the ground cell under the crosshair on the companion.

Keyboard and the existing click-drag both work. A controller is not required. Focus movement plays a short existing UI cue. No new music.

## 5. Particles

One recipe table, keyed by spell id and by a few event names. A recipe is color, glow, gravity, life, size, burst count, and trickle rate. World spawns and the XMB focus use those numbers.

World spawns go through `spawn_voxel_particle` and the 480 cap. If the cap is full, a growth success may evict the oldest mote, which is today's behavior. A growth success does not allocate past the cap and does not skip the ring in order to spawn the burst: the ring is the first 12 of the budget, the burst is the rest, and a shortfall drops the burst first.

UI motes are egui circles in the panel, cap 64, discarded when the gear screen closes. They never call `spawn_voxel_particle`.

Recipes this slice authors:

| Event | Look | Budget |
| --- | --- | --- |
| XMB focus | 2 motes per second of that spell's color, life 0.6 s, rising if the spell's gravity is low | UI pool |
| Confirm into a slot | 8 motes along the line from the column icon to the deck slot, 0.25 s | UI pool |
| Growth success | 12 motes in a ring at the new height, then up to 16 rising motes, warm gold-green, life 0.8 s | World, ring first |
| Growth reject | 6 dull motes, gravity high, life 0.4 s, no ring | World |
| Root | 12 motes around the new cap edge, then the blocks appear | World |
| Lanterncap idle | 1 mote every 0.4 s from the gills, and never more than 8 alive from that id | World |
| Conjure | The growth-success ring at small size | World |
| Bolt with Widen | The bolt's existing trail uses the spell color. The mesh or billboard radius follows `ShotMods` radius. No extra mote count. | Existing trail |

Spell colors already returned for the live ids stay the recipe color. New ids: Growth `(0.72, 0.95, 0.45)`, Conjure `(0.95, 0.78, 0.35)`, Touch `(0.85, 0.90, 0.95)`, Widen `(0.55, 0.75, 1.0)`, Seek ally `(0.45, 1.0, 0.70)`, Shrink `(0.55, 0.42, 0.28)`.

Rising motes keep the curl already in `tick_particles`. Growth motes use gravity below 0.5 so that curl runs. Reject motes use gravity above 0.5 so they fall.

## 6. The Hanging Cistern

One placed structure, not a 9-room complex. Generator version gate so an older world does not gain it under a visited chunk. Exact coordinates are left to the builder, outside the 100 m spawn clear, and outside cells `undercrown` and `dungeon` already reserve.

From the entrance the player can see an upper gallery, a broken stair, a garden, and a wild rooted lanterncap. Three ways up:

- The maintenance tunnel, always open, sized so a large lanterncap cannot follow and a small one can.
- The broken stair, bridged by rooting the small juvenile and applying one Growth. The landing height matches the normal cap in the table above. A second Growth, in a chamber whose ceiling clears the large class, is what the plate accepts.
- A dug or transmuted shortcut using spells that already write cells. No new digging tool.

A spore and a fungal-soil patch are in the garden, before the stair.

Past the gallery, one plate sinks under weight class ≥ 2. The large lanterncap, standing on it, qualifies. Two loose crate props nearby also qualify when both are on the plate, so the creature is not the only solution.

One fight in the gallery uses the existing encounter spawner with two kinds that already differ: a brawler and a ranged add-on. The rooted cap is cover because it is solid. The fight can be left by the tunnel. No new enemy kind.

A stair or shaft from the gallery drops back to the entrance. At home, the cultivator can be fed the spore culture. The carry job uses the two marked cells. If both are set and the companion is unrooted, it walks to the crate cell, takes one ground stack whose carry class it can hold, and drops that stack on the pad cell. Rooted, it stays. If the pad is blocked for this size class, or the stack's carry class is too high, it waits at the crate and plays the reject mote once. Nothing is inserted into the cultivator by the walk. No logistics network.

## 7. Performance

- Growth block writes are a fixed cluster, at most a few dozen cells, once per successful enlarge, through the existing voxel edit path. No per-frame mesh rebuild while the size lerp runs. Collision and the block write happen at the start of the change, after the clearance test. The model lerps across 0.35 s toward the class that collision already uses.
- The owned companion is one simulated body. The wild cistern lanterncap does not pathfind while rooted.
- UI particles die with the panel. World idle motes are capped at 8 per companion.
- The XMB slide is egui rect interpolation. It does not allocate.

## Tests

- `apply_growth` +1 on a small juvenile in the open leaves it normal and leaves age at juvenile. A second +1 leaves it large. Shrink from large leaves it normal.
- The same +1 with a ceiling closer than the next class leaves class, blocks, and nutrients unchanged, and still reports the mana-spending cast as a reject.
- A bolt whose payload is Conjure then Growth creates one juvenile and leaves it one class above small. A second creature in the room is unchanged.
- Widen changes projectile radius and does not change the creature's class when the payload is absent.
- Root writes blocks the walk can stand on. Unroot removes only that id's blocks.
- Save, load, and recall keep the same id, class, and health. Recall does not increment the companion count.
- A large companion fails the maintenance-tunnel fit and a small one passes, using the authored half-extents.
- The XMB confirm path emits no `VoxelParticle`. A growth success in the world emits a ring even when 470 motes are already alive.
- Spell ids 34–39 round-trip in the loadout word. An older save without them still decodes.
- One Growth on a rooted small juvenile writes a cap whose top is the normal landing height. The plate does not sink under that normal cap. It sinks under the large class.
- A cultivator cycle with an adjacent lanterncap and no clearance leaves the lanterncap's class unchanged and leaves the biomass unit in the machine. A cycle with clearance spends the unit and steps the class up once.

## Implementation order

1. Companion record, size classes, clearance test, `apply_growth`, root blocks, particle recipes for success and reject.
2. Touch, Growth, Shrink, Conjure, the trigger chain that targets the conjured id, Widen, Seek ally, and their icons.
3. XMB deck strip and vertical column on the gear screen, UI motes from the same recipes.
4. The Hanging Cistern: wild lanterncap, spore, stair height, tunnel, plate, return shaft.
5. Cultivator spore output, and the two-point carry wait.

Slice 3 can land on the existing spell list before slice 2's new ids, so the focus surface is testable early. Slice 4 needs slice 1. Slice 5 needs slice 1 and the spore item.

## Left to the builder

- Exact cistern coordinates and stair metres, as long as one Growth of a rooted small juvenile reaches the landing at the normal cap, the large class clears the plate chamber, and the small class fits the tunnel.
- Cuboid layout of the lanterncap inside the class heights.
- The numeric point-light radius on the cap, shorter than the lumen helm.
- Hint strings, as long as Widen and Growth stay distinct in the first sentence.
- Which existing UI cue plays on focus move.
