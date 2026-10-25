# 0014 Reconciling ENGINE_CORE_PROMPT_v2 with PLAN2 and the code

Date: 2026-10-23. Status: proposed, awaiting approval (session 1 of the v2 spec stops here). The user decided rows 6, 11 and 14 on 2026-10-23.

The v2 mission specification (`ENGINE_CORE_PROMPT_v2.md`) is the long-range vision. `docs/ENGINE_CORE.md` and `docs/GAME_DESIGN.md` turn it into a plan and **supersede `docs/PLAN2.md`** as the roadmap. PLAN2's decisions D1–D7 carry over unchanged, and its open phases (A6–F) are folded into milestones M1–M7 (ENGINE_CORE §11). This ADR records every place where the spec, PLAN2 and the code disagree, and says which side governs.

Original session-1 boundary: no code changes followed from that planning task. The 2026-10-24 request explicitly continues refinement and bounded implementation; ADR 0017 and the dated execution plan track that work without treating all proposed migrations as approved or completed. `CLAUDE.md` keeps describing implemented behavior.

## Conflicts and resolutions

| # | Topic | Spec v2 says | Repo and PLAN2 say | Resolution |
|---|---|---|---|---|
| 1 | Save version | Save v30 (III.10) | `VXWSAV29`, `SAVE_VERSION = 29` (D7, `CLAUDE.md`) | v29 is current. The spec's "v30" becomes the next bump, and it happens with paged chunks and LZ4 (ENGINE_CORE §10). It needs its own migration ADR. |
| 2 | Tick order | One `const` array, 18 stages (III.1.3) | Current order: expedition, factory, player, adventure, cellular/rigid/thermal, day cycle (`CLAUDE.md`) | The spec's order is the target. `CLAUDE.md` changes only in the same change that makes the new order true. See also row 13. |
| 3 | Rigid bodies | Integer fixed-point bodies, GPU surface-cell contacts, GPU flood fill (III.4) | CPU floats, GPU collider extraction deferred (ADR 0011) | ADR 0011 stands for M2. The integer body model and GPU contacts are decided at M2 exit, by a new ADR, when a scenario exceeds the 64×128 cap. |
| 4 | CA exit target | 500k awake cells at 60 tps on the low-end profile (III.3) | 250k active cells at 60 tps on the CPU reference, GPU byte-identical (PLAN2 §1a) | Both are kept. 250k on the CPU reference is the correctness exit for the CA work in M1. 500k awake cells on `lowend` with the GPU backend (ADR 0016) is the M1 performance exit. |
| 5 | Brick size | 8³ GPU bricks, 512 cells (III.2) | 8³ bricks, 32³ chunks (D1, ADR 0007) | Agreed. |
| 6 | Schedule | Milestones M1–M7, sessions 1–48 (XI) | Phases 0, A–F (PLAN2 §2) | **The spec's milestones govern** (user decision). PLAN2 is historical; ENGINE_CORE §11 maps each open PLAN2 phase onto a milestone, so no accepted PLAN2 work is lost. PLAN2's reserved ADR numbers 0012 ("shader includes") and 0013 ("terrain renderer") are void: 0012 is the horizon ADR. |
| 7 | Mobs | Mob table, nav, utility AI, villagers (III.5) | Not in PLAN2; enemies are hand-written steering plus a bounded best-first search in `crates/app/src/world/adventure/` | Mobs are M2 (ENGINE_CORE §7). Existing enemies and bosses migrate onto the mob table in M2, keeping their behaviour. |
| 8 | Function size | Functions ≤ 60 lines, nesting ≤ 3, public surface ≤ 40 items per crate (III.1.9) | Files ≤ 300 lines (`CLAUDE.md`) | The spec's limits apply to new code only. Existing code is not reformatted to meet them. A lint is added in the session that introduces it. |
| 9 | Missing reads | A PERF doc and a WORLDGEN doc are listed as required reading | Neither file exists | ENGINE_CORE cites ADR 0006 and the bench baselines for performance and `crates/worldgen/src/` for generation. Writing both docs is an M1 deliverable (ENGINE_CORE §14). |
| 10 | Lint enforcement | Float-free sim state, id matches only in `content/`, tick-order test, hash-version golden test, no unwrap (III.1, III.10) | Partial: D3 pins the CA hash; no float, unwrap, `rand`, `Instant`, file-size or public-surface lints exist | Each lint is added in the session that makes it true, not before (ENGINE_CORE §2 lists the session for each). |
| 11 | Cell layout | Two packed u32 planes (III.2) | 8-byte cell (ADR 0007); `VOLUME_FULL = 64` in code, while ADR 0007's text says 0..=8 | **Hybrid** (user decision): the 8-byte cell stays; velocity, aux and field planes are optional per-brick planes; material widens to u16 only past 255 materials. ADR 0015. ADR 0007's volume text is corrected. |
| 12 | CA authority | `CaBackend` behind every reader, with a 2-tick authoritative lag | ADR 0009 said "implemented"; the code has neither the trait nor the mirror, and the game runs the legacy f32 simulation in `crates/app/src/simulation/` | ADR 0009 is corrected to "not yet implemented". Wiring the game to `engine_sim` through `CaBackend` is M1 session 2 (old PLAN2 A6). |
| 13 | Tick agreement | Headless and window run one tick order | `CLAUDE.md` says headless and `main.rs` agree; `main.rs` runs each system once per frame with `steps × STEP` as dt, headless steps per tick. `Instant::now()` is used in `crates/app/src/simulation/residency.rs` | Recorded as drift. M1 session 2 introduces the shared tick-order array and a test that headless and window produce the same hash. The residency timer moves to a tick counter. |
| 14 | Low-end profile | `lowend` is the bench default (III.1.4, VIII.9) | No low-end profile; all numbers come from an RTX 4090 | **`lowend` = Intel Iris Xe laptop** (user decision). ADR 0016 defines the caps and how the dev machine emulates it until a device is benched. |
| 15 | Content drafts | A tech tree and a magic tree as RON files under assets in session 1 | Every `assets/*.ron` needs a loader and a round-trip test (the project's saves and content rule) | The drafts live in `docs/game/drafts/` until the session that adds their loader, validator and round-trip test (M3 for tech, M5 for magic). |

## Consequences

- The long-range sessions remain proposals. Bounded existing-system improvements authorized by the current request have their own implementation/evidence status; they do not implicitly approve the remaining roadmap.
- `CLAUDE.md` invariants are not edited for spec targets. They change with the code.
- Rows 1 and 3 each need a follow-up ADR before the session that touches them starts.
