# Tool-material progression workbench

Inspect existing material tiers and preview their effect on ore harvest gates
before changing balance. The CLI and authenticated Forge HTTP API share one
native-validated, revision-checked publication path.

```powershell
node tools/axiom-dev/material-progression-cli.mjs inspect copper
node tools/axiom-dev/material-progression-cli.mjs preview copper 2
```

The second command does not save. It shows gained and lost harvest-tier eligibility
and returns the exact `request` and `approval`. At the current baseline, copper
moving from tier 1 to tier 2 makes a matching unmodified pickaxe meet the iron,
hematite, magnetite, gold-quartz and crystal requirements. Platinum and mithril
remain gated. No balance change is supplied as a default.

After reviewing the preview, pipe its request plus approval to `apply`:

```powershell
$preview = node tools/axiom-dev/material-progression-cli.mjs preview copper 2 | ConvertFrom-Json
# Review $preview.impact and $preview.files before saving.
$request = $preview.request
$request | Add-Member -NotePropertyName approval -NotePropertyValue $preview.approval
$request | ConvertTo-Json -Depth 8 | node tools/axiom-dev/material-progression-cli.mjs apply
node tools/axiom-dev/material-progression-cli.mjs revert BUNDLE_ID
```

Apply writes only the selected existing material's `mining_level` in
`assets/tools/materials.ron`. IDs, row order, other stats, ore placement and player
saves remain unchanged. Revert refuses to overwrite later material edits. Lowering
a level can remove access for existing tools after rebuilding; review lost gates.

An optional final schema ID changes the comparison, such as `preview copper 2 axe`.
The selected schema limits the comparison, not the saved edit. `appliesToSchemas`
lists all mining schemas affected by this shared material definition.
Only matching mining classes receive the material tier. Schemas with no
mining-level parts remain tier zero. The estimate assumes the selected material
supplies every mining-level part, no modifiers and an unbroken tool. It is not an
assembly simulation, acquisition-path proof, break-speed test or gameplay test.
Zero-tier gates can be satisfied by the hand even with a different tool class.

Rules come from `assets/mining.ron`, plus the existing native Dig/tier-0 fallback
for the ore blocks in Forge's deposit registry. This is not a complete inventory
of every default-tier world block. Dependency versions cover the material table,
tool schemas, harvest table, native mining/assembly implementation and adapter.
Changes invalidate old previews. Native validation uses the existing tools parser;
stale validators block preview/apply rather than being silently skipped. Refresh
them with `node tools/axiom-dev/build-validators.mjs` in a coordinated build window.
Read-only `inspect` remains available before the rebuild.

Rebuild and relaunch Axiom after applying. Multiplayer peers need matching content.
No hot reload, new ore/material identity, generation-version change, texture import
or automatic tech-tree unlock is added. Forge's existing generic field editor remains
available; this slice adds no new graphical panel.

HTTP: GET `/api/material-progression?path=copper`; POST
`/api/material-progression/preview`, `/apply`, `/revert`. POST preview requires the
`version` from inspect, `materialId`, `miningLevel`, and optional `schemaId`.
Existing local session and origin checks apply.

## Reference and verification

The user-supplied `New Mods-20261106T153240Z-1-001.zip` contains
`TConstruct-1.12.2-2.13.0.179.jar`. Its `assets/tconstruct/lang/en_us.lang` describes
mining level as the block range a head material can mine. That verified concept
motivates this workflow. Axiom's existing material and mining rules remain the
implementation; no mod code, numerical balance or assets are imported.

```powershell
node --test tools/axiom-dev/material-progression-data.test.mjs
node --test tools/axiom-dev/material-progression.test.mjs
```

The second suite requires current validators and tests real native validation,
preview/apply/revert, stale dependencies, exact review approval, CLI parity and
HTTP authentication in a disposable checkout. It does not verify native rendering,
multiplayer or mining feel.
