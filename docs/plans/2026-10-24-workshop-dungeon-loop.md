# First expedition: workshop to fungal vault

The bounded milestone follows the generated route from the clearing to the
starter workshop, working face, fungal vault and back. The wider physics
migration and remaining systems stay in [MASTER_PLAN](../MASTER_PLAN.md).

## Player route

1. Follow the clearing north to the workshop. Use **F** at the workbench.
2. Follow the timber decline to the lit left bay. Mine copper, coal, two lumen
   fibre and a crystal with **left mouse**. At one-metre scale, use **Space** to
   climb full-block stairs on the return trip.
3. Return to the workshop. **F** at the smelter converts one ore and 0.25 coal
   into one ingot. Open **E / Tab**, then **Gear**, and upgrade the starter wand
   using the ingot, crystal and two fibre. The original first-wand recipe now
   upgrades the starter wand, which new worlds already provide.
4. Arrange the deck in Gear. **R** casts; **T** advances its next card. Continue
   down the decline into the fungal vault and defeat both guardian slots.
5. Return to the original workbench and use **F**. Homecoming persists; the
   compass then resumes the larger guardian-relic expedition.

The first workshop visit sets a safe respawn point while enemies are away.
The journal lists these milestones separately from bore crafting and the longer
workshop-heart quest. No controls were rebound.

## State and compatibility

- The generator **48** branch adds new-world bay/vault dressing and pale
  workshop infill. Versions 47 and earlier retain their sampling branches.
  Concurrent terrain work advanced the integrated generator to **49**.
- This slice adds no save bytes. Concurrent lantern persistence advanced the
  integrated save layout to **33**. Starter encounter `IVec3::ZERO` uses the
  existing site-specific bit 14 for homecoming. Foundries retain its bypass
  meaning; heart restoration keeps bit 15. No old terrain is regenerated.
- Mining awards resources after a committed cell replacement. Spell blasts
  preserve volume-weighted resource yields; ordinary rubble stays out of the
  pack. Full-pack recoverable drops use the existing bounded owner.
- Field smelting stages inventory inputs and outputs together. Failed output
  capacity spends neither ore nor fuel. Machine collection transfers what fits
  and retains remaining resources or assembler stock in the machine.
- Mining and smelting emit separate original procedural cues. Existing spell
  and enemy sounds retain the positional mixer.
- Surface dressing mixes low broadleaf rosettes with upright sedges, flowers
  and pebbles using existing placement and mesh budgets. Vault masonry, timber,
  clear combat lanes and local torch pools frame the guardians; mushroom groves
  remain around the perimeter.

## Native review

`tools/Review-WorkshopLoop.ps1` freezes the executable and launches an isolated
generated Hills world, seed `2738958700`, at one-metre resolution. It scripts
normal walking, jumping, mining and casting inputs through production paths.
Station/deck operations call production UI handlers. No camera route, granted
loot, fixture terrain, health edits or forced victories are used.

Checkpoint review compares position, inventory, resources, wand, encounter
records and mined cells with the production decoder, then installs the loaded
session. Logs/captures live in `target/workshop-loop/<label>`. This does not
establish manual menu usability, gamepad coverage, other seeds/scales, listening
approval or performance acceptance.

- `route-r1`: movement, mining, smelting and upgrade reached; actual GPU device
  loss interrupted combat. Failed full-loop review.
- `route-r2`: superseded executable, stopped before acceptance.
- `route-r3`: both guardians defeated through actual casting. Captures exposed
  obstructed combat sightlines and a scripted return waypoint crossing a wall;
  final revision clears the arena and routes the return through its entrance.
- `route-r4`: both guardians defeated; the return caught a three-metre lip from
  vault dressing overriding the decline. The fixed branch yields the complete
  centre strip back to the original decline, including its floor and headroom.
- Final review: pending execution.

No unit tests or hosted CI were added or run. Verification uses the requested
native gameplay review and offline release-fast build.
