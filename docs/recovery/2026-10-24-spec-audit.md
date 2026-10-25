# Specification and roadmap audit — 2026-10-24

Scope: read all three supplied UTF-8 documents, CLAUDE.md, ARCHITECTURE.md, ENGINE_CORE.md, GAME_DESIGN.md, PLAN2.md, all existing ADRs 0001–0019 (no 0013), research index and matrix scope/verification notes. Existing unrelated dirty documents were preserved. This audit and MASTER_PLAN.md are the documentation agent's owned outputs.

## Source intake

| User attachment | Content | Reconciled role |
|---|---|---|
| `6c1e4a8b-f35e-42a8-bf29-1f9f50cb0e93/Pasted text.txt` | Addendum A, A.1–A.16 | Full adoption scope, shared mechanisms and expanded research matrix |
| `0e71b975-fe32-4286-8a85-d8f83de91bff/Pasted text.txt` | Mission specification v2, I–XIII | Engine/game pillars, layouts, targets, original M1–M7 |
| `c6e52811-c556-4833-8c4b-dace356baae1/Pasted text.txt` | Game Bible, A.0–A.6 and B.1–B.7 | Cosmology, 24 claims, separate systems, SDD/rubric/spikes |
| Current message, second IV.11a block | Stricter description style | Governs over first IV.11a and original examples |

Canonical source inputs are preserved under `docs/specs/`, with hashes and precedence in its README and ADR0020. Attachment text and prior plans are design inputs, not proof of implementation. Current user authorization to attempt/recover work and the explicit no-tests/no-CI instruction override embedded blanket stop/CI instructions; runtime cutover and destructive cleanup still require evidence and scope control.

## Discrepancies and resolution

| Subject | Evidence | Resolution / remaining work |
|---|---|---|
| Save/generator version | `crates/app/src/world/storage.rs` line 14 says VXWSAV32; line 33 SAVE_VERSION 32; `crates/voxel/src/config.rs` line 5 generator 46 | Source snapshot governs. Initial CLAUDE/ENGINE_CORE v29/gen36 references were corrected during recovery; ADR0019 v31/gen42 describes an earlier increment. No downgrade, overwrite or invented next version |
| Long-range authority | ENGINE_CORE/GAME_DESIGN replace PLAN2; accepted D1–D7 and ADR0015 remain | MASTER_PLAN adds bible/addendum scope without repacking cells or retiring working paths |
| Current authority | ADR0018 defines sealed headless CPU region and exact two-CA-tick mirror; ordinary gameplay remains legacy | Preserve distinction between source foundation and integrated game; no whole-game deterministic/replay claim |
| Cell format | Mission packed planes disagree with accepted 8-byte hybrid | Keep ADR0015; optional fields require a new allocation/wire contract; widen materials only with explicit migration |
| Field inventory | A.15 adds neutron/radiation/fertility/planning and 16-bit presence needs beyond older list | Plan C14/X03 includes them. Compute actual memory including plane occupancy, mirrors and renderer; old 120 MB claim is invalid |
| Physics range | u16 tenths-kelvin tops at 6553.5 K, while fusion/accelerator scope is much hotter | Future thermal/nuclear SDD must define extended range/units or explicitly model abstract energy; never silently saturate or claim calibrated plasma |
| Compressible volume | Original 4-bit volume cannot hold its proposed 16–19 compression; code uses 64 levels | Retain accepted current layout; S2 decides pressure/compression and overflow/conservation before implementation |
| World-scale mass | Original D2 widths conflict with 64*8^depth | ADR0017 corrects to u8 at depth0/u16 at1–3/u32 at4 before headroom; reserved Scale is not implemented LOD |
| Tick semantics | Mission uses 60-tps target; gameplay is 120 Hz and isolated CA 60 Hz | Two ticks mean CA ticks; preserve tick IDs and deadline behavior, migrate legacy schedulers explicitly |
| Heat conservation | ADR0008 uses deterministic stochastic rounding, conserved in expectation; reaction temperatures are authored | Exact conservation claims remain unpassed; ledger/latent heat/reaction sources are design work |
| VM sequencing | Mission leaves controllers late; bible A.3 requires shared compiler in M1–M2 | V01–V05 now foundation work; avoid separate magic and machine interpreters |
| Primer | Old plan postpones education/codex | Early M3 adaptive artifact/tutorial, demonstrated experiments; M7 completes advanced lessons |
| World fiction | Existing paths/depths do not specify bible cultures, ring/core, ages or archives | W02/W03/W06/W07/W10 preserve lore obligations; no core reveal invented |
| Space implementation | Existing orbital recovery/gameplay is not evidence of integer patched conics | Preserve recovered work; S9 and C-S5 remain explicit unfinished physics obligations |
| Hardware addendum | Absent from original three-attachment audit; found in later Downloads intake | [Exact source](../specs/ADDENDUM_B_HARDWARE.md) defines H0-H5; [reconciliation](../plans/download-coverage/hardware.md) records fallback/scheduling/memory gaps. ADR0016 Iris Xe stays separate; no device result inferred |
| Missing architecture docs | PERF.md/WORLDGEN.md absent (also recorded ADR0014) | Use ADR0006/baselines and worldgen source; author accurate docs as follow-up |
| Research depth | Matrix reports 153 features and says none of 17 source results checked official docs in depth | Existing research is partial. Complete primary reference verification and A.16 schema; don't relabel as completed research |
| Matrix expansion | A.14 requires every named family; >=10 rows for eight major systems | Master routes families, but full per-feature researched expansion remains open |
| Claim coverage | Bible defines MC1–5, N1–5, F1–3, Z1–4, S1–7 | All 24 assigned owner/milestone/proposed scenario in MASTER_PLAN; none reported passing |
| Claim ambiguity | C-Z3 says >=2 solutions then requires three-way suite | Plan keeps stricter three-way gate suite. C-N2 pair denominator and C-N4 meaningful-effect metric must be defined first |
| Cost estimates | Addendum labels many features data/small code despite missing substrate | Require loader+executor+feedback+persistence proof before accepting data-only cost |
| Tooltip revisions | Current user first allows 12/25/60 words, second narrows to 8/14/50 and standard no prose | Latest second block governs all tooltip/card/machine functional text |
| Evidence rules | Mission requests tests/CI each session; active user explicitly says not to add/run them | No tests or hosted CI in this task; documented acceptance remains pending, not waived |
| Cleanup ownership | Dirty docs/code plus disparate temporary candidates | Hash/provenance inventory; recover reviewed differences; preserve unique/uncertain variants and runtime evidence |

## Recovery integration rules

Current source versions and symbols must be checked before comparing a candidate. Recover bounded source/data changes, not entire stale directory trees. A newer timestamp or compile success does not establish behavioral superiority. Never replace current saves, settings, generator gates, lockfiles or shared renderer work from a temporary copy without examining the actual delta. Keep provenance for selected hunks and rejected alternatives. Temporary build products can be removed only when confirmed disposable and outside active processes; source candidates remain until their unique content is accounted for.

## Produced versus pending

Produced: reconciled separate-system inventory, dependency layers, M1–M7 routing, all claim ownership/scenarios, addendum-family adoption routing, first-session recovery/research/SDD sequence and description contract in [MASTER_PLAN.md](../MASTER_PLAN.md).

Pending: complete research/matrix expansion, all per-system B.3 SDDs and independent B.4 scoring, S1–S10 measurements/ADRs, target-hardware definitions and results, integrated behavior/visual/save proof, global actions/replay, and the rest of the multi-month game scope. Source inspection did not run tests, benchmarks or CI, and does not establish those results.
