# Portal layout authoring

Open **Graph → Dimensions & portals**. Choose Surface, Hell or Manyfold, add two openings, then choose **Link** on the source and **Link here** on the destination. Select a card to edit its name, centre in metres, facing, and aperture. Press Enter or leave a field to commit it. Drag cards to arrange the graph; graph placement does not move the opening in the world. Select a wire to enable/disable it or allow a return crossing.

**Review & publish** runs the engine's Rust schema validator, shows the changed openings/links and route warnings, and saves the reviewed catalog revision. Undo/redo affects the draft. **Saved revisions** restores a publication only when the current file still matches that publication. Reload asks before discarding a draft. A changed source revision or changed approval hash prevents publication.

## Build and activation

```powershell
node tools/axiom-dev/build-portal-validator.mjs
node tools/axiom-dev/server.mjs
```

The native validator is a small offline build using the actual `engine_content::portal_layout` source. The server prints its loopback URL. Existing native catalog validators also need rebuilding after installing the new content module, because their source fingerprint includes `crates/content`.

The authoritative catalog is `assets/portal_layout.ron`. **Rebuild and relaunch Axiom after publishing**, and use the same build on every multiplayer peer. The catalog is embedded and immutable during play. It is not a world-save editor or a hot-reload protocol.

The default catalog is empty. Existing crystal links, Hell realm transfers, Manyfold gallery gates, terrain and world-save formats are untouched. Authored openings are additive and apply to all seeds/worlds of the selected family. Surface includes Planet, Superflat, Hills and Ridged; Hell maps to Nether. Hall is excluded. This first version has no per-save or per-seed scope.

## Runtime behavior and limits

- A `Fold` connects two openings in the same dimension. It supplies real renderer frames and real player traversal. A two-way link creates the inverse portal, preserving camera orientation and momentum through the existing transform.
- Native validation rejects unsupported dimensions/kinds, missing or duplicate IDs, duplicate outgoing routes, self-links, coincident connected centres, non-finite/out-of-range coordinates, mismatched sizes and oversized graphs. One-way links without a graph return path and disconnected openings produce warnings. Cycles with a return path are allowed.
- Opening centres and sizes use metres independently of voxel scale. Openings are vertical; facing is a yaw angle. The existing upright controller still rejects non-upright player gravity frames. Manyfold uses its existing radial crossing/landing controller.
- The graph does not carve terrain, place a physical frame, or stream an exit on demand. The runtime's existing body-fit, readiness and swept collision checks remain in force. A route preview cannot certify that a particular generated world has a clear landing.
- Up to 64 openings and 32 links are supported. Saved gates take priority at the engine's 128-portal network limit; an authored group that would exceed it is withheld as a whole, including its return links.
- Custom cross-dimension travel needs a realm-transfer adapter and is rejected. Hell's existing save/inventory transfer mechanism is unchanged. New dimensions/generators, arbitrary gravity frames, entity passage and projectile passage are not implemented here.
- Shared movement code runs on the multiplayer host. Authored links are not an extra client command and cannot bypass the host's collision decision. The existing executable compatibility check includes embedded content; the SDR source/content digest includes `.ron` assets. A different layout requires a matching rebuilt peer. A rendered multiplayer session has not been newly verified for this feature.

## Human and AI operations

The browser and command line use the same `portal-operations.mjs` reducer, `portal-service.mjs` preview/publication path and native validator. The source editor and candidate bundles also validate the catalog, so bypassing the graph UI does not bypass semantic checks.

```powershell
node tools/axiom-dev/portal-cli.mjs help
node tools/axiom-dev/portal-cli.mjs inspect
```

Pipe `{ "version": "<inspect version>", "operations": [...] }` to `portal-cli.mjs preview`. Operations are `add_node`, `update_node`, `remove_node`, `connect`, `update_link`, and `remove_link`. The help output describes their fields. To publish, send the exact request plus the returned `approval` to `apply`. Use `{ "bundle": "<publication id>" }` with `revert`. Preview never writes. Publish and restore use Forge's existing backup and revision checks.

Node IDs are stable. Positions are `[x,y,z]`; `half_size` is `[width/2,height/2]`; `canvas` is editor-only `[x,y]`. `connect` takes `{id,kind:"Fold",from,to,two_way,enabled}`. Remove referring links before removing an opening. Changes to linked aperture sizes can be submitted together in one operation batch.

## Major asset flows

This implementation establishes a reusable pattern: exact catalog IDs as nodes, native relationship types as links, contextual fields, shared operations, native preview, and revision-checked publication. It does not convert every asset family to a new graph.

| Existing flow | Next meaningful typed links | Constraint to preserve |
| --- | --- | --- |
| Recipe/advancement wiring | Ingredient → recipe → output; advancement parent | Existing catalogs already own these links; reuse their operations |
| Tool/material editors | Tool schema → part kinds → material capability | Keep saved schema/material row identities |
| Sprite/model studios | Asset record → sprite/model → authoring source | Keep generated art and editable source distinct |
| Biome/deposit/habitat editors | Biome → deposit/spawn rule → creature | Validate IDs against native registries before publication |
| Dimension layouts | Dimension → opening → Fold → opening | This slice implements openings and folds; realm transfers require a separate adapter |

The graph's **Other asset flows** disclosure links back to Recipe Wiring, Advancement Wiring and the Catalog rather than creating duplicate catalogs.

## Verification

```powershell
node --test tools/axiom-dev/portal-layout.test.mjs
cargo test --locked --offline --manifest-path tools/portal-native-check/Cargo.toml --target-dir target/portal-validator-build
cargo check --locked --offline -p minecraft_style_voxel_prototype
```

The Rust harness compiles the exact portal math and authored-network adapter from the app. Its build script only removes app module declarations from the copied portal core; the math is unchanged. It tests catalog round-trip, forward/return transforms, momentum, blocked exits, body fit, one-way/disabled routes, dimension isolation and stable IDs. It does not substitute for a full in-game traversal or multiplayer playtest.
