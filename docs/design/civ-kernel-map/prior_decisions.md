# prior_decisions

## Summary
Read-only pass over docs, manifests and tools; nothing was built, run or edited. (Read-only `git for-each-ref`/`ls-tree`/`status` were used to check ADR numbers on local branches.)

1. Binding decisions. The owner-authored direction doc (2026-11-06, "direction / governing") fixes the shape: explicit phases snapshot -> observations -> proposals -> conflict/reservation resolution -> commit -> publish, a commit stage that enforces conservation, camera/render independence, and "replicate commands where determinism is proven, otherwise accepted outcomes". The province plan adds stable logical order with fixed work limits and a resumable cursor, no universal event bus before a proven use, and deferred or failed writes that never debit stock. Decision 0022 makes per-tick quotas authoritative. Decision 0039 keeps the expansion briefs as backlog, keeps the lockstep plan for integer matter, and says cohort simulation needs its own decision before code. MASTER_PLAN still lists N09 villagers/jobs as Tier B at M3 and W02 civilizations as Tier A at M6. Crate home is an unanswered owner question; the only recorded recommendation is "land in `crates/app` first, promote to `crates/civ` once the record is stable", and not to add a crate while the tree is red. The city port audit says to use engine hashing/ticks rather than a second randomness or time owner, and not to install duplicate authorities. Residents are capped at 64, and multiplayer society authority is unproven.

2. Crate conventions. `members = ["crates/*"]` auto-enrols any `crates/<dir>`. Packages are named `engine_<dir>`, inherit `version`/`edition`, and end with `[lints] workspace = true`. Headless crates keep dependencies minimal (sim: engine_core, bytemuck, glam; chip: serde, ron; netcode: engine_core, glam), with criterion/proptest as dev-dependencies. A new crate changes Cargo.lock, which triggers the policy route (`review_policy`); `just deps` fails unless the crate is committed with its referrers. Editing `crates/core/*` triggers the `seam_route` manual check. Test layouts differ per crate (directory listing): `*_tests.rs` plus `tests/` in sim, `<mod>/tests.rs` in netcode, `tests*.rs` in chip. `docs/adr` runs 0001-0045 with 0013 absent; `docs/decisions` holds 0021-0025, 0034, 0036 and 0039, each duplicating an ADR number. Next free is 0046: no local branch and no worktree checked under `.worktrees`/`.claude/worktrees` has a higher one. The `repo_layout` doc test scans top-level `docs/*.md` and `docs/adr/*.md` for non-existent paths, so an ADR must not name unbuilt `crates/...` files.

3. Doc location. Recent civilization designs sit in `docs/superpowers/specs/2026-11-0x-*-design.md`, execution plans in `docs/plans/`, and evidence reports as top-level `docs/city-*.md`. `docs/specs/` holds preserved source briefs only.

4. Forge. The registry is `tools/axiom-dev/catalog-contracts.mjs` with five contracts: fabrication, spells, tool-parts, tool-materials, reactions. All activate by rebuild. ADR 0042 still names four. `assets/machines.ron` is not registered. The docs read show no end-to-end machine edit path: WORKSHOP stages Fabricator process rows only, with no publish/live install and no save, runtime or multiplayer testing. The live model swap is a render-only session overlay.

5. Hashing. `engine_core::hash` has only `hash_u32(u32) -> u32` and `hash01(u32) -> f32`. Keyed-draw precedents are `engine_sim::pinned::block_base`/`block_hash` (u32) and `engine_chip::lot::mix(u64, u64)`. No Philox, Threefry or other counter-based generator exists under `crates/`.

6. Time. `crates/core/src/time.rs` has `TickId(u64)` (session-local completed 120 Hz player steps, not persisted), `PLAYER_STEP_SECONDS = 1/120` s, `PLAYER_STEPS_PER_CA = 2`, and wall-clock `Budget`. There is no SimTime, day or duration type. The jobs `PULSE` is 0.25 s, i.e. 30 ticks at 120 Hz.

## Summary corrections (verifier)
Of 116 findings, 110 are confirmed at the cited file and line, five are imprecise (A9, A12, A20, B20, D17) and one is wrong against the current disk (E9). The summary is wrong or misleading in these places:

1. **Crate home.** "Crate home is an unanswered owner question; the only recorded recommendation is land in crates/app first" is misleading on two counts.
   - An untracked `crates/civ` (package `engine_civ`) already exists on disk and in `Cargo.lock`. It holds an ordered agenda mirroring the app's `(due, site, actor)` key and the FOUNDING/COUNCIL sentinels, a timing wheel, a canonical encoder/digest and a Philox draw. Its files are dated 2026-11-11 00:32-00:35 local, so they may postdate the reader's pass.
   - A second, later recommendation exists that the reader never read: `docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md` (updated 8 November). It places the work in `world/society` modules inside the app, says the first session must change visible behaviour, and says to start with a heap and add a timing wheel only on measurement.

2. **ADR number.** "Next free is 0046" is no longer safe. No 0046 file exists on disk or on any of the 63 local branches, but `crates/civ/Cargo.toml:4` already cites `docs/adr/0046-civilization-kernel-crate.md`. The `docs/TRACKING.md:445` hit the reader left open is a false match on "0.0057 degrees".

3. **Philox.** "No Philox, Threefry or other counter-based generator exists under crates/" is false now: `crates/civ/src/draw.rs:15` is Philox4x32-10 with known-answer tests. `engine_core::hash` is still only `hash_u32` and `hash01`.

4. **Resident cap.** "Residents are capped at 64" is imprecise. `crew::CAPACITY = 64` (`crates/app/src/world/farming/jobs/crew.rs:4`) bounds the local crew and each community's people (`communities.rs:94`). The 8 November record states 512 retained settlements with a 64-person executor per town.

5. **Doc location.** The answer omits `docs/design/`, where the most recent and largest civilization architecture record was filed. Recent practice is therefore split four ways: `docs/superpowers/specs/*-design.md`, `docs/plans/`, top-level `docs/city-*.md`, and `docs/design/`.

6. **Time types.** "There is no SimTime, day or duration type" is true for `crates/core/src/time.rs`, but it hides the persisted `world.farming.clock: u64` (one increment per 1/120 s fixed step, `crates/app/src/world/farming.rs:62`). That is the unit of the agenda's `due` and of the 30-tick PULSE.

7. **ARCHITECTURE.md.** The task asked for crate dependency rules from it and the summary is silent. The file has none; it has a data-flow diagram, the tick order and a "where things live" table. The only enforced crate-dependency rule is the `engine_sim` test at `crates/app/src/repo_layout.rs:141-158`.

8. **Save version.** Left as an open question; the disk says `SAVE_VERSION = 70` (`crates/app/src/world/storage.rs:91`).

9. **Villager plan status.** Presenting "nothing here has landed" (A9) as current is stale. A `Ledger` exists at `crates/app/src/world/travel/civilization.rs:110` and the community agenda is in `jobs/communities.rs`.

The Forge section holds: five contracts, all rebuild-only, `machines.ron` unregistered. `crates/app/src/world/factory.rs:301` confirms in code that the machine table is compiled in.

## Checks: 110 confirmed, 6 not

- [imprecise] A9 (docs/plans/2026-11-06-villager-civilization-slice.md:3) claim: The villager civilization slice is a plan only; nothing in it has landed.
  correction: The quote at docs/plans/2026-11-06-villager-civilization-slice.md:3 is verbatim, but it is a 2026-11-06 statement and no longer describes the tree. The later record docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:23 lists implemented behaviour ("An ordered, saved wake queue services at most eight community events per fixed tick"), and a runtime ledger exists at crates/app/src/world/travel/civilization.rs:110 ("pub(crate) struct Ledger {").
- [imprecise] A12 (docs/plans/2026-11-06-villager-civilization-slice.md:138) claim: Whether records live in crates/app or a new crates/civ crate is listed as an open question for the owner (section 7, item 3); no answer is recorded in the docs read.
  correction: The open question at docs/plans/2026-11-06-villager-civilization-slice.md:138 is quoted correctly and no owner answer was found in docs. On disk the question is already pre-empted: an untracked crates/civ exists (crates/civ/Cargo.toml:6 "name = \"engine_civ\"", files dated 2026-11-11 00:32-00:35 local) and Cargo.lock:1033 lists engine_civ. The later architecture record instead places the work in the app: docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:555 "Add proposed modules under world/society only as their first behavior lands:".
- [imprecise] A20 (docs/plans/2026-11-05-axiom-causal-world.md:136) claim: Province plan forbids adding a universal event bus before a use is proven; record operation identity and the changes consumers need.
  correction: Line is off by one. The quoted text "changes that downstream consumers need; do not add a universal event bus" is docs/plans/2026-11-05-axiom-causal-world.md:135; line 136 reads "before proving a use. Rendering, map symbols and dialogue read those".
- [imprecise] B20 (docs/adr/0045-completed-geology-cache.md:10) claim: Highest existing ADR file is docs/adr/0045-completed-geology-cache.md (untracked); listing shows 0001-0045 with 0013 absent, and docs/decisions holds 0021-0025, 0034, 0036, 0039, each duplicating an adr number. Next free is 0046.
  correction: The listing is right: docs/adr holds 44 files, 0001-0045 with 0013 absent, and docs/decisions holds 0021-0025, 0034, 0036, 0039. No file numbered 0046 exists on disk or on any of the 63 local branches. But 0046 is no longer unclaimed: crates/civ/Cargo.toml:4 reads "# (docs/adr/0046-civilization-kernel-crate.md)." and that ADR file does not exist yet.
- [imprecise] D17 (tools/axiom-dev/README.md:28) claim: Forge native dependency scanning covers core, content, audio and tools only; a new owning crate needs its coverage added before it can own a contract.
  correction: Line is off by one. "scanning currently covers core, content, audio and tools; a new owning crate needs" is tools/axiom-dev/README.md:27; line 28 reads "its dependency coverage added as well as its contract.".
- [wrong] E9 (crates/audio/src/dsp.rs:3) claim: No Philox/Threefry/counter-based generator exists under crates/ (case-insensitive grep found none); the only stateful generator hit is audio xorshift noise.
  correction: A Philox generator exists on disk now: crates/civ/src/draw.rs:15 "pub const fn philox4x32(mut c: [u32; 4], mut k: [u32; 2]) -> [u32; 4] {" (Philox4x32-10 with Random123 known-answer tests), plus a keyed "draw(seed, purpose, entity, index)" at crates/civ/src/draw.rs:43. The file is untracked and dated 2026-11-11 00:32 local, so it may postdate the reader's grep. The audio xorshift citation (crates/audio/src/dsp.rs:3) is correct.

## Missed (verifier)
- A civilization kernel crate already exists on disk as untracked work: crates/civ, package engine_civ, with src/{lib,agenda,canon,draw}.rs, src/agenda/wheel.rs and benches/civ.rs (files dated 2026-11-11 00:32-00:35 local). Cargo.lock:1033 already lists engine_civ. It declares a criterion bench with no benches/baselines.json beside it, which docs/TESTING.md:30 says fails the perf gate. [crates/civ/Cargo.toml:6] `name = "engine_civ"`
- ADR number 0046 is already claimed by that crate's manifest, but the ADR file is not on disk (docs/adr ends at 0045). [crates/civ/Cargo.toml:4] `# (docs/adr/0046-civilization-kernel-crate.md).`
- The reader did not read the newest civilization architecture record (docs/design, updated 8 November 2026, untracked, 926 lines). It places new civilization code in app modules under world/society, not a separate crate. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:555] `Add proposed modules under world/society only as their first behavior lands:`
- That record's first coding-session handoff requires visible behaviour in a 24-resident fixture and forbids ending with type registries only. This conflicts with a headless-crate-only first slice unless the owner overrides it. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:857] `The first session must change visible behavior; it must not end with only type registries.`
- That record says to start with a simple due-event heap and add a timing wheel only on measured evidence. The on-disk crate already contains a wheel (crates/civ/src/agenda/wheel.rs:13 "pub struct Wheel {"), and the user request also says not to commit to a timing wheel. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:643] `Begin with one host semantic frontier and a simple due-event heap. Add a timing wheel only if measured distributions justify it;`
- That record already specifies the keyed-randomness contract (key parts, retries reuse the draw) and tells the builder to prototype Philox against reference vectors before choosing a dependency. crates/civ/src/draw.rs does this with zero runtime dependencies (crates/civ/Cargo.toml:10 "[dependencies]" is empty), so it does not use engine_core hashing, which the port audit (docs/city-port-audit.md:22) ruled against duplicating. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:216] `Retries reuse the same draw. Repartitioning, camera motion, and worker identity are absent.`
- The reader's 'residents are capped at 64' is a per-town executor cap, not a world cap. The 8 November status states 512 retained settlements with a 64-person executor per town, and line 23 of the same file states eight community events per fixed tick. [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:29] `Retained settlement capacity is 512, with a bounded 64-person executor per town.`
- The existing batch terrain edit is documented as non-atomic. This bears directly on the cross-boundary construction transaction (validate claim, occupancy, cargo and voxel mutation together). [docs/design/AXIOM_CIVILIZATION_IMPLEMENTATION_20261107.md:78] `set_many_committed explicitly permits partial success. Do not treat it as an atomic transaction for an entire building or debit all materials in advance.`
- ARCHITECTURE.md has no crate dependency rules. Its data-flow section does record an existing conditional-commit path in world/effects.rs that returns failures or a commit receipt and is explicitly not a global transaction journal. [docs/ARCHITECTURE.md:8] `owners and wake capacity, returns explicit failures or a consumed commit receipt,`
- A persistent integer time base exists outside engine_core: world.farming.clock is a u64 advanced once per fixed step (1/120 s, crates/app/src/simulation.rs:33) and persisted in the farming sidecar. It is the unit of the community agenda's `due` (crates/app/src/world/farming/jobs/communities.rs:65 "#[serde(skip)] agenda: BTreeSet<(u64, u32, u32)>"; the agenda is not saved and is rebuilt from wakes at lines 80-84). [crates/app/src/world/farming.rs:62] `world.farming.clock += 1;`
- The current main save version on disk is 70 (magic VXWSAV70 at line 15). CLAUDE.md says 44, MASTER_PLAN says 54 and the city docs say 68. The reader left this unverified. [crates/app/src/world/storage.rs:91] `pub(crate) const SAVE_VERSION: u8 = 70;`
- Code evidence that machines have no runtime edit path: assets/machines.ron is compiled into the binary and must list every machine Kind once in enum order (assert at line 305), so a machine row change needs a rebuild and a new machine needs a new enum variant. The reader's Forge machine evidence was docs only. [crates/app/src/world/factory.rs:301] `const MACHINES_RON: &str = include_str!("../../../../assets/machines.ron");`

## Open questions
- Has the owner answered the crate-home question (docs/plans/2026-11-06-villager-civilization-slice.md section 7 item 3)? The only recorded recommendation is 'land in crates/app first, promote to crates/civ once stable' and 'do not add the crate while the tree is red'. No decision record approving a separate crate now was found.
- Is the autonomy-model owner decision (villager plan section 3, option a vs b) recorded anywhere? None found in docs/decisions or docs/adr.
- Does a new civilization-kernel crate need a decision record in docs/decisions (context/options/reversal format, global numbering) or an ADR in docs/adr? Both are in use; the most recent records 0040-0045 all went to docs/adr.
- 0046 is free in this tree, on the 63 local branches (checked via git ls-tree) and in worktrees globbed under .worktrees and .claude/worktrees. Worktrees elsewhere on disk and peers' unsaved work were not checked. docs/TRACKING.md:445 matched a 0046-0059 number pattern in a long line that was not inspected.
- MASTER_PLAN still places N09 villagers/jobs at M3 (Tier B, depends on N06/N08) and W02 at M6 (Tier A, full SDD plus critic), with M1 dam_break holding priority under decisions 0022/0039. Does the 2026-11-06 owner-authored 'governing' direction formally reorder this, or does a kernel crate need a MASTER_PLAN row and SDD first?
- Decision 0039 says cohort simulation needs its own decision before code. Does the same apply to a resident event kernel without cohorts? Not stated.
- What should the crate be called? Docs use `crates/civ` (implying `engine_civ`); the direction doc's module sketch uses society/behavior/simulation. No approved name was found.
- Should 64-bit keyed draws and a persistent SimTime go into engine_core (triggers the world_meshing gate route with the seam_route manual check, and core is peer-edited) or stay inside the new crate (the port audit says not to create a second randomness or time owner)? Unresolved by the docs.
- The civ_proto prototype (C:\Users\antis0007\Downloads\Java\civ_proto, modules det.rs, budget.rs, population.rs, policy.rs, coop.rs) is outside the repo and was not inspected. Is any of it intended as the kernel's starting point?
- Current SAVE_VERSION was not verified here: CLAUDE.md says 44, MASTER_PLAN says 54 (2026-11-05 audit), city docs say 68 with a restricted format-69 experiment. Read crates/app/src/world/storage.rs before any sidecar or save decision.
- Is there an end-to-end machine edit path outside the Forge docs read (e.g. a runtime recipe override under target/axiom-dev/recipe-preview mentioned at tools/axiom-dev/README.md:109)? That override's native side was not traced; it is described as solo-world preview only.
- Test-layout convention for a new crate is not written down. sim uses `*_tests.rs` siblings plus `tests/<name>/main.rs`, netcode uses `<module>/tests.rs`, chip uses `tests*.rs` (from directory listings only; the `mod` declarations were not read).
- The 'Known starting facts' about the agenda BTreeSet, EVENTS_PER_TICK, crew::CAPACITY and the multiplayer gating in jobs.rs were outside this topic and were not verified here, except indirectly through docs (city-port-audit.md:85-86, city-quarter-slice.md:50).

## Confirmed findings
- A1 The civilization direction doc is owner-authored and declared governing for civilization work; it is a design with proposed profiling targets, not measurements. [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:3]
- A2 The governing direction rejects a single monolithic kernel and prescribes explicit phases (snapshot, observations, proposals, resolve conflicts/reservations, commit, publish events; lines 300-303). [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:298]
- A3 Conservation invariants (no double consumption, no double counting between aggregate and local actor) are assigned to the commit stage. [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:307]
- A4 Camera/render detail must not change simulation outcomes; this is a stated test (same scenario at different detail settings). [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:108]
- A5 The direction says to add responsibilities inside the existing workspace; the boundary that matters is ownership of state, not folder/crate count. [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:350]
- A6 Networking rule for society state: commands only where determinism is proven, otherwise compact accepted outcomes with reconciliation. [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:312]
- A7 CPU allocation for civilization/behaviour is roughly 1-2 ms per frame at 60 FPS (16.67 ms frame), stated as an allocation subject to profiling. [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:38]
- A8 Profiling envelope for embodied NPCs is 64-512 ordinary (a range to benchmark, not promised capacity). [docs/superpowers/specs/2026-11-06-axiom-direction-persistent-civilization-simulation.md:121]
- A10 The villager plan already states engine_sim stays a separate cellular world and owns no settlement/resident state (matches 'do not reuse engine_sim'). [docs/plans/2026-11-06-villager-civilization-slice.md:80]
- A11 Determinism constraint for new civilization state: fixed logical intervals, positional hashing from the world seed, no rand, no wall clock selecting outcomes; new persistent state gets a versioned sidecar or a save bump. [docs/plans/2026-11-06-villager-civilization-slice.md:96]
- A13 The autonomy model (generated-plus-simulated vs emergent builders) is flagged as requiring an owner decision. [docs/plans/2026-11-06-villager-civilization-slice.md:64]
- A14 A prior design names the candidate crate `crates/civ`, core-only like crates/sim, as one of two homes. [docs/superpowers/specs/2026-11-06-world-ai-companions-and-conquest-design.md:169]
- A15 That design says a new crate changes Cargo.lock, which routes to the policy gate and needs an owner line. [docs/superpowers/specs/2026-11-06-world-ai-companions-and-conquest-design.md:171]
- A16 Recorded recommendation: prototype pure, land in crates/app first, promote to crates/civ once stable. This conflicts with creating a separate crate as the first slice unless the owner overrides. [docs/superpowers/specs/2026-11-06-world-ai-companions-and-conquest-design.md:172]
- A17 Same design: do not add the crate while the tree is red from peer edits (rust and policy routes would fail for unrelated reasons). [docs/superpowers/specs/2026-11-06-world-ai-companions-and-conquest-design.md:173]
- A18 A pure-logic first slice with no app coupling (S0 pawn core: priority loop, reservations, deterministic from a seed) is already sketched in that design. [docs/superpowers/specs/2026-11-06-world-ai-companions-and-conquest-design.md:155]
- A19 Province plan boundary rule: work proceeds in stable logical order with fixed work limits and a resumable cursor; wall time may schedule but not decide outcomes (lines 138-142). [docs/plans/2026-11-05-axiom-causal-world.md:138]
- A21 Province plan: do not require global peer lockstep or claim deterministic whole-world replay. [docs/plans/2026-11-05-axiom-causal-world.md:24]
- A22 Province plan already directs jobs and trips to be scheduled as due events with per-region budgets, under one owner through offscreen progress, save/reload and interruption. [docs/plans/2026-11-05-axiom-causal-world.md:38]
- A23 Province deliverable 4 requires that failed or deferred writes never debit or credit stock, and that observations needing unavailable edited chunks are deferred (matches Unknown-fact suspension and transactional construction). [docs/plans/2026-11-05-axiom-causal-world.md:91]
- A24 Civilization design constraint: simulation is bounded by logical work counts; wall-clock budget may yield but not select different committed outcomes. [docs/superpowers/specs/2026-11-05-civilization-on-solved-geography-design.md:385]
- A25 Civilization design: live outcomes persist under host authority when co-op is added; a baseline checksum does not prove live peer agreement, recovery or replay. [docs/superpowers/specs/2026-11-05-civilization-on-solved-geography-design.md:419]
- A26 Decision 0039: the Axiom expansion briefs (living world, planetary sim/co-op) are backlog, not inputs of record; 0022 and 0036 stand and MASTER_PLAN M1 keeps priority. [docs/decisions/0039-axiom-expansion-briefs.md:31]
- A27 Decision 0039 consequence: cohort simulation needs its own decision before code (supports deferring cohorts; a later cohort slice needs a decision record). [docs/decisions/0039-axiom-expansion-briefs.md:70]
- A28 Decision 0039: the lockstep plan stands for integer matter via engine_sim; bodies and players stay host-authoritative. [docs/decisions/0039-axiom-expansion-briefs.md:36]
- A29 Decision 0039: no save bump or generator gate for brief work without a MASTER_PLAN row and a cross-branch gate-registry check. [docs/decisions/0039-axiom-expansion-briefs.md:68]
- A30 Decision 0039 reversal clause: scope is revisited when M1 is accepted or the owner names a brief milestone as the next target. [docs/decisions/0039-axiom-expansion-briefs.md:77]
- A31 Decision 0036: ADRs 0002, 0007, 0011 and 0018 stand and simulation work stays in MASTER_PLAN M1. [docs/decisions/0036-axiom-brief-scope.md:24]
- A32 Decision 0022: deterministic quotas (items per tick) are authoritative for simulation; wall-time caps only limit render and mesh jobs. [docs/decisions/0022-master-plan-review.md:37]
- A33 MASTER_PLAN inventory: villagers/jobs is N09, Tier B, milestone M3, design-only (P), dependent on N06 utility AI and N08 factions. [docs/MASTER_PLAN.md:153]
- A34 MASTER_PLAN inventory: civilizations and regional history is W02, Tier A (full SDD plus independent critic), milestone M6. [docs/MASTER_PLAN.md:197]
- A35 MASTER_PLAN dependency rule: feedback between systems uses tick-stamped events/actions, not cyclic ownership. [docs/MASTER_PLAN.md:88]
- A36 ADR 0010: no ECS; entity kinds are SoA tables with stable u32 handles owned by World. [docs/adr/0010-no-ecs.md:5]
- A37 ADR 0001 records that gameplay systems take &mut World, so peeling them into crates is a logic rewrite, not a move (supports extracting pure policy only, not Jobs wholesale). [docs/adr/0001-workspace-split.md:5]
- A38 ADR 0018: TickId restarts per world/session and is not a persisted replay timeline; a kernel needing persistent sim time cannot reuse it as is. [docs/adr/0018-fixed-steps-and-bounded-cpu-region.md:16]
- A39 ADR 0018: the shared fixed step gives common stage execution, not deterministic legacy scheduling or headless/window hash equivalence. [docs/adr/0018-fixed-steps-and-bounded-cpu-region.md:39]
- A40 ADR 0008 precedent for keyed randomness: every random choice is a positional hash with no RNG state. [docs/adr/0008-ca-determinism.md:12]
- A41 ADR 0043 states its tests do not establish civilization support in multiplayer. [docs/adr/0043-network-transfer-fairness.md:25]
- A42 Co-op design (status: design) assigns society, economy, AI decisions, construction, ownership and stock to the host; clients consume replicated outcomes. [docs/superpowers/specs/2026-11-06-cheap-ai-and-networked-coop-design.md:70]
- A43 That design refers to `population::Schedule`; no matching module exists under crates/ (the only `struct Schedule` is in render/sky_volume.rs). It appears to describe the external civ_proto prototype. [docs/superpowers/specs/2026-11-06-cheap-ai-and-networked-coop-design.md:37]
- A44 A dependency-free Rust prototype of the civ library (civ_proto, 26 modules) exists outside the repo and was audited on 2026-11-07; no direct import was found. [docs/city-port-audit.md:7]
- A45 Port audit ruling: port decisions and missing state into the existing engine owners; do not install duplicate authorities. [docs/city-port-audit.md:11]
- A46 Port audit ruling on determinism primitives: use engine_core hash and ticks, not a second randomness or time owner. [docs/city-port-audit.md:22]
- A47 Port audit on the prototype scheduler: reuse its deterministic relevance/tie rules, not its implementation; the existing scheduler is jobs::service / crew::service_phase. [docs/city-port-audit.md:25]
- A48 Port audit lists the coupled 64 limits that must change together to go to 128/256 (crew capacity, travel sidecar authored ids, home decoder cap). [docs/city-port-audit.md:85]
- A49 Port audit: the current 64-worker schedule services at most three workers per tick (a 120 Hz fixed tick); raising capacity changes that cost contract. [docs/city-port-audit.md:86]
- A50 Port audit: multiplayer society replication is unproven and needs two clients, authoritative commands, join/rejoin and dedup tests. [docs/city-port-audit.md:91]
- A51 Frame-pacing report: real capacity is 64 workers; 128/256 input falls back to 32; ordinary saves must not be used for a 128 experiment. [docs/city-frame-pacing.md:84]
- A52 City quarter plan confirms the settler service rejects multiplayer contexts including the host, and lists prerequisites (stable ids, revision checks, retry dedup) before enabling. [docs/plans/2026-11-06-city-quarter-slice.md:50]
- A53 Work-circle review cadence: at most once per 480 fixed ticks (4 s at 120 Hz) with at most 16 candidate checks; bounded 64-worker roster. [docs/WORK_CIRCLES.md:21]
- A54 The jobs PULSE unit is defined as 0.25 s over the 1/120 s player step, i.e. 30 fixed ticks. [crates/app/src/world/farming/jobs.rs:31]
- A55 The app's fixed step constant is an alias of the core constant (1/120 s). [crates/app/src/simulation.rs:33]
- B1 Any directory under crates/ is automatically a workspace member; no root manifest edit is needed for a new crate. [Cargo.toml:3]
- B2 Workspace lints are defined once at the root (clippy allow-list); crates inherit them. [Cargo.toml:53]
- B3 Package naming pattern is engine_<dir> with a header comment describing purpose and dependency rule. [crates/sim/Cargo.toml:6]
- B4 Headless crates end with `[lints] workspace = true` (the same block is in netcode, chip, core, space and tools). [crates/sim/Cargo.toml:26]
- B5 engine_sim's only engine dependency is engine_core by path; engine dev-dependencies (engine_netcode) are allowed for tests only. [crates/sim/Cargo.toml:11]
- B6 engine_chip is the precedent for a pure data-and-functions crate needing serialization: serde 1.0.229 plus ron 0.12.2, with no engine dependency. [crates/chip/Cargo.toml:11]
- B7 engine_netcode (no sockets, no world, time passed in) depends on engine_core and glam only. [crates/netcode/Cargo.toml:10]
- B8 A layout test pins engine_sim to engine_core only; a new crate has no such test yet, so one would have to be added to enforce a core-only rule. [crates/app/src/repo_layout.rs:157]
- B9 The agent-docs layout test scans CLAUDE.md, top-level docs/*.md, docs/adr/*.md and skill files, not docs/plans, docs/decisions or docs/superpowers. [crates/app/src/repo_layout.rs:168]
- B10 In scanned docs, any token starting with crates/, assets/, docs/, scripts/, tools/ or .claude/ must exist on disk, so an ADR or top-level doc must not name unbuilt crate files. [crates/app/src/repo_layout.rs:202]
- B11 Clone-completeness gate (`just deps`, part of `just verify`) fails when a crates/* workspace member or path dependency is not in the commit. [docs/TESTING.md:61]
- B12 Benches, if added, live in crates/<crate>/benches with a baselines.json; the perf gate fails on a bench with no baseline. [docs/TESTING.md:27]
- B13 Tests run through nextest across the workspace; no test in `just test` may need a GPU or window. [docs/TESTING.md:13]
- B14 Task-gate rust route: any crates/*.rs or crates/*Cargo.toml change requires tests, clippy and the perf check. [.claude/verify/policy.json:30]
- B15 Task-gate policy route: a Cargo.lock change (which a new crate causes) additionally requires tests_workflow and a manual review_policy. [.claude/verify/policy.json:38]
- B16 Editing crates/core (e.g. adding time or hash types there) triggers the world_meshing route with the manual seam_route capture check. [.claude/verify/policy.json:34]
- B17 Clippy runs with all warnings denied except dead_code. [justfile:25]
- B18 File size rule: no new file over 300 lines when it can be split; no scratch files at the repo root. [CLAUDE.md:47]
- B19 ADR numbering is one global sequence shared by docs/adr and docs/decisions. [docs/decisions/README.md:4]
- B21 MASTER_PLAN confirms ADR 0013 is absent from the sequence. [docs/MASTER_PLAN.md:49]
- B22 Decision records with more than a week's consequence need context, options, evidence, decision/status, consequences and reversal criteria. [docs/decisions/README.md:7]
- B23 Precedent for recording a new workspace dependency edge in an ADR (app -> engine_sim). [docs/adr/0018-fixed-steps-and-bounded-cpu-region.md:5]
- B24 Precedent for the ADR dependency line, even for dev-dependencies already in the lockfile. [docs/adr/0042-shared-native-content-validation.md:14]
- B25 Determinism test convention: passes take engine_core::Budget; scenarios and tests pass Budget::UNBOUNDED. [docs/TESTING.md:113]
- C1 docs/specs is for preserved original briefs, not new designs. [docs/README.md:10]
- C2 docs/plans is for execution-ready task plans with verified file:line facts. [docs/README.md:34]
- C3 Recent civilization designs were filed under docs/superpowers/specs as dated `-design.md` files with a Status line. [docs/superpowers/specs/2026-11-06-cheap-ai-and-networked-coop-design.md:3]
- C4 Recent audit/evidence reports for the resident simulation were filed as top-level docs/city-*.md (2026-11-06/07); these are scanned by the agent-docs path test. [docs/city-port-audit.md:3]
- D1 The Forge (Axiom Dev Panel) registry covers five catalog families. [tools/axiom-dev/README.md:19]
- D2 Registry family 1: Fabricator processes (assets/fabrication.ron). [tools/axiom-dev/catalog-contracts.mjs:12]
- D3 Registry family 2: Spells (assets/spells.ron). [tools/axiom-dev/catalog-contracts.mjs:18]
- D4 Registry family 3: Tool parts and schemas (assets/tools/parts.ron). [tools/axiom-dev/catalog-contracts.mjs:25]
- D5 Registry family 4: Tool materials (assets/tools/materials.ron). [tools/axiom-dev/catalog-contracts.mjs:31]
- D6 Registry family 5: Material reactions (assets/reactions.ron). No machines.ron contract exists in the registry. [tools/axiom-dev/catalog-contracts.mjs:37]
- D7 Every registered contract activates only by rebuild; a contract test pins this, so there is no hot reload into a running game. [tools/axiom-dev/catalog-contracts.test.mjs:19]
- D8 The registry README disclaims full coverage and hot reload. [tools/axiom-dev/README.md:20]
- D9 ADR 0042's status line still lists four catalogs (no Fabricator processes), so it is stale against the five-entry registry. [docs/adr/0042-shared-native-content-validation.md:3]
- D10 ADR 0042: cross-catalog compilation for new identities and a running-game reload protocol are future work. [docs/adr/0042-shared-native-content-validation.md:25]
- D11 The Forge workshop slice edits Fabricator process rows only; it does not install content, add machine behaviour, migrate saves or touch a live world. [tools/axiom-dev/WORKSHOP.md:3]
- D12 Workshop staging writes only snapshot folders under target/; there is no publish or live-install command. [tools/axiom-dev/WORKSHOP.md:79]
- D13 Multiplayer authority, reconnect and dense-factory workload are listed as unpassed promotion gates for machine prototypes. [tools/axiom-dev/WORKSHOP.md:101]
- D14 Workshop tests do not cover native gameplay, multiplayer or save/load semantics. [tools/axiom-dev/WORKSHOP.md:109]
- D15 The only live machine edit is a session-overlay swap of a loaded factory voxel model (render geometry only). [tools/axiom-dev/ENGINE_WORKBENCH.md:96]
- D16 New native behaviour, embedded catalogs and new block IDs still require a build. [tools/axiom-dev/ENGINE_WORKBENCH.md:144]
- D18 A separate in-game 'Forge' exists as inventory tab 5 (tool station); it is not the dev-tool registry. [crates/app/src/egui_ui/inventory/tabs.rs:22]
- E1 engine_core::hash exposes a 32-bit integer mixer; signature hash_u32(u32) -> u32. [crates/core/src/hash.rs:3]
- E2 The only other function in engine_core::hash is a float mapping hash01(u32) -> f32 (unsuitable for integer policy). The module has no FNV, no u64 mix and no multi-key helper. [crates/core/src/hash.rs:12]
- E3 engine_core::fast_hash is an Fx-style HashMap hasher (FastMap/FastSet), not a keyed-draw API. [crates/core/src/fast_hash.rs:8]
- E4 Keyed-draw precedent in engine_sim: fold seed, phase counter and anchor through hash_u32 into a base. [crates/sim/src/pinned.rs:16]
- E5 Per-decision draw precedent in engine_sim: one mix of base and a salt (a draw-index analogue). [crates/sim/src/pinned.rs:25]
- E6 A public 64-bit keyed mix (splitmix64 finaliser over a ^ b*golden) exists in engine_chip, used so reloading a half-finished lot finishes the same way. [crates/chip/src/lot.rs:13]
- E7 engine_sim has a private splitmix64 finaliser used for state digests (not exported). [crates/sim/src/digest.rs:31]
- E8 FNV-1a 64 exists in engine_voxel (blueprint checksum), not in engine_core, although CLAUDE.md describes core as holding FNV hashes. [crates/voxel/src/blueprint_format.rs:9]
- F1 The only integer time type in engine_core is TickId(u64): session-local completed player steps, with checked `next()`. [crates/core/src/time.rs:3]
- F2 The player step length is 1/120 s, defined as an f32 constant in core. [crates/core/src/time.rs:5]
- F3 The experimental cellular backend ticks once per two player steps (60 Hz); `cellular_tick()` derives it from TickId. [crates/core/src/time.rs:7]
- F4 Budget in the same file is a wall-clock deadline (std::time::Instant), not a logical quota; UNBOUNDED is used by tests and headless runs. [crates/core/src/time.rs:26]
- F5 Other crates define their own tick rates locally, e.g. Fabricator process ticks at 4 per second (0.25 s each) in engine_content. [crates/content/src/fabrication.rs:10]