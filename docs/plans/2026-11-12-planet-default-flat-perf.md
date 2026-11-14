# Plan: Planet as the default world, faster flat worlds

Status: proposal, 2026-11-12. Nothing here is implemented. Gate numbers below are read from source, not assumed.

## Facts from source
- New Planet worlds already use `PLANET_GENERATOR_VERSION` = 287 (`crates/voxel/src/config.rs:152`).
- `CURRENT_GENERATOR_VERSION` = 241 is the default for every other world type (`config.rs:5`).
- The New world form already preselects Planet (`front_end.rs:27,56`), but the list labels Hills "Default" (`front_end.rs:492`).
- Audit item 16 (`docs/perf/2026-11-06-audit-findings.md:27`): solved planets and horizon samples have no disk cache, so every launch repeats them. Check whether `geology_cache` has since covered this.
- Saved worlds keep their stored generator version. Never migrate.

## Goal and rule
Planet must be a better all-round experience and not slower or laggier than the Hills default today. A planet change ships only when it meets the budgets below on the same hardware and poses.

## Phase 0: measure (no code)
1. Baseline: Hills, Superflat and Planet 287, same seed, same camera route. Record these for each:
   - time to first playable frame;
   - p50, p95 and p99 frame time in static view, motion and a flyover;
   - worst hitch and its source stage;
   - RAM and VRAM.
   Use the existing frame-bench and `frames.csv` (see `docs/perf/`). Interleave A/B passes and take the best per side, pinned to P-cores.
2. Planet first-load cost: split solve, horizon build and meshing. Note which stages the disk caches already skip.
3. Flat world: profile where its frame time goes. Worldgen is trivial there, so expect meshing, streaming, shadows and uploads. Confirm before optimizing.

Exit: a table of the numbers above that all later phases cite.

## Phase 1: planet no slower than Hills
Work from the Phase 0 table, biggest cost first. Likely candidates:
- Persist the solved-planet and horizon caches (audit item 16), keyed by seed, generator version and planet settings. A stale-cache key mismatch must rebuild, never reuse.
- Run the evolved history and refinement stages off the main thread, with a loading screen that is honest about the stage.
- Keep far-field detail within the existing tile cap and hysteresis settings (see the horizon notes); don't add per-frame work.

Budgets (proposed, to be fixed after Phase 0): Planet p95 frame time within 10% of Hills; first playable frame no more than 1.25x Hills on a warm cache, with the cold cost shown in the UI.

## Phase 2: flat worlds faster
Driven by Phase 0 item 3. Candidate levers, each needing evidence before it is built:
- Cull or skip passes a flat, uniform surface cannot need (cascade refresh cadence, far veil, water or cave cost on a world with none).
- Mesh and upload cost for large uniform chunks: use the packed block arrays (single-value chunks) end to end.
- Lower the default view and horizon cost for Superflat only, as a Superflat-specific default, not a global cut. Do not persist governor cuts to preferences (the ratchet bug in memory).

## Phase 3: make Planet the default
Small and last, so it can't land before Phase 1 passes:
- Relabel in `front_end.rs`: Planet is "Default"; Hills becomes "Classic" or similar. Player-visible text must follow `UI_RULES.md`; new names need owner approval.
- Decide whether `CURRENT_GENERATOR_VERSION` should track `PLANET_GENERATOR_VERSION`. Hills and Superflat gates must stay at their frozen values. Choose either a separate per-world-type default or a bump to 287 with a historical-gate test. Read `.claude/rules/saves.md` first.
- Multiplayer: confirm joiners get the host's world type and generator version unchanged.

## Verification
- Old-save fixture for each world type still loads with identical terrain.
- Generator A/B: `--benchmark-generator-version`, `--terrain-survey`, fixed review poses.
- Visual: before/after captures at the same poses for anything that changes the look (owner rejects unverified look changes).
- Run `python -I scripts/workflow.py verify` at checkpoints.

## Risks
- Peers are editing planet files and `planetary/` is untracked, so land Phase 1 work in small, separately committable hunks.
- A cold-cache planet may never match Hills on first load; the fallback is a cached, honest loading stage, not hiding the cost.
- Making Planet default changes first-hour pacing (spawn features, resources). Needs a playtest pass, not just frame times.
