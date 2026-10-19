# Visual style update — 2026-10-18

## Gameplay HUD

- The vitals group is centered over the **measured** hotbar rectangle. It follows hotbar size and display scale instead of using an unrelated bottom offset. Target feedback is contextual near the reticle; notifications reserve space around the status card. `src/egui_ui.rs`: `hotbar_overlay`, `adventure::hud`, `target_name`; `src/egui_ui/front_end.rs`: `journey_hud`.
- Health values lead the header; the rank label and redundant heart have been removed. Meter divisions mean 20 actual points, including maximum-health upgrades. Health and mana both have names and current/maximum values. `src/egui_ui/adventure.rs`: `resource`.
- Damage leaves a brief amber trail before the missing health recedes. Low and critical health have written labels as well as distinct colors. Incoming melee/projectile direction and amount now drive a directional marker and localized edge wash. `src/egui_ui/adventure.rs`: `HealthFeedback`, `combat_feedback`; `src/world/adventure.rs`: `damage`, melee/projectile impact paths.
- Spell icons form a centered group. R and T use keycaps, the cast instruction states that R is held, and longer cooldowns display seconds. The misleading E-to-wand-recipe hint is absent. Width adapts to the viewport; short windows use a compact resource strip and smaller hotbar slots, with the persistent item-name row hidden to keep the aiming reticle clear. `src/egui_ui/adventure.rs`: `hud`; `src/egui_ui.rs`: `draw_hotbar_slot_button`, `hotbar_overlay`.
- Shared ink/paper/jade/amber colors, borders, text, and selected-slot styling now cover the HUD and inventory slots. `src/egui_ui/presentation.rs`, `src/egui_ui/icons.rs`, `draw_hotbar_slot_button` in `src/egui_ui.rs`.

## Trees and surfaces

- New worlds use generator 8. Broadleaf trees have tapered leaning trunks, root flare, connected rising limbs, and distinct individual crown forms. Conifers have varied whorls and leafy upper tips; the wood ends below the apex. Generator 8 retains generator 7's climate and snowline improvements; saved versions 1–7 retain their generation rules. `src/world/trees.rs`, `src/world/landscape.rs`, version dispatch in `src/world.rs`, accepted versions in `src/world/storage.rs`.
- Foliage palette integration was coordinated with the active terrain owner: green, ochre, copper, and blue-green grove colors remain. Crown loading includes neighboring slopes. `src/shaders/world_shader.wgsl`: foliage surface family; `src/render/streaming/visibility.rs`: tree bounds.
- The shared palette uses lichen and sage for living ground, cool stone and peat for terrain, graphite and worn timber for machines, copper/ember for heat, and patinated teal for fluid and power. Purple remains a rare arcane mineral accent. `src/material.rs`; `src/shaders/world_shader.wgsl`.
- Alternating per-block tint parity is gone. The material model supplies stable base color, and continuous world-space fields own the variation. `src/material.rs`: `ColorModel::sample`.
- Close surfaces share a 16-texel-per-metre painting grid. Natural stone uses broken bedding instead of faux brick rectangles, leaves use quiet directional facets, and ores use irregular deposits instead of repeated diamonds. Fine detail fades with its pixel footprint. `src/shaders/world_shader.wgsl`: `block_surface`.
- Inventory block illustrations now use material-specific 8×8 motifs. The torch has a flame/handle illustration instead of a brown cube. Sprite placement and highlights align to native pixels and actual sprite dimensions. `src/egui_ui.rs`: `draw_block_icon_face`, `draw_voxel_icon`; `src/egui_ui/icons.rs`: `paint`, `Icon::Torch`.

## Blur and compositing

- The antialiasing pass reconstructs actual edges from adjacent pixels; it no longer spreads detail along a six-pixel directional span. Tone mapping is retained. `src/shaders/postprocess.wgsl`: `fs_main`.
- UI tessellation and rendering now use the same egui scale. The stale independent window-scale cache has been removed. `src/egui_ui.rs`: `end_frame`.
- The world still renders through the sRGB scene output. Egui blends through a compatible non-sRGB texture view, matching its gamma-space premultiplied colors. This removes the framebuffer-format warning and prevents terrain from washing through dark HUD panels. Bootstrap loading, regular loading, and gameplay use the same path. `src/render.rs`: surface `view_formats` and three overlay calls; `src/egui_ui.rs`: `Renderer::new`.

## Evidence and limits

- Local `cargo check --offline --target-dir target` passes, including the build script's WGSL validation. No tests or hosted CI were added or run.
- A separate executable was linked and captured at native resolution. The existing running debug executable could not be replaced because Windows holds it open; it was left running.
- `target/hud-review/final/target/cloud-captures/hills-detail/None/Hills-02-Clear-noon-3.png` shows the centered health HUD, corrected opacity, woodland palettes, leafy conifer tips, and torch illustration at 1600×900.
- The first 800×450 preview exposed the HUD reaching the aiming reticle. Its corrected rerun is in `target/hud-review/compact-fixed`. Capture arguments, exit codes, and executable hashes are recorded beside each run in `verification.json`.
- The final candidate was recaptured at 800×450 under `target/hud-review/candidate`; it exits successfully, keeps the aiming reticle clear, and has SHA-256 `98E26FD585A63B764D9F58D45464F04735B6528AEE8C2AE4C6F98033315E05AB`. Compact ward status stays inside the mana header so it cannot add another tall row.
- The candidate executable is `target/hud-review/visual-refresh.exe`; `Launch-Visual-Refresh.ps1` starts it with the project folder as working directory so assets and saved worlds resolve normally. `final-build-hash.json` identifies the latest artifact. The running game still uses its older executable until restarted.
- The terrain owner's woodland/alpine captures are under `target/terrain-review/v3`. The woodland run reached zero pending terrain work.
- Rendered captures show full health and ordinary inventory. Low-health, incoming-hit, mana, and spell transitions are implemented and compiled but have not been exercised through player input in these captures. No performance acceptance is inferred while another game instance is running.
# Pixel UI and cloud frame-time pass (2026-10-18)

- Replaced the wide green health meter with pixel hearts, partial-heart health, the amber damage trail, compact mana, and brief slot-name callouts. Removed the permanent name row, parent hotbar card, and blurry damage vignette. The selected slot now uses the same inset stone bevel as the item and spell slots.
- Installed the Tiny5 bitmap-style font under its OFL 1.1 license (`assets/fonts/`), snap-scaled egui to whole physical pixels, and set the font atlas to nearest filtering. Menus use parchment and stone colors with a block stepped landscape preview and a small voxel sunset.
- Added High (2x), Balanced (3x), and Fast (4x) cloud render scales to Graphics settings. Balanced is the default at 1440p; it keeps full terrain and UI resolution. The cloud sampler uses hardware bilinear filtering when `FLOAT32_FILTERABLE` is available; other adapters retain the four-tap float32 fallback. Low-resolution windows automatically cap the scale so cloud renders remain at least 360 pixels tall.
- Optimized far cloud-shadow sampling so the coarse shadow clipmap is sampled only where it contributes. The high-quality cloud scale preserves the former 2x resolution.

RTX 4090 / Vulkan, 2560x1440, Hills, fixed camera and time, no streaming work during measurement:

| Weather | Earlier default | Balanced default | GPU cloud raymarch |
|---|---:|---:|---:|
| Partly cloudy | 73.79 FPS, 12.876 ms GPU | 119.20 FPS, 7.774 ms GPU | 11.764 → 6.766 ms |
| Storm | 41.98 FPS, 22.867 ms GPU | 82.46 FPS, 11.517 ms GPU | 21.468 → 10.291 ms |

Frozen executables, full command lines, hashes, and raw output are preserved under `target/pixel-style-review/clean-before-*` and `clean-after-*`. The baseline and candidate were run serially. These are measurements of fixed-camera renderer performance on one desktop GPU; they do not predict every GPU or moving camera. The final menu-only art pass was made after this measurement and does not change render code.

