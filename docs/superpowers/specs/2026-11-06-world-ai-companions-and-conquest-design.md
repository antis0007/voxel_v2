# World AI, companions and conquest: design

Status: design, 2026-11-06. Owner direction: adapt the villager/civilization work to the
first-person voxel game so it adds a hyperintelligent world, RimWorld-style companions and
conquerable civilizations, without weakening the first-person modpack feel.

> **Superseded in part** by [Axiom: persistent civilization simulation](2026-11-06-axiom-direction-persistent-civilization-simulation.md)
> (2026-11-06): the "hyperintelligent world AI" as a large neural brain is replaced by a
> compact **shared policy** plus **persistent individual memory** and explicit events. The
> three tiers and the one-owner-per-quantity rule still hold.

It extends, and must agree with:

- [Civilization on solved geography](2026-11-05-civilization-on-solved-geography-design.md)
  (one record per place, one city across simulation levels, bounded history).
- [Villager civilization slice: plan](../../plans/2026-11-06-villager-civilization-slice.md)
  (the settlement record and its slices).
- [Axiom: from solved geography to persistent consequences](../../plans/2026-11-05-axiom-causal-world.md)
  (the province rules at the boundaries).

Reuse words: `L` = legacy substrate exists, `P` = design only.

## 1. The four wants, written as acceptance constraints

The engine already is a first-person builder/explorer with factory automation, wand magic,
wildlife and multiplayer. The new work must satisfy all four at once; when they conflict,
the order below is the tie-break.

| # | Want | Constraint it imposes |
|---|---|---|
| F1 | **First person is never worse** | No new camera, no forced control, no per-frame cost the player feels, no dependency of the modpack loop on the world AI. This is the strongest constraint; the rest yield to it. |
| F2 | **Hyperintelligent world AI** | A world that reads the land, keeps knowledge, sets goals, and acts through the same stocks, routes and jobs the player uses. Legible and bound by logistics, not omniscient. |
| F3 | **RimWorld-style companions** | A small roster of individually simulated people with needs, mood, traits, skills, injuries and relationships, who follow, work, fight and can be kept alive. |
| F4 | **Whole civilizations, places to conquer** | Real settlements with districts, production and ownership, that can be traded with, allied, besieged and taken as the player's own holding. |

## 2. The rule that protects the first-person game

Everything here is **player-centric and opt-in**. Concretely:

- **Player-centric LOD.** Simulation detail follows the player. A settlement you are standing
  in runs at fine detail; one across the map answers map queries from its record and does no
  per-tick work near you. Off-screen companions run at a reduced rate; off-screen
  civilizations run on a slow cadence.
- **Bounded work per step.** Every pass takes the existing `Budget` (engine_core). Wall time
  may yield to the next pass; it never selects a committed outcome.
- **Async, ordered.** Heavy work (plans, pathing, migration) runs off the tick and commits in
  canonical order. The player never sees a hitch from the world thinking.
- **Opt-in cost.** Companions are recruited, not spawned: no companions means no companion
  cost. World civics can be pinned to a world mode for players who only want the sandbox.
- **The player stays a participant.** Modpack loop preserved: build, explore, fight, automate.
  The AI adds inhabitants and stakes, it does not replace the loop with a strategy layer.

This is the same posture the engine already takes for weather (`day_night`), goals HUD and
the adventure layer: systems that read the world and never block it.

## 3. Three tiers, and one owner per quantity

| Tier | Granularity | Lives in | Owns | Cost model | Existing anchor |
|---|---|---|---|---|---|
| Companion | individual pawn | app runtime | needs, mood, skills, injuries, relationships, current job, carried item | a handful of pawns, ticked every step while near the player; reduced off-screen | `crates/app/src/world/travel/residents.rs` (named ids), `crates/app/src/world/adventure/` (animation, combat, mobs) |
| Settlement | record + jobs | app runtime (proposed `crates/app/src/world/settlement/`) | districts, buildings, residents, stock, ledger, growth | coarse cadence; fine only near the player | `crates/worldgen/src/households.rs`, `crates/worldgen/src/households/settlement.rs`, `crates/app/src/world/farming/`, `crates/app/src/world/travel/trades.rs` |
| World brain | region/faction | app runtime | knowledge, goals, orders, commitments | slow cadence, event-driven | `crates/worldgen/src/sites.rs`, `crates/worldgen/src/fernwake.rs`, `crates/app/src/world/regions.rs` |

The tier boundary is the whole design: individual pawns where the player sees faces,
records where the player sees a place, a brain where the player sees a region. The
civilization design's "one owner per quantity" applies tier to tier; a summary is derived or
owns a distinct declared stock, never a second writer of the same number.

Note the vocabulary already in the tree, so nothing is overloaded: `world/mood.rs` is sky and
weather, not a pawn's mood; `world/goals.rs` is the player's next-goal HUD, not a world goal.
This design adds pawn mood and world goals under their own names.

## 4. Companions (F3), concrete

A companion is a pawn, not a villager record. Small roster (start 1-4, cap 8), each a full
individual.

Pawn (plain struct and a stable `u32` id; ADR 0010, no ECS):

- identity: id, name (from `assets/*.ron`), kind, portrait seed.
- needs: hunger, rest and mood (0..1) with per-step rates; injuries.
- traits: two or three tags that bias work and mood.
- skills: one level per work type (haul, build, farm, cook, mine, hunt, guard, craft,
  research, socialize), raised by doing.
- relationships: sparse opinion edges to other pawns and a trust value toward the player.
- equipment: item ids and knowledge flags.
- job: the current task, its steps and reservations.

The loop (RimWorld-like but bounded): every N steps the pawn picks the highest-priority work
type from a per-pawn priority table, finds a target in the loaded area, reserves it, walks,
performs, repeats. It interrupts on danger, hunger or a player order. Mood from recent events
(ate, slept, bled, a friend died, a victory) drives thresholds; below a floor the pawn refuses
work or flees.

Player interface, all optional: recruit at a settlement; follow or hold; a bounded order (move
here, mine this, build that, guard here); assign a small work area; equip; treat a wound;
dismiss. Reuses the adventure layer's animation and combat and the named ids in
`crates/app/src/world/travel/residents.rs`.

Off-screen rule (declared): a companion outside the loaded area holds its job and its needs
pause at a floor, so they neither starve nor die off-screen unless a world mode says
otherwise. Same bounded-progress rule the province plan uses.

Co-op: pawns are authoritative on the host and replicate through the existing netcode entity
snapshots (ADR 0040 direction). A joiner sees them; only the host ticks them.

## 5. World brain (F2), concrete

"Hyperintelligent" here means a sound, legible reasoner over a small state that acts through
real logistics, never a scripted omniscient AI.

- Knowledge per faction or region: known places (settlements, sites, resources, routes) with
  their last-known state and staleness. It learns by scouting, trade and report, and forgets.
  Randomness is positional hashing from the world seed.
- Goals: a short list (grow, feed, defend, trade, raid, build, repair) scored from knowledge
  and need. Each goal names the owned quantity it will change.
- Planner: emits orders as data (raise a caravan, send a patrol, found or repair a site,
  migrate people). Orders consume real stock and people through the settlement records; a
  shortfall cancels or defers the order. Slow cadence, bounded per step.
- Legibility: every order carries a why (goal, evidence, cost), matching the engine's culture
  of explicit inputs and outcomes.

The brain never fixes a shortage by conjuring inventory; it moves what exists or sets a goal
the settlement tier satisfies through jobs. That keeps it believable and keeps it cheap: it
does nothing when nothing near the player is in scope.

## 6. Civilizations and conquest (F4), concrete

Built on the settlement record (the slice plan) plus ownership.

- Ownership: each settlement has an owner faction and a claim state (held, contested, vassal).
- Conquest loop: a force reduces defence (walls, gates and guards already exist as sites and
  hall/hell gates); on occupation the record's owner flips, stock is taken, some residents
  flee or submit, production continues under the new owner with unrest.
- Places to conquer for your own: a taken settlement becomes the player's holding; the player
  sets its priorities; it still grows, trades and can be attacked back.
- Everything persists: ownership, damage, losses and player construction survive save and
  reload in both views (the province plan's Persistence check).

## 7. What keeps it from detracting

- A hard per-step budget per tier: companions cost only when recruited and near, civics only
  when in scope, the brain only when it has a due order.
- No new per-frame loops in `main.rs`; new passes follow the existing `simulation::fixed`
  order and take a `Budget`.
- No `Instant::now()` in selection; fixed logical intervals only.
- All new state is versioned and co-op-safe; no second writer of an owned number.
- Companions and civics are configurable (world mode / settings), so a pure sandbox run is
  unchanged.

## 8. Slices (small, testable)

| Slice | Deliverable | Acceptance |
|---|---|---|
| S0 pawn core | pure pawn model, priority loop, reservations; no app coupling | unit tests: needs fall, priority picks the right job, reservations are unique, output is deterministic from a seed |
| S1 one companion on foot | recruit one; follow plus one haul job in first person | capture: it follows, hauls and survives a save and reload |
| S2 mood, traits, skills, injuries | thoughts and thresholds; skill gain from use; wound treatment | scenario: low mood refuses work; skill rises with use; a treated wound heals |
| S3 settlement record | the slice plan's S0 and S1 (record plus bind one village) | as that plan |
| S4 world brain v0 | one faction: grow, feed, defend, trade as orders with a why | two seeds diverge on one start state; a real shortfall cancels an order |
| S5 conquest | claim one site through play | per the province Conflict check; ownership persists |
| S6 co-op companions | host ticks, joiner sees | verify host, join and rejoin; no double tick |
| S7 civ growth and decline | stocks drive occupied homes and physical traces | run to growth and decline; counts match the ledger |

S0 is pure logic and can be built and tested now with no world or GPU. S1 is the first slice
the player feels. S3 reuses the settlement plan already written.

## 9. Governance, risks, open questions

- Crate home: a new pure `crates/civ` (core-only, like `crates/sim`) or
  `crates/app/src/world/settlement/`. A new crate is not a new external dependency, but it
  changes `Cargo.lock`, which routes to the policy gate and needs an owner line.
  Recommendation: prototype pure, land in `crates/app` first, promote to `crates/civ` once
  the record is stable. Do not add the crate while the tree is red from peer edits; the rust
  and policy routes would fail for reasons unrelated to this work.
- Do not touch `crates/app/src/world/adventure/growth.rs` or
  `crates/app/src/egui_ui/wand_xmb.rs` (in-progress, per CLAUDE.md).
- Risks: companions costing frames (roster cap, off-screen reduction, budget); the brain
  raising cost (slow cadence, event-driven, in-scope only); scope creep into an RTS (F1 order
  of precedence).
- Open questions: companions per player and cap; crate home; the off-screen safety rule;
  whether conquest is in this pass or follows the province plan; whether the world brain is
  per-faction, per-region or both.


