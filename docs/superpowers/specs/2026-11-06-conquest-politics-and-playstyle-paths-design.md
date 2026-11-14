# Conquest, politics, the map, and the playstyle paths: design

Status: design, 2026-11-06. Owner direction: Total War-style conquering whose results show on
the player's map, with 4X politics and benefits, inside a world you can also play as a factory
engineer, a hunter, a wizard or a dark mage.

Extends, and must agree with:

- [World AI, companions and conquest](2026-11-06-world-ai-companions-and-conquest-design.md)
  (the three tiers, one owner per quantity, the F1 rule).
- [Civilization on solved geography](2026-11-05-civilization-on-solved-geography-design.md)
  (one record per place, three views share one world).
- The four product pillars in `docs/NEW/UNDERKINGDOM_COMBINED_SPEC.md` (habitation,
  exploration, invention and automation, interaction and danger).
- The parked playstyle contract in `docs/parked/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md`,
  kept as reference: no playstyle is the mandatory prerequisite for another, and traders and
  factions give genuine conversion routes between them.

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 1. The promise, and the one rule

One continuous world, played any of several ways. Conquest and politics are **a ladder, not a
mode**: a player who only wants to automate or to delve never has to touch them, and a player
who wants to rule never has to leave the first-person game to do it.

The rule from the world-AI design still governs: **F1, first person is never worse**. The
strategy surface is an optional view and a set of tools over the same records. It adds no
camera the player is forced into, no per-frame cost while closed, and no dependency of the
sandbox loops on it.

## 2. What exists (audited 2026-11-06)

| Piece | Where | State |
|---|---|---|
| World map with persisted knowledge, minimap and a knowledge sidecar | `crates/app/src/egui_ui/map.rs`, `crates/app/src/egui_ui/map/knowledge.rs`, `crates/app/src/egui_ui/map/minimap.rs`, `crates/app/src/egui_ui/map/sidecar.rs` | L: a discovered-places map already shows on the map screen |
| Overhead camera and survey | `crates/app/src/main.rs`, `crates/app/src/world/expedition.rs` | L |
| Automation path (Factorio-grade) | `crates/app/src/world/factory.rs`, `assets/machines.ron` | L |
| Magic, arcana, rare magic | `crates/app/src/world/adventure/wands.rs`, `crates/app/src/world/adventure/arcana.rs`, `crates/app/src/world/adventure/rare_magic.rs` | L |
| Wildlife, husbandry, mounts (the hunter's ground) | `crates/app/src/world/adventure/wildlife.rs`, `crates/app/src/world/adventure/wildlife/husbandry.rs` | L |
| Audience paths named | `docs/GAME_DESIGN.md` §1 (builder, engineer, mage, explorer, survivor, student, reader) | P: a list, not a system |
| Politics, diplomacy, ownership | searched across `crates/app/src` | Absent: no `diplomacy`, `politics`, `treaty`, `vassal`, `faction` or `conquer` |

So the map already exists and shows knowledge. What is missing is the **ownership, relations
and command layers** over it, and the **sovereign path** that uses them.

## 3. The strategy surface (Total War / 4X), built so it cannot hijack first person

- **Camera.** An optional overhead view, reusing the existing overhead camera. It never
  replaces first person: you can descend from the map into the same place and continue on
  foot. This is the civilization design's "three views share one world".
- **Map layers** over `egui_ui/map.rs`, each a read model, none a writer: Knowledge (exists),
  Ownership (who holds each place), Relations (ally, truce, at war), Resources (known
  surplus or shortage), Forces (armies and companions in the open), Objectives (your standing
  orders). Opening a layer costs only what the closed map already costs.
- **Command.** Select a holding or a force and issue a bounded order: move, besiege, garrison,
  build, tax, reinforce. Orders are the **same data the world brain emits** (the world-AI
  design §5), so the player and the AI run one system rather than two behaviours to keep in
  sync.
- **No APM.** Orders are rare and commit on the fixed tick. Time control (single-player pause
  and speed) exists but never forces the player to play the map.

## 4. Conquest, ownership and benefits

Built on the settlement record (claim states Held, Contested, Vassal) from the world-AI
design.

- **Casus belli first.** A war needs a recorded reason (border friction, a broken deal, a
  claim you inherited, protection of a client), so war is legible and so an unprovoked attack
  costs standing. This is the 4X discipline that keeps map-painting from being free.
- **Raise and supply.** A force costs real people and goods drawn from your holdings
  (companions and garrison are the individual tier; the supply is the settlement stock). A
  force with no supply cannot march far.
- **March, besiege, storm.** The map shows the force move. A siege reduces gate and wall
  equipment (the hall and hell gates already exist as sites). A storm resolves on the same
  tick as everything else.
- **Occupy, then choose.** On occupation the record's owner flips, a share of stock is taken,
  some residents flee or submit, and unrest rises. Then: **annex** (settle the claim, unrest
  falls, the place is fully yours) or **vassalise** (the old owner keeps it but owes tribute
  and follows your wars). The prototype already models the flip, the stock take and unrest.
- **Benefits of owning.** A share of production (resources), a levy of people, tax, safe
  harbour and repair access, trade priority, and a visible mark on the map. Every benefit is
  drawn from the owned record's real output, so owning a poor place is genuinely less useful.
- **For your own.** An owned holding is yours to build in and automate. That is the seam
  where the warlord and the engineer are the same player: you take a place and then make it
  produce.

## 5. Politics (4X)

A thin, legible negotiation layer over the relations the world brain already reads.

- **Relations.** Each faction pair holds opinion, trust and fear, updated by deals, wars,
  gifts and shared enemies. These are the numbers the world brain scores its goals against.
- **Instruments.** A small set, each a data row: gift, trade deal, non-aggression pact,
  defensive alliance, tribute demand, vassalage offer, ultimatum (with a deadline and a
  consequence), and declaration of war. Each carries obligations and a cost.
- **Offers and answers.** An offer names what each side gives and owes; the answer is
  arithmetic over stored facts plus the asker's goals, so the AI can propose and refuse with
  a **why** the player can read (consistent with the brain design's legibility).
- **Casus belli and standing.** Broken pacts, ignored ultimatums and unprovoked wars move
  standing, which other factions read. Being a reliable ally has mechanical value.
- **Determinism.** Offers, deadlines and acceptances are content rows plus arithmetic over
  saved facts. No wall clock decides an outcome.

## 6. The paths (one world, many ladders)

A path is an entry point into the same world, not a class and not a mode. The player moves
between them freely; none is gated behind another. This table extends the seven audience
paths already named in `docs/GAME_DESIGN.md` with the sovereign path this direction adds.

| Path | Entry (today) | Core verbs | Produces | How it meets conquest and politics | First slice |
|---|---|---|---|---|---|
| Builder / habitation | block placement, farming | build, farm, decorate | shelter, food, goods | a home base can become a holding you defend | exists |
| Engineer / automation | `crates/app/src/world/factory.rs` | automate, route, power | bulk goods that fund a war | supply a campaign; industrialise a place you took | exists |
| Mage | `crates/app/src/world/adventure/wands.rs` | cast, compose | field effects, utility | turn a siege; leverage in a deal | exists |
| Dark mage | `crates/app/src/world/adventure/rare_magic.rs` (emphasis) | manipulate, terrify, sabotage | fear, disruption | shift a faction's stance without an army | P6 |
| Explorer | `crates/app/src/egui_ui/map.rs`, `knowledge.rs` | travel, survey, delve | knowledge, maps, finds | intel on rivals; found claims to press | exists |
| Hunter | `crates/app/src/world/adventure/wildlife.rs` | track, tame, hunt | food, leather, mounts | scout a route; mount and feed a force | exists |
| Survivor, student, reader | `docs/GAME_DESIGN.md` §1 | endure, learn, read | — | — | later |
| Sovereign / warlord-governor | the map (this design) | command, conquer, govern | holdings, tribute, map presence | the Total War path itself | P4 |

**Conversion between paths.** Follow the parked brief: traders and factions are the seam
between playstyles. An explorer sells finds and maps; a factory buys maps, specialist
services and local access with produced goods; a warlord buys supply for a campaign; a mage
sells enchantment. No universal adventure currency and no infinite guaranteed buyback, so no
path can be skipped by grinding another.

## 7. What keeps it from detracting

- The strategy surface is opt-in and additive. Closed, it costs no more than the map screen
  already costs; the records it reads are owned by the settlement and world-brain tiers.
- No forced camera and no RTS pace: orders are bounded, rare and commit on the fixed tick.
- Map layers are read models. No gameplay outcome depends on opening the map.
- The sovereign path shares the settlement record, the job system and the world brain with the
  other paths, so there is one behaviour to keep correct, not two.
- Multiplayer: ownership, war and politics are host-authoritative like every other owned
  quantity; the map stays a per-player knowledge sidecar (`map/sidecar.rs`), so one player's
  explored knowledge is not another's.

## 8. Slices (small, testable)

These extend the world-AI design's S0-S7. The P slices are the strategy layer; S0 (the pawn
core) can land first because it needs nothing here.

| Slice | Deliverable | Acceptance |
|---|---|---|
| P0 politics core | pure relations, instruments, offer/answer and casus belli; no world coupling | unit tests: an acceptable offer is accepted, a refused one names why, a broken pact lowers standing, war needs a casus belli |
| P1 map layers | ownership and relations layers over the existing map knowledge, read-only | the map shows owners and stances for discovered places; no gameplay depends on it; capture |
| P2 one owned holding | make a place yours end to end and gain its real production | benefits track the record's output; a poor place is visibly less useful |
| P3 diplomacy with the brain | an AI faction proposes, refuses and declares with a why | two seeds diverge on one start state; the why matches the stored facts |
| P4 sovereign path v0 | raise a force from a holding, take a second place, see it on the map | per the province Conflict check; ownership and map update and persist |
| P5 path conversion | a factory buys a map or access; an explorer sells finds | no universal currency; no infinite buyback; both routes usable |
| P6 dark-mage lever | an influence action that shifts a faction's stance without a battle | the stance shift is readable and reversible; no new proper noun |

Gate for each: the repo pre-commit path (`just check`, `just clippy`, `just test <name>`, a
headless scenario, then `just verify`), plus captures for any visible map or HUD change.

## 9. Governance, risks and open questions

- Politics, diplomacy and ownership are new systems. By the master plan's protocol they need
  their own SDDs, and new factions or proper nouns need owner approval before they are named.
  These slices are P (design) until their SDDs exist.
- Do not touch `crates/app/src/world/adventure/growth.rs` or
  `crates/app/src/egui_ui/wand_xmb.rs` (in-progress, per CLAUDE.md).
- Risks: the game drifting into an RTS (mitigate: F1 and the opt-in surface, no APM); conquest
  benefits trivialising the automation or exploration paths (mitigate: benefits come from the
  owned record's real output and conversion stays trade-based); one path becoming mandatory
  (mitigate: the parked brief's rule, no gated cross-path prerequisites).
- Open questions: is the sovereign path an eighth path or folded under a broader "sovereign"
  heading; is conquest single-player-first, with co-op politicking later; how a vassal's war
  obligations bind a co-op party; which two or three paths get a first slice before politics.



