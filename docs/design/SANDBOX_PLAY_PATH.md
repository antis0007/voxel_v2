# Sandbox play path: Workshop friend slice

Source audit, not playtest results. Keep the existing route, saves and rules.
Use this backlog to make the next session more inviting before adding more tiers.
The target loop is **notice a place → choose a job → bring something useful
home → change what the group can do**. The journal should help players resume
that loop after wandering or dying, without making it mandatory.

| Priority | Friction visible in the current path | Design direction |
| --- | --- | --- |
| P0 · Find the way in | Workshop targeting changes from the bench to the underground heart. The saved journal marker uses the quest name for both. A precise bearing can send players toward a roof or cliff instead of the entrance. | Make the lit decline legible from the clearing and the bench. Track the entrance while approaching; show the interior objective once the route is apparent. Keep the destination stable across reloads. Check the walk before adding more markers. |
| P0 · Recover from a bad start | The preparation check accepts an upgraded wand or a weapon, but ownership does not tell a player how to equip it, find ingredients, or retrieve it after death. A blocked bench respawn can be hard to distinguish from a successful one. | Show the nearest useful action in context: equip a carried weapon, source one missing ingredient, or clear the threat at the bench. Confirm respawn only when it is set. Keep a visible regroup point and verify a recoverable return to dropped gear. |
| P0 · Make the first machine work | The iron goal names the output, pipe and storage. Projects shows recipes and counts, but placed parts alone do not explain power, input/output faces or why nothing arrived. | Guide from the first failing link in the actual circuit: fuel, power, ore, output face, pipe, storage space. Show that at the relevant block or in Production. Complete on the existing accepted delivery, and show the group what the line now provides. |
| P1 · Give the next trip a purpose | Workshop completion can move tracking to the next available journal entry. “Claim the foundry relic” says what to collect but gives little reason to choose it over a different site or staying at base. | Offer a few destinations with distinct uses, such as a new spell behavior, a material for the forge, or a route shortcut. Reveal the category of payoff and rough distance after a clue; let the exact reward be a discovery. No forced tracking switch for someone who opted out. |
| P1 · Make traversal earn its space | Traversal lists gear and bindings, but a recipe cannot show why a bounce, glide or powered ascent helps. A chest slot swap also changes the return trip. | Give each tool a forgiving route to try: a sling launch with an open landing, a safe drop to learn gliding, a return climb that rewards powered lift. Keep a walkable alternative. Give travel shortcuts repeat value on supply runs. |
| P1 · Make upgrades change play | The tool route lists parts and mining levels; the wand route shows slot counts. Both can read as a shopping list if players cannot tell which part or spell order will solve the problem in front of them. | Compare one relevant outcome at the point of choice: rock it can mine, reach or repair cost, or the next cast sequence with mana and recharge. Let players try a build cheaply at home before risking a trip. Start with a useful choice between two builds rather than a longer tier ladder. |
| P1 · Let late joiners contribute | The Workshop clear and iron receipt are world milestones. A friend arriving mid-session can see a later stage without seeing the setup or knowing what the group needs. | Show shared progress separately from personal readiness. Make the current job visible and offer parallel jobs with immediate value: recover a pack, bring fuel, repair a route, or test the line. Keep one shared completion receipt; no requirement for everyone to replay the same task. |
| P2 · Avoid a dead end in other worlds | Workshop availability depends on generator version and world type. An empty expedition list can be correct for the world and still look like missing content. | Offer an applicable next action from existing Projects when no expedition applies: a local supply route, a workbench project, or a known destination. Never invent a waypoint for a site the generator did not place. |

Keep the default HUD quiet. Put physical cues in the world first, the immediate
state on the object being used, and detailed costs or recipes in Projects.
Support detours: players should be able to reclaim the workshop after building
the iron line, explore before taking the quest, or choose a different useful job.

For the next friend playtest, start a fresh host and have one friend join after
preparation. Give them the launch instructions only. Observe where they stop,
ask what to do, make a useless trip, or lose the way back. Record the first useful
action, first missed turn, first machine stall, and what each player chooses
after the first iron delivery. Include one death/rejoin and one deliberate
detour. Repeat on a supported alternate terrain seed; check an unsupported
world separately for the empty journal path. Scripted completion can establish
that a route works; unaided play is needed to establish that players understand
and want to follow it.

## P0 implementation

- The existing Workshop marker points to the generated decline entrance during
  the vault stage, including in journals that cached the old underground target.
  The bench and completion stages still use the same progression checks.
- The death screen shows the active map binding and how to collect drops when
  stacks were lost. The existing Death waypoint can be centered in the map.
  Workshop bench feedback distinguishes a set checkpoint from a hostile block.
- Workbench, Projects → Machinery, and Production show one live iron line
  status before the first delivery. The readout uses the inspected furnace
  when nearby, otherwise the nearest electric furnace. It checks the actual
  power network, directed pipe route and storage admission. The original
  delivery receipt still determines completion.

These changes require no save migration or regeneration. Confirm the entrance
walk, death recovery and line troubleshooting in the unaided friend playtest.

## After the first iron delivery

The first delivery proves the line works once. The next goal should show what
owning that line lets the party do repeatedly.

1. **Shared stock and requests.** Start with one searchable view of connected
   storage and one request for an existing component batch. Preview available
   stock, missing inputs and required stations. Use the existing recipe planner,
   machines and transport; a job finishes when its output reaches storage.
   Surface one reason when blocked and show who requested the batch.
2. **An upgrade worth rebuilding for.** Compare raw smelting with the existing
   crushing/washing path on yield, power, water and time. Make both reasonable
   choices. Spend the output on a real party goal, such as travel gear or a
   field supply, so increasing throughput changes the next session.
3. **Construction and debugging at scale.** Preview machine faces while placing
   them, copy filters and face settings without copying contents, and trace one
   selected line. Later, build on the creative blueprint preview for survival
   material lists and placement ghosts. Keep the world view clear when inactive.

Friend slice acceptance: after the first iron receipt, a late joiner can find
shared stock, request a useful batch, diagnose one deliberately stalled link,
and take the result on a trip. Rejoin and verify the material totals and job
state. Check whether the group voluntarily improves the line for the next trip.

### Conduit inspection

The Machine panel shows each conduit face as Linked, Open, or Closed. Click a
face to toggle it. Machine item roles retain their Both/In/Out/Off cycle.
Item pipe modes are Transfer, Extract, and Distribute. The tube arms and
selection bounds follow the same face and cable tier checks.

The factory gathers cable, item pipe, and fluid pipe rebuild seeds in one pass.
The routing pass skips absent networks and a full packet queue. Resource order,
route priority, transfer cadence and saved machine settings remain the same.
Compare frame and simulation timings on the same large factory before claiming
a measured performance gain. A placeable conveyor is a separate feature.

### First production request

In Production, choose an outpost or All Outposts and open Shared Stock to
search stored resources. Near a Cultivator, Request Fuel switches an idle
machine to fermentation and requests four bio-ethanol. Supply biomass, fibre
and power, and connect an item pipe to storage that accepts the fuel. The
readout shows the outstanding ingredients and delivery count. Collect fuel
from the receiving container for the burner jetpack.

One request occupies one Cultivator. Completion counts accepted storage
deliveries and stops that machine from starting another batch. Clear & Pause
preserves inputs and finished output. The save records reserved pipe shipments
as delivered, including their fuel receipts. No layout migration is required
to load older worlds in the new build.

### Repeatable stock reserves

Connect an electric furnace, alloy kiln or fermenting cultivator to a storage
crate through item pipes. Filter the crate to the desired product. Inspect
the machine and open Maintain Stock to choose 4, 8, 16 or 32 items. The machine
selects the nearest eligible crate on a directed route. If two orders compete
for a crate, only one maintains it.

The reserve counts stored items, incoming shipments and finished output before
starting a batch. A withdrawal creates demand again. Power, feedstock, machine
mode and pipe directions still apply. Clear & Pause keeps material already in
the machine. The order uses the existing saved machine target and leaves
previous fuel requests intact.

Projects → Machinery → Iron Routes compares raw smelting, crushing and washing
using the actual recipe outputs and processing energy. Reserve fuel for the
burner jetpack or feed the iron reserve into a steel line for traversal gear.

### First orbital return

The first freight order gives each workshop a reason to keep its line running.
Steel spends iron and coal. Bio-ethanol spends renewable biomass and fibre and
asks the player to build a Cultivator and deliver a batch to storage. Both
routes can prepare a Bronze Battery Bank at home before leaving. One returned
mithril cell gives that bank 64 kWh for the next project.

Use the trip to test navigation, pressure doors and the return logistics.
Record active gathering, setup and travel separately from machine and freight
waits. Test both orders in fresh Normal Survival saves on the same seed and
verify the upgraded bank after reloading each world.
