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

## Husbandry and provoked animals (2026-11-06)

- A neutral brawler grazer fights back once hurt (`wildlife::fights_back`):
  the Boar row sets `brawler`, `attack_range`, `windup`, `cooldown` and
  `melee`, and leaves the wildlife flee path for the generic melee path.
  Peaceful calms provoked residents instead of removing them.
- `lays` on a grazer drops its product under a grown herd member once per
  regrowth. It reuses the member's fleece clock, so the herd sidecar keeps
  its layout and a row cannot have both `shear` and `lays`. Marsh Fowl lays
  `Food(Egg)` (FoodKind 24, sprite 74) once a day after it is fed.
- Feathers are resource 24 (save 66, factory format 20); Marsh Fowl drops
  1-3 on death.

## Creating a creature (panel, agent or by hand)

Kinds from `bestiary::FIRST_GENERATED` (1000) are created rows: the Axiom panel
(**World → Animals → New animal**, **Creatures → New creature**) appends them,
and an agent can write the same row. Everything the game needs comes from the
row and `assets/spawns.ron`.

1. Row: an animal needs `role: Grazer` and `animal_body: Some(49..=52)` to
   reuse the cow, boar, deer or fowl body; other creatures use the generic
   body. `bestiary::row_problems` lists rule breaks; `just test
   creature_contract` runs it over the whole table.
2. Format: the panel writes one-line rows. Run
   `cargo test dump_bestiary_ron -- --ignored` before committing, or
   `bestiary_ron_round_trips` fails on the format.
3. Spawns: add a `spawns.ron` row (panel: **Add habitat**). Rows whose created
   kind no longer has a bestiary row are skipped at runtime; the spawns test
   still fails until they are removed.
4. Saves: residents and herd members of a removed created kind are dropped on
   load; an unknown authored kind is still treated as corrupt data.
5. Playtest: `/summon <name>` places one. Art evidence:
   `tools/Review-Creatures.ps1 -Creatures 'Name,1000'` (or `-Scene fauna|bestiary`)
   writes a lineup capture under `target/creature-review/<label>`.
