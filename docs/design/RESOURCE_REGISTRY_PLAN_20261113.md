# Factory resource registry: plan (2026-11-13)

Goal: adding a factory resource (stone, wood, sand, ...) becomes a data-only
change, with no save version bump. This needs one migration. After it, most
Thermal and Create parity rows become data-only work.

Status: step 1 done 2026-11-13 (uncommitted): `assets/resources.ron`, plus
`resource_table()` and its tests in `crates/content/src/factory_resources.rs`.
engine-cli now also accepts resource keys. Steps 2-6 open.

## Today

- 25 resources, hard-coded in `crates/content/src/factory_resources.rs`
  (`RESOURCE_COUNT`, `NAMES`). This is a content table in Rust, which
  CLAUDE.md forbids.
- Ids are positional `Item::Resource(u8)` indices. `assets/machines.ron`
  (`accepts`, costs) and `assets/recipes.ron` (`Resource(n)`) refer to them
  by number.
- Every save section holding one value per resource has a fixed width,
  inferred from the version: `resources_in_save(version)` and
  `resources_in_format(format)` in `world/factory/persistence.rs:28-50`.
  Each new resource so far (22→23→24→25) needed a save bump plus one more
  match arm.
- `SAVE_VERSION` = 73 (`world/storage.rs:110`), `FACTORY_FORMAT` = 21.
  Check both again before landing; other branches claim numbers (see the
  gate-collision note).

### Places that write or read a per-resource width

| Place | Location |
|---|---|
| Node `input`/`output` (encode/decode) | `world/factory/persistence.rs:87`, `:170` |
| Node filter / smeltery target checks | `persistence.rs:229`, `:278` |
| Multiplayer serde widths | `world/factory/persistence/network.rs:12-24` (`read_counts`) |
| Inventory `resource_remainders` | `world/inventory.rs:2012`, `:2096` |
| Inventory item payload check | `world/inventory.rs:461` |
| Expedition resources, session footer | `world/storage.rs:356` (write), `:766` (read) |

`RESOURCE_COUNT` appears 105 times in 39 files. Most are array sizes
(`[u32; RESOURCE_COUNT]`) or `0..RESOURCE_COUNT` loops.

## Design

1. **Resource table in data.** Add `assets/resources.ron`: one row per
   resource (`key`, display name, later sprite/tags). Rows are append-only,
   and the id is the row index. A test pins the first 25 keys to today's
   order, so ids never shift and old saves keep their meaning. The Rust
   `NAMES` list becomes that test's fixture.
2. **Fixed capacity, count read at startup.** Arrays become
   `[u32; RESOURCE_CAPACITY]` with `RESOURCE_CAPACITY = 64`. That keeps
   `Node` a plain fixed-size value (2 × 256 B) and avoids rewriting it as a
   `Vec`. `resource_count()` comes from the RON table. Startup fails clearly
   if it exceeds the capacity. Loops use the count; array types use the
   capacity.
3. **Saves record their own width (one bump: save 74, factory format 22).**
   Every section in the table above writes its resource count first.
   Readers take `min(saved, current)` entries, fill the rest with zero, and
   refuse a save whose count exceeds the current table ("saved by a newer
   build"). Versions up to 73 keep their inferred widths through the
   existing `resources_in_save` arms, which stay as they are.
4. **Optional, later:** let `machines.ron` and `recipes.ron` refer to
   resources by `key` instead of index, resolved when the RON loads. This is
   data-only and needs no save change.
5. **No saved name table.** The count plus the append-only rule is enough.
   A name table is only needed if resources may ever be removed or
   reordered. Add one then, with a second migration.

## Steps

1. Add `resources.ron`, the loader in `engine_content`, and the
   order-pinning test. Behaviour unchanged.
2. Mechanical change: `RESOURCE_COUNT` becomes `RESOURCE_CAPACITY` in array
   types and `resource_count()` in loops, across 39 files.
3. Migration: bump to 74, format 22, count prefix in the six places above,
   new `resources_in_save`/`resources_in_format` arms.
4. Tests: `just test storage`, `just test migrate`, `just test factory`,
   `just test inventory`. Load an old fixture (≤73) and check that node
   contents, inventory remainders and expedition stocks survive. Add a test
   that appends a 26th resource and loads a v74 save written before that
   resource existed, with no bump.
5. Update the rules. `.claude/rules/saves.md` and the automation skill
   should say that from v74 new resources need no bump, while new machine
   kinds still do. Update the memory note.
6. First data rows: stone, wood, sand, plus sprites in
   `assets/icons/resources16.txt`.

## Risks and coordination

- Step 2 touches about 20 factory files that hold peers' uncommitted edits.
  Do it when the factory owner is quiet, or on a branch from HEAD that lands
  after they commit. A tree-wide rename made in the shared dirty tree will
  conflict.
- Multiplayer: host and client must agree on the resource table. Send the
  count (or a hash of the keys) in the join handshake, and refuse when they
  differ.
- Determinism: unaffected. Ids and tick order do not change.
