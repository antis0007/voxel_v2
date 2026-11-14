# Evidence-driven improvement

Forge's AI panel now offers **Propose Next Move** and **Run Bounded Loop** alongside the existing improvement queue. Select an engine, performance, art or tooling lane, attach an owner or system-board context, and provide a focused brief. A loop investigates at most three moves and stops at the first candidate for review. It never publishes automatically.

## What the commands do

The same authenticated local API is available to the panel and other tools. Use the session token printed by the existing launcher; do not put it in source control.

| Command | Request / result |
| --- | --- |
| `POST /api/agent/context` | `{task, paths?, targets?, symbols?, modDigests?}` returns bounded source excerpts, SHA-256 versions, diagnostics and reference provenance. |
| `POST /api/improvement/next` | Context request plus `{lane}` returns one investigation with owner, pinned dependencies, hypothesis and acceptance criteria, or the missing evidence / prior work blocking it. No model call. |
| `GET /api/improvement/decisions` | Durable proposals, claims and outcomes in this checkout. |
| `POST /api/improvement` | `{provider,model,tasks:[{decisionId}]}` runs the existing audit/repair path on a claimed investigation. |
| `POST /api/improvement/loop` | Context request plus `{lane,provider,model,maxSteps:1..3}` runs finite planning, ownership, audit/repair and an allowlisted check when available. |
| `POST /api/improvement/check` | `{id:runId,checkId:"context-relevance"}` runs the registered baseline/candidate regression for the supported owner. |
| `POST /api/improvement/cancel` | `{id}` stops the controller before another investigation starts. |

Provider requests retain the existing configured Codex, Ollama and LM Studio destinations. Context is sent only when a model run starts. Gathering context and proposing a move are local operations.

## Evidence and ownership

Plain-language briefs search file contents as well as filenames. Explicit tooling scopes prioritize Forge code; ordinary engine briefs do not collect developer-tool tests just because they share common words. Nearby tests are retained as separate counterevidence even when their excerpt overlaps implementation code. Retrieval is lexical, not a complete call graph.

Attached RON and JSON catalogs also trace exact quoted asset paths in native Rust sources. Context keeps each catalog and a bounded native reference per catalog, with the referenced asset path and source hash. These references remain read-only and become stale when the native source changes. Relative include paths and raw strings are supported; computed paths and complete call graphs are not. A partial or unavailable search is reported as a warning. Source references are leads, not proof that a loader or gameplay check ran.

Upload a JAR using the existing mod inspector before attaching its digest. Context reads the server's hashed archive bytes rather than accepting client-written reference summaries. Archive entry paths retain exact casing. Recipes, class-method leads, declarations and license-file paths remain reference facts, separate from proposed Axiom adaptations. Metadata licenses are unverified declarations. Reference contents are untrusted data, never instructions. Original stand-ins are the default; asset reuse needs a separate license determination.

Every decision pins retrieved source versions and attached board versions. Context is rechecked around model calls, before retaining a proposal, and before verification. A stale read-only caller invalidates the run just as a stale editable source does. Missing or evicted JARs require reimport.

The ledger uses an exclusive file lock, atomic writes, immutable acceptance criteria and explicit claimant ownership. It checks all retained Forge receipts and candidate bundles, not only the short UI history. Unchanged completed, blocked, no-change and review-needed attempts suppress repeats. Claims conflict across lanes on the same owner. An abandoned lock, malformed history or full ledger stops planning for inspection rather than silently forgetting work.

Claims currently coordinate this checkout's Forge ledger only. Other worktrees and external agents are not automatically visible. The separate Foundry tools already own cross-worktree experiments, claims and performance comparisons; this change does not duplicate or replace them. Check that work before choosing overlapping engine/performance tasks.

## Verification boundaries

Automatic repair cannot edit tests, fixtures, benchmarks or the Forge verification, ledger, execution and publication guard modules. Rust files with inline tests use a conservative lexical guard: test text, attributes, imports, macros, configuration expressions and test enclosure must remain unchanged. This guard is not a Rust parser or native execution proof.

The registered `context-relevance` check currently supports only `tools/axiom-dev/agent-context.mjs`. It reconstructs anchored edits in separate baseline and candidate snapshots, keeps the harness frozen, runs the same fixed Node test command, requires a reproduced baseline failure and candidate pass with matching test names/counts and no skips, and records output hashes. The model cannot supply a shell command. Receipt reuse rejects changed source, proposal, harness, verifier or saved output. Temporary snapshots are retained for inspection; they are not an operating-system security sandbox.

Other JavaScript candidates are only **Syntax checked**. Rust, WGSL and unregistered runtime changes remain **Checks pending**. A model's proposed check list is never an execution receipt. A regression receipt does not authorize merge or establish visual quality, whole-game correctness, native save compatibility or a performance gain. Human review and relevant native checks remain necessary before adoption. Existing version-checked candidate publication and rollback remain the save path.

## Local verification

Run from the checkout:

```powershell
node --test tools/axiom-dev/agent-loop.test.mjs tools/axiom-dev/agent-loop-http.test.mjs tools/axiom-dev/context-relevance.test.mjs tools/axiom-dev/content-context.test.mjs tools/axiom-dev/agent-checks.test.mjs
```

Tests cover relevance, reference provenance, stale source/dependencies, durable duplicate suppression, ownership, late outcomes, immutable verification, authenticated routes, unavailable providers, bounded-loop stop conditions, baseline/candidate results and cached-evidence tampering. They do not call an external model.

The concrete repair behind this slice preserves nearby inline test evidence that context gathering previously discarded. Its local evidence contains a failing baseline, passing candidate, unchanged harness and exact-byte publisher rollback. That repair was prepared locally, not generated by a nested provider run. The installed Codex-provider demonstration has not run: automatic approval review blocked sending private Forge source to that external provider pending explicit authorization. Controller orchestration is covered locally; successful live provider execution remains unverified.

A supplied CraftingAutomat archive was also inspected against Axiom's assembler. Axiom already checks output space before consuming inputs, so no duplicate assembler behavior was added. Static reference leads do not establish a measured performance advantage or justify copying assets.

## Rendering measurement scope

Foundry labels `gpu_pass_ms` as a measured GPU pass subtotal, not whole-frame GPU
cost. `gpu_coverage.unique_samples` describes timestamp sample availability;
`gpu_coverage.timing_scope` describes the separate limits of pass instrumentation.
The source-audited inventory lists auxiliary work without timestamp coverage,
including atmosphere LUT compute, sky-volume baking, sky probes and portal
passes. This inventory is not verified against a captured binary. Unmeasured
work has unknown cost, never zero cost. Queue and presentation waits are not
part of the subtotal.

Captured `[shader-validation]` log receipts appear in
`auxiliary.shader_validation` and the AI performance packet. They preserve world
shader FNV identity, filtering mode, build-match result and raw-log stream/line.
They describe the logged world program only, not all shaders or visual quality,
and are not associated with individual CSV frames. Missing or malformed receipts
do not become successful validation. Older reports without these fields remain
unknown; gathering context does not manufacture evidence from the current tree.

New native captures append `gpu_sky_probe_ms` for the sky-probe compute dispatch.
It uses the existing optional timestamp query ring and sample identities. The
legacy `gpu_pass_ms` subtotal deliberately excludes it so adaptive cloud quality
and old subtotal comparisons keep their previous meaning. Foundry exposes this
extra cost separately; absent columns remain unmeasured. The probe's subsequent
buffer copy and other auxiliary compute passes are still outside this coverage.
