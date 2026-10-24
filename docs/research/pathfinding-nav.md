# Pathfinding and navigation (HPA*, Recast, DF, Minecraft goals)
Source: Botea, Muller and Schaeffer, "Near Optimal Hierarchical Path-Finding", Journal of Game Development, 2004 (https://webdocs.cs.ualberta.ca/~mmueller/ps/hpastar.pdf); Recast Navigation docs (https://recastnav.com); Tarn Adams interview, StackOverflow blog, 2021; Minecraft goal selector (Yarn docs, unverified for 1.21+).
Adopt: HPA* style cluster abstraction with precomputed entrance-to-entrance costs, rebuilt per dirty chunk; Recast-style corridor then waypoint path output; a priority goal selector with mutex controls for factory and creature AI.
Reject: Precomputed shortcut structures on a fully mutable map (Adams: DF cannot use them well); Recast's float navmesh build; Minecraft's per-mob mutable goal lists.
Why: Cluster costs are integer-friendly and rebuildable per brick, but pathing must be deterministic and budgeted per tick, and a connectivity map is still unverified for our scale.
