# Resident work and presentation

Use **Graph → Residents and settlement life** to open the actual model, schedule, job, construction and guard sources. Attach the relevant source node to the AI workbench when changing a behavior. Use **Workspace → Residents** for Fernwake names and authored dialogue. The [civilization direction](../../docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md) covers population, continuity and distant simulation.

## Behavior and art review

### Realm orders and campaigns

Use **Playtest → Civilization** or `/realm` in solo chat on the active Planet. Stand within 22 metres of a registered settlement and issue **inspect** to find its owner, stores, linked settlement IDs and expedition state.

1. Recruit a registered resident from an extension home or the last original household of a settlement. Bring them to an abandoned generated site with 12 biomass and 8 planks. Use **found**. Confirm that the same resident leaves the old household list and works from the new outpost. Use **supply food** or **supply timber** to transfer further stock from the pack.
2. At a foreign settlement, use **pact**, **peace**, or **war**. A pact must end before war. The relation applies to all holdings of that charter. At a player holding, enter a linked settlement ID and use **march**. Compare the source's stores and the resident's identity before and after the order.
3. Follow the expedition into local view, block its march or wound it, then leave and reopen the same save. A local expedition uses one combat actor. Distant progress uses the saved route cursor, supply and health. Siege pressure spends the attacker's supply and the defender's material reserve. A successful siege transfers the holding. Use **withdraw** or make peace and inspect the return march.

The campaign unit is one mobilized household per source holding. It can fight the player and stationed guards. Siege progress currently accounts for reserves and control of the approach. It has no engine for siege weapons or wall demolition. The first founding route reoccupies an abandoned generated site. Site designation on arbitrary terrain is separate. Inactive planets and dimensions remain saved while the active world runs.

### Westbank convoy review

1. In **Playtest**, build the game and launch a disposable Planet world with generator 243 or later. The freight overlay identifies Westbank Market, Eastbank Farms and the service crossing. Use **Inspect convoy** to read its position, stable driver ID, health, active body count and cargo balances even while the site is offscreen.
2. Observe a shipment at its holding. Record the driver ID, source stock, cart cargo and market stock. Leave the area and inspect again. The route should advance while active bodies drop to zero. Return before delivery and check for one active body with the same identity. The driver can report their order and cargo when approached.
3. Cut the main deck across both lanes while the shipment is on the east side. Check that the same load takes the service crossing or waits if the cart cannot safely reach the fork. Save during the detour, relaunch the same disposable save, and compare the ledger before and after exactly one delivery.
4. Wound the driver. The crew and load should backtrack on safe ground, then resume. Save during the interruption and confirm health and cargo carry over. In a separate disposable run, kill the driver, recover the stranded biomass with an empty inventory slot, and repeat the interaction after reload. Recovery must reduce the cart balance by the amount actually inserted. A fallen driver's holding cannot dispatch a replacement under that identity.

The convoy record owns the cart, route and named driver. The loaded actor owns immediate combat reactions, which write back to the record before the actor is removed. The save reader still accepts `PFR1` shipments. `PFR2` retains driver wounds and the interruption deadline. The trip currently ends at delivery. A continuous return trip and independent civilian work at the terminals remain separate work.

1. Use the disposable solo world from **Playtest**. Find a human resident and inspect their face, workwear and jade Talk prompt. Record the resident name, seed and scale. Compare the same resident after leaving and returning. Compare several residents from front and side for readable silhouettes, grounded boots, and visible headwear and clothing differences.
2. Assign the resident to settlement work through their interaction page. Follow the page's stockpile and designation controls. Watch an empty-handed trip, mining or timber work, and delivery. The axe or pick pose reads the active work counter and target material. The carried bundle reads real job cargo. Inspect the target block and stockpile to verify the actual transfer.
3. Queue a supported build with the required stock. Compare a successful placement against a blocked or occupied site. The placement gesture reads a recent committed edit. Interrupt mining by removing the target. Check that the work pose stops and the resident can take another job.
4. Interrupt a worker by damaging them while they have cargo. Check the safety status, retreat, and eventual work resumption. Check the cargo and stockpile for lost or repeated transfers. Assign a different resident to a guard post and compare the badge and strike pose against real guard activity.
5. Record what happened in the native world and attach the exact build and source revision to the work packet. Presentation follows authoritative activity. Contact timing, visual quality, and multiplayer replication need their own evidence.

## Inspiration routes

- **RimWorld.** Keep hostile pawns recognizable as people with equipment, prior roles and relationships. Build injuries, retreat and reappearance into their identity and history. In Axiom, use the same commitment and inventory contracts on both sides of a conflict. [Ludeon’s RimWorld overview](https://rimworldgame.com/index.php?lang=en)
- **Dwarf Fortress.** Let preferences, skills and commitments change which jobs people choose, with inspectable reasons and sensible defaults. The first decision trace should show the real task, required resources, route and interruption. [Bay 12 on personalities](https://www.bay12games.com/media/df_talk_11_transcript.html)
- **Songs of Syx.** Connect citizens to workplaces, needs and supplies, and aggregate distant work from those same commitments. Preserve the worker and cargo when the player returns to the site. [Songs of Syx citizens](https://songsofsyx.com/wiki/index.php/Citizens)
- **RollerCoaster Tycoon and RollerCoaster Tycoon 2.** Use Chris Sawyer’s management games as a readability reference. Give each visible worker a concise status and make traffic, blocked access, waiting and idle facilities legible in the world and in inspection. [Chris Sawyer’s game history](https://www.chrissawyergames.com/info.htm)

The next behavior slice is a bounded choice between available work and safety or household obligations, with a reason record for each decision. The next hostile slice should start with a small named squad that draws equipment from actual stock, can abandon an objective, and preserves surviving identities. The next animation slice should fit hands and tools to reachable interaction points before the authoritative contact moment.
