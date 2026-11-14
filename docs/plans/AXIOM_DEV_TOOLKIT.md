# Axiom Dev Toolkit specification

The toolkit turns a one-sentence feature idea into an editable, playable candidate with a recorded route to acceptance. It should build routine content immediately in a disposable workspace and break new engine work into dependency-ordered slices. The [delivery backlog](MOD_PARITY_DELIVERY.md), owning system specs and recorded design decisions supply direction. Models propose work and flag missing evidence. The repository owns IDs, permissions, validation and promotion.

## Reuse the current Dev Panel

Extend [Axiom Dev Panel](../../tools/axiom-dev/README.md). It already has a reference graph, idea forms for recipes, spawn rules and advancements, a food creator, sprite/model editors, backed-up saves with conflict checks, native playtest controls, and an AI workbench that stages scoped proposals. Reuse its editors and draft flow. Expose the same planning and staging service to both the panel and a headless runner.

Use the [Haiku sprite generator](../../tools/generate_sprites.py) as a concrete adapter. Separate generation and validation from writes, then return a validated sheet addition and contact sheet to the existing art draft workflow. Preserve the option to paint or approve a replacement in the editor. Keep external-provider calls explicit and send only the task, approved art references and schema. The [local code advisor](../design/CODE_ADVISOR.md) is a possible review signal. The [classifier workflow brief](../specs/downloads-2026-10-24/voxel_claude_fable_classifier_workflow.txt) supplies routing and evidence rules.

## From idea to candidate

1. **Capture and resolve.** Accept a sentence, linked source, matrix ID, graph node or idea note. Find similar features and recover existing work before creating a new ID. Resolve the owning SDD, applicable decisions, content catalog, subsystem dependencies and earliest playable slice. If the idea conflicts with an existing decision, surface the precise conflict for resolution.
2. **Compile a work packet.** Store a stable feature and slice ID, intent and player story, source references and revision, prerequisites, touched contracts and allowed paths, content and art outputs, save/network/generator impact, performance envelope, acceptance criteria, verification policy and rollback. Provide a direct build path for a known content template. If an executor or adapter is missing, open a dependency slice for its owner.
3. **Build in isolation.** Use existing editors and schemas for data. Reserve IDs from current registries without rewriting existing entries. Run a bounded code worker only for missing behavior. Stage generated RON, Rust, WGSL, art, audio, localization, help, and other relevant artifacts together. Use a disposable workspace, separate outputs, and disposable fixture worlds. Coordinate registry reservations and reject stale source, dirty-file overlap and competing ownership before applying.
4. **Check and show.** Validate references, schema, IDs, recipes, reachability, persistence contracts, and resource budgets with deterministic checks chosen for the packet. Run only checks authorized by current project policy. Show a readable diff, art preview, player route, observed failures, and evidence receipts. Bind each receipt to the exact candidate revision, inputs and config.
5. **Promote or repair.** Apply an approved packet as a recoverable bundle with backups or a reversible commit. Recheck the base and affected files at promotion. Repair in the disposable candidate within the run budget, or record the blocker and continue with independent work. A human reviews product choices and changes to authority, save/protocol, published worlds or release state.

| Request type | Toolkit capability to build |
|---|---|
| Add an item, tool part, food, recipe or progression node | Compose stable registries, acquisition and unlock routes, recipe dependencies, stats and labels, art, tooltips, search, inventory and persistence. |
| Add a machine, process, network part or controller | Compose typed ports, resource and energy debits, stages, bounded buffers, upgrades, I/O controls, diagnostics, failure feedback, model and sprite. Reuse existing network and transaction services. |
| Add a creature, spell, item ability or encounter | Compose behaviors, costs, damage/status rules, spawn/arena gates, loot, animation/audio/UI, accessibility and owner-authoritative state. |
| Add a biome, deposit, structure, realm or vehicle | Compose material/climate/world profiles, layouts, physical properties, generation rules, compatibility gates, performance limits, and native capture routes. |
| Improve engine behavior | Prepare a focused repro, ownership and invariants, reference behavior, relevant trace or benchmark, implementation patch and evidence under comparable conditions. Cover CPU/GPU, worldgen, streaming, rendering, scheduling, physics or networking as the owning contracts require. |

## Classifier and token budget

Route deterministic cases from content types, source paths, actual symbol/contract changes, dependency edges and diagnostic codes. For ambiguous packets, call a small local classifier once per meaningful checkpoint. Give it a compact task summary, selected source excerpts or diff hunks, the applicable rules and existing checks. Accept only known labels for affected domains, missing context, extra checks, review type, and `unknown`. Cache by exact input hash, policy and model identity. Timeouts, truncation and malformed results take the deterministic fallback. The classifier can add scrutiny and request context. It has no authority to remove required checks, authorize commands or promote work.

Prefer data templates, deterministic generators and reuse over model calls. Retrieve owning spec sections and exact symbols using the Dev Panel index. Keep a short structured run summary between worker calls and retain full evidence on disk. Escalate to a code model for a bounded implementation packet, and to an independent reviewer for cross-cutting contracts or conflicting evidence. Set per-run limits for attempts, tokens, elapsed time, changed files, build/check work and provider spend. Stop when the evidence is sufficient or the budget is exhausted. Keep local inference out of timed gameplay and benchmark windows.

## Improve from observed outcomes

Record the packet fingerprint, routing rule or classifier result, selected context, time/tokens, diagnostics, candidate revision, accepted changes, human corrections, rollbacks, and subsequent regressions. Keep unresolved or unverified results separate from accepted evidence. When several outcomes support a change, propose an updated template, retrieval rule, routing hint or check. Evaluate the proposal on frozen historical packets and a held-out set before promotion. Compare required-check recall, useful extra-check rate, accepted candidates per thousand tokens, idea-to-playable latency, escapes and rollbacks. Keep a versioned previous policy for rollback. Model confidence and its own review are inputs to this process. Independent evidence determines whether the update improves the toolkit.

## Bounded autonomous backlog runner

Once packet creation and isolated execution are reliable, let an explicitly started run select ready slices from the feature matrix and owner backlog. Rank by unlocked dependencies, player value, available acceptance evidence and remaining budget. Prefer the smallest slice that closes a visible player loop. Claim overlapping paths and registry IDs, recheck the source revision before applying work, checkpoint between slices, and support pause, resume and cancellation. Route blocked slices back to the ledger with a concrete reason and continue with independent ready work. Authority-sensitive or ambiguous choices become reviewable candidates. Candidate promotion and changes to the toolkit’s own policy have distinct review gates.

The long-term target is for the runner to work through the remaining game in successive bounded batches. Routine additions should produce a candidate quickly. Engine features inherit their actual implementation, build and verification costs. Safety comes from isolation, deterministic checks, evidence, rollback and limited permissions at each step.

### Dot readiness

The Minecraft intake can read declared compatibility from Legacy Forge, Forge-style, Fabric, Quilt, and NeoForge descriptors. It identifies dependency leads, nested archives, native libraries, and engine patches. These are intake findings. Behavioral parity requires running the source feature and the native Axiom result through the same player journey.

Before starting a Dot loop, qualify three routes on real mods spanning the loader generations: a content and recipe feature, a stateful production or simulation feature, and a feature that requires new engine behavior. Each route needs a reproducible baseline, obtainable and usable native content, save/reload evidence, and a reversible promotion. Exercise blocked dependencies, overlapping changes, and resume after a failed candidate in the same batch. Keep workflow speed and token use in the receipts. An individual pass can qualify only the workflow and feature it exercised.

## Implementation milestones

| Stage | First usable result | Exit evidence |
|---|---|---|
| 1. Feature intake | Search all matrix IDs in the Dev Panel and create a packet linked to its owner, source and acceptance. | Every canonical ID resolves exactly once. Missing dependencies and overlapping work are visible. |
| 2. Content vertical slice | Build a complete recipe/item variant with a generated icon, draft preview, disposable playtest and rollback. | Show craft, use and persistence in a normal play route. Reject invalid IDs, missing sprites and stale drafts before promotion. |
| 3. Engine vertical slice | Run one machine or simulation change through scoped code editing and build, scenario and performance evidence. | Record the expected failure path and prove the intended behavior under the same fixture/config, within its budget. |
| 4. Classifier and evaluation | Add optional local classification and a versioned evidence-based feedback loop. | Demonstrate reduced context or routing cost against deterministic routing on held-out work, with required-check coverage intact. |
| 5. Batch execution | Let the runner select and complete independent ready slices with durable progress and bounded review. | Demonstrate cancellation, restart, an injected failure, rollback, concurrent-edit protection and continuation past a blocked slice. |

Follow [repository verification guidance](../TESTING.md) and the active user policy when turning acceptance requirements into executable checks. Keep unavailable or unauthorized evidence visible in the receipt.
