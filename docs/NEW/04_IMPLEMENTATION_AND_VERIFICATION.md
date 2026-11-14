# 04 — EXECUTION PLAN, CLAIMS, ACCEPTANCE AND AI-AGENT PROTOCOL
**Revision:** U1 — 2026-10-26 | **Status:** planned and verified as document structure; no active repository build has been run here.

## 1. Operating principle: an architecture is a hypothesis until measured

The source document's Part B is excellent in spirit: a rich bible cannot replace experiments, narrow system designs and evidence. Preserve its plan-generation protocol but prohibit analysis paralysis: **a minimal first playable vertical slice is the highest-value integration test.** Use architecture documents to constrain code that exists, not to schedule a year of paperwork before a single player can mine a block, cast a spell or make a machine work.

All claims are traced to a named scenario, owner, target milestone and evidence class: `CODE / TEST / SCREEN / PLAYED / MEASURED / UNSUPPORTED`. A written assertion is not evidence. Every completion report states the exact branch/commit, command lines, build hash, seed, hardware, resolution, content version and change ownership. Migrations/backups and concurrent worktree constraints are first-class. Do not fabricate test outcomes or cite inaccessible Windows files as inspected.

## 2. Product goals and metrics

### P0 correctness gates (must never regress)

- Old saved worlds and generator versions load correctly; no edit, item, power, recipe, boss reward or machine inventory silently disappears or doubles.
- Near/far geometry covers all visible world surfaces in valid residency states; no chunk holes or unacceptable seams/floating roofs or disconnected tree crowns during streaming.
- The player's physical edit has a prompt and correct visible response, collision and persistent material amount across scale levels.
- Actual factory utility and spell resource costs are accounted authoritatively; no invented free Joules, watts, goods or mana.
- All scene claims have reproducible evidence and known limitations; no agent calls a shader compilation a successful visual overhaul.
- Stable code ownership prevents concurrent edits from silently overwriting unrelated work.

### Proposed hardware matrix (must be measured)

| Class | Prototype description | Normal-play target | High-load target | Explicit caveat |
|---|---|---|---|---|
| H0 | Low-spec integrated GPU | 30 FPS at reduced 720p–1080p | Defined reduced simulation budget; prefer safe responsiveness to full effect fidelity | Real hardware profile needed |
| H1 | Mid-tier discrete GPU | 60 FPS 1080p | p99 controlled at reduced fidelity | GPU/CPU specs TBD |
| H3 | High-end 4090-class 24 GB | 120 FPS at 1440p stretch | 60 FPS stretch in bounded large incident | Not a promise; settings/scenes must be frozen |

Render FPS and physics tick are separate. Primary authority may use 60 Hz while other islands/networks run appropriate rates. Hard-code **budgets** only after profiling. Scaled quality must not alter authoritatively credited inventories, material reactions or boss mechanics.

### Gameplay satisfaction and art gates

- First meaningful survival action within 5 min; basic automation discovery within around 2 h for uncoached tests.
- At least two genuinely different solutions to major authored physical gates; at least one player-proposed third route discovered during playtests.
- At least six excellent, distinct actual in-game wands before procedural catalog enlargement; spell-synergy builds discovered by players without author hints.
- Integrated scenic world: six place families tested for navigability, sightline, structured silhouettes, palette and player use; neutral/dungeon light both acceptable.
- A single full workshop→delve→salvage→factory upgrade loop tested end-to-end rather than only counted as implemented feature flags.
- The world exposes mysteries without requiring external documentation; humans can solve at least one seeded secret chain under an allotted, reasonable test session with clues.

## 3. Corrected claims matrix from the supplied C-* design bible

The original author IDs are preserved for traceability. Each change below is an explicit refinement, not an accidental replacement.

| Claim | Design target and revision | Verification scenario | Gate |
|---|---|---|---|
| **C-MC1** | Energy converters connect compatible domains and debit/credit known inputs, losses and declared magical sources. **Revision:** not every energy pair convertible; no free-loop assumptions. | `energy_converter_loop`, `missing_source`, `typed_port_rejection`, input-output ledger | M5 |
| **C-MC2** | Machine classes respond to hazards physically relevant to their materials and internals; visible damage/repair. **Revision:** not every acid/fire/radiation hazard necessarily damages every material equally. | `hazard_machine_matrix`, `repair_after_flood`, `heat_cable_fault` | M5–M7 |
| **C-MC3** | Adding new material data produces valid derived tools/caps/armor behavior where the applicable component rules exist. | `data_only_orichalcum`, `derived_mass_strength`, mod sandbox validator | M7 |
| **C-MC4** | One authoritative ledger/model per physical **concept**, multiple implementations/proxies permitted where mathematically appropriate. **Revision:** no blanket ban on alternate solvers or helper implementations. | `unique_ownership_lint`, `port_model_matrix`, code architecture review | M1–M7 |
| **C-MC5** | Uncoached newcomer reaches first genuinely useful automation with in-game Primer hints in around two hours. | `human_first_automation`, controlled tutorial/cohort notes; bot supplementary | M4 |
| **C-N1** | >=3 seeded hidden recipe families with explicit in-world clue chains and bounded hypothesis space. | `seeded_recipe_discovery`, 20-seed validity + human hint study | M8 |
| **C-N2** | Every material contact has a resolver result (possibly inert). **Revision:** reject forced 30% nontrivial pairwise chemistry; curate reactions by properties and game value. | `reaction_resolver_totality`, `signature_chain_coverage`, `combustion_acid_water` | M3, M8 |
| **C-N3** | >=6 optional world-scale secrets with place/time/condition, seeded feasible clue paths and lasting consequences. | `secret_graph_validate_50_seeds`, human cold-run solve, persistence | M9–M11 |
| **C-N4** | Strongly compositional spell grammar and high measured useful diversity. **Revision:** raw million combinations not acceptable proof of distinct player-facing behavior. | `spell_semantic_fuzz`, `trigger_order`, `recursion_budget`, expert playtest | M4–M9 |
| **C-N5** | Deep 3D reactive materials and destruction at stable frame/tick targets. **Revision:** compare *causal behavior*, not pixel-for-pixel with 2D Noita. | `dam_break`, `acid_floor`, `oil_fire`, `collapse_into_water`, capture/perf | M3–M7 |
| **C-F1** | Physical editable vessels with independent subsystems and task-based NPC/automation repair. | `crawler_hull_breach`, `battery_reroute`, `crew_repair_access` | M8–M10 |
| **C-F2** | Expeditions with travel/risk choices, seeded physical sites and persistent outcomes; explicit travel semantics. | `expedition_route`, `retreat_partial_salvage`, `persistent_event` | M9 |
| **C-F3** | Optional high-risk delve/permadeath preserving history in world/codex. | `delve_body_legacy`, `archive_survives`, `base_unaffected` | M8–M10 |
| **C-Z1** | >=3 coherent layered civilizations with language, material culture, technology, purpose and downfall in generation. | `culture_10_seeds`, manual art/story review, machine/city consistency | M6–M9 |
| **C-Z2** | Distinct named landmarks with roles and curated perceived density. **Revision:** 1/500 m underground universally would create clutter and harm wilderness scale. | `named_place_density_histogram`, exploration recognition playtest | M6 |
| **C-Z3** | >=2 independent physically valid solutions for every authored hard gate; stretch >=3 for key sites. **Revision:** do not require all random obstacles solved exactly three ways. | `dam_3_routes`, `resonance_gate_2_routes`, `builder_bypass` | M6–M9 |
| **C-Z4** | Optional room-description layer grounded in actual surfaces, history and sensory summaries; authored-quality templates. | `description_grounding`, `false_object_absence`, readability review | M8 |
| **C-S1** | Primer grounded in actual world demonstration, not reading a book page or guessed progress. | `primer_siphon`, `primer_circuit`, `observed_mastery`, onboarding cohort | M4 |
| **C-S2** | Feed/matter compilation costs matter, energy, manufacturing operations and complexity; editable blueprints. **Revision:** raw compression-bits-per-block not universal cost metric. | `compiler_matter_ledger`, `blueprint_complexity`, `noise_exploit` | M11 |
| **C-S3** | Glyph and machine controls compile into common bounded typed VM IR; behavior manipulation only via modeled utility API. | `cross_compile_same_output`, `vm_fuel`, `auth_no_raw_world_write` | M4–M8 |
| **C-S4** | Archives, deep time, faction knowledge and optional age/NG+ transition. **Revision:** avoid unwarned destructive permanent loss. | `archive_saved`, `optional_age_transition`, `reburied_ruins` | M9–M11 |
| **C-S5** | Orbital engineering with comprehensible transfer/stress/thermal models. **Revision:** don't mandate integer patched conics everywhere; use validated stable numeric formulation. | `orbit_transfer`, `tether_load`, `habitat_thermal`, solver-error budget | M11–M12 |
| **C-S6** | Toy-honest cryptography puzzles alongside real library-backed secure network features. | `cipher_primer`, `reproducible_encrypted_clue`, `secure_channel_boundary` | M8 |
| **C-S7** | Pocket/dimensional/virtual worlds have explicit time/physics/transfer boundaries and persistence. **Revision:** 2 levels nested is stretch after safe first level, not baseline. | `domain_enter_exit`, `inventory_transfer`, `nested_clock` | M10–M12 |

**All C-* claims remain in the formal master plan** as preserved or explicitly modified. The engine must not ship shallow proxies to force early PASS on late claims.

## 4. Phase dependency graph: vertical slices first

```
M0 Repo archaeology + frozen benchmark + ownership map
│
├─ M1 Canonical world transaction + save/LOD revision contracts
│   ├─ M2 Stable render/streaming/horizon/voxel precision + pixel art baseline
│   │   └─ M3 Sparse CA/material interactions + structural/thermal test sandbox
│   └─ M4 Gameplay foundation: survival, first spells, Primer, one small powered line
│         ├─ M5 Factory graphs: power + belts + fluid + two actual process machines
│         └─ M6 Generator55 place plans: settlement, mine, library, first dungeon + physical alternatives
│                 └─ M7 Integrated First Valley workshop/ruin, six wands, basic creature anatomy
│                       ├─ M8 VM/runes/AI/ecology/vessel proof + richer magitech
│                       ├─ M9 Deep civs, multi-site secrets, persistent expedition
│                       └─ M10 Advanced manufacturing, controller/IC design, big plants
│                              └─ M11 Founder Work, Feed, ages/domains, ring prototype
│                                    └─ M12 Orbital/legendary secrets/megabase quality and polish
```

This graph has parallel opportunities: art R&D and procedural semantic layouts can progress during M1–M5 in independent worktrees with stable interfaces; entertainment quality gets continuous attention, not delayed to M12. Claim targets belong to several systems, while *test implementation* has one primary owner.

### M0 — Repo archaeology, runnable baselines, safety (first working session)

- Inspect `CLAUDE.md/AGENTS.md`, Cargo workspace and locked versions, branch/dirty files, generator 54 R7 reports, actual render and world store, current saves, active concurrent ownership.
- Generate source-based map of storage/rendering/physics/manipulation/UI and current tested vs merely reported features. Verify path and file existence before claiming code knowledge.
- Freeze reproducible native scenes and hashes: first-person tool, horizon, roof/town/library, functioning factory, starter ecology. Establish CPU/GPU profiler and minimal capture receipts.
- **Exit:** build/test/capture baseline commands plus failures documented, visual evidence (or explicitly not run), no code rewrite.

### M1 — World kernel contracts + preservation

- Implement / formalize region identity, world units and scale levels, exact persistent edits, transaction ownership, artifact dependency revisions and safe old-generator fallback.
- Add `same_mass_across_scale`, `stale_collider_discard`, `save_upgrade_legacy`, `neighbor_revision` tests.
- **Exit:** a tiny edit seam / save/load demonstration, proven before cosmetics.

### M2 — Render/streaming correctness + first art benchmark

- Fix reported horizon seams/holes, fallback proxy continuity, near/far trees, chunky roof primitive and memory/upload backpressure; verify actual renderer scene, not shader compilation only.
- Make neutral-light stone/wood/metal/crystal reference and first genuinely pixel-authored workshop items.
- **Exit:** no visible holes across a fixed traversal route; stable captures at 1 m and refined geometry; p99/VRAM measured; old saves unaffected.

### M3 — Sparse material island proof

- One 3D sand/granular area, water basin, fire/heat and small acid reaction, only active bricks simulated; conservative coarse-fine contact and bulk ejection. Compare CPU vs GPU only where appropriate.
- **Exit:** dam breach/material conservation, reproducible 60 Hz/variable time benchmark under fixed H0/H1/H3 scenarios, no cross-region race/material dupes.

### M4 — One enjoyable survival-to-automation-to-magic loop

- First home, resource gathering, simple machine with real input/output and visual state, basic crafting, several spell compositions, Primer demonstration and small functional archaeological clue.
- **Exit:** uncoached tester moves from start to automated useful output and solves a physical secret without external docs. Runtime screenshots and UI review.

### M5 — Factory logistics and utility foundations

- DC/source/return, safe converter ledger, basic motors/heat, compact belt runs, inventories and tank/pump, one validated real multiblock. Don't immediately implement full AC/HV or all machines.
- **Exit:** ore→crusher→furnace→storage at measurable rates, respond correctly to a wire cut, a blocked belt and insufficient coolant, save/reload and benchmark.

### M6 — Art-directed procedural site framework

- Adopt semantic PlacePlan of Generator55, six place families, protected navigable voids and meaningful lighting; procedural culture tags; one serious multi-solution dungeon; preserve generator 53/54 exact output.
- **Exit:** headroom, route graph and interaction checks plus comparison images under native renderer for frozen seeds and different supported resolutions. One castle/town/mine/library should feel intentionally designed.

### M7 — The First Valley (first game-complete milestone)

- Beautiful small region, compelling caves/ruin, 17-asset workshop scene, functioning factory ↔ one magical machine interaction, 6 distinct wands, 3 movement/combat enemy archetypes with functional parts, short story/dungeon encounter and optional alternative route.
- **Exit:** a 90–180 min playtest with no out-of-band debugging required; beautiful native evidence; survival/world edits persist; frame times acceptable on defined tier.

### M8 — Unified Work programming and systemic encounters

- Explicit typed VM IR, ladder/glyph frontends, world effect capability checks, machine controller, first biologically grown creature, basic vessel/crawler/hull emergency, regional threat and useful enemy salvage.
- **Exit:** same sensor-control-spell behavior expressed via both machine and glyph frontends under equivalent conditions; deterministic safe energy debit; meaningful encounter alternatives.

### M9 — The deeper world, cultures and secret chains

- Engineers/Choristers/Founders coherent sites, maps, seed-dependent clue graph, archive factions, legend and physical secret, expedition and persistent ruin restoration. Avoid lore dumping and checklist puzzles.
- **Exit:** uncoached users can pursue several different mysteries; seeded graph passes validity checks and has authored-quality art/locations.

### M10 — Deep factory and electronics

- AC power, major logistics, working chip editor + real characterized controller, chemical fab stages, robot/crew loops, capturable industrial ruins. Nested editor is separate from overworld nanovoxels.
- **Exit:** two player-made chip variants behave differently in a factory, genuine throughput/energy budget, reusable design and persisted lot revision.

### M11 — Founder Work / matter compiler / world ages / ring research

- Founder archives, Feed proof, optional Age change with backup, first dimensional domain, orbital transfer spike and physically constrained tether. No automatic game-finish screen after first ring.
- **Exit:** verified separate orbital domain physics interface, compiler ledger and human story quality testing. Rule-bearing material semantics documented and safe.

### M12 — Content depth and long-term polish

- Extended civilizations/biomes, exceptional 3D dungeons, dozens of advanced spells and material interactions, boss capture/farming, original VFX, sound, deep optional endings, improved low-end GPU path and modding. Long-duration soak tests plus repeated blind human sessions.
- **Exit:** performance + correctness + significant player retention/playtest evidence; honest comparisons against chosen reference games in equivalent scenarios.

## 5. Required technical spikes (measured *before* binding ADRs)

| Spike | Competing candidates | Fixture/output | Decision |
|---|---|---|---|
| S01 Hot brick sim | 8³ vs 16³, CPU SIMD vs WGPU compute, halo vs global | `sand_flood`, `heat_front`, active cells/s, bytes, sync | Brick/kernel choice |
| S02 Fluid model | Local conservative cellular, pressure-column, small finite-volume | U-tube and dam-break equality/mass/feel | Fluid base model |
| S03 Geometry | Bitwise greedy raster, compact vertex-pull, smooth voxel mesh, direct raymarch for select | Cave/castle/edited overhang at 1440p; edit latency/VRAM/p99 | Renderer path by class |
| S04 LOD topology | Parent-cover vs alternate transition strategy; Transvoxel if smooth | Edit small hole at far/near boundary, camera flight, animated morph | Safe residency/stitch rules |
| S05 Structural splits | Hierarchical region graph vs local flood after deletion | Cut bridge/tower then fall/settle, memory/CPU | Detach algorithm |
| S06 Machine network | Dense nodes vs compiled cables/belts, isolated islands | Long linear network and branched system; idle ticks | Graph representation |
| S07 VM | Small typed stack vs register interpreter; compiled glyph/text frontend | branch/sensor/shape operations/s; worst recursion | IR and fuel |
| S08 Resonance | Cached modal graph vs PDE-lite vs purely authored field | Chorister chamber chord, editable geometry, cost & observability | Grounded magic field |
| S09 Primer | Rule-template + event certificates vs scripted achievements | First 20 min, experimental tasks, hint behavior | Instruction/UX |
| S10 Artifact renders | Flat sprite/2.5D/mesh; varying pixel density | 12 silhouettes in inventory/FOV, near/far shader light | Asset representation |
| S11 Cities | PlacePlan+protected void vs direct stamps | Generator 54 comparisons, 10–30 seeds, traversability | Worldgen structure |
| S12 Planet/ring | Planar/patched sphere/domain transitions, orbital double vs fixed | Huge coordinate stability, transfer conditions, resource cost | Long-term topology |
| S13 Secret generation | Hand-linked graphs vs procedural logic constraint solver | 20–50 seeds, reachability, human clue path | Mystery generator |
| S14 Matter compiler | Primitive-op complexity vs compressor size alone | Simple/noisy/complex comparable builds, economic exploits | Cost metric |

**Spike discipline:** one focused session where practical; include exact compiled code, hardware and results, no selection by trendiness. A failed idea remains a documented rejected alternative, not a hidden half-implemented dependency. Long-term ADR only after data.

## 6. SDD structure: per-system executable design

Each material subsystem or deep machine domain must have a **2–6 page concise SDD** (more only when warranted) with:

1. Purpose, player-visible need and C-* traceability.
2. Scope (in/out), dependency/owner map and stable interface.
3. Data layout, field sizes, alignment, CPU/GPU/storage residency, invariants, lifetime and migration.
4. Operations and pseudocode, time/memory/bandwidth complexity and numerical/determinism contracts.
5. Authoritative commands, typed input/output ports, events and permitted VM/UI queries.
6. Tick scheduling, island/sleep/wake rules, adaptive fidelity limits and safe failure states.
7. Budget targets on hardware tiers, low-end feature fallback and monitoring counters.
8. Data/RON schemas with simple working example, source content rules and validation.
9. Unit, property, conservation, replay, fuzz, visual and actual-playtest cases.
10. Named performance fixture with before/after required.
11. Compatibility with existing saves, generator versions, and explicit deleted/replaced code.
12. Risks, spike results and at least two meaningful alternatives rejected.
13. Small ordered implementation sessions and checkable exit conditions.
14. Evidence ledger: reported/code inspected/build/test/native visible/played/profiled.

**Independent critic** reviews ownership, exactness, perf, data-driven complexity, migration, visual integration and target player behavior. Reviewer should not approve vague "it feels alive" without mechanism: *active nest state emits an alarm when vibration threshold is met and nearby defenders respond* is a mechanism. Three failed review rounds → short human escalation with disagreement, not endless rewrite loops. Do **not** treat all 12 rubric dimensions as literally equal priority if a simple SDD legitimately has no GPU data; justified N/A is acceptable.

## 7. CI, property tests and visual reviews

### Automation commands (names proposed; adapt to repo)

```
just check           # fmt, clippy, cargo tests, WGSL validation
just verify          # focused integration, saved fixture round-trips
just bench           # deterministic CPU/GPU scene benchmark with receipts
just scenes          # headless scripted play scenes and invariant assertions
just captures        # native game capture receipts at frozen cameras
just claims          # C-* scorecard with PASS/FAIL/NOT_RUN + evidence paths
just assets-validate # RON/material/texture/LOD/port validators
just docs-check      # SDD claim ID, ADR and reference-source completeness
```

Never change tolerance or remove a failing test simply to obtain a green score. Build pipeline must validate shader compilation and render results separately. Every new generator version is tested against old-version golden generation. Scene timeouts and physics heat/friction tolerance differences must be explained.

### Scenario specification format (proposal; adapt to real harness)

```
id: "m1_mixed_lod_edit_roundtrip"
seed: 2738958700
world_generator_version: 55
voxel_scales_m: [1.0, 0.5, 0.125]
steps:
  - spawn_player_at_named_testsite: "refined_wall"
  - excavate_volume_m3: { volume: 0.125, region: "fine_seam" }
  - stream_away_and_return: { distance_m: 2048 }
  - save_and_reload: true
assert:
  - saved_exact_world_delta: true
  - occupied_volume_change_matches_action: true
  - collision_ray_matches_visual_opening: true
  - no_horizon_gap_on_camera_path: true
metrics: [cpu_p95_ms, gpu_p95_ms, frame_p99_ms, gpu_resident_mb,
          jobs_queued_max, compressed_region_bytes]
```

**This is a schema idea, not current runnable test YAML.** Machine-generated evidence includes build SHA, asset SHA, frozen camera pose, GUI/capture state, device info, command and logs. Recorder replays apply authoritative inputs/events with versioned RNG streams. CPU/GPU comparison is required only when solving the *same declared model*; higher-fidelity reference numerics can serve tolerant truth tests otherwise.

### Gameplay acceptance methods

- **Think-aloud uncoached**: early automations, first magic and interpreting hazards.
- **Expert sandbox:** discover wand synergies, abuse physics, invent boss traps, reverse-engineer machines.
- **Dungeon route audit:** at least two solutions physically traversed per key barrier; no false 'three ways' claimed from static design text.
- **Blind visual:** compare in-game screenshots and camera path with style reference; record real objective flaws: silhouette, palette, texel scale, tree LOD, clutter, light.
- **Mod compatibility:** add new material/recipe via data, version content pack, validate and profile without rebuilding unrelated systems.
- **Soak:** hours of ordinary edits/exploration/saving; test RAM/VRAM leak and world/collider correctness.

## 8. Concurrency and repo ownership safety

**Required order before code:** read working tree status, branch and untracked files; review local instruction docs, build toolchain, Cargo.lock, test histories and asset pipeline. Record file ownership per worktree (terrain/worldgen/shader/horizon/item/UI). Use isolated branches/worktrees for research, meshing, worldgen and art. One designated integrator owns shared files and staging. Do not `git reset --hard`, `git clean -fd`, auto-stash or overwrite concurrent edits. Don't auto-push production changes, merge into main, alter saves or change user-facing branches without permission/context.

**Work composition:** research agents gather verified references; physics author implements CPU reference while GPU specialist prototypes optimized kernel against identical fixtures; procedural art/Blender specialist provides actual source assets and importer manifest; independent critic verifies runtime/memory/visual claim. These agents coordinate through interfaces, not simultaneously editing the same WGSL files.

**Status vocabulary:** `PROPOSED`, `INSPECTED`, `IMPLEMENTED`, `BUILT`, `UNIT_TESTED`, `INTEGRATION_TESTED`, `NATIVE_SEEN`, `PLAYTESTED`, `PROFILED`. A single item may have different statuses in different dimensions. Partial completion must be reported explicitly. No background promises or invented benchmark durations.

**Per-session output**: `BASELINE`, `SCOPE`, `DECISIONS`, `CHANGED`, `VALIDATION`, `PERFORMANCE`, `RISKS`, `NEXT`. Evidence table with exact paths and test output. End with smallest high-value next slice; do not plan arbitrary 30-step futures instead of finishing a feature.

## 9. Risk register and mitigation

| Risk | Consequence | Containment |
|---|---|---|
| Universal abstraction overdose | Months of code without playable result | Thin vertical slices; measure before layering |
| Global fine voxels | VRAM/RAM exhaustion, stalls | Sparse uniform regions + exact edit overlays + island refinement |
| Magic → energy perpetual motion | Broken economy/physics | Typed conversion graph, explicit sources, entropy/dissipation ledger |
| GPU simulation authority | Cross-driver divergences and readback stalls | CPU reference, event-owned authoritative state, tolerant model boundaries |
| Procedural interiors empty or blocked | Bad dungeon gameplay | Protected navigation volumes, role grammar, test traversals and artistic camera reviews |
| Wands generic/bulky | Noita parity failure despite lots of code | Hand-authored silhouette families, pixel icon inspection, actual Blender/FOV testing |
| Too many materials/recipes | Busywork and opaque combinations | Unique physical traits, curated high-value interactions, sensible alternatives |
| Overambitious 40 h orbit | Rushed narrative and incomplete midgame | Ring glimpsed early; usable space late; playable deep-earth first |
| Puzzle clue dead ends | Wiki-only secrets / unwinnable seed | Constraint solver, 2+ hint paths, recoverable items, human blind tests |
| AI agent conflicts | Lost code or broken shader branches | Worktree ownership, integrator, no auto resets, frozen captures |
| Performance 'optimizations' hide graphics | Lower time but worse world | Paired pixel/geometry and invariant gates |
| NPC ecological infinite spawns | Player frustration and simulation overload | Habitat-based population budgets; local threat logic |
| Dynamic rigid body explosion | Physically plausible but unplayable stalls | Cohesive body segmentation, aggregated debris, body count budget |
| Save corruption on generation updates | Permanent user data loss | Generator versioning, atomic journals, backed migrations |

## 10. Research source index and evidence date

Technical references verified before or on 2026-10-26:

- Noita engine/designer GDC 2019: https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design/
- Noita wand appearances and statistics: https://noita.wiki.gg/wiki/Wand / https://noita.wiki.gg/wiki/Wand_Appearance
- Factorio transport lines, sim optimization: https://factorio.com/blog/post/fff-176
- Factorio 2.1 quality/automation release notes (2026-10-23): https://www.factorio.com/blog/post/fff-449
- Transvoxel by original algorithm author: https://transvoxel.org/
- WGPU API 30.0.1 and experimental ray/mesh restrictions: https://docs.rs/wgpu/30.0.1/wgpu/
- WGPU timestamp queries example: https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html
- Rapier/Parry voxels and localized changes: https://docs.rs/rapier3d/latest/rapier3d/geometry/struct.Voxels.html
- Blender 5 Python API, version-specific access: https://docs.blender.org/api/5.0/
- Aseprite batch export: https://aseprite.com/docs/cli/

**Freshness precaution:** A search result for a paper titled *Dynamic GPU Sparse Grids and Voxel-sequential Processing in NanoVDB* is marked **December 2026**, after this document's October 26 date. It is not treated as published current evidence or an implemented engine dependency. The idea of flat active-voxel iteration is included as a *research direction* to validate using currently available code and hardware.

**Source-specific references in this work:**

- User uploaded `Pasted markdown(20261026-113904).md` (Original Game Bible and Plan-Generation Protocol), retained the C-* codes and core narrative/plan protocol, with explicit normative corrections.
- Prior `00_MASTER_DESIGN_AND_INTEGRATION_V2.md` / *THE LIVING ENGINE* from user's file library: engineering/circuit/nested chip inspiration, no invented verification of current live code.
- Prior `Generator_55_Places_Quality_Overhaul.md`: six place families, observed R7 defects, seed and version targets; code evidence is a reported status, not a runtime audit.
- Prior `Axiom_Magitech_Art_Production_Prompt_Pack_2026-10-26.md`: visual grammar, Blender workflow, material shaders, asset families, true first-person evidence requirements.

## 11. What to build right now — first sprint definition

**Sprint 00 (bounded)** — perform full repository reality audit and produce one replayable **First Valley baseline** with frozen camera positions and CPU/GPU captures: first-person current wand, active old machine, castle roofs, town streets, mine/pit/library, horizon with tree canopies, starter cave. Avoid refactoring or adding features during this audit. Record actionable discrepancies between reported generator 54 R7 state and what the executable actually does.

**Sprint 01 (bounded)** — repair the most damaging observed integration invariant (world LOD/proxy hole / stale collision / physical roof geometry as evidence dictates) and improve one actual 3D asset to art acceptance, with pixel source and in-game view. Don't start 900 wand variants or a full space subsystem.

**Sprint 02 (bounded)** — implement *one* complete gameplay loop linking a mined resource, a machine with real inputs, a Noita-like spell affecting the world and a physically findable ruin clue. Demonstrate fixed seeds, actual camera footage, save/reload and CPU/GPU metrics. Only then expand catalog.

The engineering and narrative specifications should be **a forcing function for excellent playable slices**, not a pretext for leaving everything at the proposal stage.
