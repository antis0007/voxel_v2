# Start here — Codex / Astra Light

This package replaces the execution style of the sprawling earlier prompts while retaining their compatible game scope. It is a reviewed design handoff, not a patch to the game. Keep your selected Codex/Astra Light configuration; no undocumented model flags, context sizes, or autonomous-agent capabilities are assumed.

Place the folder somewhere inside the repository, for example `docs/axiom_reviewed/`, or provide Codex its actual accessible path. Do not overwrite existing root instructions. `AGENTS_FRAGMENT.md` is an optional short supplement to merge deliberately, not a replacement for the repository's `AGENTS.md`.

## Paste this into Codex

```text
Work inside the existing Axiom/Meridian Rust + raw-WGPU repository. Use this reviewed package as the product reference. This request authorizes implementation of one bounded, complete improvement, not another rewritten design bible or a whole-engine replacement.

Locate the package containing DESIGN.md, TASKS.md, REVIEW.md and TASK_GRAPH.json. Read the repository's applicable AGENTS.md/override instructions, DESIGN.md D00 and D18, TASKS.md T00, and the current concise tracking/session state. Then read only the source and design sections required by the selected task. Do not load every historical prompt. If a package is unavailable, report its real path problem rather than invent its content.

Preserve these constraints:
- Existing Windows-first Rust/raw WGPU, no Bevy/replacement engine/browser-demo substitute.
- Historical saves/generators, player edits, one authoritative owner per material/item/entity, and active contributors' work.
- Variable physical voxel scale is separate from render LOD, texture density and simulation fidelity.
- Native visual/gameplay evidence is separate from compilation and automated tests.
- Relevant client physics and the existing networking path remain; exact command replication is promoted only where complete-domain replay is proven, with explicit state-sync fallback.
- Early building/storage/traversal must stay convenient; no new compulsory grind, AFK, cruelty, or forced quest sequence.

Begin with T00. Establish the actual checkout, relevant owners, launch/test/capture commands and a repeatable current failure. Previous generator numbers, test totals, source paths and diagnoses are leads, not current facts. Audit only enough to choose and verify the task.

Prioritize confirmed crashes, save/ownership duplication, wrong collision or coverage over cosmetics. If those gates already pass, use reference/FAILURE_REFERENCE.png as a failure reference, not a target to reproduce and default to T03: make one ambiguous ground-surface class physically and visually coherent. T04 vegetation or T05 building/site work is appropriate if verified dependencies or ownership make it the better ready task. Do not simultaneously redesign terrain, trees, shadows, networking and the sky.

Before editing, write a short work card: selected proof, player benefit, actual source paths, ownership/exclusions, one root-cause hypothesis, smallest failing check, acceptance IDs, native reproduction, relevant budget and rollback policy. Pick reversible defaults for minor open details. Ask only when genuinely missing authorization or an irreversible product decision blocks safe work.

Implement the smallest complete change through existing interfaces. Add a focused regression check. Run the repository's applicable validation and fix failures caused by this change. For visual work, launch the actual game where available, capture matched before/after and motion at walking and aerial scales, inspect them, and revise weak results. Writing a Blender script without executing it is not Blender evidence; an offline render is not native acceptance.

For shader work, verify the current Rust/WGSL data layout and pinned toolchain. Do not conceal holes by filling real voids or haze, change shadow combine rules without valid-domain reasoning, disable legitimate shadows, or claim a speedup from instruction counts. For worldgen, use an explicit new-version policy rather than rewriting saved geography. For assets, track all required original sources/exports and use the actual supported importer.

Use one writer per hot path. Never reset/clean/blanket-stage other work, silently alter global permissions, upgrade unrelated dependencies, or loosen tests to obtain a pass. Use isolated disposable fixtures; verify safety before running commands. No commits/pushes or destructive migrations unless the repository/user workflow authorizes them.

Complete and review the selected proof, not merely its first code draft. Run/repeat tests when changes or evidence justify it; do not loop identical full suites without cause. If a required tool/native environment is unavailable, identify the verified limitation, complete safe useful portions, leave exact reproduction steps, and mark the missing evidence BLOCKED or NOT_RUN. Do not fabricate success or expand scope to compensate.

Stop after that proof meets its scoped gates, or when a precise blocker leaves no safe relevant work. Update the existing concise tracker (SESSION_STATE.md only if no equivalent) with exact paths, decisions, command/results, evidence, limitations and the next smallest step. Report IMPLEMENTED, AUTOMATED-TESTED, NATIVE-OBSERVED, MEASURED, and NOT VERIFIED separately. Do not claim that the whole feature or game is complete.
```

## Which file serves which purpose

- `DESIGN.md`: the complete coherent product and subsystem contract; read relevant sections on demand.
- `TASKS.md`: bounded implementation packets with first proofs, exclusions and checks.
- `REVIEW.md`: critical corrections, rationale, sources and remaining unknowns.
- `AGENTS_FRAGMENT.md`: optional small persistent execution policy.
- `SESSION_STATE.md`: an empty handoff template, not a report of current implementation.
- `TASK_GRAPH.json`: dependency graph and acceptance routing.
- `ACCEPTANCE.json`: 122 inherited plus added requirements; initial game statuses are all NOT_RUN.
- `reference/`: provenance and the unchanged inherited-case text; not automatically loaded agent context.
- `tools/validate_spec.py`: checks the package's internal references/DAG/status discipline; it does NOT compile, test, benchmark, or review the actual game.

There is intentionally no requirement to paste the full design into AGENTS.md. Progressive disclosure keeps the active task small without losing the larger design. After completion, reuse the prompt with the next explicit proof recorded in session state; do not re-audit the entire project from scratch unless the relevant baseline changed.
