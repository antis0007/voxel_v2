# Generator 41: courts, streets and connected workings

New worlds select version 41. All changes below are gated at 41; older
generation paths retain their previous layouts. Building cache keys include
the actual version so adjacent previews of different versions cannot share
the wrong castle blueprint.

- Foundry castles gain low timber and stone service ranges with doors facing
  the court. They sit below the existing keep/watch-turret skyline, outside
  the gate axis and central forge fixtures, within the original plot.
- The valley settlement has staggered inward-facing plots, a paved market
  square, covered stalls, wider main street, entrance spurs and a connected
  rear service circuit. The inn and workshop have distinct positions; all
  roads and parcels use the existing surveyed village grade and footprint.
  This develops the existing town grammar, not a new large-city generator.
- The valley tower gains an upper lookout deck, parapet and study doorway.
- Mine crosscuts branch from the existing decline at 26 metres and return
  to the fungal chamber through two descending timber-framed galleries.
  Copper and tin distinguish their outer crosscut faces. The original
  decline retains ownership of its central walking strip.
- Non-mine ruin pits use supported annular stairs and side-room landings
  every 22 metres, with inset lights and supplies. Four turns meet each
  landing; depth is limited by available height above world bedrock. Mines
  use their decline instead of intersecting it with the old ruin pit.
- Dungeon service/ceremonial rooms respond to branch position and distance
  from arrival. If level assignment removes every loop, planning recovers
  a slope-compatible return edge when possible, respecting core and sealed
  antechamber access. Existing puzzle and objective selection is preserved.

Validation: local offline `cargo check -p engine_worldgen --target-dir target`
passed. No tests, hosted CI, game launch or visual review was performed.
Source and compilation establish implementation only: native review is still
needed for coarse-scale stairs, doorway clearance, town grading, mine joins,
castle silhouettes and saved-world streaming boundaries.
