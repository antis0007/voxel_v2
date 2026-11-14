# Assembled-tool behavior authoring

Forge can edit existing assembled-tool schemas through one native-validated command path. The controls change actual `ToolAssembly` capabilities consumed by mining and offhand blocking. They do not introduce a second tool registry or instance class.

## Human journey

1. Run `node tools/axiom-dev/build-validators.mjs` after changing native content code. Set `CARGO_TARGET_DIR` to a drive with sufficient space.
2. Open Forge using `tools/Open-AxiomDev.ps1`, then Graph > Reference Map > Materials and alloying > Edit tool behavior. The same action appears on `assets/tools/parts.ron` nodes and selects the containing schema for nested fields.
3. Choose an assembled tool. Toggle Mining and Offhand blocking; choose the mining style. Advanced behavior exposes charged mining and bamboo cutting, plus tree felling for Cut mining.
4. Review changes runs the real native parser and semantic checks. Apply saves exactly that reviewed revision through the existing reversible bundle store. Undo saved change refuses to overwrite later edits.
5. Rebuild and relaunch Axiom. Definitions are compiled into the engine; this workflow does not hot reload them. Multiplayer peers need matching content builds.

The dialog blocks accidental switching or closing while a draft exists, checks other editor drafts before writes, and refreshes clean source documents after saving. Reload explicitly discards this dialog's draft. Existing schema IDs, part layouts, material assignments and item save encoding remain unchanged.

## AI and automation

The UI's authenticated HTTP endpoints and CLI call the same `toolBehaviorCommand` implementation:

- `node tools/axiom-dev/tool-behavior-cli.mjs inspect [schemaId]`
- `node tools/axiom-dev/tool-behavior-cli.mjs preview` reads a JSON request from stdin.
- `node tools/axiom-dev/tool-behavior-cli.mjs apply` reads that request plus the exact preview `approval` from stdin.
- `node tools/axiom-dev/tool-behavior-cli.mjs revert <bundleId>`

A preview request is `{ "schemaId": "pickaxe", "version": "<inspect version>", "capabilities": <complete capabilities object from inspect> }`. Modify only the desired values. Read schema IDs, mining choices, defaults and the revision from inspect rather than copying stale examples. Preview returns before/after capabilities, exact file text and the approval hash. Apply revalidates both the request and revision; it returns the reversible bundle ID. Invalid combinations, unknown fields, altered approvals and stale revisions are rejected before publishing.

HTTP: GET `/api/tool-behavior` (optional `path=<schemaId>`) and POST `/api/tool-behavior/{preview,apply,revert}` accept the same payloads. They retain Forge's existing origin/session checks. This control does not send prompts or project content to an AI provider.

## Sources and extension points

- `assets/tools/parts.ron`: one capability record per existing schema.
- `crates/tools/src/capabilities.rs`: native capability types, enum choices and semantic rules. `MiningClass::ALL` comes from the same declaration as the serialized enum.
- `crates/tools/examples/check_tool_parts.rs --describe`: native discovery; normal path/stdin modes still validate complete catalogs.
- `tools/axiom-dev/tool-behavior.mjs`: shared revision/preview/apply/revert commands, reusing native receipts, RON syntax editing and the existing bundle store.
- `crates/app/src/world/mining.rs` and `adventure.rs`: existing gameplay consumers. Broken tools still follow existing runtime restrictions.

The editor discovers native boolean capability fields and mining choices; friendly labels are optional presentation. A new engine primitive still requires Rust implementation, native types/validation and meaningful tests. Damage profiles, ranged attacks, new effect types, ore placement, material identities and per-instance overrides are outside this control. This is not arbitrary Minecraft mod import support.

## Verification

`node --test tools/axiom-dev/tool-behavior.test.mjs` exercises native discovery, semantic rejection, no-op handling, exact review approval, stale edits, concurrent-edit-safe undo, CLI parity and authenticated HTTP writes in a disposable fixture. Build the current validators first. The fixture does not edit player saves or shared content.

Native verification used the actual bronze/stone/wood Pickaxe assembly and its encode/decode methods. Changing blocking and charged mining through the real Forge UI changed both runtime capabilities from false to true after rebuilding. The encoded size stayed 65 bytes and computed durability stayed 460; UI undo restored the exact original source revision. This verifies the native data and save-codec path, not a rendered combat playtest or multiplayer session.
