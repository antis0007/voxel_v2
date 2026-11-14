# Depth features: what keeps players engaged deep into a run

Research date: 2026-11-13. Scope: features and design patterns from adventure mods, voxel/space games, Noita, Terraria and Zelda that sustain engagement after the first hours, mapped to concrete proposals for this engine.

Method: web searches plus fetched wiki and design-talk reports (sources at the end). Fan wikis disagree on version details (boss order, exact numbers), so per-source notes record the mechanism and why it works, not exact values. Mechanics marked *(recalled)* come from general knowledge of the game and were not re-verified in this pass.

Naming rule for proposals: every proposal uses a generic descriptive working name. None copies a name, character, art asset or level from the sources. Player-visible names still need owner approval under `crates/app/src/egui_ui/UI_RULES.md`.

Systems already in the tree that proposals build on (checked 2026-11-13, module headers only):
- **Bosses**: one designed module per guardian under `crates/app/src/world/adventure/`, with site variants, e.g. `bosses.rs`, `thunderhen.rs`, `tollkeeper.rs`, `glassweaver.rs`, `matron.rs`, `hall_warden.rs`, `engine.rs`.
- **Burrowers**: segmented ground worms, in `burrowers.rs`.
- **Deep encounters**: `aberrations.rs`.
- **Expeditions**: `resonance.rs`. Only unbanked resonance and boons are lost on death.
- **Discovery**: `discovery.rs` (spell log), `chronicle.rs`, `inscriptions.rs`, `reliquaries.rs`, `records.rs`.
- **Wands and spells**: decks with TRIGGER, MULTICAST, timer, homing and teleport in `assets/spells.ron`. Rune grammar and casting circles live in `runes.rs` and `circles.rs`.
- **Hazard zones and conductors**: `affinity.rs` (zones) and `conductors.rs`.
- **Realms and portals**: `world/portals`, `world/hell_gate`, `world/undercrown`, `worldgen/nether.rs`, `worldgen/moon.rs`.
- **Simulation**: cellular fluids, thermal and weather.
- **Factory**: machines and signals.
- **Worldgen**: ruins, vistas, karst and deep layers.

---

## 1. Per-source findings

### 1.1 Twilight Forest (Minecraft mod)

1. **Biome locks with readable pressure.** Biomes that hold later bosses apply an escalating debuff (blindness, hunger, burning, freezing, acid rain) or a barrier until the previous boss dies. A small messenger creature carries a book naming the boss you skipped. *Hook:* the world tells you where you are not ready yet without a hard wall, so exploring off-path teaches you the order.
2. **Reward chain where each boss drop counters the next hazard.** Fur from the frost boss lets you survive the frozen region that leads to the final ice boss, and its scales make armour for later fights. *Hook:* the loot is the key, so every kill visibly opens a route.
3. **A boss arena that is the structure itself.** A courtyard for the serpent, a spiralling tower for the lich, a labyrinth for the bull, a fire swamp for the hydra, a stronghold for the phantom knights, a trap tower for the ghast, an aurora palace for the ice queen. *Hook:* reaching the boss is half the fight, and the place is remembered as much as the boss.
4. **Reflect-to-break shields.** The tower lich hides behind shields that break only when you bounce its own bolts back. Its sceptre drop fires the same bolt, so a later fight can skip that phase. *Hook:* the boss teaches a skill, and its trophy rewards you for mastering it.
5. **Trap-tower puzzle kit** *(recalled)*. Vanishing and reappearing blocks triggered by proximity, locked doors opened by signal blocks, and floating minion spawners. *Hook:* a vertical climb through readable machinery.
6. **Magic map and maze map.** A map focus reveals biomes and marks boss structures. A maze map draws the labyrinth as you walk it. *Hook:* the map becomes a progression item, and the empty map is a promise.
7. **Vaults that punish brute force.** The labyrinth's bottom vault pays out only if you reach it without blowing it up. *Hook:* respecting the rules is rewarded in a game where digging normally solves everything.
8. **Trophies and fetch side-quests.** Placeable boss heads make ambient sound. A side-quest ram wants all 16 wool colours and rewards a fast stone-breaking horn. *Hook:* a display shelf of proof, plus long collection goals.
9. **Ritual portal entry** *(recalled)*. You drop a diamond into a flower-ringed pool. *Hook:* entry is a small discovered ritual, not a crafted block.
10. **No surface night spawns in the forest.** *Hook:* the exploration layer stays calm, and danger concentrates in authored places.

### 1.2 Other dimension, structure and creature mods

- **The Aether.** Three tiered sky dungeons, each with a key-locked reward chest. The block-golem boss can only be hurt with a pickaxe. The sun boss can only be hurt by reflected ice bolts, and beating it ends the realm's permanent day. *Hook:* each boss redefines what counts as a weapon, and the last kill changes the sky.
- **Blue Skies.** Two realms, a day realm and a dawn realm. Overworld tools do reduced damage there, which forces realm-native gear. Keystone bosses unlock deeper dungeons. A conquered lair's keystone can be re-armed with purchased keys for a harder rematch with better loot. *Hook:* a second progression ladder, and replayable bosses in shared worlds.
- **The Betweenlands.**
  - A decay meter rises just from being in the realm.
  - Imported food rots on arrival, and imported tools fail.
  - Eating the same food repeatedly gives diminishing returns.
  - A sludge labyrinth ends in a tentacled boss.

  *Hook:* the realm feels hostile in a systemic way, and you must learn to live there.
- **Deeper and Darker.** A portal in the deep city is lit with a heart taken from its blind sound-hunting guardian. A later item links signals across dimensions. *Hook:* the entry key is taken from the most-feared mob, and stealth becomes the skill.
- **Undergarden** *(recalled)*. A dark underground realm with a catacomb guardian, spreading blight, and a realm-native toolset. *Hook:* a gloomy alternate world with its own tool ladder.
- **Atum** *(recalled)*. A desert realm with sandstorms. A pharaoh boss is summoned by disturbing a sarcophagus, and its guards spawn first. Relics are tied to deities. *Hook:* you choose when to provoke the fight.
- **Erebus** *(recalled)*. An insect realm with giant arthropods, an offering altar and an antlion-style overlord in desert pits. *Hook:* scale inversion, where the player is small.
- **AbyssalCraft** *(recalled)*.
  - A tiered necronomicon book gates rituals.
  - A plague status spreads from the abyss.
  - A chain of dimensions is unlocked by ritual and boss.

  *Hook:* forbidden-knowledge escalation, where each book tier unlocks scarier content.
- **Dungeons Arise and YUNG's structures** *(recalled)*. Huge multi-level landmarks visible from far away: castles, ships, temples, sky towers. Reworked mineshafts, strongholds and dungeons get varied rooms and secrets. *Hook:* a silhouette on the horizon is an invitation, and verticality makes a structure a level rather than a box.
- **Repurposed Structures** *(recalled)*. Every vanilla structure gets a variant per biome. *Hook:* familiar shapes in new materials keep exploration fresh at low cost.
- **Alex's Caves.**
  - You find an underground cabin with tablets that point to six themed cave biomes: magnetic, primordial, toxic, abyssal chasm, dark hollow and candy.
  - Each biome has signature mobs: magnetic constructs, dinosaurs, creatures that mutate under radiation, deep-sea leviathans, and cultists who sacrifice to make watchers.

  *Hook:* finding a cave biome is a treasure hunt, and each one has its own physics fantasy.
- **Alex's Mobs** *(recalled)*. Dozens of creatures, each with one unique interaction: mimics that copy your held weapon, a portal-hopping void worm boss, tameable oddities. *Hook:* a bestiary of surprises where every creature teaches something.
- **Mowzie's Mobs.** The armoured axe knight is immune except from behind while its axe is stuck after a slam. A rare sleeping frost giant guards a crystal you can steal or win. A sun-caller boss uses telegraphed sunstrikes. *Hook:* pattern reading beats gear, and a sleeping giant invites a stealth heist.
- **L_Ender's Cataclysm.** Each boss has an elaborate dungeon. Lava pools and elevation in the forge arena shape movement. A shielded fire knight punishes impatience. The recommended order is soft, not enforced. *Hook:* arenas are part of the moveset.
- **Ice and Fire** *(recalled)*.
  - Dragons grow through life stages over days.
  - Roosts sit on scorched terrain with hoards, and dragons burn the terrain.
  - Petrifying gazers and a forge built from dragon parts.

  *Hook:* an apex creature that changes the land, and can eventually be raised.
- **Born in Chaos** *(recalled)*. Night events with themed waves and holiday-style mini-bosses. *Hook:* nights become scheduled spikes of tension.
- **Biomes O' Plenty, Terralith and Tectonic** *(recalled)*. Large biome variety and dramatic large-scale landforms built from vanilla blocks. *Hook:* the walk itself is rewarding, and vistas pull players to the next ridge.

### 1.3 Seed of Andromeda and related space/voxel games

- **Seed of Andromeda.** Fully round voxel planets in generated solar systems. Its pitch was to crash-land, settle, build technology and escape the planet. Development stalled around 2015. *Hook:* "that light in the sky is a place you will stand on", which is the promise this engine already partly delivers.
- **Vintage Story.**
  - Temporal stability is a meter that drops underground and during storms.
  - Rifts spawn in unstable areas and wards suppress them.
  - Storms bring distorted enemies.
  - Broken paired teleporters in ruins are repaired with a gear and link to a partner 400 to 8000 blocks away.
  - Story locations have fixed stability.

  *Hook:* depth carries a psychological cost, and repairing a teleporter is a gamble on where it leads.
- **Outer Wilds.**
  - No gear progression at all.
  - A ship log records locations, notes and unresolved rumour links.
  - A short time loop.
  - Places change on a schedule: sand flows between twin planets, an island is thrown into orbit.

  *Hook:* knowledge is the only key, and the "aha" moment is the reward.
- **Astroneer** *(recalled)*. A terrain tool reshapes the ground. Tethers form an oxygen lifeline. You research found objects. Each planet's core holds a unique goal. *Hook:* a lifeline creates a safe radius you extend, and the core is a vertical destination.
- **Space Engineers** *(recalled)*. Oxygen, hydrogen and power logistics, grid-based ships, and survival on planets with gravity. *Hook:* the thing you built is your survival, and physics failures make stories.

### 1.4 Noita in depth

1. **Material world as weapon and hazard.** Every pixel is a liquid, powder, gas or solid that burns, conducts, freezes or reacts. Most deaths come from the environment (oil plus fire, electrified water, acid floors). *Hook:* emergent stories, and the player learns chemistry by surviving it.
2. **Per-world alchemy.** Three high-value substances (one turns liquids to gold, one heals, one is a powerful precursor) have recipes randomised per world, of at most one solid and two or three liquids. Cauldrons, and clues in fan tools, lead to them. *Hook:* each run hides a secret worth experimenting for.
3. **Fungal reality shift.** Eating certain fungi permanently remaps one material to another for the run.
   - A held flask has a 75% chance of being involved, which lets you target the swap.
   - There is a 5-minute cooldown and a cap of 20 shifts.
   - Shifts carry into New Game Plus.
   - Risks: shifting teleport liquid disables portals, and shifting water to a hazard changes every wet stain.

   *Hook:* the player gets to rewrite the rules of the world, with real consequences.
4. **Holy Mountain rest stops** between biomes.
   - A perk altar offers three choices plus a reroll machine.
   - There is a spell or wand shop, a full heal and a spell refresher.
   - Wands can be edited only here.
   - The ceiling collapses as you leave, so there is no backtracking.
   - Damaging the brickwork angers the gods and spawns a deadly guardian in every later rest stop.
   - Clearing a biome without killing gives a pacifist chest.

   *Hook:* a pacing beat with real choices, plus a temptation (steal from the shop, dig in) with a lasting price.
5. **Wand building.** Triggers, timers, multicast, modifiers and always-cast slots compose into combos the designers never planned. *Hook:* build-crafting depth, where discovery is personal and shareable.
6. **Orbs, parallel worlds and the long secret chain.** Orbs are scattered off the main path. The world repeats horizontally into parallel worlds that are harder and richer. The sun quest:
   - A seed from a parallel-world ghost miniboss becomes a stone on the pyramid.
   - The stone becomes an egg after an explosion near the moon.
   - The egg needs 100 kills.
   - Four elemental stones then make a sun, or with a dark extra a dark sun.
   - Bringing both suns together causes a world-burning supernova.
   - Permanent flags remember what you did.

   *Hook:* a multi-session quest with escalating stakes and a world that remembers.
7. **Unsolved cipher.** Nine eye-glyph messages in the parallel worlds, seeded positions, glyph counts divisible by three, trigram values spanning 0 to 82. Still unsolved; the community runs cryptanalysis projects. *Hook:* a mystery that outlives the game's own content.
8. **Enemy design around systems.**
   - Polymorph liquid turns you into a sheep, and a few enemies cast it.
   - Hiisi gunners use the same wands as you.
   - Worms tunnel through terrain.
   - Shielded and teleporting mages.
   - A crystal deters worms.

   *Hook:* enemies follow the same rules as the player, so the player's tools are both counterplay and threat.
9. **Permadeath and risk/reward.** Gold, perks and wands all depend on greed: digging for that chest under the lava, picking the glass-cannon perk. *Hook:* every decision matters because loss is total.

### 1.5 Terraria and Zelda

**Terraria**
1. **Summoning items.** Most bosses are fought when you choose to summon them, with crafted or found items that need conditions such as night, a biome or an altar. *Hook:* the player controls the pacing and prepares an arena.
2. **The world-turning gate boss.** A screen-tall wall boss is summoned by dropping a doll into lava in the underworld. It sweeps across the layer, and a tongue drags in anyone who falls behind. Defeating it permanently turns the world harder:
   - Two diagonal V-shaped stripes of corruption and holy biome spread.
   - New ores appear when altars are smashed with its hammer drop.
   - New enemies, biomes and gear arrive.

   *Hook:* the world you know changes overnight, which effectively starts a second game.
3. **Splitting worm boss.** A segmented worm of about 70 segments splits into two worms when a middle segment dies. Each segment can drop ore. It must burrow before it can lunge again. *(Recalled:)* its mechanical successor launches drone probes and fires lasers from each segment. *Hook:* the fight's shape changes as you damage it, and you collect loot mid-fight.
4. **Telegraphed escalation.** Lasers grow faster and the wall speeds up as health drops, and minions detach in later phases. *Hook:* a readable rising curve of difficulty in one fight.
5. **Events and invasions** *(recalled)*. Blood moons, goblin armies and pumpkin or frost moons with waves and points. *Hook:* scheduled set pieces that use the base you built.

**Zelda: Ocarina of Time / Breath of the Wild**
1. **Chemistry engine.** Three rules: elements change materials, elements change each other, and materials do not change each other. Fire, water, ice, electricity and wind are elements; trees, rocks and the player are materials. The designers call it "liar's physics", a simplification players can reason about. *Hook:* players can predict outcomes, so creative solutions feel earned.
2. **Shrines** *(recalled)*. About 120 short puzzle rooms, each testing one idea (magnetism, stasis, wind, heat), paying a heart or stamina token. *Hook:* bite-sized mastery with a steady reward drip.
3. **Korok seeds** *(recalled)*. About 900 tiny environmental micro-puzzles (lift a rock, complete a stone ring, shoot balloons) that pay inventory slots. *Hook:* the world is dense with small "huh, what's that?" moments.
4. **Guardians** *(recalled)*. Ancient automata with a visible laser lock-on line that tightens before firing. A perfectly timed shield parry reflects the shot. Cutting legs cripples them. *Hook:* terror that turns into mastery, and the telegraph is the tutorial.
5. **Stone Talus, Hinox and Lynel** *(recalled)*.
   - The rock giant is camouflaged as terrain, and its weak point is an ore cluster you climb onto.
   - The one-eyed giant sleeps with loot on its necklace, which you can steal.
   - The centaur is an elite duel with mountable counter-windows.

   *Hook:* each miniboss is a different verb: climb, steal or duel.
6. **Enemy camps** *(recalled)*. Camps hold a treasure chest, and you can kill them with explosive barrels, rolling boulders or fire on grass. *Hook:* an open-ended tactics puzzle using the environment.
7. **Ocarina of Time dungeons** *(recalled)*.
   - Rotating blade traps and timed platforms.
   - Each dungeon item opens the dungeon behind it and then the boss.
   - Boss key and map/compass structure.

   *Hook:* the new tool is the key to its own dungeon, and every room teaches it.

---

## 2. Cross-cutting patterns

1. **Knowledge-gated progression.** Outer Wilds, Noita alchemy and ciphers, and the Twilight Forest messenger book. The unlock is information. It survives death and is shareable, and it fits a persistent world plus roguelite expeditions.
2. **The loot is the key.** Twilight Forest drops counter the next hazard, Zelda dungeon items, the Deeper and Darker heart portal, and the Terraria hammer. Every reward is a visible, testable route opener, not a stat bump.
3. **Soft walls that explain themselves.** Biome debuffs, Vintage Story stability, Betweenlands decay. Instead of "you cannot enter", the world says "you can enter but you will hurt, and here is why". This keeps exploration open while steering.
4. **Boss-gated world change.** Terraria hardmode, the Aether sun ending its permanent day, the Noita sun and supernova flags. A kill rewrites the world state, so the save itself becomes a progress bar.
5. **The arena is the moveset.** Twilight Forest structures, Cataclysm lava forges, Ocarina dungeons. The approach and the arena geometry are part of the fight, and reaching the boss is earned.
6. **Bosses that redefine what a weapon is.** Pickaxe-only, reflect-only, behind-only, stump cauterising. Each fight teaches one rule and rewards learning it rather than gear checks.
7. **Emergent system interactions with simple rules.** The Zelda chemistry rules and Noita materials. Few rules applied uniformly to every actor (player, enemy, terrain, items) create combinations the designers did not script.
8. **Secrets layered from obvious to cryptic.** Korok seeds, then shrines, orbs, alchemy, the sun quest and finally the cipher. Each audience gets its depth, and the deepest layer builds community.
9. **Temptation with lasting price.** Noita's angered gods, the Twilight Forest vault that explodes, Noita perks with downsides. Agency is meaningful only when breaking a rule has a remembered cost.
10. **Pacing beats.** Holy Mountain, shrines and bosses on summon. Rest, choose and refill, then dive again. Control over when danger starts.
11. **Hostile realms with their own ladder.** Betweenlands, Blue Skies, Undergarden. Imported gear underperforms, so each realm is a fresh progression without new core systems.
12. **Silhouettes as invitations.** Dungeons Arise, Tectonic and the twin planets in Outer Wilds. Visible distant landmarks set self-chosen goals.
13. **Enemies obey player rules.** Noita wand users and polymorph, Zelda camps with barrels. Counterplay is the same toolkit, so mastery transfers.
14. **Replayable bosses for persistent worlds.** Blue Skies keystones, Terraria summons, Twilight Forest sceptre skips. Shared and multiplayer worlds need repeatable, scaled boss content.

---

## 3. Ranked feature proposals

Size: S is days and data-first; M is one to two weeks and touches one system plus content; L is multi-week, crosses systems and may need save or generator-gate work. Every runtime world-state change must respect the save-version and generator-gate rules in `CLAUDE.md`. World-changing effects should be save-side overlays, not edits to historical generators.

Ranking weighs hook strength against fit with existing systems and cost.

### 1. Guardian-held region pressure with counter-drop chain
A guardian's territory (biome region, realm band or dungeon district) imposes an escalating environmental pressure on anyone who enters before that guardian falls. Examples: scalding air, deepening frost, blinding spore fog, corrosive drizzle. The pressure grows the longer you stay rather than blocking entry. A small fleeing creature or a weathered signpost in the region names, in plain words, which guardian holds it. That guardian's drop is the specific counter for the next region: an insulating hide, a lantern that burns through fog, a coating that resists acid. With the counter equipped, the pressure falls to a mild level. The kill also lifts the pressure permanently for that world.
- **Source:** Twilight Forest biome locks and reward chain; Vintage Story stability.
- **Hook:** readable soft walls, and every kill visibly opens a route.
- **Size:** M.
- **Builds on:** bosses (site records), `affinity.rs` zones, weather/thermal, statuses.

### 2. Sanctuary chambers between expedition depths
Expedition dungeons and realms get a sealed rest chamber every few depths:
- an altar offering three run boons with one paid reroll;
- a small spell shop;
- one heal and one wand refill;
- the only place where wand decks can be edited during an expedition.

The entrance seals behind you as you leave. Damaging the chamber's warded walls, or carrying shop items out unpaid, wakes a wrath warden. It appears in every later sanctuary of that expedition and grows stronger with each kill. Finishing the previous depth without kills leaves a gift chest.
- **Source:** Noita Holy Mountain.
- **Hook:** a pacing beat with real choice, plus temptation with a remembered price.
- **Size:** M.
- **Builds on:** `resonance.rs` expeditions and boons, wand editing, dungeon generation.

### 3. Seeded alchemy secrets with clue tablets
Each world generates two or three secret recipes for high-value fluids from its seed. Examples: a liquid that converts touched ore to a richer grade, a regenerating draught, a precursor needed by an endgame machine. Each recipe uses two or three fluids and at most one powder. Clue tablets found in ruins state true constraints such as "begins with something that burns", "never touches salt" or "needs the cold of the deep layer". Enough clues narrow the space to a guessable set. Mixing happens in real cellular-fluid vessels, so failed mixes have real reactions.
- **Source:** Noita alchemy; Outer Wilds clue design.
- **Hook:** every world hides a secret worth experimenting for, and it becomes a community "what's your recipe" topic.
- **Size:** M.
- **Builds on:** cellular fluids, `assets/brewing.ron`, reliquaries/inscriptions, positional seeded hashes.

### 4. Field journal with linked rumours
The journal records places, readings and clue fragments as nodes. It draws links between rumours that reference each other, such as "the tide-cut steps" and "the drowned door opens at low water". Unresolved links show as dangling threads with a question mark. Clue entries carry no stat reward. Their value is that they point to a time, place or condition that opens a route, and that knowledge survives death and expeditions.
- **Source:** Outer Wilds ship log; Twilight Forest messenger book.
- **Hook:** knowledge is the key, and the dangling thread nags the player into the next session.
- **Size:** M.
- **Builds on:** `chronicle.rs`, `records.rs`, `inscriptions.rs`, journal UI.

### 5. Splitting burrower guardian that carves real tunnels
This is a guardian-scale burrower. Killing a middle segment splits it into two shorter worms, each with its own head. Killing a head or tail only shortens it. It must re-enter the ground before each lunge, and it really excavates. Its tunnels are saved terrain edits that collapse sand and drain fluids into them, so the arena reshapes during the fight. Each segment can drop ore from the layer it died in. The burrower flees deeper if you leave its depth band.
- **Source:** Terraria splitting worm.
- **Hook:** the fight changes shape as you damage it, and loot rains mid-fight.
- **Size:** M.
- **Builds on:** `burrowers.rs`, voxel edits, cellular sand/water.

### 6. Multi-head guardian that regrows unless cauterised
A swamp or volcanic-vent guardian with several heads. A severed head regrows two heads after a delay unless its stump is sealed: with fire, molten material, acid or a frost spell that freezes it. The arena supplies hazards: oil seeps you can ignite, a lava channel you can divert, a cold spring. The body is damaged only while the heads are reduced.
- **Source:** Twilight Forest hydra, classical hydra myth, Noita materials.
- **Hook:** the solution comes from the material system, not a damage check.
- **Size:** L.
- **Builds on:** bosses, cellular fluids, thermal, spell statuses.

### 7. One state-rule table for every actor, plus environmental kill credit
Codify a short chemistry rule set:
- elements (heat, wet, frost, charge, wind, corrosion) change material and creature states;
- elements change each other;
- materials do not change each other directly.

Apply the rules identically to player, enemies, items, crops, machines and terrain: wet creatures conduct charge, frozen enemies shatter from blunt hits, burning wood spreads to wooden machines. Kills caused by the environment (fluids, falling blocks, fire, enemy friendly fire) are credited to the player and pay a small bonus drop, and the journal logs new combinations.
- **Source:** Breath of the Wild chemistry rules; Noita environmental deaths.
- **Hook:** predictable rules let players invent solutions, and creative kills are recognised.
- **Size:** M (mostly an audit plus unification).
- **Builds on:** thermal, `conductors.rs`, statuses, cellular fluids.

### 8. World-turning gate guardian
A deep guardian is summoned deliberately with a ritual at a deep site. Defeating it permanently shifts the world into a harsher phase:
- dormant deeper ore seams awaken;
- nights bring stronger kinds;
- two blight bands start spreading slowly from fixed seeded origins.

The factory can contain the spread with purifier machines and barrier blocks. Implement it as a saved world-phase flag plus a runtime spread overlay, never as a change to historical generation. Offer the summon only once, with clear warning text.
- **Source:** Terraria world-turning boss.
- **Hook:** the familiar world changes overnight, a second game inside the first, and it gives the factory a purpose.
- **Size:** L.
- **Builds on:** bosses, factory, saves, `affinity.rs` blight zones.

### 9. Micro-puzzle token trail
Seed hundreds of tiny environmental micro-puzzles per region:
- a lone stone that hides a token when lifted;
- a broken ring of standing stones to complete;
- a floating target to shoot;
- three flowers to light in order;
- a block to push into a socket.

Each solved puzzle pays a token. Tokens buy inventory rows, wand capacity or spell-slot upgrades from a recurring wandering trader.
- **Source:** Breath of the Wild seed collectibles.
- **Hook:** the world is dense with "what's that?" moments, with a steady reward drip at near-zero authoring cost per instance.
- **Size:** M.
- **Builds on:** worldgen placer and ruins, deterministic seeded placement, inventory.

### 10. Multi-stage celestial ascension quest
A rare seed dropped by a parallel-depth miniboss starts a quest that crosses the solar system:
1. Plant the seed on the highest summit of the home world to make a stone.
2. Detonate a large blast beside the stone on the moon to make an egg.
3. Feed the egg a set number of kills, or it slowly fades.
4. Infuse four elemental cores, each earned on a different planet, to make a small bound star.

Bringing the star home awards a permanent sky change and a crown-tier relic. A dark variant exists, and combining both causes a world-scarring flare. World flags remember the outcome.
- **Source:** Noita sun quest.
- **Hook:** a multi-session goal with escalating stakes that uses the planets and space.
- **Size:** L.
- **Builds on:** planets, moon, space navigation, bosses, saved flags.

### 11. Reflect-to-break shields and a rebound modifier
Add a rebound wand modifier and a timed blade parry that return enemy projectiles. Design one or two guardians whose shield breaks only to their own returned projectiles. The guardian's drop casts that projectile, which lets later rematches skip the shield phase.
- **Source:** Twilight Forest lich; Aether sun boss.
- **Hook:** the boss teaches a skill, and its trophy rewards mastery.
- **Size:** S.
- **Builds on:** wands/spells (new modifier), `blade.rs`, bosses.

### 12. Hostile realm sustainment and native gear
Each realm (underworld, deep realm, future ones) adds a slow realm-specific strain meter. It is restored only by food and remedies made from realm-native materials. Imported food spoils on arrival, and imported tools work at reduced effectiveness until reforged with a realm material. Eating the same food repeatedly gives diminishing returns.
- **Source:** Betweenlands; Blue Skies.
- **Hook:** every realm becomes a fresh progression ladder built from existing systems.
- **Size:** M.
- **Builds on:** `hunger.rs`, tool workshop/forge, realms.

### 13. Seeded glyph cipher layer
Seed deep, hard-to-reach walls with inscriptions in an original glyph script. The script encodes real, verifiable world facts: coordinates of a sealed vault, one alchemy clue, the condition for a hidden door. The encoding is deterministic from the seed, consistent across worlds and decodable by careful players. Nothing requires it, and solving it gives an advantage.
- **Source:** Noita eye glyphs.
- **Hook:** a community mystery that outlives authored content.
- **Size:** M.
- **Builds on:** inscriptions, reliquaries, worldgen ruins.

### 14. Summoning offerings with conditions
Make guardian fights opt-in at their lairs. The player assembles an offering on an altar circle under stated conditions: night, rain, a fluid in the basin, a specific moon phase. This gives preparation time and makes rematches possible. A re-armed conquered lair, using a crafted key, spawns a harder variant with better loot.
- **Source:** Terraria summon items; Blue Skies keystones; Twilight Forest portal ritual.
- **Hook:** player-controlled pacing and repeatable bosses for shared worlds.
- **Size:** S.
- **Builds on:** bosses and variants, `circles.rs`, weather/time.

### 15. Signal-driven trap and puzzle dungeon kit
Add a dungeon block kit run on the factory signal network:
- rotating blade arms on axles;
- timed crushers;
- blocks that vanish on proximity and return after a delay;
- doors opened by signal plates;
- spawner lanterns that can be shot out.

Rooms are authored as small templates and assembled by the dungeon generator. Simulation is fixed-step and deterministic.
- **Source:** Ocarina of Time blade traps; Twilight Forest trap tower.
- **Hook:** readable machinery turns rooms into skill and puzzle beats.
- **Size:** M.
- **Builds on:** dungeon generation, factory signals, rigid bodies.

### 16. Single-mechanic trial rooms
Short sealed chambers, each testing one physical idea: fluid pressure lifting a float, a conductor path through wet floor, heat expanding a gate, buoyancy, a counterweight. Each completion pays a health or stamina shard, and four shards make one upgrade.
- **Source:** Breath of the Wild shrines.
- **Hook:** bite-sized mastery that teaches the simulation the rest of the game relies on.
- **Size:** M.
- **Builds on:** cellular fluids, thermal, conductors, dungeon room templates.

### 17. Laser-telegraph sentinel automatons
Dormant automaton turrets stand in ancient ruins on open terrain. When they wake, a visible beam line tightens on the player over about two seconds before firing. A perfectly timed parry or rebound spell reflects the shot, and breaking leg joints immobilises them. Disabled sentinels yield parts for factory upgrades.
- **Source:** Breath of the Wild guardians.
- **Hook:** terror that turns into mastery, with a telegraph that is its own tutorial.
- **Size:** M.
- **Builds on:** `robots.rs`, ruins worldgen, factory parts.

### 18. Living-rock miniboss
A miniboss looks like an ordinary outcrop until disturbed, then stands. It is immune to normal damage. Its weak point is a cluster of rich ore on its back that you climb onto and mine or blast while it shakes. When it dies it collapses into real terrain debris and the ore drops.
- **Source:** Breath of the Wild rock giant; Aether block golem.
- **Hook:** mining becomes the combat verb.
- **Size:** M.
- **Builds on:** voxel edits, rigid-body debris, enemy kinds.

### 19. Committed-attack openings for armoured foes
An armoured elite is invulnerable from the front. Its heavy slam buries its weapon, and for a moment its back is exposed. Missing the window or getting greedy is punished by a sweeping recovery.
- **Source:** Mowzie's axe knight.
- **Hook:** reading the pattern beats gear.
- **Size:** S.
- **Builds on:** enemy tactics (`tactics.rs`), boss modules.

### 20. Sleeping giant heists
A rare giant sleeps at the heart of a camp or snowfield, wearing or guarding a valuable item. A stealthy approach (noise from footsteps, armour and lit torches) lets you steal it. Waking it turns into a chase or fight.
- **Source:** Breath of the Wild one-eyed giant; Mowzie's frost giant.
- **Hook:** a different verb (stealth) with a high-tension payoff.
- **Size:** M.
- **Builds on:** habitats, footstep sounds (`assets/footsteps.ron`), enemy AI.

### 21. Environmental-tactics enemy camps
Generate enemy camps with tactical props: explosive kegs, hanging rock nets, a dry grass ring, a water tank above the fire pit. The camp chest unlocks when the camp is cleared. Camp enemies also use the props against you.
- **Source:** Breath of the Wild camps; Noita.
- **Hook:** an open-ended tactics puzzle that showcases the simulation.
- **Size:** M.
- **Builds on:** worldgen settlements, enemies, cellular fluids, fire.

### 22. Rule-respecting vaults
Some vaults hide their reward behind a fragile seal. Digging through the vault walls or blasting the room collapses or burns the treasure. The intended solution (find the key, solve the lock, step on the right tiles) pays fully. Clearing the preceding area without kills leaves a gift chest.
- **Source:** Twilight Forest labyrinth vault; Noita pacifist chest.
- **Hook:** respecting rules in a dig-anything game feels clever.
- **Size:** S.
- **Builds on:** dungeon generation, `mirage.rs`-style saved edits, chests.

### 23. Themed deep cave biomes with signature physics
Add several rare deep biomes, each with one physics fantasy:
- **Magnetic caverns:** iron items and drops drift toward lodestone, and lodestone blocks levitate iron slabs.
- **Toxic pits:** acid pools and a radiation dose that mutates local creatures.
- **Abyssal trench:** crushing pressure below a depth and near-total darkness, needing sealed gear.
- **Echo hollows:** blind hunters that track sound.

Found tablets point toward the nearest one.
- **Source:** Alex's Caves; Deeper and Darker.
- **Hook:** finding the biome is a treasure hunt, and each has its own physics.
- **Size:** L.
- **Builds on:** `deep.rs`, karst, cellular fluids, new generator gate.

### 24. Parallel depths
Past the normal world edge or below the deepest layer, a repeat of known regions waits with harder enemies and richer, stranger loot. This is where the rarest seeds and cipher walls live. Make it an expedition-only band to keep save and generation scope contained.
- **Source:** Noita parallel worlds.
- **Hook:** "there is more" after you think you have seen everything.
- **Size:** L.
- **Builds on:** expeditions, worldgen, difficulty scaling.

### 25. Material remap fungus
A rare fungus, eaten within a bounded zone (a realm or a radius around a shrine), permanently remaps one material to another inside that zone. A held flask biases which material is chosen. Examples: lava to water saves a base, water to oil makes a disaster. Store it as a small saved remap table per zone with a cooldown and a lifetime cap.
- **Source:** Noita fungal shift.
- **Hook:** rewriting the rules of the world with real consequences.
- **Size:** L (save format).
- **Builds on:** materials table, cellular fluids, saves.

### 26. Transformation hazards
Add a rare transforming fluid and a few enemies that cast transformation. While transformed, the player becomes a weak creature, cannot use wands and moves differently for a short time. A cleansing fluid cures it. Players can bottle the fluid and throw it at enemies or guardians: it is weaker against guardians and turns elites into livestock.
- **Source:** Noita polymorph.
- **Hook:** a sudden total loss of power that turns into a tool.
- **Size:** M.
- **Builds on:** statuses, cellular fluids, mob kinds.

### 27. Placeable guardian trophies
Each guardian drops a trophy block. It is animated, makes ambient sound, and has a small aura buff in a radius (one per kind per base). It also shows the kill date and variant. A trophy hall with all trophies unlocks a decorative set.
- **Source:** Twilight Forest trophies.
- **Hook:** visible proof of progress in the base.
- **Size:** S.
- **Builds on:** bosses, block models, base dashboards.

### 28. Survey charts and walking maps
Two craftable map items. A charted survey, focused with a guardian drop, reveals the surrounding region's biomes and marks guardian lairs and landmarks. A walking map draws a dungeon's interior as you walk it and marks found secrets.
- **Source:** Twilight Forest magic and maze maps.
- **Hook:** the empty map is a promise, and the map becomes a progression reward.
- **Size:** M.
- **Builds on:** `map_probe`, navigation, dungeon layout data.

### 29. Repairable paired waystones
Ruins hold broken waystones. Repairing one with a crafted resonant part links it to a seeded partner 400 to 8000 m away, at a place you have not seen. Both ends become permanent fast travel.
- **Source:** Vintage Story translocators.
- **Hook:** a gamble on the destination, plus earned fast travel.
- **Size:** S.
- **Builds on:** `world/portals`, ruins, travel.

### 30. Depth unease and distortion storms
Unease rises underground, in darkness and during rare distortion storms, and falls near home light and wards. High unease adds auditory and visual warping and spawns distorted variants of local creatures. Crafted wards suppress it in a radius.
- **Source:** Vintage Story temporal stability.
- **Hook:** depth carries a psychological cost, and safety becomes something you build.
- **Size:** M.
- **Builds on:** `mood.rs`, weather, night spawns.

### 31. Escalating night events
Moon phases schedule night events:
- themed waves (bone, mire, frost);
- a points bar;
- a capstone miniboss when the bar fills;
- an event-only drop.

Events start at the player's base and use its defences, and players can opt out with a ward.
- **Source:** Terraria moon events; Born in Chaos.
- **Hook:** scheduled tension spikes that test the base you built.
- **Size:** M.
- **Builds on:** `night.rs`, factory defences, mob kinds.

### 32. Pickaxe-damaged construct guardian
A guardian built from blocks that can only be hurt by mining. Each mined block is removed from its body, which changes its silhouette and attacks. Fallen blocks become rigid-body debris it can re-absorb if you leave them.
- **Source:** Aether block golem.
- **Hook:** inverts what a weapon is and plays to the voxel engine's strength.
- **Size:** M.
- **Builds on:** voxel edits, rigid bodies, bosses.

### 33. Heart-keyed realm entry
Entry to a deep realm needs a heart taken from a sound-hunting sentinel in a deep ruin city. Players mostly avoid it, and killing it is an endgame feat. Once taken, the heart re-lights the portal any number of times.
- **Source:** Deeper and Darker.
- **Hook:** the key is taken from the most-feared creature, so stealth is the skill.
- **Size:** S.
- **Builds on:** portals, `hell_gate`, footstep noise.

### 34. Growing apex creatures
An apex flier hatches from rare eggs and grows through stages over in-game days. Adults make roosts that scorch or freeze the surrounding terrain and pile hoards of ore. A stolen egg can be raised, with care, into a mount.
- **Source:** Ice and Fire dragons.
- **Hook:** a creature that changes the land, and a long-term companion goal.
- **Size:** L.
- **Builds on:** habitats, wildlife, thermal, save data.

### 35. Horizon landmark megastructures
Very rare, huge multi-level structures visible from kilometres away: a sky spire, a grounded ark, a terraced temple. Their interior routes, vertical shafts and secret floors have loot layered by depth. Biome-material variants of common structures add cheap freshness.
- **Source:** Dungeons Arise, YUNG's structures, Repurposed Structures.
- **Hook:** a silhouette on the horizon sets the player's own goal.
- **Size:** L.
- **Builds on:** worldgen ruins/vistas, far-field LOD, horizon.

### 36. Cross-planet phenomenon puzzles
A puzzle where the explanation for a strange place lies on another body. Examples:
- A coastal vault opens only at a tidal extreme set by the moon's real orbital phase.
- A shadow cast by a giant planet at a set time points to a buried door.
- A sand flow between paired moons exposes ruins on a schedule.

The journal logs the observations.
- **Source:** Outer Wilds.
- **Hook:** the solar system is one puzzle box, and scheduled change rewards attentive players.
- **Size:** L.
- **Builds on:** planets, orbital mechanics, journal.

### 37. Planet-core destinations
Each planet's deep core holds one unique authored-procedural structure and reward: a relic, an elemental core for the ascension quest, a new realm entry. Reaching it requires the gear progression that planet teaches, such as heat, pressure or gravity.
- **Source:** Astroneer cores; Seed of Andromeda.
- **Hook:** a vertical goal on every planet, so landing matters.
- **Size:** L.
- **Builds on:** planets, deep layers, space travel.

### 38. Tiered forbidden-knowledge tomes
A tome gains tiers as you feed it guardian essences. Each tier unlocks scarier rituals: summon circles, realm openings, curse-weaving. Higher tiers raise a spreading blight risk near the player's base unless warded.
- **Source:** AbyssalCraft.
- **Hook:** escalating forbidden power with a price.
- **Size:** M.
- **Builds on:** rune grammar, `circles.rs`, `affinity.rs` blight.

---

## 4. Suggested sequencing

- **First, small and data-first:**
  - 11 Rebound shields
  - 14 Summoning offerings
  - 19 Committed openings
  - 22 Rule-respecting vaults
  - 27 Trophies
  - 29 Waystones
  - 33 Heart-keyed entry
- **Core depth loop (M):**
  - 1 Region pressure
  - 2 Sanctuaries
  - 3 Alchemy secrets
  - 4 Rumour journal
  - 7 State rules
  - 9 Micro-puzzles
  - 5 Splitting burrower
- **Landmark arcs (L):**
  - 8 World-turning guardian
  - 10 Ascension quest
  - 6 Regrowing heads
  - 23 Deep biomes
  - 36 and 37 Planet puzzles and cores

Open questions for the owner:
- Do world-phase changes (8) and material remaps (25) fit the save policy?
- Which realms count as "hostile" for proposal 12?
- Should the cipher (13) ever be explained in-game?

---

## Sources

- Twilight Forest: [GTNH notes](https://crazedd0ge.github.io/gtnhnotes/twilight-forest), [minecraft-guides](https://www.minecraft-guides.com/mod/the-twilight-forest/), [Craft Down Under guide](https://craftdownunder.co/guides/mods/twilight-forest)
- Betweenlands: [Basic Guide to Survival](https://the-betweenlands.fandom.com/wiki/Basic_Guide_to_Survival), [CurseForge](https://www.curseforge.com/minecraft/mc-mods/angry-pixel-the-betweenlands-mod)
- Alex's Caves: [wiki.gg biomes](https://alexscaves.wiki.gg/wiki/Biomes), [CurseForge](https://curseforge.com/minecraft/mc-mods/alexs-caves)
- Cataclysm: [CurseForge](https://www.curseforge.com/minecraft/mc-mods/lendercataclysm), [minecraft-guides](https://www.minecraft-guides.com/wiki/cataclysm/)
- Blue Skies: [minecraft-guides](https://www.minecraft-guides.com/mod/blue-skies/), [Modrinth](https://modrinth.com/mod/blue-skies)
- Deeper and Darker: [Otherside wiki](https://www.github.com/KyaniteMods/DeeperAndDarker/wiki/The-Otherside), [CurseForge](https://www.curseforge.com/minecraft/mc-mods/deeperdarker)
- Aether: [minecraft-guides](https://www.minecraft-guides.com/mod/the-aether/), [Modrinth](https://modrinth.com/mod/aether)
- Mowzie's Mobs: [Wroughtnaut](https://www.minecraft-guides.com/mod/mowzies-mobs/wiki/ferrous-wroughtnaut/), [Frostmaw](https://www.minecraft-guides.com/mod/mowzies-mobs/wiki/frostmaw/)
- Vintage Story: [Temporal stability](https://wiki.vintagestory.at/Temporal_Stability), [Rifts](https://wiki.vintagestory.at/Temporal_rift), [Translocator](https://wiki.vintagestory.at/Translocator)
- Seed of Andromeda: [ModDB](https://www.moddb.com/games/seed-of-andromeda/news)
- Outer Wilds: [GDC curiosity-driven exploration](https://gdconf.com/article/attend-gdc-and-learn-how-outer-wilds-nailed-curiosity-driven-game-design/), [Game Developer](https://www.gamedeveloper.com/design/live-die-repeat-how-i-outer-wilds-i-piques-curiosity-in-an-ambivalent-solar-system)
- Noita: [Fungal Shift](https://noita.wiki.gg/wiki/Fungal_Shift), [Holy Mountain](https://noita.wiki.gg/wiki/Holy_Mountain), [Sun Quest](https://noita.wiki.gg/wiki/Sun_Quest), [Eye Messages](https://noita.wiki.gg/wiki/Eye_Messages)
- Terraria: [Wall of Flesh](https://terraria.wiki.gg/wiki/Wall_of_Flesh), [Eater of Worlds](https://terraria.wiki.gg/wiki/Eater_of_Worlds)
- Breath of the Wild: [Thumbsticks GDC 2017 report](https://www.thumbsticks.com/gdc-17-breath-of-the-wild-science-lies), [GamesBeat](https://gamesbeat.com/the-legend-of-zelda-breath-of-the-wild-makes-chemistry-just-as-important-as-physics/)
- Existing local notes: `docs/research/reference-noita.md`, `docs/research/mod-family-dimensions.md`, `docs/research/mod-family-fauna-bosses.md`
