# Generator 55: work, materials and flooded salvage

This continues generator 54's architecture work with places that communicate
what happened there and contain usable material cells.

- Forge rooms are 22 x 16 metres, with separate basalt-lined hot and cold bays.
  Their contents are actual lava and water blocks. A broken vessel can release
  its contents through the existing simulation; no scripted flood or special
  fake liquid is introduced. Workbench, smelter block and feedstock occupy the
  opposite work bays. The centre cross and doorway approaches remain reserved.
- One settlement garden becomes a smith's service yard, with a roof, fuel and
  ore stacks, workbench and salvage storage. Other gardens remain. Street and
  service circulation take priority over the yard.
- Pit landings have water-filled side bays. A dry cross
  and perimeter connect the shaft, storage bays and salvage; four supported
  lamps mark the side routes. Existing structural
  ribs, solid floor and shaft enclosure are retained.

Generator 54 and older saves retain their previous geometry. New cameras:
`room:forge` and `place:pitflood` (first submerged landing).

The inspiration is systemic material interaction (Noita), readable work/storage
organisation (RimWorld), processing spaces (modded Minecraft), and atmospheric
flooded exploration (Obscuria). This does not implement colony jobs, new machine
recipes, gas pressure, boss phases, or a full Obscuria biome. Generated machine
blocks do not by themselves establish an operating production network.
