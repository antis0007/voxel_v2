# Recovered feature integration

Final application check after the compact inventory navigation and concurrent
reliquary access repair: **passed, exit code 0**, default multiplayer features
enabled. Receipt: `target/recovery-integration/reliquary-receipt.log`.
The reliquary repair adds a read-only focused-distance query; interaction code
no longer reaches into its private collection.

The five candidates in `source-completeness.md` were authorized for integration.
Their source modules were adapted individually; no old branch was merged wholesale.

| Feature | Current integration |
| --- | --- |
| Player records | Persistent statistics and advancement IDs, mining/placement/crafting/combat/movement hooks, Statistics inventory tab, unlock notifications, clock and map items/recipes. Scanner crafting also counts. |
| Starter guide | Goals inventory tab, current-recipe ingredient queries, placement/food/sleep/night/machine/gate signals, persistent completed IDs. HUD tracker defaults off and has an optional unbound remappable action. Existing quest tracking remains independent. |
| Biomes and sites | Validated RON family parameters, stable named families, weighted geology/climate selection beneath current habitat variants, biome-aware site lottery with existing numeric identities. |
| Districts and roads | Cached bucket-grid district sampling, port/office/tower/hull geometry, industrial materials, regional graded roads/bridges, vegetation clearance, tower residency bounds, machine encounters and rewards. |
| Chest tiers | Existing Chest is the wood tier; copper/iron/steel append new IDs. Capacities 27/36/45/54. Block data remains canonical; scanner policy, aggregation, crafting and transactions support mixed capacities. Hinged lids use transient presentation state. |

World generation uses fresh gate 144. Saved versions through 143 retain their
stored generation behavior. New worlds also activate the already-authored
128–143 features; existing saves are not migrated to the new generator.

Chest sidecar version 4 stores each chest's capacity. Versions 1–3 still read
exactly 27 slots. Cache/machine slots remain fixed at 27. Presentation state is
not part of the save. Player records and goals have their own checksummed
sidecars and are included in background checkpoints and imports.

No tests were added or run, and no hosted CI was used. Compilation and native
runtime acceptance are separate: shader validation/type checking do not establish
in-game visuals, fluid realism, frame rate, save/reload behavior or multiplayer feel.

Initial isolated offline application check passed in 11m34s. A follow-up check passed in 58.48s and validated all 27 composed WGSL programs (world FNV-1a 411e8ff5fd6c27c7). The final check includes later admission, tooltip and diagnostic edits; see the validation receipt below.

Command: cargo check --offline --target-dir target/recovery-integration/build -p minecraft_style_voxel_prototype -j 2 --message-format=short. The default feature set includes multiplayer. Logs: target/recovery-integration/. No native game run has been performed for these five integrations.

Final source receipt: exit code 0 in 57.85s after all Rust edits, including the bounded biome report. The reviewed recovery modules also passed git diff --check. Existing unused-code and incremental-cache hard-link warnings remain. UTF-8 review restored recipe and documentation punctuation after recovery.
