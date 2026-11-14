# Mana, magitech and world-magic design

Date: 2026-11-12. Status: proposed, owner review needed. No source changed.
All proper nouns below are placeholders until approved (UI_RULES.md).

## 0. Problem

Magic pieces exist but do not share one model:

| Piece | Where | Gap |
|---|---|---|
| Natural mana pressure + recovery | `world/ambience.rs` | Location profile only; nothing to build on |
| Blight concentration/exposure | `adventure/affinity.rs`, magic rework brief §1 | No source objects, no purifier machine |
| Enchanting | `adventure/enchanting.rs` | Two bindings and costs hard-coded in Rust (violates "no content tables in Rust") |
| XP | `world.progression.xp` | One sink (enchanting) |
| Reliquaries | `adventure/reliquaries.rs` | Discovery shrines, not equipment |
| Resonance boons | `adventure/resonance.rs` | Run-scoped, unrelated to world mana |
| Factory power | automation skill / machines.ron | No mana carrier, no conversion |

Goal: one essence model, one field, one carrier, one conversion table. Every
feature below reads those; none keeps its own copy.

## 1. Single sources of truth

| Data | File (new unless noted) | Owner type |
|---|---|---|
| Essence kinds (id, colour family, material ids, sound, opposed kind) | `assets/essences.ron` | `engine_content::essences` |
| Node archetypes (shape, size, essence, yield, rarity per biome/realm) | `assets/mana_nodes.ron` | worldgen placer + runtime |
| Multiblock patterns (collector, converter, farms, altar) | `assets/multiblocks.ron` | one pattern matcher |
| Conversion recipes (mana ↔ power ↔ items, losses) | `assets/recipes.ron` (existing) new `Transmute` kind | recipe index |
| Enchantments + XP costs | `assets/enchantments.ron` | replaces `BINDINGS` const |
| Artifacts (slot, effect, essence upkeep) | `assets/artifacts.ron` | equipment system |
| Hell flora/fauna | `assets/flora.ron`, `bestiary.ron` (existing) realm tag | existing tables |

Rust holds behaviour and validation only. Content loads at startup, validated
once (ids unique, referenced essences exist, patterns closed), errors name file
and entry.

## 2. Essences and the mana field

- 6 essence kinds to start (placeholder: Ember, Tide, Verdant, Stone, Gale,
  Void). Void = hell/blight side; its opposed pair drives blight interaction.
- Field: sparse per-region scalar per essence, quantised u8, stored at 16 m
  cells. Computed from placed nodes + biome base; never per-voxel, never per
  frame. Recomputed on node place/break/drain only (dirty-cell list).
- `ambience.rs` natural density becomes `sum(field)` read, so existing recovery
  curves keep working; `affinity.rs` blight concentration = Void field minus
  purification. One field, two readers.
- Simulation: fixed tick, positional seeded hash for regrowth. No wall clock.

## 3. Mana nodes (crystal hedrons)

- Shapes authored as voxel stamps in `mana_nodes.ron`: octahedron, elongated
  bipyramid, cluster of 3. Integer coordinates, vertical axis = local up,
  grid-aligned (no rotation other than 90° about up). Heights 3–9 voxels.
- Built from per-essence crystal BlockIds (one new block per essence, shared
  emissive material slot, colour from `essences.ron`). Ordinary chunk-mesh
  voxels: no new draw path, no per-node draw call.
- Placement: worldgen placer under a new dormant generator gate (grep all
  branches for free gate per `generator-gate-collisions`). Rarity by biome/realm,
  min spacing via positional hash. Existing worlds: nodes only in newly
  generated chunks.
- Node state (charge 0..cap, regrowth) keyed by anchor position in a region
  sidecar. Drained nodes dim (material variant), regrow on fixed ticks.
- Breaking a node yields shards (essence item) and destroys its field
  contribution — tension: harvest once vs. tap forever.

## 4. Capture multiblock (resonant collector)

- Pattern in `multiblocks.ron`: frame of worked stone + focus block placed
  within R of a node, line to node unobstructed (voxel DDA once on validate).
- Validation on edit events touching registered cells only (register supporting
  cells, per brief §6); never flood-fill per frame.
- Output: essence mana into a buffer block; rate = node yield × alignment tier.
  Collector taps node charge; node regrows, so sustainable rate is bounded by
  regrowth. Over-tapping causes local Void rise (soft cap, readable).
- Visual: beam between node and focus as one instanced line in existing
  magic_fx batch; LOD off beyond 64 m.

## 5. Carrier and conversion (balanced magitech)

- One carrier: mana in integer milli-units per essence, moved through the
  existing item/power network graph as a new channel, not a parallel network.
- Conversion machines (recipes.ron `Transmute`):
  - Essence → neutral mana (loss 10%).
  - Neutral mana ↔ factory power (each direction loss 25%; round trip 56%).
  - Essence + item → item (infusion: crystals, alloys, artifacts).
- Rule (existing brief): no energy from refunds or loops. Enforced by a startup
  check: any cycle in the conversion graph must have product of efficiencies < 1.
- Purifier machine: consumes mana, lowers Void field in radius (brief §1 PURGE).
- Art: authored voxel models only (`machines-mekanism-look-rejected`): stone +
  brass frames, crystal inset showing essence colour, no float greebles. Shared
  look via `assets/models/machines/*.ron`.

## 6. Farms without spawn exploits

- Tree farm: multiblock with footprint; plants saplings it holds, harvests
  mature trees inside footprint using existing growth stages, outputs to
  adjacent storage. Rate from growth table, deterministic.
- Mob farm: captured-creature vessel (catch with a spell/tool, kind stored in
  item) + Verdant/Void mana input → drops at table rate from `bestiary.ron` loot.
  No mobs spawned, no darkness/fall tricks. Cost scales with creature tier.
- Crop/animal farms reuse `crops.ron`/habitats; the farm only automates the
  existing action. One rule: automation never beats manual yield per input.

## 7. Artifacts / bauble slots

- 4 slots: 2 charm, 1 amulet, 1 belt. Equipment struct with Option<ArtifactId>.
- Effects from `artifacts.ron` as typed modifiers into existing `modifiers.rs`
  / `buffs.rs` (one modifier pipeline). Some draw essence upkeep from carried
  vessels.
- Save: needs version bump; read current constant first; old saves = empty.
- Reliquaries stay discovery shrines; they can award artifacts.

## 8. XP sinks at bookcases

- Enchanting table counts bookcases in a 5×5 ring (multiblocks.ron pattern),
  plus essence crystals adjacent → choose essence-flavoured enchantments.
- Offers deterministic: hash(world seed, table pos, item id, offer counter).
  No reroll by re-placing; counter advances only on purchase.
- Other sinks: study (research unlocks), artifact attunement, collector tuning.

## 9. Hell realm

- Procedural flora: L-system-free voxel grammars from `flora.ron` with realm tag
  (fungal spires, ember reeds, glass thorn). Placed by the hell generator under
  its own gate; uses same flora block pipeline as overworld.
- Fauna: 3 kinds via existing bestiary/spawn tables, behaviours from existing
  monster AI states; Void nodes as spawn anchors for readable danger.
- Mood: existing sky moods (`celestial-and-realm-sky`) + Void field driving
  haze density.

## 10. Performance contract

- Zero new render passes. Nodes = chunk voxels; glow = emissive material;
  beams/motes = existing instanced fx batch with fixed cap and distance LOD.
- Field updates event-driven; per-tick work O(active machines). Machine tick in
  existing factory scheduler and budget.
- Pattern matching only on edit events in registered cells.
- Each slice proves cost with frame-bench before/after on fixed pose.

## 11. Polish reality check (honest)

Not polished relative to a mature modpack (ATM/FTB-class). Evidence:
new-player audit (2026-11-04) found first-hour friction; many systems landed
"uncommitted" or "dormant gate" per session notes; several key features lack
runtime review (resonance doc: "no game launch"). Manyfold: owner reports
inaccessible. Breadth is high, integration and onboarding are low. Recommend a
polish milestone before new breadth: first-hour route, Manyfold access path,
HUD/tooltip consistency, commit/land dormant work.

## 11b. Status 2026-11-12

- Slice 1 enchanting half done: `assets/enchantments.ron` + `enchanting::TABLE`
  / `binding(mode)`; wand stats, upkeep, wand board, station page and guide read
  it. Modes 4/5 unchanged (save-stable). `cargo check` clean; no runtime review.
- A peer session landed `adventure/hedrons.rs` + `assets/mana_sources.ron`
  (seeded hedrons, 4-pylon capture, `supply()` into caster/circle recovery).
  That supersedes §3/§4 as the starting point. Review notes for its owner:
  hedrons are drawn as model cuboids rebuilt every frame (cost grows with
  count, conflicts with §10); `captured()` column-scans per call when close;
  its `Essence` enum is a second essence list until `essences.ron` exists.

## 12. Slice order

1. `essences.ron` + loader + validation; move enchanting BINDINGS to data.
2. Field + ambience/affinity read it (no visible change; A/B equal).
3. Node stamps + gate + capture review pose.
4. Collector multiblock + buffer + beam.
5. Carrier channel + transmute + loop check + purifier.
6. Bookcase enchanting.
7. Artifact slots (save bump).
8. Farms.
9. Hell flora/fauna.
Each slice: owner sign-off on captures for visual changes.

## Open decisions for owner

- Essence names and count.
- Conversion loss numbers.
- Artifact slot count.
- Polish milestone before or after slice 1.
