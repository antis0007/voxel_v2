# Implementation prompts, 2026-10-30: index and shared rules

Written for: coding agents picking up one prompt each. Every prompt is
self-contained, but all of them inherit the rules below. Read this file first.

## Why these exist

A review on 2026-10-30 found the repo in this state:

1. The Manyfold, portal, crystal-gate and hell-gate work (about 27 files, one
   to two days of sessions) is **untracked or uncommitted** in a shared tree
   with 665 dirty files. HEAD (`a678a81c`) contains none of it. A safeguard
   snapshot exists as branch `wip/whole-tree-20261030`; do not delete it until
   prompt 01 is finished and its commits are verified.
2. The Manyfold brief (`docs/NEW/The_Manyfold_Design_and_Implementation_Brief.txt`)
   asks for an inhabited maze dimension built in stages. What exists is a
   traversal-only tube (Stage A of the separate open-maze brief), a portal
   engine and a hell-gate link. Brief Stages 1, 2, 4, 5 and 6 do not exist.
3. C: ran out of disk space while a world was open; the game shows
   "Saving needs attention: Region write failed ... os error 112".
4. Held-item art is poor: first-person swords and tools are extruded 16x16
   icons with one generic pose, and the torch flame is a smooth six-sided mesh
   with continuous colour and motion, not pixel art.

Owner approval of the names Manyfold, Webway (module name stays
`portal_network`), Crimson Nexus and Hall of the Crimson King is recorded in
`docs/decisions/0039-axiom-expansion-briefs.md` (addendum 2026-10-30). Other
brief names stay unapproved.

## Prompt list and order

| # | File | Depends on | Touches saves or gates |
|---|------|-----------|------------------------|
| 01 | `01_LAND_UNTRACKED_MAZE_PORTAL_WORK.md` | none, do first | no (lands existing gates) |
| 02 | `02_GATE_REGISTRY_AND_TRACKING.md` | 01 | docs only |
| 03 | `03_DISK_FULL_AND_SAVE_RESILIENCE.md` | none, can run in parallel | no save bump |
| 04 | `04_TORCH_PIXEL_FLAME.md` | none | no |
| 05 | `05_HELD_TOOLS_SWORDS_AND_ARM_ANIMATION.md` | 04 shares the held-item file | no |
| 06 | `06_MANYFOLD_STAGE1_FOLDFIELDS_AND_STILLCOURT.md` | 01, 02 | new generator gate |
| 07 | `07_MANYFOLD_STAGE2_ILLUSION_VOCABULARY.md` | 06 | new generator gate or same as 06 |
| 08 | `08_MANYFOLD_STAGE3_STAIR_FOLD.md` | 01, 06 | none expected |

01, 03, 04 can run at the same time in separate worktrees. 05 must wait for 04
to merge or be rebased onto it, because both edit
`crates/app/src/mesh/models/held_item.rs`.

## Rules every prompt inherits (from CLAUDE.md, `.claude/rules/`, memory)

Process
- One writer per prompt. Work in your own worktree under your session
  scratchpad. Never remove a worktree you did not create. After `EnterWorktree`
  the Bash tool refuses git and scripts; use PowerShell `git -C <path>`.
- The shared tree is edited by other sessions at the same time. Own your scope
  and finish it. Never wait on or poll another session. Checks that fail only
  because of someone else's files are reported as "unverified: <reason>".
- Commit your own files only. When a file mixes your hunks with a peer's, use
  the recipe in memory `commit-mine-via-clean-worktree` (assemble HEAD plus your
  hunks in a clean worktree, build and test there, then compare-and-swap the
  branch). Plain `git commit` in the shared tree sweeps peers' staged files in.
- Commands are only the `just` recipes in CLAUDE.md. End the session with
  `just session-report`. Gate: `just gate_status`, `just gate`.
- No `cargo fmt` (the repo has no rustfmt.toml and it rewrites whole crates).
- C: may be full. If it is, build to D: with a separate target dir and say so.
  Only `target/{debug,release-fast}/incremental` is safe to delete.

Code
- No `unwrap`/`expect` outside tests and `main.rs` startup. No `Instant::now()`
  in simulation. No `rand` in game code (positional hashing from the world
  seed, `engine_core::hash`). No glob re-exports. No new dependency without an
  ADR line. No content tables in Rust: rows go in `assets/*.ron`; changed
  struct means rerun the ignored `dump_*_ron` test.
- No new file over 300 lines when it can be split. No scratch files in the repo
  root (`repo_layout` test).
- Comments state why or an invariant. Default to none. Doc comments open with
  what the item does in under 12 words. No em-dash asides.

Text
- Player-visible text follows `crates/app/src/egui_ui/UI_RULES.md`: states and
  nouns, numbers with units, no second person, no exclamation marks. Add
  clean files to `egui_ui/text_rules.rs`. Run `just lint-ui`.
- Banned words in any text: seamless, robust, leverage, delve, elevate,
  comprehensive, crucial, ensure, streamline, powerful, elegant. In player text
  also: amazing, ultimate, epic, unleash, journey, immersive, explore, harness,
  mysterious, ancient, forgotten, echo, essence, resonance.

Saves and worldgen (`.claude/rules/saves.md`, `world.md`)
- `SAVE_VERSION` and `CURRENT_GENERATOR_VERSION` are read from
  `crates/app/src/world/storage.rs` and `crates/voxel/src/config.rs`, never
  from CLAUDE.md or older notes (they are stale). A save bump needs a migration
  and an old-save load test (`just test migrate`, `just test save`).
- Worldgen behaviour changes need a generator gate claimed in
  `docs/worldgen/GENERATOR_GATES.md` first. Never edit a historical gate.
  Dormant work names its gate with a `pub const`.
- Prefer sidecar files (pattern: `world/portals/sidecar.rs`, `gates.ron`;
  `world/farming/sidecar.rs`) and voxel edits over new save fields.

Visual work (`.claude/rules/render.md`, `.claude/skills/render/SKILL.md`)
- A host struct and its WGSL declaration are one contract; change both in the
  same patch. Every `.wgsl` is listed in `shader_sources.rs`; `cargo check`
  validates shaders through `build.rs` but does not prove the picture.
- Anything glued to the camera is textured in view space (`is_viewmodel`,
  `vm_position`). One pixel size per scene. Palette:
  `assets/pixel_palette.txt`. User feedback on record: smooth continuous
  shading, no posterised look in the world; but viewmodel and fire accents are
  deliberately pixel art.
- Evidence is a before/after capture from the same pose with a unique label
  via `tools/Review-VisualDirection.ps1`, with PNGs inspected, stored under
  `target/<label>/` (never overwrite a label). Passing tests do not prove a
  look change; the owner rejects unverified look changes.

Report format at the end of every prompt: what works, what was captured and
inspected, what was exercised in play, what failed or was skipped and why.
