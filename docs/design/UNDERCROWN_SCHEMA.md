# Undercrown schema and interaction contracts

Status (2026-10-22): generator 19–31 worlds get the box plan (29 rooms as joined boxes, flag-only props). Generator 32 worlds get the authored region below: real spaces, mechanisms that write voxels, claim rewards and a sealed Glass Archive. Save identity (district, room, prop ids and flags) is unchanged between the two.

## Files

| File | Purpose |
|---|---|
| `crates/worldgen/src/undercrown.rs` | Module gate, plan cache, `block`/`reserved`/`location` |
| `crates/worldgen/src/undercrown/plan.rs` | Districts, rooms, props, actions; the generator 19 box plan; `Plan::for_config` picks the generator 32 layout |
| `crates/worldgen/src/undercrown/state.rs` | Permanent `ObjectId`, bounded `State`, flags, encode/decode |
| `crates/worldgen/src/undercrown/v2/mod.rs` | Generator 32 shapes, tunnels, sampler, `Mech` states, `edits`, `current` |
| `crates/worldgen/src/undercrown/v2/layout.rs` | Site search and the Rootbound Halls and Lantern Gardens |
| `crates/worldgen/src/undercrown/v2/layout_chasm.rs` | Crown Chasm, bell tower, observatory, choir |
| `crates/worldgen/src/undercrown/v2/layout_deep.rs` | Drowned Borough, Ashen Foundry, Glass Archive |
| `crates/worldgen/src/undercrown/v2/feature.rs` | Ramps, pillars, bridges, huts, counterweight, tower, waterfall, lava, house fronts, shelves |
| `crates/app/src/world/undercrown/interact.rs` | Focus, actions, mechanism edits and messages |
| `crates/app/src/world/undercrown/rewards.rs` | Claim rewards from `assets/undercrown_rewards.ron` |

## Identity

```text
district: 4 bits (16)
room:    10 bits (1024 per district)
prop:     8 bits (256 per room)
```

`ObjectId` is permanent and authored explicitly. Prop `0` on a room id is the room itself; props `1` and above are mechanisms. Never renumber: a cut room stays allocated.

| District index | District | Rooms |
| --- | --- | --- |
| 0 | Rootbound Halls | 1 well, 2 landing, 3 counterweight hall, 4 broken stair, 5 service corridor, 6 door behind the first camp, 7 pilgrims' kitchen, 8 chapel growing through the prison |
| 1 | Crown Chasm | 1 chasm, 2 bridge village, 3 hall of patient bells, 4 freight cradle, 5 bell tower, 6 observatory, 7 listening stair, 8 stone giant's pocket, 9 observatory stair |
| 2 | Drowned Borough | 1 chain lift, 2 street beneath the air bell, 3 courthouse, 4 reservoir control, 5 ferry landing, 6 maintenance duct, 7 bathhouse, 8 aqueduct house |
| 3 | Lantern Gardens | 1 orchard mouth, 2 seed bank |
| 4 | Ashen Foundry | 1 furnace mouth |
| 5 | Glass Archive | 1 archive (generator 32) |
| 6 | Frozen Choir | 1 choir antechamber |
| 7 | Hollow Sea | Reserved |

## Generator 32 region

The site is the best of 40 seeded hills 220–480 m from spawn whose whole footprint stands over the landing's roof. Levels hang off the surface S at the well: landing L = S − 30, chasm floor C = L − 34, borough B = C − 14.

- **Rootbound Halls (L).** Spiral well down from the surface. A vaulted landing with the creed inscription. A 20 m counterweight hall entered from a gallery 10 m up, with a ramp down, a hanging counterweight and a static jib. Kitchen, chapel and the door behind the camp.
- **The broken stair.** A collapse chamber on the way to the chasm. The first descent is a 4 m drop, so the chasm is one-way until the bell or the repaired platform brings you back.
- **Crown Chasm (C).** A 60 m noise-carved cavern with a rock shelf where the stair arrives and a ramp down. A bridge village of plank huts stands on a pillar. The waterfall falls into a plunge pool. The bell tower has a spiral stair, a bell chamber and three doors.
- **Around the chasm.**
  - The listening stair climbs back to the landing level.
  - The observatory sits on the east cliff, with its stair continuing to a second surface exit.
  - East: the hall of patient bells, with the frozen choir's ice pipes beyond it.
  - South: the freight cradle.
- **Drowned Borough (B), behind the chain lift's portcullis.** A flooded vaulted street with house fronts and a jeweler's balcony.
  - The courthouse sits under 5.5 m of water.
  - Reservoir control holds the sluice.
  - Also: the ferry landing, a duct above the flood line, the bathhouse and the aqueduct house.
- **Ashen Foundry (C + 2).** A lava channel across the furnace hall. The Glass Archive lies beyond it.
- **Glass Archive.** Sealed in tombstone, its passage too, so nothing digs around the door. It holds shelves and a reliquary.

## Mechanisms

A prop's flag implies a mechanism state (`v2::current`). When an action changes that state, `Layout::edits` diffs the sampler between the old and new state over the mechanism's boxes. The app writes exactly those cells with `World::set_many_mechanism`, the only edit path that may replace tombstone. Edited chunks persist like any player edit; flags stay history, not permission.

| Prop | Action | Effect |
|---|---|---|
| 0.4 Freight platform | Repair | A ramp up the collapse chamber |
| 1.2 Repair the water lift | Repair | A bridge from the village to the tower |
| 1.5 Bell clapper | Ring | The listening stair's missing flight returns |
| 2.1 Chain lift | Operate | The portcullis at the lift's foot rises |
| 2.4 Bypass sluice | Open / Close | The courthouse drains / refills |
| 4.1 Cooling gate | Open | The lava channel crusts to basalt |
| 1.4 Receiving platform | Repair | A bridge over the lava (the other crossing) |
| 1.6 Observatory lens | Restore | The archive door opens |

Claiming a plinth grants the row for its id in `assets/undercrown_rewards.ron`. The named relics (ring, gloves, buckler) have no items of their own yet; the rows are stand-ins.

## Interaction constants

Reach 3.4 m, aim radius 0.55 m, focus height 0.9 m, cooldown 0.35 s. Focus needs a clear `trace::solid` line. Every generator 32 prop stands on a pedestal.

## Tests

- `crates/worldgen/src/undercrown/v2/tests.rs` flood-fills real sampled voxels on foot over eight seeds. It checks:
  - with no mechanism set, every room except the borough and the archive is reachable;
  - the winch opens the borough;
  - the archive needs the lens plus a crossing;
  - the chasm is one-way until the bell or the stair repair;
  - every prop has a standing spot in reach;
  - edits are bounded and idempotent;
  - the sluice only drains;
  - ids are a superset of generator 19's;
  - the archive shell is tombstone.
- `crates/app/src/world/undercrown/interact.rs` checks that the bell writes solid stair cells, a player edit cannot open the seal while the lens does, and a claim grants once.
- `rewards.rs` checks that the asset round-trips and covers every plinth.
