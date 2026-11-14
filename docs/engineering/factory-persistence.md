# Factory state and persistence ownership

The live factory and its persistence adapter have separate responsibilities:

- `crates/app/src/world/factory.rs` owns live nodes, network routing and simulation.
- `crates/app/src/world/factory/catalog.rs` owns stable machine identities, their
  iteration order and count, and access to authored `assets/machines.ron` rows.
  One declaration supplies each kind and its persisted numeric ID. A compile-time
  check keeps array order equal to those IDs; existing IDs must never be renumbered.
  The factory facade re-exports `Kind`, `MachineDef` and `MACHINES`, so consumers
  need no module-path changes. Machine balance and recipes remain authored data.
- `crates/app/src/world/factory/persistence.rs` owns the factory format version,
  historical layout widths and binary encoding/decoding. Existing callers use
  the re-exported factory API.
- `crates/app/src/world/factory/persistence/network.rs` owns multiplayer count
  codecs. One checked reader implements exact widths, explicitly accepted legacy
  prefixes, zero-filled tails, surplus rejection and optional value limits.
  Machine kits retain their exact 40-entry tuple and 4096 network maximum. The
  disk limit remains 65536; resource, gas and block arrays retain their existing
  unbounded `u32` values and historical widths. These are distinct contracts,
  not interchangeable capacities.
- `engine_content::factory_resources` remains the current resource catalogue.
  Persistence consumes its current count; historical counts stay explicit in the
  persistence adapter so content changes cannot silently rewrite older layouts.

World save versions and factory format versions are different numbering schemes.
`resources_in_save` translates a world save version into the same resource layout
used by the factory codec. Inventory and expedition persistence keep using that
function. Do not duplicate these widths in a new consumer.

The writer preserves the existing format-8-and-newer fixture layouts. The decoder
also reads the older non-fluid layouts; tests build those fixtures independently
rather than using the current writer to claim backward compatibility.

Encoding settles reserved shipments into destination stock. Decoding restores
stocks and relevant completion receipts, but does not restore in-flight packets.
This behavior remains part of the codec contract.

## Checks

Run the focused compatibility tests with:

```text
cargo test --offline -p minecraft_style_voxel_prototype world::factory::persistence::tests
```

They pin historical widths, exercise every supported writer format, load older
hand-built fixtures, check shipment settlement and truncated records, and cover
accepted/rejected network array widths. Existing factory machine and world-save
migration tests remain necessary for a format change. The extraction itself does
not introduce a new save format or change generator gates.

Catalogue tests also pin all 40 existing numeric IDs and serialized names, verify
unique reversible machine-block mappings, and round-trip every available kind
in each supported writer format (8 through the current format). Run these with
`cargo test --offline -p minecraft_style_voxel_prototype world::factory::catalog`.

Network compatibility tests run with `world::factory::network_tests`. The same
fixtures pass before and after consolidation: JSON/RON tuple identity, every
accepted/rejected width, all machine-kit positions at and above the boundary,
malformed values, full-range resource/gas/block values and historical disk limits.
