# Crystal finishes and tech progression

Crystal facets now have fixed, pixel-aligned normals rather than time-wobbling
normals. The normal and colour use the same shard field; unresolved facets
increase roughness. Sun and local lights provide the highlights through the
shared BRDF. The remaining inner rim and resonance ribbon are restrained;
resonance speed follows the magic motion setting. The duplicate emissive glints
and extra sharp sun lobe are removed.

Copper, tin and iron inclusions share one mask between colour, roughness and
metallicity. Host rock stays rough, coal stays non-metallic, and unresolved
inclusions converge to average coverage. No time-driven ore flashes are added.
Mithril retains its cyan/pearl ribbons without a blanket emissive wash. Star
glass keeps its stars with a quieter body glow and one-texel tile joints.

## Playable route

1. Mine copper, tin, iron, coal and crystal; use the field smelter to bootstrap.
2. Salvage ancient circuits to build the first bronze-and-iron fabricator.
3. Power its new Circuits mode: 2 copper ingots + 1 tin ingot + 1 crystal
   produces 2 circuits in 12 seconds at 16 kW (192 kJ per batch).
4. Manufacture coils and grow biomass. Essence mode consumes 1 crystal +
   2 biomass in 12 seconds at 24 kW. Core mode consumes crystal, circuit and
   essence in 7 seconds at 14 kW. These feed the existing advanced machines,
   battery upgrades, arcane power and equipment economy.
5. Build with re-formed crystal (1 resource to 1 block), copper cladding
   (4 ingots), essence-infused star glass (2 crystal + 4 sand + 1 essence
   to 4 blocks), or mithril (2 steel + 2 crystal + 1 essence + 1 core).

The existing basalt star-glass and mirage recipes remain available. All new
grid recipes appear in the shared recipe browser. Fabricator mode labels,
input routing and save output validation include manufactured circuits.

No resource, block or machine identifiers or serialized layouts were added.
New grid recipes follow existing machine-kit recipes, preserving saved
assembler target indices. New decorative block recipes are manually crafted;
the assembler continues to support resources and machine kits only.

## Native review

Set `VOXEL_TORCH_REVIEW_MODE=minerals`, `VOXEL_REVIEW_INPUT_ISOLATED=1` and
`VOXEL_CAPTURE_DIR` to an empty evidence folder, then launch the rebuilt binary
with `--benchmark-torch-review --benchmark-world=hills
--benchmark-capture-frames --benchmark-warmup=4 --benchmark-duration=28
--benchmark-view-chunks=4 --benchmark-width=1280 --benchmark-height=720`.
The unsaved fixture compares coal, iron, copper ore, tin ore, resonant crystal,
copper cladding, mithril and star glass from left to right. Its first twelve
seconds are daylight; later frames show local torch lighting at night.

Build and native capture outcomes are recorded with the review artifacts.
Captures establish presentation only; normal survival pacing and player-led
factory construction still require play review. No tests or hosted CI run.

### This implementation's validation

The final offline release-fast attempt passed validation of 16 composed WGSL
programs (`world_fnv1a64=36e0562c55e540bf`) and emitted Rust compilation warnings,
but could not replace `target/release-fast/minecraft_style_voxel_prototype.exe`
(`Access is denied`, OS error 5). This is not a successful final build. The
earlier completed build preceded the fixed-facet-normal and comparison fixture
changes, so its executable is not final evidence. No process was terminated or
executable moved to clear the lock. A separate coordinator requested frozen
sources and exclusive compiler/native slots during its coherent build review;
no further compilation or native launch was started here. The material fixture
has not been viewed. The optional local advisor was unavailable (WinError 10013).

Files changed by this increment: `src/shaders/world_shader.wgsl`,
`src/world/factory/recipes.rs`, `src/world/factory.rs`,
`src/world/inventory.rs`, `src/world/recipe_index.rs`,
`src/egui_ui/factory.rs`, `src/torch_review.rs`, `src/main.rs`, and this document.
Existing changes from other sessions in these files were preserved. Preferences
changes and save-version changes belong to other sessions. This increment
adds no save format or version changes. Live referenced sources are now frozen
for the coordinator's review; `reviews/crystal-tech-source.json` records hashes.
