# MC-RS inspiration and parity audit

## Reference and method

`MC-RS` is a packaged Minecraft 1.6.4 Rust port, build 0.6.18. It contains executables and game assets, not Rust source. This comparison uses its readme, server scripts, documented flight recorder and server tools, and established 1.6.4 game behavior. No assets or code were copied.

The useful inspiration is interaction clarity: a held action responds on the next frame, block damage has staged surface feedback, entities have readable motion, and a dedicated host owns shared state. Axiom should keep its own materials, magic, machinery, world generation, interface and bestiary.

## Current parity

| Area | MC-RS reference | Current project | Assessment and next gap |
|---|---|---|---|
| Shared play | Dedicated server, operators, whitelist, profiles, console, backups and flight recorder | Host-owned survival, persistent player identities, relay invites, social UI, per-peer pacing, chunk and cell replication, periodic saves | Strong original base. Add multi-client survival soak evidence and acknowledged edit prediction. The generic edit predictor is not connected to session writes. |
| Movement | Server-owned vanilla movement with established packet behavior | Joiners step their own bodies at 120 Hz; the host validates poses and sends corrections | Local motion does not wait for a round trip. Keep testing poor links and several clients, not only direct movement unit tests. |
| Block breaking | Hold to break, visible crack stages, strike and break sounds, material drops | Material hardness, tool classes and tiers, harvest rules, progress HUD, dig audio, debris and bounded edit queues | Single-player rules were present. Joiners lacked the local presentation path. This slice adds the same cues in network play. Actual block writes remain host-owned. |
| Rendering | Fixed vanilla 1.6.4 presentation | wgpu HDR scene, atmosphere, clouds and shadows, water optics, material finishes, far terrain, authored viewmodels and post-processing | No reason to imitate the reference renderer. Spend effort on captures, frame-time checks and readability during combat and mining. |
| Overlays | Selection box, cracks, hotbar, health, inventory and debug display | Diegetic prompts, tool telemetry, break progress, inventory and spell interfaces, selection and placement overlays | Network mining now has selection, cracks and tier warnings. Remote player action poses are a separate follow-up. |
| Animation | Vanilla held-item swings and entity limb motion | Spring viewmodels, gait and equipment poses, articulated mob parts, boss tells, impact effects | Held items now repeat a mining strike locally in single-player and shared play. Replicate semantic remote actions rather than transmitting every rendered limb. |
| Mobs | Familiar hostile and passive 1.6.4 species | Original data-driven species, wildlife and husbandry, tactical roles, affixes, encounters, habitats and bosses | Feature depth is already different rather than smaller. Prioritize shared-world AI ownership, reproduction and combat soak tests over adding lookalike species. |
| Blocks | Vanilla terrain, ores, plants, utility blocks and redstone | Original terrain and ores, fluids and gases, doors and trapdoors, fittings, furniture, machines, pipes and factory blocks | Functional depth is strong. Decorative slabs, stairs, rails and more building variants are content batches, not engine rewrites. |
| Combat | Melee, bows, armor, hunger, potions and hostile mobs | Modular melee combos, spells and projectiles, statuses, armor, hunger, death, boss tells and terrain interactions | Keep the original combat. Next feel gap is immediate local action presentation with host-confirmed damage and clear remote windups. |

## Mining slice landed

- `mesh/mining.rs` generates eight deterministic crack stages on all six faces. It uses the existing inverse line pass and adds no texture or render pipeline.
- `multiplayer/mining.rs` reads host mining progress and advances presentation-only progress for a joiner with the same hardness and tool table as authority.
- The network frame path now draws target selection, crack stages, progress and tool-tier warnings, and plays material dig strikes. It does not mutate cells, inventory or drops.
- Joiners step their viewmodel locally. Delta images no longer overwrite that presentation state.
- Held blocks, tools and items have a repeated mining swing. Reduced camera motion also scales this movement.

## Remaining order

1. Connect cell prediction to explicit host acknowledgements, including refusal restoration and repeated edits to one cell. Do not treat presentation progress as permission to write.
2. Run several real clients through mining, placement, combat, death, inventory and reconnect on delayed and jittered links. Record bandwidth, correction counts and worst snapshot age.
3. Add compact remote action states for mining, melee, casting and interaction, then interpolate those poses alongside remote bodies.
4. Capture mining, combat and mob scenes before and after visual changes. Run frame-time checks on the target GPU.
5. Add decorative building variants through `BlockId` and `assets/materials.ron`, with placement, collision, meshing, saving and multiplayer coverage in the same batch.

## Validation

- `cargo check -p minecraft_style_voxel_prototype --message-format=short` passed. Receipt: `target/mcrs-mining-check.exit`.
- The mining test filter passed 16 tests with one ignored. This covers crack staging, invalid progress, translation, viewmodel motion and held-item topology. Receipt: `target/mcrs-mining-tests.log`.
- All six multiplayer replication tests passed, including preservation of local viewmodel motion across image updates. Receipt: `target/mcrs-replication-tests.log`.
- `git diff --check` passed for the files in this slice.

No visual approval or live multi-client soak is claimed. The initial check encountered audio errors from concurrent work; the final application check passed after those call sites changed elsewhere in the shared tree.

## UI quality follow-up (2026-10-28)

Reference inspected locally: `MC-RS/README.txt` and the packaged `widgets.png`
and `container/inventory.png` in `vendor-private/1.6.4/client.jar`. The assets
show a low, square-slot hotbar, a distinct selection frame, recessed container
slots, and a separated inventory hotbar row. Reference images are inspection
artifacts under ignored `target/ui-reference`; player assets remain original.

| Area | Current implementation | Remaining evidence or gap |
|---|---|---|
| HUD proportions | Larger 26-point slots, 22-point icons, square slot bounds; small corner shortcut labels replace the extra keycap row | In-game review at windowed/fullscreen sizes |
| Counts | Shared inventory/HUD count painter, four-digit width reservation, inset shadow, automatic fitting for longer counts | Review 9999 with each selectable number font and DPI setting |
| Scaling | Hotbar icons and counts follow the independent slot scale; health, hunger and armor can grow with available width | Check min/max hotbar settings with breath and wand rows |
| Selection | Stable slot geometry and original gold selection border; hover/focus accents retained | Review contrast over bright and dark terrain |
| Tooltips | Hotbar shares inventory item details, stack limit, owned quantity, armor comparison and Shift expansion | Pointer interaction review with inventory open |
| Inventory actions | Existing split/place, quick transfer, drag distribution, double-click collection and number-key swaps are present in source | Interaction parity is not established by asset inspection |
| Vitals layout | Explicit health/hunger gap accounts for egui spacing; hidden respawn details no longer reserve an empty row | Live HUD capture still required |

Local offline application `cargo check` passed after this UI slice. No tests,
runtime comparison, full feature parity or visual acceptance are claimed.

## Creative mode follow-up

Creative immunity now follows the authoritative game mode while grounded as well as flying. Environmental and combat damage are suppressed, hunger does not drain, and breath stays full. The host rejects client-reported environmental damage for creative actors; survival environmental damage uses the shared difficulty and cause handling. The creative HUD hides health, hunger, armor and breath while keeping the hotbar and contextual wand display.

The integrated executable is rebuilt through `tools/Play-Integrated.ps1`'s build command. Build receipt: `target/creative-immunity-build.log`. No new tests or live gameplay/visual acceptance are claimed for this slice.

Creative flight now defaults to 24 m/s with a 3x sprint boost. The creative panel exposes 4–64 m/s speed, 1–4x boost, camera-directed travel, landing on descent, and a direct flight toggle. Alt applies 20% precision speed; Space and Shift retain vertical control. Flight toggles clear velocity and accumulated fall speed. Settings travel with player checkpoints; the legacy single-player save format does not yet persist these new flight options.
