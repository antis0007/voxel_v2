# Generator 54: inhabited place silhouettes and excavation spaces

New worlds use generator 54. Generation changes are version gated; older saves
retain their stored generator. No save migration is included.

- Foundry castles: pale masonry, two steep capped towers, two open terraces,
  a gabled keep and an unobstructed overhead courtyard view. Wing plans are
  rebuilt alongside the roof changes so their rooms and stairs remain defined.
- Valley settlements: two additional two-storey homes with upper windows,
  pale timber infill, warm wooden roofs and a covered market well. Existing
  street, doorstep and service-lane reservations are retained.
- Valley lookout: open upper lantern and hollow steep basalt cap.
- Mines: taller working faces with vaulted ceilings, timber ribs, wall-supported
  torch brackets and a dark haulage strip. The strip is visual, not a rail system.
- Pits: taller landing chambers widened by six metres on each horizontal axis,
  side-bay ribs and a west shaft viewing opening
  above a retained parapet. The east landing remains the entrance.
- Inhabited dungeons: room dimensions vary by purpose; major communal rooms
  receive taller ceilings and structural side aisles. Door and stair approaches
  take priority over the new arcade sampler.
- Complete tree candidates are excluded from occupied campuses rather than
  leaving a clipped trunk or crown across their buildings.

Opt-in review cameras use actual generated sites (`place:castle`, `place:town`,
`place:tower`, `place:mine`, `place:pit`) and an axial `room:library` view.
They are rendering tools, not collision or traversal checks.

See `docs/design/WORLD_OVERHAUL_20261026.md` for native evidence and limitations.
