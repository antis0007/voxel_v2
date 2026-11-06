# Creature habitats, roster growth and shared realm crossing

Owner brief (2026-11-05): more mobs, 5e creatures and ordinary animals for
resources, spawn integration with biomes, the underground and dimensions, and
the planetary Nether reachable in single and multiplayer.

## Intent

Spawning becomes a data question. One file says where each kind rises; the
code owns only the context (biome, realm, time) and the deterministic roll.
New creatures are a bestiary row, a spawn row and a body function.

## Goals

- `assets/spawns.ron`: habitat rows per kind (weight, biomes, realms, time).
- Night, cave and grazer spawners consult the same table.
- Ten new kinds: four resource animals, six creatures from the 5e bestiary.
- The hell gate generates on planets (gate 139, dormant until the planet
  version bumps) and a Nether derived from a planet mirrors that planet.
- A hosted session crosses the gate together: the host walks through, every
  joiner is rebuilt on the far side.

## Non-goals

- New items, blocks or machines (no save bump). Drops reuse hide, wool and
  mutton rows the sheep already use.
- Per-creature boss modules. New kinds use the generic attack codes.
- Steam `Packet::World` kinds above 2 and lobby `HostInfo` refresh.
- Headless `--multiplayer-host` crossing (no local saves there).

## Anti-goals

- A spawn that differs between two runs of one seed.
- Terrain loaded to spawn a creature.
- Content tables in Rust.

## Design

### Habitat table (`adventure/habitat.rs`, `assets/spawns.ron`)

```
Realm = Surface | Underground | Nether | Shell
Time  = Any | Night | Day
Row   = (kind, (weight: u8, biomes: [Biome], realms: [Realm], time: Time))
```

Empty `biomes` means every biome. `Context { biome, realm, night }` is built
once per spawn pass from the candidate cell: realm is `Nether` for
`WorldType::Nether`, `Shell` for Manyfold and Hall, `Underground` when the
cell is roofed and more than 6 m under `terrain_height_at`, else `Surface`.
Biome is `landscape::sample(cfg, x, z).biome`. `pick(ctx, roll, filter)`
sums the weights of matching rows and walks them in file order, so one roll
maps to one kind forever.

The bestiary `night` field stays as data but the picker no longer reads it;
a test asserts every kind with `night > 0` has a spawn row.

### Spawners

- `night.rs`: `pick_kind(roll)` becomes `habitat::pick(ctx, roll, hostile)`.
- `wildlife.rs`: `candidate` picks a grazer kind for the patch from rows
  whose kind is passive, using the patch centre's biome.
- Cave salamanders keep their karst rule.

### New kinds

| kind | name | role | home |
|---|---|---|---|
| 49 | Cow | Grazer | Meadow, Savanna, Heathland |
| 50 | Boar | Grazer, provoked fights | Forest, Taiga, RedwoodGrove |
| 51 | Deer | Grazer | Forest, Taiga, CloudForest |
| 52 | Marsh Fowl | Grazer | Marsh, Fen, Coast |
| 53 | Owlbear | night brawler | Forest, Taiga, RedwoodGrove |
| 54 | Gnoll | pack 3, charges | Savanna, Badlands, Desert |
| 55 | Stirge | flying brawler | Underground |
| 56 | Gelatinous Cube | slow brawler | Underground |
| 57 | Hell Hound | pack 2 brawler | Nether |
| 58 | Yeti | brawler | Tundra, Alpine |

Bodies: `adventure/fauna.rs` (animals, drawn by the wildlife pass) and
`adventure/monsters.rs` (hostiles, drawn by `bestiary::draw_body`). Melee
damage moves to an optional `melee` field on `Species`; `folklore::melee_damage`
reads it first.

### Hell realms and crossing (found in flight, not built here)

The shared tree already carries the realm session's work: Hell planets on
their own generators (`engine_voxel::HELL_*_GENERATOR`, 143 to 159, mirror at
150), player-built thresholds (`world/hell_gate/built.rs`), and a multiplayer
crossing path (`multiplayer/realm.rs`: `CROSS_GATE` tag 31, host `begin_realm`,
`Session::request_gate`, per-joiner realm baselines). This slice does not
touch those files. The habitat table maps every `WorldType::Nether` world to
the `Nether` realm, so Hell rows need no further wiring once a realm is
entered. The gate 139 idea from the first draft is withdrawn.

## Left to the builder

Body proportions, exact weights, test seeds.
