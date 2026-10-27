# Untracked inventory

Generated 2026-10-27 02:00 by `python -I scripts/tracked_deps.py inventory`; regenerate rather than edit.

505 untracked files; 75 build edges from tracked files; 64 commit sets.

## Commit sets

Each set must land in one commit with its tracked referrers, or the referrers' commit does not build.

### Set 1: 7 files, 7 tracked referrers

Referrers: `crates/app/src/main.rs`, `crates/app/src/world.rs`, `crates/app/src/world/adventure.rs`, `crates/app/src/world/adventure/reliquaries.rs`, `crates/app/src/world/factory.rs`, `crates/app/src/world/interaction.rs`, `crates/app/src/world/inventory.rs`

- `crates/app/src/multiplayer.rs`
- `crates/app/src/multiplayer/actions.rs`
- `crates/app/src/multiplayer/authority.rs`
- `crates/app/src/multiplayer/gameplay.rs`
- `crates/app/src/multiplayer/inventory.rs`
- `crates/app/src/multiplayer/link.rs`
- `crates/app/src/multiplayer/text.rs`

### Set 2: 2 files, 5 tracked referrers

Referrers: `crates/app/src/egui_ui.rs`, `crates/app/src/egui_ui/front_end.rs`, `crates/app/src/egui_ui/settings.rs`, `crates/app/src/world/adventure.rs`, `crates/app/src/world/storage.rs`

- `crates/app/src/world/adventure/difficulty.rs`
- `crates/app/src/world/storage/difficulty_tests.rs`

### Set 3: 9 files, 4 tracked referrers

Referrers: `crates/app/src/headless.rs`, `crates/app/src/player.rs`, `crates/app/src/simulation.rs`, `crates/app/src/world/storage.rs`

- `crates/app/src/headless/session.rs`
- `crates/app/src/player/checkpoint.rs`
- `crates/app/src/player/movement.rs`
- `crates/app/src/simulation/tools.rs`
- `crates/app/src/world/storage/session.rs`
- `crates/app/src/world/storage/session/actors.rs`
- `crates/app/src/world/storage/session/format.rs`
- `crates/app/src/world/storage/session/worker.rs`
- `crates/app/src/world/storage/session/writer.rs`

### Set 4: 9 files, 2 tracked referrers

Referrers: `crates/app/src/simulation/cpu_region.rs`, `crates/app/src/simulation/cpu_region/backend.rs`

- `crates/app/src/simulation/cpu_region/backend/checkpoint.rs`
- `crates/app/src/simulation/cpu_region/backend/gpu.rs`
- `crates/app/src/simulation/cpu_region/checkpoint.rs`
- `crates/app/src/simulation/cpu_region/checkpoint/codec.rs`
- `crates/app/src/simulation/cpu_region/checkpoint/file.rs`
- `crates/app/src/simulation/cpu_region/checkpoint/ingress.rs`
- `crates/app/src/simulation/cpu_region/checkpoint/ownership.rs`
- `crates/app/src/simulation/cpu_region/ingress.rs`
- `crates/app/src/simulation/cpu_region/ownership.rs`

### Set 5: 2 files, 2 tracked referrers

Referrers: `crates/app/src/world.rs`, `crates/app/src/world/adventure/enemy_review.rs`

- `crates/app/src/world/adventure/enemy_review/furnishings.rs`
- `crates/app/src/world/fittings.rs`

### Set 6: 1 files, 2 tracked referrers

Referrers: `crates/app/src/egui_ui/orbital.rs`, `crates/app/src/world/orbital.rs`

- `crates/app/src/world/orbital/dynamics.rs`

### Set 7: 1 files, 2 tracked referrers

Referrers: `crates/app/src/egui_ui/recipe_book.rs`, `crates/app/src/world.rs`

- `crates/app/src/world/technology.rs`

### Set 8: 23 files, 2 tracked referrers

Referrers: `Cargo.toml`, `crates/app/Cargo.toml`

- `crates/lobby/Cargo.toml`
- `crates/lobby/src/directory.rs`
- `crates/lobby/src/lib.rs`
- `crates/lobby/src/main.rs`
- `crates/lobby/src/random.rs`
- `crates/lobby/src/server.rs`
- `crates/lobby/tests/relay_roundtrip.rs`
- `crates/net/Cargo.toml`
- `crates/net/README.md`
- `crates/net/build.rs`
- `crates/net/src/admin.rs`
- `crates/net/src/commands.rs`
- `crates/net/src/invite.rs`
- `crates/net/src/lib.rs`
- `crates/net/src/lobby/mod.rs`
- `crates/net/src/lobby/relay.rs`
- `crates/net/src/lobby/wire.rs`
- `crates/net/src/main.rs`
- `crates/net/src/protocol.rs`
- `crates/net/src/session.rs`
- `crates/net/src/social.rs`
- `crates/net/src/steam.rs`
- `crates/net/src/transfer.rs`

### Set 9: 1 files, 2 tracked referrers

Referrers: `crates/worldgen/src/sites.rs`, `crates/worldgen/src/undercrown/v2/mod.rs`

- `crates/worldgen/src/undercrown/v2/observatory.rs`

### Set 10: 4 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models.rs`

- `assets/models/factory/cable_arm.vox`
- `assets/models/factory/pipe_arm.vox`
- `assets/models/factory/pipe_hub.vox`
- `crates/app/src/mesh/models/voxel_asset.rs`

### Set 11: 2 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models.rs`

- `assets/models/machines/furnace.ron`
- `crates/app/src/mesh/models/machine_casing.rs`

### Set 12: 12 files, 1 tracked referrers

Referrers: `Cargo.toml`

- `assets/tools/materials.ron`
- `assets/tools/parts.ron`
- `assets/tools/traits.ron`
- `crates/tools/Cargo.toml`
- `crates/tools/src/assembly.rs`
- `crates/tools/src/codec.rs`
- `crates/tools/src/lib.rs`
- `crates/tools/src/materials.rs`
- `crates/tools/src/parts.rs`
- `crates/tools/src/progression.rs`
- `crates/tools/src/tests.rs`
- `crates/tools/src/traits.rs`

### Set 13: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/wand_checks.rs`

- `assets/viewmodel/hand.ron`

### Set 14: 1 files, 1 tracked referrers

Referrers: `crates/app/src/main.rs`

- `crates/app/src/ca_window.rs`

### Set 15: 1 files, 1 tracked referrers

Referrers: `crates/app/src/egui_ui/inventory.rs`

- `crates/app/src/egui_ui/inventory/screen.rs`

### Set 16: 1 files, 1 tracked referrers

Referrers: `crates/app/src/egui_ui.rs`

- `crates/app/src/egui_ui/online_browser.rs`

### Set 17: 1 files, 1 tracked referrers

Referrers: `crates/app/src/egui_ui.rs`

- `crates/app/src/egui_ui/semiconductor.rs`

### Set 18: 1 files, 1 tracked referrers

Referrers: `crates/app/src/headless.rs`

- `crates/app/src/headless/options.rs`

### Set 19: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh.rs`

- `crates/app/src/mesh/coarse_seams.rs`

### Set 20: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/ground_detail.rs`

- `crates/app/src/mesh/ground_detail/library.rs`

### Set 21: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models.rs`

- `crates/app/src/mesh/models/conduit.rs`

### Set 22: 2 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models.rs`

- `crates/app/src/mesh/models/network_player.rs`
- `crates/app/src/mesh/models/network_player/kit.rs`

### Set 23: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/orbital.rs`

- `crates/app/src/mesh/models/orbital/detail.rs`

### Set 24: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/orbital.rs`

- `crates/app/src/mesh/models/orbital/ground.rs`

### Set 25: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/orbital.rs`

- `crates/app/src/mesh/models/orbital/ground_monument.rs`

### Set 26: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/orbital.rs`

- `crates/app/src/mesh/models/orbital/palette.rs`

### Set 27: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models/orbital.rs`

- `crates/app/src/mesh/models/orbital/pressure.rs`

### Set 28: 1 files, 1 tracked referrers

Referrers: `crates/app/src/mesh/models.rs`

- `crates/app/src/mesh/models/wand_asset.rs`

### Set 29: 1 files, 1 tracked referrers

Referrers: `crates/app/src/quality.rs`

- `crates/app/src/quality/outdoor_tests.rs`

### Set 30: 1 files, 1 tracked referrers

Referrers: `crates/app/src/render/streaming/columns.rs`

- `crates/app/src/render/streaming/columns_tests.rs`

### Set 31: 1 files, 1 tracked referrers

Referrers: `crates/app/src/shader_sources.rs`

- `crates/app/src/shaders/architectural_surfaces.wgsl`

### Set 32: 1 files, 1 tracked referrers

Referrers: `crates/app/src/shader_sources.rs`

- `crates/app/src/shaders/furnishing_surfaces.wgsl`

### Set 33: 1 files, 1 tracked referrers

Referrers: `crates/app/src/simulation.rs`

- `crates/app/src/simulation/edit_tests.rs`

### Set 34: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure/engine.rs`

- `crates/app/src/world/adventure/engine/mechanisms.rs`

### Set 35: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure/engine.rs`

- `crates/app/src/world/adventure/engine/review.rs`

### Set 36: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure/engine.rs`

- `crates/app/src/world/adventure/engine/save.rs`

### Set 37: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure.rs`

- `crates/app/src/world/adventure/observatory.rs`

### Set 38: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure.rs`

- `crates/app/src/world/adventure/water_contact.rs`

### Set 39: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure.rs`

- `crates/app/src/world/adventure/water_transfer.rs`

### Set 40: 4 files, 1 tracked referrers

Referrers: `crates/app/src/world/adventure.rs`

- `crates/app/src/world/adventure/wildlife.rs`
- `crates/app/src/world/adventure/wildlife/model.rs`
- `crates/app/src/world/adventure/wildlife/persistence.rs`
- `crates/app/src/world/adventure/wildlife/tests.rs`

### Set 41: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world.rs`

- `crates/app/src/world/ca.rs`

### Set 42: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/effects.rs`

- `crates/app/src/world/effects/water.rs`

### Set 43: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/factory/chips.rs`

- `crates/app/src/world/factory/chips/orders.rs`

### Set 44: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/factory/chips.rs`

- `crates/app/src/world/factory/chips/production.rs`

### Set 45: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/factory.rs`

- `crates/app/src/world/factory/quarry.rs`

### Set 46: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/factory.rs`

- `crates/app/src/world/factory/sides.rs`

### Set 47: 1 files, 1 tracked referrers

Referrers: `crates/app/src/world/storage.rs`

- `crates/app/src/world/storage/wildlife_tests.rs`

### Set 48: 1 files, 1 tracked referrers

Referrers: `crates/content/src/lib.rs`

- `crates/content/src/fittings.rs`

### Set 49: 1 files, 1 tracked referrers

Referrers: `crates/sim_gpu/src/resident.rs`

- `crates/sim_gpu/src/resident_async.rs`

### Set 50: 1 files, 1 tracked referrers

Referrers: `crates/sim_gpu/src/resident.rs`

- `crates/sim_gpu/src/resident_edits.rs`

### Set 51: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/architecture.rs`

- `crates/worldgen/src/architecture/composition.rs`

### Set 52: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/architecture.rs`

- `crates/worldgen/src/architecture/plan.rs`

### Set 53: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/dungeon/v2/mod.rs`

- `crates/worldgen/src/dungeon/v2/arcades.rs`

### Set 54: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/dungeon/v2/mod.rs`

- `crates/worldgen/src/dungeon/v2/industry.rs`

### Set 55: 2 files, 1 tracked referrers

Referrers: `crates/worldgen/src/lib.rs`

- `crates/worldgen/src/meridian_terminal.rs`
- `crates/worldgen/src/meridian_terminal/monument.rs`

### Set 56: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/sites.rs`

- `crates/worldgen/src/sites/review.rs`

### Set 57: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/surface/routes.rs`

- `crates/worldgen/src/surface/routes/bridges.rs`

### Set 58: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/trees.rs`

- `crates/worldgen/src/trees/conifer.rs`

### Set 59: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/valley/settlement.rs`

- `crates/worldgen/src/valley/settlement/craft.rs`

### Set 60: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/valley/settlement.rs`

- `crates/worldgen/src/valley/settlement/facade.rs`

### Set 61: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/valley/settlement.rs`

- `crates/worldgen/src/valley/settlement/hall.rs`

### Set 62: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/valley/settlement.rs`

- `crates/worldgen/src/valley/settlement/workshop.rs`

### Set 63: 1 files, 1 tracked referrers

Referrers: `crates/worldgen/src/valley.rs`

- `crates/worldgen/src/valley/skyline.rs`

### Set 64: 2 files, 0 tracked referrers

Referrers: none (reached only from untracked files)

- `crates/app/examples/viewmodel_probe.rs`
- `crates/app/src/viewmodel/asset.rs`

## Runtime-only

Untracked assets named by a string literal; the build passes, the load fails.

- `assets/groundcover/library.ron`
- `assets/models/factory`
- `assets/tools`

## No referrer

Not reached from any code edge: generated, stale, documents, or work not yet wired.

- `assets/source`: 130
- `assets/art_direction`: 54
- `assets/viewmodel`: 24
- `tools/blender`: 22
- `docs/design`: 18
- `crates/worldgen`: 12
- `docs/adr`: 12
- `docs/NEW`: 9
- `assets/runtime`: 7
- `tools/blender_artificer`: 7
- `docs/plans`: 6
- `tools/pixel_kit`: 6
- `assets/groundcover`: 3
- `docs/superpowers`: 2
- `tools/blender_groundcover`: 2
- `ART_DIRECTION.md`: 1
- `assets/concepts`: 1
- `docs/Axiom_Magitech_Art_Production_Prompt_Pack_2026-10-26.md`: 1
- `docs/BLENDER_GUI_REVISION.md`: 1
- `docs/SHADER_AND_NETCODE_STUDY_20261026.md`: 1
- `docs/VIEWMODEL_ASSET_PIPELINE.md`: 1
- `docs/integration`: 1
- `tools/Play-Multiplayer.ps1`: 1
- `tools/Review-Architecture.ps1`: 1
- `tools/Review-Artificer.ps1`: 1
- `tools/Review-Furnishings.ps1`: 1
- `tools/Review-MobArt.ps1`: 1
- `tools/Review-OrbitalStructure.ps1`: 1
- `tools/Review-Ossuary.ps1`: 1
- `tools/author_artificer_sculpt.py`: 1
- `tools/gen_machine_models.py`: 1
