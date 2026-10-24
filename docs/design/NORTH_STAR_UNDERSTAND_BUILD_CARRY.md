# North star: understand it, build it, carry it

Date: 2026-10-22. Design intent, not current code: verify against source before acting.
Origin: a design pass over the inspirations (heavily modded Minecraft, Factorio, Noita,
real science, semiconductor fabrication). Every idea below carries a verdict so the next
reader can tell what was kept, bent, deferred or dropped. Lore-facing text still follows
`WORLD_LORE.md`; this file is mechanics and scope.

Verdicts: **Adopt** (build it), **Adapt** (build it, changed), **Have** (already in the
repo, pointer given), **Defer** (good, not before the named gate), **Scrap** (reasoned out).

## 1. The promise

If the player understands how something works, they can turn that knowledge into a
spell, a machine, a farm, or a piece of equipment they carry.

The loop, in order, and the order is the design:

**discover → experiment → use by hand → control → automate → miniaturise → reach somewhere new.**

Miniaturising is a first-class progression axis: a contraption that fills a workshop
becomes a module in a wand. This is the same ladder as `WORLD_LORE.md` "Future focus:
semiconductor tiers", seen from the player's side instead of the fab's.

Test for any major system: can one discovery feed combat, ecology, power, automation
*and* equipment? If it feeds only one, it is a recipe, not a system.

Worked example (keep as the canonical illustration): a cavern organism discharges when
disturbed → learn what triggers it and how the discharge travels → use it against another
creature → enclosure that collects the output → sensors and a feeding/discharge cycle →
compact controller → portable version as a defensive wand component.

## 2. Each discipline has a job (Adopt)

| Discipline | Strength | Player enjoys |
| --- | --- | --- |
| Materials / chemistry | what reacts, conducts, dissolves, retains heat, grows | finding useful properties |
| Magic | immediate, flexible intervention | improvising in exploration and combat |
| Machinery | sustained work, containment, throughput | dependable processes |
| Electronics | sensing, timing, feedback, coordination | making builds behave intelligently |
| Biology | growth, reproduction, regeneration | cultivating living systems |
| Enchantment | explicitly fictional properties with consistent rules | unusual combinations |

Rule: several methods solve one problem and each stays attractive for a different
reason. A heating spell and an electric furnace both heat a crucible; the furnace adds
insulation, steady supply and unattended running, the spell adds portability and
immediacy. A furnace heated by a mounted wand is legal: whether it is good depends on
wand power, supply and cost. That is the engineering question, so do not forbid it.

Maps to: Phase D "magic is a rule with a `Mana` condition; chemistry is a rule with a
`Temperature` condition" (`docs/PLAN2.md` D-1). Same schema, different condition.

## 3. Shared physical rules (Adopt, mostly Have)

One property table serves terrain, equipment, creatures, spells and machines. Insulating
is insulating in a furnace wall, a suit and a cave. A conducting liquid matters in a
cave, a vessel and a trap. This is already the direction of Phase A1 (properties in the
material table) and Phase C (conductivity from material).

| Process | Exploration and combat | Construction and production |
| --- | --- | --- |
| Heat, phase change | melt, freeze, extinguish, overheat | smelt, cast, cool, distil, grow crystal |
| Conduction, insulation | redirect discharge, isolate hazard | distribution, sensing, electrochemistry, motors |
| Flow, containment | drain, flood, move gas, exploit leaks | pumps, vessels, cooling loops |
| Light, optics | reveal, blind, beam triggers | measurement, signalling, patterned exposure |
| Structure, strength | break supports, brittle surfaces, temporary routes | pressure vessels, tools, protection |
| Growth, habitat | lure, nests, change habitats | agriculture, breeding, bioprocessing |
| Fields, stored mana | cast, bind, sense, summon | enchantment, actuators, field collection |

### Distinctions worth keeping (each creates a real choice)

- **Energy stored vs power delivered.** A big battery can still fail to run a hungry machine.
  (Have: SI units and battery banks, `POWER_MULTIBLOCKS_AND_RECIPES.md`.)
- **Temperature vs thermal energy.** A large cool reservoir and a tiny hot chip behave differently.
- **Signal vs the power it controls.** A sensor tells the pump to run; the pump still needs energy.
- **Catalyst vs consumed ingredient.**
- **Purity vs composition (doping).** Removing contamination and adding a chosen one are different operations.
  Pure silicon is not "a better conductor". (Have: purity in nines, `WORLD_LORE.md` process vocabulary.)

Electrolysis is the model example: water in, hydrogen and oxygen out at 2:1, so the
player can reason about where energy and material went. Prefer reactions that balance on
inspection.

### Magic has explicit sources and limits (Adapt)

Every effect names what sustains it, and the player can inspect what happens when that
supply stops. A cooling spell moves heat into a reservoir the player carries. A
matter-making spell draws from a defined resource. A growth spell speeds a process and
raises its nutrient demand. Spell = short burst, ritual = area upkeep, machine = steady
throughput; a good build does all three.

Connects to the existing "Superconducting magic: the hard rule" in `WORLD_LORE.md`.

## 4. Wand construction as engineering (Adapt)

Existing spell table and deck evaluator stay (`SPELLS_AND_DISCOVERY.md`, `wands.rs`).
The new reading is a vocabulary for *what a card is*, useful for naming and for the
future rules schema:

| Part | Examples |
| --- | --- |
| Action | heat, cool, push, transfer charge, light, move fluid, stimulate growth |
| Delivery | contact, projectile, beam, cone, persistent field |
| Condition | on impact, after delay, above a temperature, when a sensor fires |
| Modification | split, repeat, focus, widen, redirect, regulate |
| Supply | mana, stored energy, carried material, connected reservoir |

Idea to keep: **Condition cards that read sensors.** Electronics widens what a spell can
respond to; manufacturing improves how compact and reliable the response is. A
cooling bolt gated on "surface above X degrees" mounted beside a crucible *is* a
thermostat. The same optical detector can open a door, arm a trap, sort items, trigger
a wand, or hold a machine until the workpiece arrives.

Wand housing, conductor, lens, reservoir and controller each contribute a functional
property (Tinkers-style), so one set of parts builds a rugged industrial tool, a
precision instrument or a combat wand. Keep inspectable presets: start from something
useful, change one part at a time.

Status: Defer to Phase D (rules schema). Do not add sensor conditions as Rust special
cases before then.

## 5. Electronics useful early (Adopt)

Players meet control long before chips. Ladder:

| Stage | Tools | New capability |
| --- | --- | --- |
| Direct | levers, valves, shutters, mechanisms | operate by hand |
| Regulation | floats, switches, simple sensors, relays | hold one condition |
| Circuit control | comparators, timers, counters, memory | coordinate several conditions |
| Compact | packaged sensors and controllers | fit behaviour into portable gear |
| Specialised fab | custom devices and ICs | optimise size, sensing, power, timing, tolerance |

This solves the bootstrap: the first fab equipment runs on mechanical control, relays
and salvage, which matches tiers 0 to 2 in the lore ladder.

Three audiences, one set of ports, signals and devices:
the ordinary player installs a thermostat and picks a range; the builder wires
temperature, level and power availability together; the enthusiast opens the schematic,
edits the logic and fabricates a variant.

Starter modules: thermostat, liquid-level controller, timer/pulse generator, counter,
light detector, motor controller, alarm and emergency stop, sequencer.

Feedback rule (from Factorio's circuit redesign): **show live signal values on the device
and on the look-at panel.** First-person cable tracing is the main frustration; the
default inspect view shows inputs and explains the output. Maps to Phase C
(`logic_gate` scenario) and the existing look-at panel.

## 6. Semiconductor fabrication as payoff (Adopt, extends the lore ladder)

The payoff is manufacturing *your own invention* as a compact, useful component with an
identifiable function: a light sensor for a trap, a power switch for a pump or mining
tool, a wand sequencer, a habitat controller, a low-power remote monitor.

### Separate five variables (this is the core of the fab design)

| Variable | Meaning in play |
| --- | --- |
| Purity | uncontrolled contamination left |
| Doping | intended electrical behaviour of chosen regions |
| Pattern | the device or circuit being built |
| Process consistency | how closely parts match the design |
| Packaging | how the part connects to and survives its surroundings |

Refining, process control and circuit design become three activities that reinforce
each other. Tier gates in `WORLD_LORE.md` stay as they are.

### Compressed player-facing stages

Prepare material → prepare substrate → choose a design (reusable pattern) → build
patterned layers (arrange equipment, supply, conditions) → inspect → separate, package,
test. Different device families use different process programs; machines repeat the
steps. The player's work is arranging and improving the system, not clicking each step.
The full real vocabulary stays in `WORLD_LORE.md` "Real chipmaking".

### Environment matters, a little at a time (Defer)

A forgiving sealed workshop makes forgiving parts. Later buildings can separate a dirty
receiving/refining area, sealed transfer, a controlled room, separate cooling and power,
inspection stations and automated handling. Particles are a defensible difficulty
source; the Bloom already gives cleanliness a story reason. Introduce a variable only
when it creates an improvement the player can see. Start with two or three.

### Yield must be explainable (Adopt)

A process report names a cause, not a percentage:

> 24 of 32 devices passed. Third patterning step lost alignment after the exposure
> tool's supply dropped.

That gives levers: improve supply, add storage or regulation, change the schedule, slow
down, use a more forgiving design, fit a stabiliser. Hard rules:

- Failures leave evidence. Variation is deterministic from the batch seed
  (`engine_core::hash`), so reloading a save never rerolls a batch.
- A defective circuit is not a slower circuit. Test sorts parts into working,
  salvageable-with-reduced-function, and recycle.
- Yield state is saved process state (save-format rule: modes and fields, not new kinds).

### Branches stay useful (Adopt)

Mature nodes keep real advantages (analog and power parts often prefer them; smaller is
not strictly better). Lasting specialisations, each with a reason to exist after the
ladder is climbed:

| Branch | Reward |
| --- | --- |
| Power electronics | drive motors, pumps, heaters, converters |
| Sensors, analog | measure physical conditions accurately |
| Digital logic | complex behaviour in compact gear |
| Optoelectronics | detect and send light, needs suitable materials |
| Packaging | compact, sealed, cooled or rugged assemblies |
| Magical interfaces | turn a defined magical phenomenon into an ordinary signal |

Packaging has real gameplay value: a controller for a flooded ruin, a tiny wand and a
hot foundry want different packages. Rule from section 12 applies: no uniformly
superior tier.

## 7. Factories use three-dimensional space (Adopt, partly Have)

- Vertical consequences: gravity-fed handling, elevated reservoirs, ore chutes and
  sorting towers, rails across ravines, underground thermal storage, dirty/clean floors,
  cooling at local water, optical paths needing clear sightlines.
- Land is a design prompt. A waterfall, cavern or tower should make a particular
  factory attractive. The player should sometimes look at terrain and think "I could
  build something good here."
- Functional assemblies: chamber, heat source, insulation, ports, instruments,
  controller; the player varies capacity, placement and support. **Common devices stay
  convenient** (a small pump or standard controller never needs a construction ritual);
  big processes get the spatial design.
- Logistics each have a purpose, readable by throughput, capacity, distance, setup and
  flexibility: belts/chutes (local solids), pipes (fluids, gases), rails (between
  settlements), lifts (vertical), mobile carriers (flexible, power and capacity bound),
  magical transport (odd routes with explicit cost).
- By-products are opportunities, not punishment: waste heat warms a greenhouse, residue
  feeds a creature, reclaimed water rejoins a cooling loop. Reward discovering the link;
  do not make every line a waste chore.

Have: Railcraft-style multiblocks and Factorio-style automation (`POWER_MULTIBLOCKS_AND_RECIPES.md`).

## 8. A living wilderness (Adopt)

Every region offers a distinct mix of conditions, materials, organisms, architecture,
hazards and recoverable knowledge. Sketches (candidate Undercrown or surface leads; fill
the five-part lore slot before building any):

| Region | Opportunity |
| --- | --- |
| Flooded observatory | recover optics, restore drainage, use existing channels (see section 11) |
| Fungal cistern | cultivate, manage humidity and water, harvest bioproducts |
| Volcanic foundry | abundant heat; equipment survival and cooling |
| Storm marsh | study discharges, insulation, storage, electric creatures |
| Astral cavern | measure explicitly fictional field behaviour |
| Self-growing tower | learn how its build process senses, consumes and deposits |

A ruin teaches a technique through its machinery: a broken regulator, a disconnected
channel, a damaged optical assembly is something to investigate and repair.

**Creatures are participants in systems.** Each declares: what it senses, conditions it
prefers, what it consumes, what it outputs, how it changes its environment. A
mineral-feeding creature yields a shell material; harvest wild, cultivate, or build
conditions for a giant. Growth augmentation changes practical requirements (food, room,
movement, containment, output). Summoning ties to a defined creature or resource. This
gives the existing growth work a place in the economy. (`adventure/growth.rs` is
in-progress; do not touch it for this.)

**Successful farms stay successful.** Boss, mana and XP farms are legitimate
engineering. Their output has visible sources and limits: food and habitat, maturation
time, available flux, material supply, collection capacity, containment difficulty.
Renewable XP or enchant fuel funds upgrades; *discovering a new principle* still
requires meeting or investigating the phenomenon. Boss dependencies stay exploitable:
drain its coolant, redirect its energy, enclose it, and the expected thing happens.

## 9. Discovery should read well (Adopt)

Feedback decides whether depth lands.

- **Instruments that answer questions:** targeted material probe, temperature and flow
  overlays, voltage/current probe, signal trace, process history, circuit test bench, a
  machine that says why it stopped.
- **Explain stoppage down the dependency chain:**
  "Waiting for coolant. The supply tank is full, but the inlet valve is closed."
  Follow the real dependency far enough to help.
- **Notebook records experiments:** materials and apparatus, the variable changed, the
  observed result, a saved arrangement, the conditions to reproduce it. Real-world
  facts are in references; unknown world phenomena are investigated. Discovery without
  guessing hidden recipes.
- **Discoveries pay immediately:** a safer way to harvest a hazard, a steadier furnace,
  cheaper cooling, a trap, a habitat, a portable instrument, a spell modification.
  Research recognises what the player demonstrated in the world, not what they clicked.

Have: found-spell log and discovery rules (`SPELLS_AND_DISCOVERY.md`). Extend, do not
replace. The grimoire UI is the natural home for the notebook. Mechanics in the
notebook are plain; story lines stay separate (`WORLD_LORE.md` voice rules).

## 10. Simulate at several scales (Adopt, mostly Have)

One authoritative world; different bounded models per job.

| Scale | Representation |
| --- | --- |
| Terrain, open material interaction | local voxel geometry, active cellular sim |
| Machines, vessels | inventories, process state, energy, ports, operating conditions |
| Transport networks | connected graph with capacity and transfer rules |
| Digital circuits | compiled connectivity, bounded signal evaluation |
| Analog equipment | simplified component models for small networks |
| Fabricated chips | validated function, ports, power, timing, heat, limits |

A chip keeps a circuit's behaviour without simulating electrons; a sealed machine uses
the same process model near or far (visuals cheapen with distance, accounting does not).
**An open fire or spreading flood needs explicit treatment and must not quietly become a
harmless recipe when the player walks away.**

Rules (these are the testable form; many already live in `CLAUDE.md` and Phase C):

1. Each mutable quantity has one authoritative owner.
2. A transfer across a boundary is accounted for once. (Phase C: debug conservation invariants.)
3. Rendering consumes world state, never owns it.
4. Machines obey capacity, input, output and energy limits at every distance.
5. Save/load preserves process state.
6. Active sims and circuit evaluation have measured budgets (`Budget`, `just bench-check`).

Lesson from Factorio's fluid rewrite: give up detailed flow where players cannot make a
decision from it, to buy predictability and scale. Pick abstractions by the decisions
they serve. First integrated version is measured in real gameplay with rendering,
creatures, building and reactions together.

## 11. First complete slice: the Flooded Observatory (Adopt)

The smallest setting that proves the whole concept. An abandoned observatory in a
luminous cavern; the lower workshop floods; instruments are damaged; a territorial
creature reacts to disturbance in the water. Restore one instrument that reveals
something useful elsewhere.

Beats: (1) enter and investigate: water, wrecked machinery, an unfamiliar organism,
recoverable parts, a wand that already interacts with materials. (2) Solve the obstacle
several ways: redirect water, run a pump, make access with magic, or strip the part and
repair it elsewhere. (3) Restore a small process: power, a heated vessel, basic
processing, simple sensing. (4) Improve control: a process needs stable conditions, a
pump needs automatic control, a shutter must follow a beam. (5) Make a simple device: a
photodetector via a deliberately simplified fab, usable with relay circuitry (advanced
logic comes later). (6) Reuse it: workshop control, a trap, a portable instrument.
(7) Leave with a new capability that makes a hard place approachable.

It succeeds only if a player can: solve the core problem several ways and say why it
works; apply a spell and a machine to the same physical state; diagnose and improve a
process; build something useful before mastering electronics; carry an invention into
another activity; leave and return without resource or sim inconsistency; and enjoy it
as an adventure before optimising.

### Lore slot (all five, per `WORLD_LORE.md`)

1. **Era:** the Anchor's working period; flooding is after the fall.
2. **Purpose:** the Watch office's instrument house: it read the light on the braid and
   fixed the timetable. It sat beside the Sluice's drains because the braid's weight
   and water's weight were balanced against each other.
3. **Evidence:** a lens train with one mirror missing; a drain gate wedged at 40%; a
   timetable plate whose last column is blank; a hand-drawn tally of descents in the
   margin of a log (the watchtower log's older sibling).
4. **Link:** the held breath (Sluice) and the carriages' three-day timetable (Watch).
5. **Gap:** the log records a descent that did not appear on any timetable. Do not say
   who was aboard.

Suggested inscription (Watch, Storm lens; 220 chars max, ends on the turn):
*"Drain gate set at four tenths. The braid was heavy that week. We held the water so
it would not have to."*

Implementation seam: scenario `assets/scenarios/` first (flood, pump, sensor, door),
then worldgen site, so the loop is provable headless before it is pretty.

## 12. Scope control

For every major addition ask:

1. What new action or decision does it give the player?
2. Which existing systems does it touch?
3. How does the player understand success or failure?
4. Can its recurring work be automated?
5. Does it stay useful after later technology arrives?
6. Does it fit a measured simulation budget?

Cut or postpone: long ingredient chains for their own sake, unexplained random failure,
repeated maintenance chores, tiers that are strictly better than the last.

## 13. Verdict ledger

| Idea | Verdict | Where it lands |
| --- | --- | --- |
| Understand→build→carry promise, miniaturisation as progression | Adopt | this file; PLAN2 north-star note |
| Discipline roles; multiple solutions per problem | Adopt | section 2 |
| Shared property table across terrain, gear, creatures, spells | Have | Phase A1, C, D |
| Energy vs power, temperature vs heat, signal vs power, purity vs doping | Adopt | section 3, fab design |
| Magic with named sources and limits | Adapt | section 3; lore hard rule |
| Spell part vocabulary (action, delivery, condition, modification, supply) | Adapt | section 4, Phase D |
| Sensor condition cards in wands | Defer | after Phase D schema |
| Early electronics ladder, live signal display | Adopt | section 5, Phase C |
| Five-variable fab, explainable yield, deterministic batches, branch specialisation | Adopt | section 6 |
| Cleanroom variables beyond two or three | Defer | until the first fab loop is fun |
| Vertical, spatial factory consequences | Adopt | section 7 |
| Creatures as system participants | Adopt | section 8; blocked on growth work finishing |
| Farms respected, principles still discovered | Adopt | section 8 |
| Notebook of reproducible experiments | Adopt | section 9, grimoire UI |
| Multi-scale simulation table | Have | section 10; Phase A/C |
| Flooded Observatory slice | Adopt | section 11 |
| Single "everything is the same physics" engine (electron-level chips, true fluid dynamics) | Scrap | abstraction by decision, section 10 |
| Per-tier cleanroom minigames, mandatory maintenance, random hidden failures | Scrap | section 12 |
