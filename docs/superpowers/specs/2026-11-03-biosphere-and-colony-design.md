# Biosphere stats and settlement management: design

Status: proposed, awaiting owner approval. No new proper nouns or lore are introduced; every term below is a plain noun and can be renamed.

## Intent

Two linked additions:

1. **Biosphere stats**: a readout of the living state of the land around the player. Wildlife, plant cover, climate. It is built from simulation data the engine already holds, not from new simulated quantities.
2. **Settlement management** (RimWorld-inspired): villagers who live at a player base, take jobs from zones and stockpiles the player marks, and have needs that the base must meet. The player stays first-person. The colony is a layer on the voxel world, not a separate top-down game.

The biome audio work (fauna beds, grazer calls) is the first slice of "the land feels alive". Stats make that state readable. The settlement gives the player a reason to care about it.

## Goals

- Stats come from data that already exists: `landscape::sample` (biome, climate), `adventure.enemies` / wildlife herds, thermal ambient and the plant blocks.
- The settlement runs on the fixed 1/120 s tick with a `Budget`, is deterministic (positional hashing, no `rand`, no `Instant`), and runs headless so scenarios can test it.
- Villagers do real voxel work: haul items between containers, mine marked cells, build from blueprints, farm and tend herds. Their effects go through the same edit paths as the player's, so meshing, water and multiplayer edit sync work unchanged.
- No save version bump in phase 1. Settlement state lives in a sidecar file, following the pattern `travel/sidecar.rs` already uses.

## Non-goals

- No top-down camera or RTS selection box. Orders are given in first person with a designator tool and a panel.
- No mood/trauma storyteller, raids or relationship sim in phase 1.
- No new ecosystem simulation (populations, food webs). Stats describe the state; they do not drive it.

## Anti-goals: failure even if it works

- Villagers that teleport, clip through walls or edit cells out of reach. Every job is reachable by pathing or it is not taken.
- A tick cost that grows with base size without a bound. The scheduler has a fixed per-tick budget, and jobs queue rather than run late.
- UI that explains itself in prose (`UI_RULES.md`). Panels show states, nouns and numbers.
- A colony that makes the player's own crafting and building pointless. Villagers work slower than the player and need supplies.

## Part 1: Biosphere stats

### Data (all read-only)

| Stat | Source | Unit |
|---|---|---|
| Biome, and the blend weight of a neighbouring biome | `landscape::sample` at the player plus a ring of 8 samples at 64 m | name, % |
| Temperature, humidity | climate fields on the same sample, thermal ambient | °C, % |
| Animals within 96 m, grouped by species | `adventure.enemies` + wildlife herd identities | count per species |
| Young/adult split, tamed count | wildlife age and owner fields | count |
| Plant cover | fraction of surface columns in a 33×33 grid (2 m spacing) whose top block is a plant, leaf or crop class | % |
| Water cover | same grid, top block water | % |
| Sound activity | the current `Fauna` voice (birds, frogs) | 0–100 |

The plant cover sample is 1089 `world.get` column-top lookups. It runs on the existing 0.5 s soundscape probe cadence, spread across ticks with a `Budget`, and is cached by player chunk.

### Presentation

- A **Biosphere** tab on the existing map panel. No new keybind, because `keybinds.ron` has peer edits.
- Rows: biome name with blend bar, temperature, humidity, plant cover, water cover, then a species table (icon, count, young). Numbers carry units. No prose.
- Later: a map overlay layer that tints sampled plant cover per map tile.

### Module layout

- `crates/app/src/world/biosphere.rs`: `Census { biome, blend, temp_c, humidity, plants, water, species: Vec<(SpeciesId, u16, u16)> }` and `fn survey(world, at, budget) -> Option<Census>`. Pure apart from reading the world. Unit-tested on a fixture world.
- `crates/app/src/egui_ui/map/biosphere.rs`: the tab, added to `text_rules.rs` once it is clean.

## Part 2: Settlement management

### Who the settlers are

Assumption, needs owner confirmation: settlers are **villagers recruited from existing villages**. Trade reputation, or a service in `travel/services.rs`, lets the player invite one to their base. This reuses existing villager models and introduces no new faction.

### Core loop

1. The player places a **settlement marker**: a block, data in `assets/machines.ron` as a mode on an existing block if possible, to avoid a save bump. Its radius (48 m to start) is the base area.
2. The player designates with a tool (a mode on the existing wand/tool, or a held item):
   - **Stockpile zone**: an area of cells; haulers bring matching items to containers inside it.
   - **Mine designation**: cells to dig.
   - **Blueprint**: ghost cells of a block to place; builders fetch the material and place it.
   - **Farm plot**: cells to till, sow and harvest (uses existing crops).
   - **Pen**: an area where tamed herd animals are fed and sheared (uses husbandry).
3. Villagers pick jobs by priority and skill, path to them, work, and haul the outputs.
4. Needs (food, rest, shelter) fall over time. Unmet needs slow work, and in the end the villager leaves. This is the pressure that keeps the player supplying the base.

### Simulation design

```
settlement::State
  marker: IVec3, radius_m
  zones: Vec<Zone>            // stockpile | farm | pen, as cell AABB lists
  designations: BTreeMap<IVec3, Designation>   // mine | place(BlockId)
  settlers: Vec<Settler>
  jobs: JobBoard

Settler { id, pos_m, path: Path, job: Option<JobId>, carry: Option<ItemStack>,
          needs: [u8; 3] /* food rest shelter, 0-255 */, skills: [u8; 4], priorities: [u8; 6] }
```

- **Job board.** Jobs are created from designations and zone deficits (stockpile wants item X; farm cell is ripe). Each job has a target cell, a kind and a reservation (settler id). It is rebuilt incrementally from the `World` change journal (`afb92d31`), not by rescanning.
- **Assignment.** On each settlement tick (10 Hz, every 12th fixed step), idle settlers score open jobs: priority × skill / path cost estimate (octile distance), and the best one is reserved. Ties break by `hash(seed, settler, job)`. Deterministic.
- **Pathing.** A* on a walkable-cell graph (2-high air above a solid cell, step up/down 1), limited to the settlement radius plus a margin. It is bounded by a node budget per tick and resumable across ticks. A path is invalidated when the change journal touches a cell on it. Unreachable jobs are marked and retried after 30 s, so they cannot spin.
- **Work.** Mine and place go through the same world edit entry the player uses (with cellular wake, remesh and MP edit sync). They are timed by block hardness × skill. Settler reach is 1 cell.
- **Movement.** Kinematic, metre space, using the same collision helper as grazers (wildlife bodies). Settlers move only on settlement ticks and are interpolated for render.
- **Budget.** `settlement::tick(world, budget)` is called from `simulation::fixed` after adventure, with a separate cap (for example 0.3 ms window, unbounded headless). It stops early and resumes.
- **Chunk residency.** Settlers outside loaded chunks freeze. Their job reservations drop after 60 s.

### Persistence

The sidecar file `settlement.ron` sits next to the save and follows the `travel/sidecar.rs` load/merge pattern. Missing or corrupt means no settlement, and the world is unaffected. A save bump is deferred until the format settles (phase 3), and it would then go through `just test migrate`.

### Multiplayer

The host is authoritative. Settler poses ride the prioritised entity snapshot (`engine_netcode`), and edits ride the existing cell deltas. Clients send designation commands as reliable messages. Phase 1 can be single-player only, behind a host check.

### UI

- **Settlement panel** (map panel tab or a page in the front end): settler rows with need bars, current job, and a priority grid (job kinds × settlers, 0–4), as in RimWorld's work tab. There is also a job queue count per kind.
- **Designator HUD**: the current designation kind, cell count, and a ghost overlay. The ghost blocks are drawn in the existing selection/preview pass; see the render skill before touching it, and capture A/B per the visual rule.

### Content (RON, no Rust tables)

`assets/settlement.ron`: job kinds with base times, need decay rates, skill names, recruit cost, and the food item values. A round-trip test is included.

## Phases

| Phase | Scope | Proof |
|---|---|---|
| 0 | Biosphere stats tab | unit test on fixture; capture of tab |
| 1 | Marker, one settler, haul + mine designations, sidecar, headless scenario `settlement_haul.ron` | scenario: 20 items moved, 30 cells mined, deterministic over 2 runs |
| 2 | Blueprints, farm plots, needs, priority grid | scenario: builds a 5×5 hut from a blueprint |
| 3 | Pens/husbandry jobs, multiple settlers, recruiting, MP, save version bump | MP lobby E2E; migrate test |

Each phase ends with `just verify` and a bench check that the settlement tick with 8 settlers costs under 0.3 ms p95.

## Risks

- **Pathing cost in caves and on cliffs.** Bounded by the node budget and the radius cap; measured in phase 1.
- **Edit storms waking cellular water** (see the cave-water backlog memory). Settlers mining near water create CA work. Mining is limited to 1 cell per settler per 0.5 s.
- **Peer ownership.** Wildlife, multiplayer and keybind files have uncommitted peer edits. Phase 0 and 1 touch only new files plus one call site in `simulation::fixed` and one tab registration.

## Owner decisions needed

1. Settlers recruited from villages, or something else (for example, robots from the anchor ruins)?
2. Is "settlement" acceptable as the plain noun for the base? It is not a proper noun.
3. A first-person designator only, or should an overhead planning camera be on the table later?
4. Should needs make settlers leave (soft failure), or only slow them?
5. Should Biosphere stats sit on the map panel tab (proposed) or on its own key?

## Left to the builder

The exact scoring weights, the walkable-graph representation, ghost overlay styling (within the visual rules), and the panel layout details.
