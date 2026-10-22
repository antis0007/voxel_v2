# Repo audit (Phase 0, 2026-10-22)

Single crate `minecraft_style_voxel_prototype`, branch `growth-xmb`, rustc 1.93.1. Numbers measured on the working tree, not HEAD (HEAD does not build alone). Companion: `PLAN.md`.


## A. Size (no `tokei` installed; `wc -l`, blank+comment lines included)

| Area | Files | LOC |
|---|---|---|
| Rust `src/` | 231 | 121,941 |
| WGSL `src/shaders/` + `src/fluid_compute.wgsl` | 29 | 9,356 |
| `docs/` (md+txt) | 115 | 12,858 |
| `tools/` (ps1/py) | 30 | 2,332 |

Per top-level module (file + dir): world 55,342 · render 23,188 · egui_ui 16,336 · mesh 7,598 · main.rs 4,386 · simulation 2,803 · fluid 2,556 · material 1,691 · creative 1,295 · audio 864 · review harness modules ~3,300 · rest < 700 each.

Top 15 files: render.rs 4833 · main.rs 4386 · world.rs 3767 · world_shader.wgsl 3287 · egui_ui.rs 3203 · world/adventure.rs 2863 · world/dungeon.rs 2603 · world/inventory.rs 2435 · adventure/wands.rs 2376 · world/factory.rs 2153 · world/sites.rs 2152 · world/landscape.rs 2057 · render/streaming.rs 2055 · render/horizon.rs 2028 · mesh/models.rs 1984.

Rule-of-300 status: **118 of 231** `.rs` files exceed 300 lines. Largest fns: `Gpu::step_inner` ~1,119 lines (render.rs:1897), `Adventure::update` ~1,500 (adventure.rs:1028), `main()` event loop ~3,000 (main.rs:1366), `build_ui` 628 and `build_performance_ui` 628 (egui_ui.rs), `parse_benchmark_config` 286 (main.rs:242).

## B. Git / repo state — **blocking precondition**

| Fact | Value |
|---|---|
| Tracked files | 270 |
| Untracked files (all) | 1,242 |
| `src/` tracked / untracked / modified / deleted | 167 / **97** / 94 / 3 |
| Untracked `src/` LOC | **25,979** |
| Working tree vs HEAD (tracked only) | 108 files, +12,781 / −7,703 |
| `main..HEAD` | 26 commits, 193 files, +45,327 / −6,690 |
| Other branches | `snapshot/2026-10-22-working-tree`, `spells-followup` |

- HEAD does not build (tracked shaders include untracked `mood_shared.wgsl`, `underwater_shared.wgsl`; AGENTS.md says so). ~21% of the code exists only on disk.
- `.cargo/config.toml` untracked (pins `target-dir = "target"`, overriding global Pyrenet cache).
- `AGENTS.md` says the tree is edited by several sessions at once. A multi-phase refactor cannot proceed under concurrent edits.
- Repo root violates its own `repo_layout` test: `.codex/`, `RTK.md`, `horizon.patch`, `horizon.rs` (779-line stale copy), `world_shader.wgsl` (1,797-line stale copy), `horizon_terrain_upgrade.zip`, `terrain_shader_upgrade.zip`, `review-artifacts/` (zips, pngs, gifs), `reviews/` (full source snapshots `baseline/src/...`, `candidate/src/...`), plus gitignored `*.cmd`, `*.log`, `*.exe`, `out.txt`. `cargo test repo_layout` currently fails.
- Tracked binaries: 4 PNG icons + 2 TTF fonts in `assets/` (legitimate). No tracked exe/zip.
- Disk: 16 GB free of 953 GB; `target/` is 14 GB.

## C. Dependency graph (top-level modules, `crate::X` reference counts)

```
world(55k)  -> fluid 28, simulation 12, fast_hash 12, mesh 11, player 9, tool_effects 5, material 3
render(23k) -> world 87, mesh 39, fluid 10, shader_sources 7, terrain_review 4, material_studio 3, egui_ui 1
egui_ui(16k)-> world 138, render 17, preferences 7, torch_review 6, player 3, audio 3, material_studio 2
mesh(7.6k)  -> world 39, material 12, wand_style 3, tool_effects 3, fluid 3, player 2
simulation  -> world 18, fast_hash 2, player 1, mesh 1, fluid 1
fluid       -> world 2, fast_hash 1
preferences -> egui_ui 5, world 3, audio 2
player      -> world 3, fluid 2
material, audio, fast_hash, shader_sources -> (nothing)
```

Cycles: **world↔mesh** (11/39), **world↔simulation** (12/18), **world↔fluid** (28/2), world↔player (9/3), render↔egui_ui (1/17), egui_ui↔preferences (7/5), egui_ui↔material_studio, mesh↔tool_effects, world→mesh→wand_style→world, world→tool_effects→mesh→world.
Shipping code depends on review harnesses: render→terrain_review, material_studio; egui_ui→torch_review, material_studio.

God modules (responsibilities): `world.rs` = chunk storage + day/night/weather/lighting + terrain sampling/generation + World struct (inventory sync, save migration, eat_food) + voxel get/set + light propagation + coord helpers (8 themes, 80 pub fn, 38 submodules). `render.rs` = settings + stats + profiler + uniforms + Gpu bootstrap + 1,100-line frame step + uploads + capture (8 themes). `main.rs` = benchmark CLI (40 flags) + input + review hooks + 3,000-line event loop. `egui_ui.rs` = UI state + 3D held-item projection + loading UI + HUD dispatch. `world/adventure.rs` = enemies + projectiles + combat AI + spells + save codec + 1,500-line `update`.

`world/` (55k) is really five things: voxel storage/streaming (chunks, regions, storage, activity, water, resident ~5k), world generation (landscape, sites, dungeon, architecture, valley, surface, terrain_variety, trees, vegetation, zones, undercrown ~14k), automation (factory, inventory, expedition, habitats, recipe_index ~7k), adventure/magic (adventure/ 16.5k), environment (weather, mood, lore, orbital ~2.5k).

## D. Dead code

- `cargo check --all-targets`: **114 warnings**, all dead-code class (26 unused fns, 15 unread fields, 12 unconstructed structs, 11 unused consts, 6 unused enums …). Concentrated in `src/fluid/{tank_view,processes,network,containers,catalog,machine}.rs` (47), `adventure/growth.rs` (9), `egui_ui/wand_xmb.rs` (7), `undercrown/{plan,state}.rs` (11).
- **Never-compiled files (no `mod`/`#[path]` anywhere):** `mesh/lod.rs` 259, `mesh/sculpted.rs` 242 (both tracked), `adventure/{choir 305, ferryman 399, glassweaver 382, manta 279, miller 347, unfinished 471}.rs` (untracked) = **2,684 LOC**.
- `src/fluid/{catalog,containers,machine,network,processes,tank_view}.rs` (~2,200 LOC): compiled, referenced only by each other. AGENTS.md says "not wired"; accurate wording is "compiled but unused". Per AGENTS.md their owner decides — **needs user decision**.
- Review/benchmark harness compiled into the shipping binary: sandbox_review 350, receiver_review 313, torch_review 466, terrain_review 101, horizon_review 549, wand_review 578, water_review 158, reference_environment 383, material_studio 367, dungeon_review 41, adventure/enemy_review 403, habitats/review, gen_probe 215 = **~4,000 LOC**, gated only by CLI flags or env vars at runtime.
- `#[allow(dead_code)]`: 6 sites + 3 `allow(unused)` (fluid/ripples.rs:78, shader_sources.rs:24/29/105/161, undercrown.rs:16/20, world.rs:51).
- `src/render/axis_loop_reference.wgsl.txt` escapes the shader-registry test via its extension.
- `cargo-udeps`/`cargo-machete` not installed. 14 direct deps, 359 in graph; `anyhow`, `env_logger`, `pollster`, `image(png)`, `windows`, `ash` all look used. No `[features]`.

## E. Duplication

Rust: `smoothstep` (world.rs:1380, weather.rs:692), `hash2` (world.rs:1394, weather.rs:729), `hash3` (world.rs:1400, render.rs:1166, render.rs:1333 — two identical copies in one file), `hash`/`mix`/`mix32` RNG helpers in 8 files (world.rs, lore.rs, weather.rs, discovery.rs, status.rs, ground_detail.rs, wand.rs, render.rs), `lerp` ×2, `river_top` ×4, `encode`/`decode` ×16 hand-rolled per-type save codecs. Two parallel resource systems: `src/fluid/*` vs `src/world/factory/*`. Repo-root `horizon.rs` / `world_shader.wgsl` are stale copies.
WGSL: no include mechanism; `shader_sources.rs` concatenates `include_str!` constants. Copy-pasted ×3: `weather_uv`, `weather_level_weight`, `weather_edge_distance`, `weather_clip_region`, `sample_weather_bilinear` (cloud_shader, cloud_composite, world_shader); ×2: `srgb_encode/decode`, `sky_dir`, `rayleigh_phase`, `shadow_field_at_world`; `struct Lighting` declared in two files, `struct CameraUniform` in two files.
Docs: 115 files, 12.9k lines; ~49 top-level docs not named in `docs/README.md`; `ENGINE_AUDIT_AND_REFACTOR.md` ("start here") cites deleted `src/render/clouds.rs`; `WORLD_OVERHAUL.md` cites deleted `fluid/legacy_gpu.rs`, `fluid/active.rs`; `docs/legacy/implementation_plan.md` cites 15 missing files.

## F. Misleading / hidden control surface

- TODO/FIXME: 0. Commented-out code: negligible (≤3 lines per file).
- **~70 distinct `std::env::var("VOXEL_*")` hooks** control behaviour at runtime (horizon ×12, review hooks ×15 in main.rs, hart ×7, tollkeeper ×5, render ×4 …) plus unprefixed `GEN_PROBE_ORDER`, `LANDMARK_SEED`, `PROFILE_VERSION`. Three are set by `tools/` scripts but read nowhere (`VOXEL_HORIZON_EDITS`, `VOXEL_TERRAIN_EDITS`, `VOXEL_BENCHMARK_NORMAL_PRESENT`).
- **~40 `--benchmark-*` CLI flags** parsed by hand in main.rs:246–513.
- `tools/`: 17 `Review-*.ps1` launchers (all flags still exist), 3 python horizon comparers, 4 summarisers, icon/palette generators, `code_advisor.py`, `__pycache__/` junk. `.codex/hooks.json` points at an absolute `D:\AI\...` venv.
- AGENTS.md claims verified: `repo_layout` test exists (currently failing on root junk); every `.wgsl` is registered (true); fluid "not wired" (wrong wording).

## G. Build / test / run surface

| Command | Time (warm `target/`) | Noise |
|---|---|---|
| `cargo check` after touching one file | 1.7 s | 105–114 warnings replayed every time |
| `cargo check --all-targets` after touch | 2.2 s | same |
| `cargo test --no-run` after touch | 11.6 s | |
| `cargo nextest run` (414 tests run, 469 `#[test]` attrs) | 17.7 s | **6 failures on the current tree**: `audio::output::tests::equal_cue_voices_share_one_normalized_bus`, `repo_layout::tests::repo_root_holds_only_project_entries` (root junk), `world::storage::tests::{save_and_load_preserves_survival_inventory_and_migrates_v12, save_version_17_is_accepted_and_magic_agrees, v22_keeps_portable_charge_drops_matrix_pool_and_partial_assembler}` (save-format tests; 3 of them, likely one shared cause), `world::sites::causeway_review::causeway_deck_gap_and_reward_exist_at_each_voxel_scale` |
| `cargo build --release` | not measured this session (thin LTO, codegen-units=1; AGENTS.md notes 14 GB target) | |

Observations: single crate, so any edit rechecks 122k LOC; `profile.dev` already uses opt-level 1 and deps at opt 3; no `mold`/`lld`/`sccache` on PATH (Windows; `rust-lld` ships with the toolchain and can be set via `-Clink-arg=-fuse-ld=lld` on msvc = `-Clinker=rust-lld`); no `rust-toolchain.toml`; no `justfile`; no `benches/`, `tests/`, `examples/`; no criterion; no `.claude/settings.json` (empty `.claude/`). `build.rs` validates every composed WGSL program with naga on every shader change. Running the game to review a change = window + GPU + 40-flag command line + env vars; evidence captured under `target/visual-direction/`.

## H. Hot paths, determinism, content

Hot paths: meshing `SolidMesher::build` mesh/solid.rs:279, `coarsened_mesh` mesh.rs:714 (LOD); streaming `render/streaming.rs:693–1323`; distant terrain `render/horizon*.rs` (8k LOC, 16 files via `#[path]`); CA `simulation/cellular.rs:212 step` at fixed 1/120 s (`simulation.rs:22`), `rigid.rs:81`, `thermal.rs:30`; factory `factory.rs:1461 tick`, `factory/extraction.rs:118`; frame orchestration inline in main.rs:3578–3900 + `render.rs:1897 step_inner`; 9 `create_render_pipeline`, 15 `begin_render_pass`, 10 compute passes. Existing measurement: `src/perf.rs` CPU phase timings, in-app `--benchmark-*` sweeps, ad-hoc timing loops inside tests (dungeon.rs:2152, gen_probe.rs, sites.rs:2125). **No criterion benches.**

Determinism: fixed-step accumulator exists, but (1) `factory/extraction.rs:158,175,361` and `expedition.rs:796` cap work per tick by **wall-clock deadline** → machine-speed-dependent state; (2) factory and expedition receive batched `dt` rather than per-step; (3) no `rand` crate, 8 ad-hoc hash RNGs, world seed threaded through terrain gen but not verified for adventure/status/mesh hashes; (4) **no headless mode** — every `--benchmark-*` path opens a winit window; (5) save format is a hand-rolled binary `VXWSAV28`, version 28, per-type `encode/decode` fns, no serde.

Hardcoded content (→ data files):

| Content | Where | ~LOC | Entries |
|---|---|---|---|
| Materials/blocks | `material.rs:237–1439` `define_materials!` → `BlockId` | 1,200 | ~92 |
| Spells | `adventure/spell_defs.rs:64–1489` `static SPELLS` | 1,425 | ~79 |
| Recipes | `inventory.rs:394/657/723` const slices | 360 | ~39 |
| Items | `inventory.rs:117–330` enum + 10 match fns | 210 | |
| Machines | `factory.rs:42–280` `Kind` (27) + 8 match fns | 240 | 27 |
| Bestiary stats | `adventure/bestiary.rs:41–180` | 140 | 10 |
| Icons (ASCII sprites) | `egui_ui/icons.rs:24–835` | 810 | |
| Biomes | `landscape.rs:28` | 55 | |

≈ 4,400 LOC of tables; moving to RON saves ≈ 3,900 LOC net of a ~400-line serde loader. Requires adding `serde` + `ron` (**dependency decision**).
