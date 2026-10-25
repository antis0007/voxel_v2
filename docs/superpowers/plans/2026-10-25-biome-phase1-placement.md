# Biome Overhaul Phase 1 (Placement) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** At generator 56, choose biomes from continuous family weights instead of the hard if-chain, blend surfaces across ecotones, and add the volcanic field and karst upland families.

**Architecture:** A data table (`assets/biomes.ron`, loaded by `engine_content`) holds one prototype per family in a field space (temperature, moisture, elevation, slope, wetness, substrate). A pure worldgen module computes per-column fields and softmax weights; `classify_landscape` takes the argmax as `Landscape::biome` and keeps the runner-up and its weight for blending. `column_from_landscape` dithers between the two families' surface tables by that weight, so resident and horizon terrain blend identically. Generators below 56 are untouched.

**Tech Stack:** Rust (workspace crates `engine_content`, `engine_worldgen`), RON via `ron` + `serde`, criterion bench, cargo nextest via `just test`.

**Spec:** [docs/superpowers/specs/2026-10-25-biome-overhaul-design.md](../specs/2026-10-25-biome-overhaul-design.md) §5, §6, §12, §13 phase 1.

## Global Constraints

- Generator gate: `pub const BIOME_WEIGHTS_GENERATOR: u32 = 56;` Every new behaviour is `cfg.generator_version >= BIOME_WEIGHTS_GENERATOR`. Never edit an existing version gate. Do not change `CURRENT_GENERATOR_VERSION` in the shared tree.
- Worldgen stays a pure function of `WorldSampleConfig` and coordinates; randomness only via positional hashing (`hash_u32`, value noise); no `rand`, no `Instant`.
- No `unwrap`/`expect` outside tests (asset load follows the `materials.ron` pattern: `unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"))`).
- Content tables live in `assets/*.ron`, never as Rust tables; add a round-trip test.
- Do not edit: `classify_landscape`'s carve block (lines that compute `h` from `drainage`/`lakes`), `column_from_landscape`'s drainage and lake shore block, `drainage.rs`, `lakes*`, `trees*`, any shader. Ping session voxel-v2-backup-04 before touching other `landscape.rs` code beyond the steps below.
- New files under 300 lines. Comments state why, not what. No banned words (seamless, robust, leverage, delve, elevate, comprehensive, crucial, ensure, streamline, powerful, elegant).
- New family labels exactly: `"Volcanic field"`, `"Karst upland"`.
- Perf: new `terrain_generate_8_chunks_v56` bench gets its own baseline; it must stay within 8 % of `terrain_generate_8_chunks` measured at generator 55 in the same session.

## Review Focus

- Columns where substrate gates exclude every family (e.g. carbonate rock in a climate no family fits) must still get a family: the weights fall back to the best ungated family, never NaN or an empty top. Test in Task 3.
- Chunk and LOD borders: the blend dither must depend only on world coordinates, so two chunks and the horizon agree at a border. Test in Task 5 across negative coordinates.
- Old worlds: generators 52-55 must classify exactly as before. Test in Task 4 compares labels at 55 against the unchanged if-chain over a negative and positive grid.
- Spawn disk, ocean and coastline rules keep their precedence at 56 (spawn forest/meadow, `h < 10.5` ocean, `shoreline_biome`). Test in Task 4.
- Extreme fields (temperature or moisture at 0.0 or 1.0, height above 300 m, slope above 2.0) produce finite weights summing to 1. Test in Task 3.

---

### Task 1: Biome table asset and loader

**Files:**
- Create: `assets/biomes.ron`
- Create: `crates/content/src/biomes.rs`
- Modify: `crates/content/src/lib.rs` (add `pub mod biomes;` beside the other content modules)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
  pub struct Axis { pub centre: f32, pub width: f32 }
  #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
  pub enum Substrate { Volcanic, Carbonate, Anomaly }
  #[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
  pub struct FamilyDef {
      pub name: String,            // Biome variant name, e.g. "Forest", "VolcanicField"
      pub temperature: Axis,
      pub moisture: Axis,
      pub elevation: Option<Axis>, // metres
      pub slope: Option<Axis>,
      pub wetness: Option<Axis>,   // 0..1, drainage/lake edge
      pub requires: Option<Substrate>,
      pub bias: f32,               // added to log-weight
  }
  #[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
  pub struct BiomeTable { pub families: Vec<FamilyDef> }
  pub fn biome_table() -> &'static BiomeTable;
  ```
- Families in the table: `Meadow, Forest, Taiga, Desert, Badlands, Tundra, Alpine, Savanna, Rainforest, Heathland, Fen, VolcanicField, KarstUpland`. Ocean, Coast and Marsh stay rule-placed (spawn, ocean, shoreline) and are not in the table.
- Starting prototypes (centre, width), tuned later from captures *(my call; derived from the current thresholds)*:

  | name | temperature | moisture | elevation | slope | wetness | requires | bias |
  |---|---|---|---|---|---|---|---|
  | Meadow | 0.46, 0.16 | 0.42, 0.14 | – | – | – | – | 0.0 |
  | Forest | 0.46, 0.16 | 0.70, 0.14 | – | – | – | – | 0.0 |
  | Taiga | 0.16, 0.10 | 0.62, 0.20 | – | – | – | – | 0.0 |
  | Tundra | 0.12, 0.10 | 0.25, 0.18 | – | – | – | – | 0.0 |
  | Alpine | 0.20, 0.30 | 0.50, 0.50 | 135, 30 | – | – | – | 0.0 |
  | Desert | 0.78, 0.12 | 0.18, 0.12 | – | 0.05, 0.15 | – | – | 0.0 |
  | Badlands | 0.74, 0.14 | 0.24, 0.14 | – | 0.45, 0.25 | – | – | 0.0 |
  | Savanna | 0.66, 0.10 | 0.40, 0.10 | – | – | – | – | 0.0 |
  | Rainforest | 0.70, 0.12 | 0.84, 0.12 | – | – | – | – | 0.0 |
  | Heathland | 0.34, 0.10 | 0.46, 0.12 | 48, 18 | – | – | – | 0.0 |
  | Fen | 0.46, 0.20 | 0.86, 0.10 | 20, 5 | 0.05, 0.08 | 0.8, 0.3 | – | 0.0 |
  | VolcanicField | 0.50, 0.50 | 0.40, 0.50 | – | – | – | Volcanic | 0.6 |
  | KarstUpland | 0.50, 0.25 | 0.62, 0.22 | 45, 30 | – | – | Carbonate | 0.4 |

- [ ] **Step 1: Write the failing tests** in `crates/content/src/biomes.rs` `#[cfg(test)] mod tests`:
  - `biomes_ron_round_trips`: `ron::to_string(biome_table())` parsed back equals `*biome_table()`.
  - `biome_table_has_every_placed_family_once`: names are exactly the 13 listed above, no duplicates; every `width > 0.0`.
- [ ] **Step 2: Run** `just test biomes_ron` → FAIL (module/asset missing).
- [ ] **Step 3: Implement** the types and `biome_table()` with `OnceLock` + `include_str!("../../../assets/biomes.ron")`, and write `assets/biomes.ron` with the table above.
- [ ] **Step 4: Run** `just test biome_table` and `just test biomes_ron` → PASS.
- [ ] **Step 5: Commit** `content: biome family table (assets/biomes.ron)`.

### Task 2: Two new families in `Biome`

**Files:**
- Modify: `crates/worldgen/src/landscape.rs` (enum `Biome`, `label()`, base surface table arms in `column_from_landscape`)
- Modify: any exhaustive `match` on `Biome` the compiler reports (`terrain_detail.rs`, `map_probe.rs`, `trees.rs`, `trees/forms.rs`, `trees/giant.rs`, `river_ice.rs`); for trees files add only the minimal arm and message voxel-v2-backup-a1 with the diff.

**Interfaces:**
- Produces: `Biome::VolcanicField` (label `"Volcanic field"`), `Biome::KarstUpland` (label `"Karst upland"`), `pub fn Biome::from_name(name: &str) -> Option<Biome>` (variant names as in the RON), `pub const Biome::PLACED: [Biome; 13]` in table order.
- Surface rules for the new arms *(my call)*: VolcanicField: surface `Basalt`, `Gravel` where the existing `patch` noise > 0.70, filler `Basalt`, vegetation × 0.15. KarstUpland: surface `Grass`, `Limestone` where `patch` > 0.58 (pavement), filler `Limestone`, vegetation × 0.6. Relief profile (terrain_detail): VolcanicField uses Badlands' profile, KarstUpland uses Heathland's.

- [ ] **Step 1: Write the failing test** `biome_names_round_trip` in landscape tests: for every `b` in `Biome::PLACED`, `Biome::from_name(&format!("{b:?}")) == Some(b)`; labels of the two new variants equal the exact strings.
- [ ] **Step 2: Run** `just test biome_names` → FAIL.
- [ ] **Step 3: Implement** the variants, `label`, `from_name`, `PLACED`, and the surface arms; fix every exhaustive match with the arms above (non-exhaustive matches need no change).
- [ ] **Step 4: Run** `just check` and `just test biome_names` → PASS. Run `just test gen_golden` → PASS (no generator output changes yet).
- [ ] **Step 5: Commit** `worldgen: volcanic field and karst upland families`.

### Task 3: Fields and weights module

**Files:**
- Create: `crates/worldgen/src/biome.rs` (register `mod biome;` in `crates/worldgen/src/lib.rs`)

**Interfaces:**
- Consumes: `engine_content::biomes::{biome_table, BiomeTable, Substrate}`, `Biome::from_name`, `TectonicSample` (`volcanism`, `continentalness`, `orogeny`).
- Produces:
  ```rust
  pub const BIOME_WEIGHTS_GENERATOR: u32 = 56;
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Fields { pub temperature: f32, pub moisture: f32, pub height: f32, pub slope: f32,
                      pub wetness: f32, pub volcanic: f32, pub carbonate: f32, pub anomaly: f32 }
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Weights { pub first: Biome, pub second: Biome, pub second_weight: f32 } // second_weight in [0, 0.5]
  pub fn substrate(tect: &TectonicSample) -> (f32, f32); // (volcanic, carbonate)
  pub fn weights(table: &BiomeTable, f: &Fields) -> Weights;
  ```
- `substrate`: `volcanic = smoothstep(0.40, 0.58, tect.volcanism)`; `carbonate = smoothstep(0.30, 0.10, tect.continentalness) * (1.0 - tect.orogeny.clamp(0.0, 1.0))` (mirrors the limestone rule in `sample_stratigraphic_bed`).
- `weights` algorithm (not determined by the signature):
  ```
  for each family: d2 = Σ over present axes ((value - centre) / width)²
  gate g = 1 for no requirement; else the matching Fields value (volcanic, carbonate, anomaly)
  score = exp(bias - d2) * g
  if every score is 0 (all gated and far): use exp(bias - d2) ignoring gates for ungated families only
  normalise; first = argmax; second = next; second_weight = s2 / (s1 + s2)
  ```
  Ties break by table order so results are deterministic.

- [ ] **Step 1: Write the failing tests** in `biome.rs`:
  - `prototype_centres_pick_their_family`: for each ungated family, `Fields` at its centres (elevation/slope/wetness at centre when present, else height 40, slope 0.1, wetness 0.2; gates 0) gives `first == that family`.
  - `substrate_gates_volcanic_and_karst`: Forest-centre climate with `volcanic = 1.0` → `first == VolcanicField`; with `volcanic = 0.0` → not VolcanicField; same for carbonate/KarstUpland at its centre.
  - `weights_are_finite_and_ordered_at_extremes`: grid over temperature and moisture in {0, 0.5, 1}, height {−5, 40, 320}, slope {0, 2.5}, gates {0, 1}: `second_weight` finite, in `[0.0, 0.5]`, and `first != second`.
  - `weights_change_continuously`: stepping moisture 0.30→0.90 by 0.001 at temperature 0.46: whenever `first` is unchanged between steps, `second_weight` changes by < 0.02.
- [ ] **Step 2: Run** `just test biome::` → FAIL.
- [ ] **Step 3: Implement** `substrate` and `weights`.
- [ ] **Step 4: Run** `just test biome::` → PASS.
- [ ] **Step 5: Commit** `worldgen: biome fields and family weights (generator 56, dormant)`.

### Task 4: Weighted classification in `classify_landscape`

**Files:**
- Modify: `crates/worldgen/src/landscape.rs` (`Landscape` struct; the biome selection in `classify_landscape` after the carve block)

**Interfaces:**
- Consumes: Task 3 `Fields`, `weights`, `substrate`, `BIOME_WEIGHTS_GENERATOR`.
- Produces: `Landscape { .., pub secondary: Biome, pub blend: f32 }` where `blend = second_weight` at generator ≥ 56 and `secondary = biome, blend = 0.0` below 56.
- At ≥ 56 keep the existing precedence: spawn disk → `h < 10.5` Ocean → `shoreline_biome` result. After those, call `weights` with: temperature/moisture/height/slope already in scope, `wetness = drainage.bank.max(lake edge wetness)` where lake edge wetness is `smoothstep(12.0, 0.0, lake.edge_m)` when `lakes::sample` reports a lake, else 0 (read-only use of session 04's API), `anomaly = a.strength * (a.weights[1] + a.weights[3])` from `terrain_variety::anomaly` (same expression as today), substrate from `sample_tectonics(wx, wz, cfg.seed, cfg.generator_version)`.

- [ ] **Step 1: Write the failing tests** in landscape tests:
  - `generator_55_classification_is_unchanged`: before editing, add a private copy of today's if-chain as `fn legacy_biome_for_test(...)` in the test module; then over x, z in −2048..2048 step 128 at generator 55, `sample(cfg,x,z).biome` equals it and `blend == 0.0`.
  - `generator_56_keeps_spawn_ocean_and_shore_rules`: at 56, origin column is Forest or Meadow as at 55; a column with `h < 10.5` is Ocean; columns where `shoreline_biome` returns Some keep that biome.
  - `generator_56_places_new_families_somewhere`: seed 2738958700, hills, grid ±6144 m step 96: at least one `VolcanicField` and the ratio of columns with `blend > 0.25` is between 0.05 and 0.6.
- [ ] **Step 2: Run** `just test generator_5` → the 56 tests FAIL, the 55 test PASSES.
- [ ] **Step 3: Implement** the struct fields and the gated selection.
- [ ] **Step 4: Run** `just test generator_5` and `just test gen_golden` → PASS.
- [ ] **Step 5: Commit** `worldgen: weighted biome classification at generator 56`.

### Task 5: Ecotone surface blending

**Files:**
- Modify: `crates/worldgen/src/landscape.rs` (`column_from_landscape`: the base surface-table selection only)

**Interfaces:**
- Consumes: `Landscape::{secondary, blend}`.
- At ≥ 56, before the base table lookup: `let family = if hash01(x_cell, z_cell, cfg.seed ^ 0x6563_6f74) < landscape.blend { landscape.secondary } else { landscape.biome };` with `x_cell, z_cell` = world metres floored to whole metres (scale-independent), then use `family` for the base surface/filler/vegetation table. Later overrides (slope rock, snow, tectonic, anomaly, drainage, lake shore, stands) are unchanged and still read `landscape.biome` where they do today.

- [ ] **Step 1: Write the failing tests**:
  - `ecotone_surface_mix_follows_blend`: at 56 find 200 columns with `blend` in each bin [0, 0.1), [0.2, 0.3), [0.4, 0.5]; the share whose surface comes from `secondary`'s table rises bin to bin (≈0, ≈0.25, ≈0.45 within ±0.1).
  - `blend_dither_agrees_across_chunk_and_lod`: for x in −40..40 (crossing 0 and −32), the resident column surface at 56 equals the horizon `preview_surface_canonical` surface for the same column whenever both compute the same `Landscape` (extend the existing `horizon_agreement` pattern).
  - `generator_55_columns_unchanged`: surfaces over a −512..512 step 16 grid at 55 equal the values recorded by running the same loop before this task (store as a hash constant computed in Step 0).
- [ ] **Step 0: Record** the generator-55 surface hash with the current code and paste it into the test.
- [ ] **Step 2: Run** → the 56 tests FAIL.
- [ ] **Step 3: Implement** the dithered family choice.
- [ ] **Step 4: Run** `just test ecotone`, `just test blend_dither`, `just test generator_55`, `just test horizon_agreement` → PASS.
- [ ] **Step 5: Commit** `worldgen: dithered ecotone surfaces at generator 56`.

### Task 6: Variety test, bench and captures

**Files:**
- Modify: `crates/worldgen/src/biome.rs` tests (move them to `crates/worldgen/src/biome/tests.rs` if the file would pass 300 lines).
- Modify: `crates/worldgen/benches/terrain.rs`, `crates/worldgen/benches/baselines.json`
- Modify: `docs/superpowers/specs/2026-10-25-biome-overhaul-design.md` (phase table: landmark forms move to phase 2)

**Interfaces:**
- Consumes: Tasks 3–5.

- [ ] **Step 1: Write the failing test** `variety_every_16_km2`: for seeds 1..=20, hills, generator 56, sample `landscape::sample` on a 64 m grid over the 4 km × 4 km square centred at (3000, −3000): at least 4 distinct biome labels and at least 20 columns with `blend > 0.3`.
- [ ] **Step 2: Run** `just test variety_every` → expect PASS if tuning is right; if it fails, adjust widths in `assets/biomes.ron` (not code) until it passes, and record the change in the commit message.
- [ ] **Step 3: Add bench** `terrain_generate_8_chunks_v56` (same body as the existing bench with `generator_version: 56`), run `just bench terrain_generate_8_chunks` interleaved with the new one three times, then `just bench-save terrain_generate_8_chunks_v56`. Accept only if the v56 median is within 8 % of the v55 median measured the same way.
- [ ] **Step 4: Captures** in a scratch worktree (never the shared tree) with `CURRENT_GENERATOR_VERSION = 56`: `tools/Review-VisualDirection.ps1 -Case daylight -ExtraArgs '--benchmark-generator-version=55'` and `=56` at poses `576,1150,150,3.14,-0.9` (alpine/volcanic), `0,0,160,0,-1.2` (spawn overview), and one ecotone pose found with `--terrain-survey`. Labels `biome-p1-v55-*` / `biome-p1-v56-*`. Inspect: smooth transitions, no colour rings, volcanic field reads as a region not a stain.
- [ ] **Step 5: Run** `just clippy` (scoped to worldgen/content if peers break the workspace) and `just test` for `engine_worldgen` and `engine_content`.
- [ ] **Step 6: Commit** `worldgen: biome variety test and v56 bench baseline` including the design phase-table edit.
