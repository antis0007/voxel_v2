# The Undercrown

Date: 2026-10-19

## Intent

The Undercrown is one persistent buried landscape: inhabited bridges, flooded streets, root temples, furnaces, archives, ice caverns, and a deep inland sea. An expedition ends when the player can explain something they only glimpsed on the way down, has reached a destination, and is holding an unfinished lead. Another route through the same network can complete a different expedition of the same weight.

The journey written for this region is one such expedition. It is not a script the player must perform. Arrival, a recovered object, and a repaired machine count. A required guardian-death flag does not.

## What already exists

`src/world/undercrown/` is the contract. `Plan::for_config` returns an empty plan. `CURRENT_GENERATOR_VERSION` stays 18 and production saves stay `VXWSAV16` until the first geometry lands. The activation checklist in `docs/UNDERCROWN_SCHEMA.md` is unchanged: generator 19, save 17, then content.

`District`, `RoomKind`, `PropKind`, `Action`, and `LinkKind` are the vocabulary. `ObjectId` is `(district, room, prop)` with the allocation in the schema. Flags record history. Voxels and edits are what make a route real. `CLAIMED` blocks a second grant. Focus, reach, and `set_many` are the only interaction path. There is no dialogue system and no NPC scheduler.

Random sites, caverns, and dungeon complexes stay outside the reserved footprint. They are not Undercrown rooms.

## Calls that bind the proposal to this code

- The 64 room descriptions are a content pool. The first build authors the 18 rooms allocated in the schema. Cutting one later does not renumber the others.
- People are props. A ferryman, a salvager, a caretaker, and the bridge household use a new `PropKind::Person`. Their first-slice actions are the existing `Inspect`, `Repair`, and `Claim`. A line on the prompt card is the whole conversation. Trade and intimidation beyond those three actions wait.
- The six verbs are an exploration selector, not wand slots and not spell ids 28–33. The combat deck in `wands.rs` stays. Each verb is one world operation with a bound on energy, range, and what it is allowed to touch.
- Equipment that must persist (the ring, the gloves, the buckler) is an inventory item with a stable id. `CLAIMED` on its plinth is the anti-duplication record. Losing the item does not clear `CLAIMED` and does not spawn a second copy. A learned verb stays learned.
- Water that the player can change is a list of cells on the sluice prop, written with `set_many`. It is not the backup GPU fluid sim. The two outcomes of the borough bypass are two authored cell sets: supply kept, or streets drawn down and the ferry link closed. Both sets leave an exit.
- A chasm sightline is longer than passage fog. Rooms that own a `LinkKind::Sightline` use a thin haze that keeps the bell, the observatory, and the waterfall door readable at the authored distance. Corridor rooms keep the short analytic cave fog from the expedition-depth design. Cave fill is not raised.
- The bell guardian is one large creature with a windup and emissive eyes, using the threat read from the expedition-depth design. It does not narrow a corridor into a soft lock. Repair, a caretaker bargain, a sneak route, and a fight are all authored in that room. The fight is the resolution that sets no `RESTORED` flag. Repair sets `RESTORED` and writes the lift edits in the rooms those lifts connect.
- Digging and destruction stay allowed. An edit that removes a support the plan marked load-bearing drops the span it carried and writes `FAILED` on that prop. It does not delete unrelated rewards. If the only copy of a critical function is destroyed, another prop in the slice still provides that function or another route already reaches the destination.
- Saving stays the normal world save. No altar.

## The first expedition, as places

These are the 18 rooms. Each names the approaches the first build must actually support. Further approaches from the pool can be added without new ids.

Rootbound Halls. The well is on a surface road. The player can see the landing before dropping.

| Id | Room | Approaches in the first build | Pays with |
| --- | --- | --- | --- |
| 0.1 | Ruined well | Ladder, rope anchor, or the pump-house slope. Blue lamp is a moving prop on a sightline, not a creature. | The promise of scale. No item. |
| 0.2 | Landing | Inscription and footprints are `Inspect`. The shrine is a marker. | `KNOWN` on the inscription. |
| 0.3 | Counterweight hall | Operate the ballast. The suspended crate is the safe demonstration. Plinth holds the Counterweight Ring. A balancing beam accepts the ring and refuses an over-limit pair. | Ring, `CLAIMED` on the plinth. |
| 0.4 | Broken stair | Lower the freight platform, or use the ring on the ballast beside it, or build across with carried blocks. | Passage to the chasm overlook. |
| 0.5 | Service corridor | Inhabited alternate around the stair. A person prop points at the village. | The same overlook, by the other link. |
| 0.6 | Door behind the first camp | Present as a sealed face on the landing side and as a recognizable back wall here. Stays sealed in this slice. `Inspect` says what would open it. | An unfinished lead, not a shortcut yet. |

Crown Chasm. The overlook is large enough that the bell, the figures under it, and the waterfall door are real props on a sightline. The door is not enterable in this slice.

| Id | Room | Approaches in the first build | Pays with |
| --- | --- | --- | --- |
| 1.1 | Crown Chasm overlook | Walk the near ledge. Three links leave: village (freight), gardens (sightline only; the tunnel mouth is visible and blocked), chain lift (passage down). | The destination, seen. |
| 1.2 | Bridge village | Public walk. Repair the water lift. One person wants the current chart; Inspect hears the lack, Repair after the borough chart fills it. One person trades a relic clue for the missing-partner flag, which this slice can set and cannot yet resolve. | Bed (`REPAIRED` refuge), deposit prop, return link to the landing, rumor `KNOWN`. |
| 1.3 | Hall of Patient Bells | Chains and a diagram are visible. Rebalance ballast, or Operate an anchor, or walk the supported edge, or take the hatch. A fall lands in a lower shelf of the same room with a stair back. Gloves sit on a marked hook, off the fall line. | Bellkeeper’s Gloves. |
| 1.4 | Freight cradle | Unload ballast or Repair the receiving platform. | A freight link that stays after save. |
| 1.5 | Bellkeeper’s crown | See the jammed guardian before it wakes. Repair the cradle, or Restore resonance on the clapper, or sneak the interior ledge, or fight. Steal is the sneak path’s optional plinth. | `RESTORED` reconnects the village lift and the chain lift by edits. The ring of the bell is audible at the overlook. |
| 1.6 | Observatory | Climb the restored service route. Windows frame the bridge and the courthouse roof. Restore the lens or leave it. The upper door exits onto the far cliff above the known road. | A surface exit. Lens `RESTORED` places a beacon the road can see. |

Drowned Borough. Entered by the chain lift. The courthouse vault is visible before the water choice.

| Id | Room | Approaches in the first build | Pays with |
| --- | --- | --- | --- |
| 2.1 | Chain-lift landing | The lift is the link from the chasm. It works as a passage before the bell is restored and becomes a return after `RESTORED`. | Arrival. |
| 2.2 | Air-bell street | Ferry link, or the duct, or a short dive between two authored air pockets. Breath is a timer on the player while the head is in water, already implied by underwater state. | A dry neighborhood. Optional cache on the jeweler balcony stays visible and unclaimed if skipped. |
| 2.3 | Flooded courthouse | Roof, air pocket, or the lowered water from 2.4. | Rimeglass Buckler on a plinth. |
| 2.4 | Reservoir control | The basin and the village supply are both visible. Repair the bypass: vault water drops, supply cells stay. Open the full sluice: more streets become walkable, the ferry link’s cells fill, the ferry person says the route is shut. | A world change the player can walk back to. |
| 2.5 | Ferry | Person. Repair is “chart the current,” which sets `KNOWN` and satisfies the village ferryman. | The chart, and a crossing that the drawdown closes. |
| 2.6 | Maintenance duct | The overlap route into the courthouse and back to the air bell. | A way that ignores the ferry schedule. |

The waterfall door, the singing ice, and the vanished barge are `Inspect` rumors on the overlook, the observatory wall, and the ferryman. They have no rooms in this slice.

## Verbs, equipment, and the one combination the slice must teach

All six verbs exist in the selector. Each one has a single authored demonstration in this slice, plus the general rule that it affects a prop tagged for it. It does not grow a new simulation.

| Verb | Demonstration in this slice | Bound |
| --- | --- | --- |
| Kindle | Hearth in the village kitchen lights an authored fuel cell. | One cell, short range. |
| Rime | Buckler recharge at the courthouse basin. Also freezes one marked aqueduct gap into ice cells that sit on existing supports. | Ice is voxels. It does not hover. |
| Impulse | Shoves a marked loose load, including the ice panel and the counterweight crate, inside a speed cap. | One body. |
| Bind | One visible tether between two anchors. Used to hold the ice panel or the bell floor. | Breaks when either anchor is gone. |
| Resonate | The bell clapper and the patient-bell diagram answer it. A hollow in room 0.6 reports as a draft on Inspect, not as a new map. | No structural collapse from vibration in this slice. |
| Bloom | One rough pier in the service corridor accepts growth into a handhold. | Fails with a visible reason on smooth dry stone. |

The reservoir bridge is the combination the slice teaches, and only after each piece has been safe somewhere else: Rime at the basin, Bind in the bell hall, Impulse on the crate. An ordinary plank of carried blocks also crosses that gap.

Signature items in this slice are the ring, the gloves, and the buckler. The other eleven treasures in the proposal stay named pool entries with no ids until a later slice. The ring’s rule is the one in the proposal: it moves load between two marked bodies, keeps inertia, and cannot lift more than its limit. The beam in room 0.3 is where that limit is shown before the broken stair.

Perks are out of this slice. One verb behavior change is enough for the next slice, not this one.

## Light, water, and cost

Passage rooms use the short cave fog and the existing torch and lantern lights. The overlook, the air-bell street, and the observatory ascent use sightline haze. Landmark props on those links carry a small emissive so they read before the player is close: the blue lamp, the bell’s rim, the lens.

Borough water uses the resident Gerstner palette from the expedition-depth design, including torch caustics under the air bell. The chasm waterfall is the same surface on a short run of cells, not a new material.

The plan is one authored footprint. Do not fill the hollow with a simulated sea. Prop counts for the slice stay inside the existing focus scan. New verbs do not allocate per frame. A tether is one record on the player, not a body per segment.

## Tests before the slice is real

- Version 18 sampling of the footprint matches the current golden hash. Version 19 contains the well and does not contain a dungeon corridor in the reserved margin.
- From the well, the landing is visible, and both the ladder and the slope reach it with the baseline kit.
- Operating the crate moves the platform. The ring on the beam rejects an over-limit pair and accepts a pair inside the limit.
- The broken stair can be crossed by the platform and by the service corridor. Removing either route leaves the other.
- The overlook sightline reports the bell, the village, and the waterfall door as focuses.
- Bypass repair and full sluice write different cell sets. Each set still has a walk back to the chain lift. Save and load restore the cells and do not grant the buckler twice.
- `RESTORED` on the bell writes the lift cells. A fight that kills the guardian does not.
- Destroying the ice panel does not remove the plank route.

## Later slices, in order

1. Lantern Gardens and Ashen Foundry, joined to the chasm and the borough by links that already have mouths.
2. Glass Archive and Frozen Choir, including the singing ice and the waterfall door.
3. Hollow Sea, the barge, and the second surface exit behind the glacier.

Each slice adds rooms at the ends of the schema table. None of them renumber 0.1–2.6.

The pool those slices draw from is the room catalogue in the proposal: eight districts, eight rooms each, the trap table (weighted floor, steam, crystal alarm, light gallery, sluice, freight trolley, ward, pit), and the fourteen signature rewards. A room from that pool enters a slice only with an `ObjectId`, two approaches that fit through the real movement kit, and a clue the player can perceive before the hazard.
