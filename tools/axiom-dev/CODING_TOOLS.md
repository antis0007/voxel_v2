# Focused engine design

Use one question and the smallest relevant tool. These commands work from the repository root without a paid model call.

- Known symbol or file: use a bounded `rg -n` search directly.
- Unclear ownership or cross-file behavior: `node tools/axiom-dev/context-cli.mjs "How does mana recovery work?" --symbol mana_recovery`. Up to five optional source paths; compact JSON defaults to 6,000 characters. Repeat `--symbol` for known identifiers; raise `--max-chars` only when missing evidence matters. Omitted counts and excerpt flags identify missing evidence; this is a character budget, not an exact token count. Read cited ranges before editing. Collection uses the existing panel retrieval and may scan multiple files; skip it for trivial lookups.
- Catalog ownership: `node tools/axiom-dev/catalog-cli.mjs describe tool-materials` gives the native schema, reader, validator and activation contract. Use `list` only when the catalog is unknown.
- Live evidence: `node tools/axiom-dev/engine-cli.mjs status`, then `inspect player location` against an already-running panel. Read `ENGINE_WORKBENCH.md` for supported operations. Do not launch a game just to answer a source question. CLI context does not attach live state; the panel's Gather Context supports it.

Design from evidence: state the observed issue, owning subsystem, smallest change, compatibility constraint, and acceptance condition. Distinguish source inference from measured behavior. Keep saved-content changes separate from live previews and rebuild requirements. Preserve authored visuals and concurrent edits.

Implement directly for small tasks. Delegate only a bounded search to Haiku when it saves main-context work; pass the question and relevant paths, not full transcripts. Return references and uncertainty. Avoid parallel design committees and repeated full-tree scans.

Run tests only when requested. For requested performance or visual verification, use one representative baseline and one comparable after capture; report binary/source identity and unverified gaps. Do not claim improvement from source changes alone.

## Background improvement with low-cost models

`node tools/axiom-dev/improve-cli.mjs scan --limit 5` lists small rule-backed chores (stray `unwrap`, stale notes, wall-clock reads in simulation) with the cheapest suitable model tier. `brief <id>` prints a 1,500 to 3,000 character plain-text prompt that works in Claude or ChatGPT; `check <id> <diff>` guards the reply; `done` records the outcome so a chore is never offered twice. The `improver`, `improver-sonnet` and `improver-opus` agents take one brief each. Read `IMPROVE_LOOP.md` before running it unattended.

## Generator gate audit

`node tools/axiom-dev/gates-cli.mjs check` fails when a gate number has more than one literal definition, when a gate constant has no row in `docs/worldgen/GENERATOR_GATES.md`, or when the registry's generated table differs from source. `next` gives the first free number; `index --write` refreshes the generated table (the only write). A number shared by several modules is defined once in `crates/worldgen/src/gates.rs` and each module aliases it. `check` also counts bare numeric comparisons such as `generator_version >= 47`; that count is reported, not enforced.

## Shared tool discovery and graph context

`node tools/axiom-dev/tools-cli.mjs route assets/spells.ron` returns relevant entry points. `describe wiring` adds the editing flow and documentation path. `list` is for initial discovery, not every task. Registry commands describe capabilities; they never execute tools or grant permission to mutate content.

Graph nodes expose **Copy coding context** and **Open copied context** through the context menu and source inspector. The versioned JSON carries a source target, revision and relevant CLI entry points, not entire files or transcripts. Paste it into another agent or checkout's graph dialog; the destination must contain that path. Opening navigates only. Re-read source and revalidate named targets before editing; revisions can differ between checkouts. Native recipe/material copy and paste remains in the owning editors.

## Extend without a parallel editor

Put each feature's semantic operations in a module shared by its panel and CLI. Register its entry point and path routing in `tool-capabilities.mjs`. Keep schema/ownership in the existing native catalog contract, not a second JavaScript definition. Graph controls and contextual actions call the same operations as agents.

Use stable IDs and typed ports for links; reject incompatible connections. Separate navigation/reference copying from content duplication. Duplicate authored content through the owning operation, assigning fresh IDs and preserving references intentionally. Reuse the shared context menu, graph canvas and inspector where their contracts fit. Keep draft preview, undo, revision-checked apply and rebuild/live activation explicit. Do not silently execute commands carried in copied context.
