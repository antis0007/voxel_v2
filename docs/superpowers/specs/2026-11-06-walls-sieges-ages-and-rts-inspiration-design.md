# Walls, sieges, epochs and other strategy inspirations: design

Status: design, 2026-11-06. Owner direction: bring Total War: Warhammer sieges, Songs of Syx
cities, Civilization tech and politics, and Age of Empires 2 and 4 features (walls, gates,
towers, ages, counters, wonders, relics) into the first-person voxel world, for fun, without
losing the modpack feel.

Extends, and must agree with:

- [Conquest, politics, the map and the playstyle paths](2026-11-06-conquest-politics-and-playstyle-paths-design.md)
  (the strategy surface and the F1 rule).
- [World AI, companions and conquest](2026-11-06-world-ai-companions-and-conquest-design.md)
  (three tiers, one owner per quantity).
- The four product pillars in `docs/NEW/UNDERKINGDOM_COMBINED_SPEC.md`, especially "matter is
  real" and "systems over scripts".

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 1. What each reference contributes, and what we refuse to copy

| Reference | Take | Refuse |
|---|---|---|
| Age of Empires 2 | tiered walls (palisade to masonry), gates, towers, town centre, four ages, a resource economy, the unit counter triangle, wonders, relics, market | ages with no physical reason; HP inflation as "a new age" |
| Age of Empires 4 | stone walls with gates and outposts, a **landmark choice** at each age-up, stealth forest, sacred sites, influence, siege | a separate instanced battle map |
| Total War: Warhammer | provinces with city management, **layered sieges** (ladders, ram, breach, street fighting), army stances, agents | the campaign/battle split; separate tactical map |
| Civilization | a tech tree, city-states, culture and wonders, districts | turn-based abstraction laid over a real voxel world |
| Songs of Syx | cities of rooms, jobs and delegated labour, population and logistics | (already reverse-engineered separately) |

The through-line: every feature must be **physically present** in the one world, buildable or
breakable on foot, and legible. Nothing is a menu-only abstraction.

## 2. What exists (audited 2026-11-06)

| Piece | Where | State |
|---|---|---|
| Buildable blocks and materials (walls are voxels today) | `assets/materials.ron` (317 KB), `crates/content/src/lib.rs` (`BlockId`) | L |
| Capability branches over the recipe graph (the tech substrate) | `crates/app/src/world/technology.rs` (`Branch`), `assets/progression.ron`, `crates/app/src/world/progression.rs` | L |
| Relics and reliquaries | `crates/app/src/world/adventure/reliquaries.rs` | L |
| Realm portals (not fortifications) | `crates/app/src/world/portals.rs`, `crates/app/src/world/hall_gate.rs`, `crates/app/src/world/hell_gate.rs` | L |
| Combat, mobs, spells, tactics | `crates/app/src/world/adventure/` | L |
| Trade, caravans, travel | `crates/app/src/world/travel/` | L |
| Fortifications, territory, wonders, unit counters, conversion, stealth | searched across `crates/app/src` | Absent (`rampart`, `moat`, `territory`, `wonder`, `monk`, `stealth` all zero) |

So walls can already be built from blocks, and tech already exists as branches. What is
missing is the **fortification record**, **territory**, **epochs gating content**, and the
**counter/siege/wonder** layer that gives walls a reason to be attacked.

## 3. Fortifications: walls, gates, towers, moats

The wall is **two things at once**, with one owner each so the tiers never disagree:

- The **body** is voxels (any material). It renders, collides and navigates like terrain.
- The **record** is a compact fortification entry (grade, defence, gate state, breaches) so a
  siege, the overhead view and the map never read voxels. A breach writes both: it removes the
  voxels and lowers the record.

**Wall grade comes from the material.** `assets/materials.ron` already gives hardness and
other columns; a grade (light, masonry, reinforced) is a content row over material, not a new
noun or a new block. Upgrading a wall is replacing blocks, which is exactly what the game
already lets a player do by hand.

**Gates** are a buildable span with a state:

| State | Navmesh | Defence | Notes |
|---|---|---|---|
| Open | portal (walk through) | low | trade and civilians pass |
| Closed | barrier | high | the default when threatened |
| Locked | barrier | high | only the owner's side opens it |
| Breached | permanent opening | none | a repair job restores it |

A gate is the seam between my world-AI navmesh (portals, §10) and defence: opening a gate is
what lets an army walk in, which is why a siege is about the gate and the wall, not a separate
map.

**Towers and outposts** are structures with an attack radius, line of sight through the octree
raycast, and room for a small garrison (companions and levies). They auto-fire at a besieger
inside their radius. Guard behaviour reuses the adventure combat.

**Moats** are voxel water with a navigation cost; a bridge is the only dry crossing, so it is
the water equivalent of a gate. Water already simulates in the tree, so a moat is a
construction, not a new system.

**Siege** is equipment against the record. Attackers bring a small set of counters (a ram
against a gate, ladders or a breach engine against a wall, an engine against a tower), each a
content row with a siege strength. Each tick of contact reduces the target's defence; at zero
the body is removed and the record flips to breached. Defenders repair between assaults (a job
above). This resolves on the fixed tick like every other system, so it is deterministic and
testable headless.

## 4. Territory and control (so a wall means a border)

- **Influence.** Structures project a radius: a keep or outpost claims more than a tower, a
  tower more than a wall. Claimed cells are **derived** from influence plus walls, so there is
  one writer (the influence fields) and the claim is a read.
- **Walls define hard borders, influence defines soft borders.** A wall does not need to be
  painted: control follows the built line. This is the AoE/Civ feeling (control the pass,
  control the crossing) expressed through real geometry.
- **The map shows territory.** A new layer over `crates/app/src/egui_ui/map.rs` and the
  prototype `worldmap` model: owner tint, contested hatches, walls as lines. Read-only, like
  every other map layer.
- **Roads and terrain bias control** (a pass, a ford, a bridge) so geography matters, matching
  the civilization design's "places have causes the player can read from the land".

## 5. Epochs (ages) with a landmark choice, built on the Branch graph

- **The tech already exists as branches.** `crates/app/src/world/technology.rs` defines
  `Branch` (Building, Equipment, Metallurgy, Power, Logistics, Processing, Biology, Control,
  Arcana) over the live recipe graph. Epochs sit **on top of that**, they do not replace it.
- **An epoch is a named capability tier** with prerequisites and unlocks: fortification grade,
  unit roles, machines, magic capabilities. Epochs are content rows, and the cost to advance
  is resources plus a completed **landmark**.
- **Landmark choice (AoE4).** Advancing offers two or three structures; building one grants a
  distinct bonus and is the physical seat of the new epoch. This turns an age-up into
  something you can walk to, defend and lose, instead of a button.
- **Naming is a content decision.** The game already has ages, catastrophes and three
  historical strata (`docs/NEW/UNDERKINGDOM_COMBINED_SPEC.md` §3). Epochs must reuse or
  reconcile with those, and any new name needs owner approval. Treat the epoch names in any
  prototype as placeholders.

## 6. Age-of-Empires-style systems, for fun

| Feature | How it lands here | Substrate |
|---|---|---|
| Resource economy (food, wood, stone, gold analogue) | map AoE's four to existing materials and goods via content rows; no new currencies | `assets/materials.ron`, `assets/recipes.ron` |
| Town centre and drop-off | the settlement's storage is the drop-off; a town-centre building anchors influence | settlement record (world-AI design) |
| Population cap from housing | housing limits people exactly as the settlement record already does | prototype `settlement` |
| Unit roles and counters | line (reach), missile (ranged), shock (fast, heavy), siege (engines), with an AoE-like multiplier matrix | companions + levies; `crates/app/src/world/adventure/` combat |
| Siege units (ram, engine) | counters with a siege strength against a target class | new content rows, §3 |
| Wonders | a multi-stage build with a payoff and a defensive requirement while it stands | settlement record + build jobs |
| Relics and sacred sites | hold a relic or site for a trickle of a resource or a bonus | `crates/app/src/world/adventure/reliquaries.rs` |
| Conversion | a magic capability that turns a unit, i.e. the monk, as a spell rather than a new class | `crates/app/src/world/adventure/wands.rs`, `rare_magic.rs` |
| Stealth forest | foliage reduces detection range | octree line of sight; `crates/worldgen/src/trees.rs` |
| Trade routes and caravans | income along roads you hold | `crates/app/src/world/travel/`, world-AI caravans |
| Stances and formations | aggressive, defensive, hold; loose and tight, per company | company data (conquest design) |

**The pillar test applies.** Everything counts must be explainable physically (armour versus
impact, reach versus speed, water versus force), not an arbitrary multiplier, so "matter is
real" survives contact with an RTS counter chart. Where a counter cannot be explained, it is
cut.

## 7. Keeping first person first (F1)

Every feature here is playable on foot, and the strategy surface stays opt-in:

- You can lay a wall block by block, cut a gate open with a spell, man a tower with a
  companion, or walk a breach; and you can also command the same wall from the overhead view.
- The map and territory layer are read models; closing the map costs nothing extra.
- A siege resolves on the fixed tick, so a player standing in the street sees the same event
  the map shows, not a cut to a separate battle scene.
- Budgets bound every new pass (fortification, influence, siege), so a wall the player is not
  near is a record read, not per-frame work.

## 8. Slices (small, testable)

| Slice | Deliverable | Acceptance |
|---|---|---|
| W1 fortification core | pure record: grade, defence, gate state, breach; a siege tick reduces defence and breaches at zero | unit tests: a ram breaches a gate; a stouter grade needs more siege; repair restores |
| W2 wall and gate in the world | a built wall and a gate as voxels with a navmesh portal when open and a barrier when closed | walk through an open gate; a closed gate blocks; capture |
| W3 territory from influence | influence fields derive claimed cells; the map shows owners and contested cells | two structures project two regions; a wall line follows the built blocks; capture |
| W4 epochs and a landmark | advance an epoch by building one landmark; a new unlock becomes available | advancing without the landmark fails with a why; the bonus matches the row |
| W5 unit roles and counters | the role counters resolve one skirmish, physically explained | spear beats shock, missile beats line, each with the stated physical reason |
| W6 siege equipment vs a wall | ram, engine and breach against a wall, a gate and a tower, with repair | the wall falls only when its defence reaches zero; no voxel and record disagreement |
| W7 wonders, relics, routes, stances | a staged wonder, a relic trickle, road income, a company stance | each is a content row with a headless scenario; no new currency |

Gate for each: the repo pre-commit path (`just check`, `just clippy`, `just test <name>`, a
headless scenario, then `just verify`) plus captures for any wall, gate or map change.

## 9. Governance, risks and open questions

- New systems (fortifications, territory, epochs, counters) need their own SDDs by the master
  plan's protocol. Names are content decisions; nothing here invents a proper noun.
- The fortification body is voxels, so meshing and navigation must stay inside budget when a
  player builds a long wall (reuse the incremental navmesh rebuild from the world-AI design,
  not a full rebuild).
- Risks: drift into an RTS (mitigate: F1 and the opt-in surface, §7); counters fighting the
  physical-sim pillar (mitigate: the pillar test in §6, cut what cannot be explained); epoch
  names clashing with the existing ages and strata (reconcile before naming).
- Open questions: is a wall a free player build or does it need a fortification tool; do
  epochs gate content the game already unlocks through branches (if so, epoch gating may be
  light); is territory one layer per faction or a single owner map; do towers auto-fire or
  only with a garrison.



