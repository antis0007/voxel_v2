# Undercrown schema and interaction contracts

Status: **contract milestone implemented and verified; no authored Undercrown content is generated yet.**

This pass establishes the data model, persistence layout, interaction protocol and reservation rules that the
authored region will use. `Plan::for_config` deliberately returns an empty plan, `CURRENT_GENERATOR_VERSION`
remains 18, and production saves remain `VXWSAV16`. The only player-visible change is that the existing prompt
card can display a mechanism prompt; with no props it never does.

The content design is `docs/superpowers/specs/2026-10-19-undercrown-region-design.md`. The first playable
slice authors 18 rooms. The other scenarios in that design stay a catalogue until a later slice.

## Files

| File | Purpose |
|---|---|
| `src/world/interaction.rs` | Shared `Prompt { title, action }` used by relics and mechanisms |
| `src/world/undercrown.rs` | Module gate, plan cache, block/reservation/location/focus/interact entry points |
| `src/world/undercrown/plan.rs` | Districts, rooms, links, props, actions, empty plan and test fixture |
| `src/world/undercrown/state.rs` | Permanent `ObjectId`, bounded `State`, flag semantics, encode/decode |
| `src/world/undercrown/interact.rs` | Reach/aim/line-of-sight focus, action dispatch, cooldown, edit contract |
| `src/world/trace.rs` | Shared metre-space DDA, moved from `adventure/trace.rs` |
| `src/world/sites.rs`, `src/world/dungeon.rs` | Footprint reservation hooks |
| `src/world/storage.rs` | Dormant v17 footer/decode layout and synthetic round-trip test |

## Identity

```text
district: 4 bits (16)
room:    10 bits (1024 per district)
prop:     8 bits (256 per room)
```

`ObjectId` is permanent. IDs are authored explicitly, never derived from array order or coordinates. Reordering
content must not renumber saves. `District` needs an explicit `index(self) -> u8` match. A derived `as u8`
would renumber saves if the enum is reordered.

Prop `0` on a room id is the room itself (`KNOWN`, `DISCOVERED`, `FAILED`). Props `1` and above are mechanisms
and people. The first slice allocates:

| District index | District | Rooms |
| --- | --- | --- |
| 0 | Rootbound Halls | 1 well, 2 landing, 3 counterweight hall, 4 broken stair, 5 service corridor, 6 door behind the first camp, 7 pilgrims' kitchen, 8 chapel growing through the prison |
| 1 | Crown Chasm | 1 overlook, 2 bridge village, 3 hall of patient bells, 4 freight cradle, 5 bell, 6 observatory, 7 listening stair, 8 stone giant's pocket, 9 observatory stair |
| 2 | Drowned Borough | 1 chain lift, 2 air-bell street, 3 courthouse, 4 reservoir control, 5 ferry, 6 maintenance duct, 7 bathhouse, 8 aqueduct house |
| 3 | Lantern Gardens | 1 orchard mouth, 2 seed bank |
| 4 | Ashen Foundry | 1 furnace mouth |
| 5 | Glass Archive | No rooms yet. Index reserved. |
| 6 | Frozen Choir | 1 choir antechamber |
| 7 | Hollow Sea | No rooms yet. Index reserved. |

These numbers are the save identity. Room display names live in the region design. Do not compact the table
when a room is cut; mark it unused.

## State

`State` is a sorted `BTreeMap<ObjectId, u32>` with at most 2048 records. Known flags:

| Flag | Meaning |
|---|---|
| `DISCOVERED` | The object has been observed or focused |
| `OPERATED` | A one-shot mechanism has moved |
| `REPAIRED` | A durable repair has been completed at least once |
| `OPEN` | A gate or sluice is currently open |
| `RESTORED` | A bell, lift or system has been restored |
| `CLAIMED` | A reward has been claimed at this object |
| `KNOWN` | The player has learned a route or fact |
| `FAILED` | A local objective has been lost (history only) |

Flags record **history and knowledge, not permission**. `OPEN`, `REPAIRED` and `RESTORED` never make a route
usable by themselves; the actual voxels and mechanism are authoritative. Destroying a repaired bridge removes the
route even though `REPAIRED` remains set. `CLAIMED` prevents duplicate reward grants.

Unknown flag bits are masked on write and rejected on load. Capacity exhaustion returns `false` without
disturbing existing records.

## Serialization (dormant v17 contract)

The undercrown block belongs after `adventure` and before `sandbox`:

```text
u32 count                         // <= 2048
repeat count times, sorted by raw id:
    u32 object_id_raw
    u32 flags                     // subset of 0x00FF
```

Decoding rejects oversized counts, invalid raw IDs, unsorted/duplicate IDs and unknown flags. Trailing or
truncated data is an error.

`SAVE_VERSION` is currently 16 and `MAGIC` remains `VXWSAV16`. `session_footer(world, save_version)` writes the
block only for version 17 or later, and `load_exact` decodes it only for version 17 or later. A synthetic v17
manifest in `storage.rs` proves the layout now. `save_version` temporarily accepts 1–17 so that test and future
reads work; production still writes 16.

## Interaction

Constants: reach 3.4 m, aim radius 0.55 m, focus height 0.9 m, cooldown 0.35 s.

Focus requires finite eye/forward, target within reach, perpendicular offset inside the aim radius, and an
unobstructed segment using the shared `trace::solid` DDA. The nearest valid prop wins; an occluded nearer prop
cannot hide a visible farther one.

`interact` returns `Outcome { used, changed, message }`. Reliquary focus takes priority; the caller asks for a
mechanism prompt only when no relic is focused. A one-shot action returns `used = true` with an explanatory
message on repeat, but no edits and no flag change. A failed focus returns `used = false` and does not consume
the cooldown. Edits are applied through `world.set_many`; `changed` contains only cells whose contents differed.
`main.rs` marks those blocks and their chunk neighbours dirty for remeshing.

The current action dispatch records flags only. No content produces voxel edits yet.

## Generation integration

`sample_block_in_column` calls `undercrown::block` before random sites and caverns. Reservation hooks keep random
caverns, dungeon cells, surface sites and homesteads out of the footprint. With an empty plan every hook returns
`false` immediately, and the v18 generation hash is unchanged.

## Version activation checklist

When the first authored content lands:

1. Bump `CURRENT_GENERATOR_VERSION` to 19.
2. Set `SAVE_VERSION` to 17 and `MAGIC` to `VXWSAV17`.
3. Replace `Plan::for_config` with the authored plan.
4. Use the district/room allocation table already recorded above. Do not renumber it.
5. Replace the v18/v19 equality test with a v18 golden-hash regression test; v19 is now allowed to differ.
6. Add connectivity, water-outcome and reward-duplication tests for the real rooms.
7. Add the counterweight / ring / cistern content and the first equipment item.

## Verified in this milestone

- `cargo test --offline --target-dir target`: **55 passed, 0 failed, 4 ignored** (full suite).
- Undercrown contract tests: 12 passed, plus the storage v17 and v18/v19 generation guards.
- Golden generation test: v18 and an empty v19 sample identical over a fixed grid.
- Synthetic v17 save: state round-trips through `session_footer`/`load_exact`.
- v16 save load leaves the state empty.
- Existing trace, inventory, storage, site, dungeon and wand tests remain green.

## Explicitly not implemented

- Any room, shaft, well, cistern, chasm, bridge, bell or observatory geometry.
- Any treasure, equipment item, spell verb, NPC, settlement, trade or dialogue.
- Any water-level outcome, durable repair, shortcut or reward.
- Any generator or save-version activation.
- Any performance validation of a large authored cavern.

These remain the content milestones in `docs/superpowers/specs/2026-10-19-undercrown-region-design.md`.

