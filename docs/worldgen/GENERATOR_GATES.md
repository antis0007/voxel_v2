# Generator gate registry

`CURRENT_GENERATOR_VERSION` (`crates/voxel/src/config.rs`) is the save
compatibility contract: `generator_version >= N` must mean one feature set
forever, on every branch. Numbers are shared across branches and worktrees,
so a gate is claimed here before it is used in code.

## Claiming a gate

1. Take the lowest number in **Reserved** that no row uses, or the next after
   the highest row. Add a row with branch, feature, status, commit when known.
2. Grep every branch and the shared tree before committing the claim:

   ```
   git grep -n -E "version >= N[^0-9]|= N;" $(git for-each-ref --format='%(refname:short)' refs/heads) -- crates
   grep -rn -E "version >= N[^0-9]" crates
   ```

3. Dormant work keeps `CURRENT_GENERATOR_VERSION` unchanged and names its gate
   with a `pub const` (`river_ice::RIVER_ICE_GENERATOR`,
   `biome::BIOME_WEIGHTS_GENERATOR`) so renumbering is one edit.
4. The bump to a gate and the baseline re-save (`just bench-save`, golden
   hashes) land in one commit.
5. A collision is resolved by renumbering the dormant branch, never the
   committed or live tree.

Gates 1 to 53 are history (`landscape.rs`, `lib.rs`, `sites.rs`,
`dungeon/v2` at `VERSION = 32`); the rows below start where numbering stopped
being owned by one session.

## Rows

Status: `live` (in HEAD and current for new worlds), `dormant` (committed,
below current), `uncommitted` (in the shared tree or a worktree only),
`planned` (doc only).

| Gate | Feature | Branch / owner | Status (2026-10-26) |
|---|---|---|---|
| 53 | Lakes, regional climate, smaller seas, river fit, climbable galleries | growth-xmb HEAD 8c258df8 | live |
| 54 | River ice (`river_ice::RIVER_ICE_GENERATOR`) | 95155596 | dormant |
| 54 | Castle composition, dungeon room sizes, settlement roles, workings, valley | shared tree, peer session | uncommitted |
| 55 | Forge room, workings floor, settlement well | shared tree, peer session | uncommitted |
| 56 | Settlement craft houses and paving | shared tree, peer session | uncommitted |
| 57 | Roof tiles, conifer classic form, terrain variety | shared tree, peer session | uncommitted |
| 58 | Settlement role 2 interior | shared tree, peer session | uncommitted |
| 59 | Boss access ossuary | shared tree, peer session | uncommitted |
| 60 | Undercrown observatory | shared tree, peer session | uncommitted |
| 61 | Valley profile, settlement role 2 | shared tree, peer session | uncommitted |
| 62 | Route bridges, drainage crest wetness, surface min reduce | shared tree, peer session | uncommitted |
| 63 | Undercrown observatory revision | shared tree, peer session | uncommitted |
| 64 | unused, reserved by the shared tree sequence | shared tree, peer session | uncommitted |
| 65 | Elevator monument base (`docs/design/ELEVATOR_MONUMENT_20261026.md`); `CURRENT_GENERATOR_VERSION = 65` in the shared tree | shared tree, peer session | uncommitted |
| 66 | Biome fields and family weights (`biome::BIOME_WEIGHTS_GENERATOR`), was 56 | biome-p1 | dormant, renumbered 2026-10-26 |
| 67 | Abandoned Anchor districts, industrial materials, dock machines (`ruins::VERSION`), was 55 | anchor-ruins d013536b | dormant, renumber pending |
| 68 | Underground 3D biome cells (`docs/plans/2026-10-25-underground-biomes.md`), was 55 | none | planned |
| 69 | Surface site kind picked by the region's biome (`sites::kind::BIOME_SITES_GENERATOR`) | sites-enum | dormant |
| 70 | Teaching site stair base only where the ground reaches it (`habitats_region::STAIR_GROUND_GENERATOR`) | habitat-stair-ground | dormant |
| 71 | Planet terrain from a drainage solve: plate uplift, stream-power erosion, climate, rivers, permafrost (`geo::GEO_GENERATOR`, ADR 0038) | growth-xmb shared tree | dormant |

## Reserved

72 onward is free. Rows 54 to 65 belong to the peer session that holds the
shared tree's uncommitted worldgen edits; when that work commits, replace
"uncommitted" with the commit hash. If it lands under different numbers, the
rows above move with it and 66 to 68 stay as claimed.

## Collisions found 2026-10-26

- 54: river ice (committed) and the shared tree's castle and settlement work.
- 55: anchor-ruins, the underground plan and the shared tree's forge room.
- 56: biome-p1 and the shared tree's craft houses.

biome-p1 moved to 66. anchor-ruins moves to 67 when it next rebases. The
underground plan takes 68 when it is built.
