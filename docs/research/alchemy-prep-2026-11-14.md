# Alchemy system: preparation notes (2026-11-14)

Status: preparation for a design conversation. Not a design and not approved.
No source changed. Proper nouns are placeholders (UI_RULES.md). Fits inside
`docs/superpowers/specs/2026-11-14-living-world-integration-design.md` (the
shared world quantities and link table) and
`docs/superpowers/specs/2026-11-12-mana-magitech-design.md` (MM: essences,
field, carrier, conversion table).

Evidence was gathered with `parity-cli describe`, `catalog-cli list` and
read-only source reads. Line numbers are from that pass; re-check before edits.

## 1. What exists

| Piece | Where | What it does | Limits for alchemy |
|---|---|---|---|
| Block reactions | `assets/reactions.ron`, `crates/content/src/reactions.rs:36` (`ReactionRow`), catalog `reactions` | contact/heat reactions between blocks: bronze 3:1, lava+water, acid | block in, block out; cannot yield items |
| Melt/boil/burn | `assets/materials.ron` `sim:` columns (`melt`, `boil`, `ignite`, `melts_into`...) | phase changes from the thermal model | per material, not per recipe |
| Brewing stand | `assets/brewing.ron`, `world/stations/table.rs:57-83`, `stations/brewing.rs` | wild essence base + modifier → potion, needs adjacent water, fixed steps | outputs are `FoodKind` only; one base item; no heat or apparatus |
| Machine processes | `assets/processes.ron`, `world/factory/process_table.rs:70` | inputs → output + by-product, ticks, power; coke oven is already distillation-like (coal → coke + creosote) | recipe ids saved in machines: append only |
| Fabricator | `assets/fabrication.ron` row 2 | crystal + 2 biomass → wild essence | |
| Transmute spell | `assets/spells.ron:262` "Touch of gold", `adventure/spells.rs:789` `alchemical_transmute` | stone → copper/iron ore or crystal by coordinate hash; soil → glowcap | **hard-coded table in Rust**; costs only mana; no conservation |
| Mana flowers | `assets/crops.ron:824-847`, `world/farming/flowers.rs` | eat loose wild essence → 16 mana each | ledger L079 playable |
| Runes | `assets/runes.ron`, `crates/content/src/runes.rs` | phrase grammar; Material runes build blocks | no transmutation runes; hue list (Fire, Frost, Shock, Venom, Water, Force, Life, Earth, Arcane) is the nearest thing to aspects |
| Hedron elements | `assets/mana_sources.ron`, `adventure/status.rs:12` | Fire, Frost, Shock, Toxic, Wet | element mana, not items |
| Draft magic tree | `docs/game/drafts/magic_tree.ron` (not loaded) | 8 aspects; nodes `alchemy.cauldron` (cauldron, alembic), `alchemy.transmute` (circle, auto-smelting) | draft only |

Absent: aspects in code or tables, crucible/alembic/retort/athanor apparatus,
distillation as a general process, alchemical item reagents beyond resource 18,
seeded secret recipes (owner tag G06 "seeded grammar and hint synthesis pending").

## 2. Rules any alchemy design must keep

From the parity matrix and design docs:
- **C-MC1** (A14-022): no profitable conversion cycles; values are declared game rules.
- **C-MC2** (B-TC-08): contamination affects matter and is repairable.
- **C-MC4** (B-TC-07): one chemistry/ritual implementation. Alchemy must not be
  a third recipe executor beside `processes.ron` and `reactions.ron`
  (GAME_DESIGN §9.1 one process shape: inputs + conditions + energy + time →
  outputs + by-products + heat + risk).
- B-TC-01 note: "do not treat mystical tags as measured chemistry".
- Conservation (master design): matter, energy and mana are accounted;
  transmutation pays for mass changed.
- Disciplines (GAME_DESIGN §6.3): science-only worlds keep real chemistry;
  magic-only worlds keep alchemy without powered machines.
- Player text: "essence" is banned except the item name "wild essence"
  (`egui_ui/text_rules.rs:60-64`).
- Save: new `FoodKind` rows append without a bump (FOOD_WORKFLOW.md); new factory
  resources wait on `docs/design/RESOURCE_REGISTRY_PLAN_20261113.md`.

## 3. The opportunity: alchemy is real lab chemistry plus one magical step

The classical alchemical operations are real laboratory operations. Making them
the backbone keeps alchemy educational, gives science-only worlds a chemistry
lab, and leaves transmutation as the one magical step with an explicit cost.

| Operation | Real chemistry | Apparatus (placeholder) | Shared quantities |
|---|---|---|---|
| Calcination | heat a solid to drive off gas: limestone → quicklime + CO2 | crucible, kiln | heat in; matter; **carbon out** |
| Carbonation | quicklime + CO2 → limestone (calcium looping, a real capture method) | lime bed beside a stack | carbon **in** (capture) |
| Dissolution | solvent extracts a soluble part | water bath / vat | water, matter |
| Filtration / separation | split solids from liquor | filter, settler | matter |
| Distillation | boil and condense by boiling point (ethanol, creosote, acids) | alembic, still, retort | heat, matter, air (fumes) |
| Fermentation | yeast/fungi turn sugar into ethanol + CO2 | vat (reuse bioreactor) | matter, carbon (small) |
| Sublimation | solid → gas → solid (sulfur, salts) | sublimation pot | heat |
| Crystallisation | cool or evaporate to grow crystals | crystalliser | heat, water |
| Coagulation / precipitation | mix solutions to drop a solid | vat | matter |
| Transmutation (magic) | none: the magical step | circle, athanor near a hedron | mana by element; land pressure (spills) |

Educational hooks: lime cycle (calcination releases CO2, carbonation captures
it) ties alchemy directly to the carbon account; distillation teaches boiling
points read from `materials.ron`; fermentation ties to the bio-ethanol resource.

## 4. How it plugs into the living world

- Reads: matter, heat (thermal model and Fire hedrons), water, mana by element,
  land pressure (purity of reagents), knowledge (capability graph).
- Writes: matter, local air (acid fumes, fuel gas via `world/industry.rs`),
  carbon (calcination, fermentation; capture via carbonation), land (spills,
  failed transmutations raise pressure: C-MC2 repairable contamination).
- Other systems that read alchemy output: brewing (reagents as modifiers),
  smeltery (fluxes, refined metals), tool forging (alloy additives), farming
  (fertiliser, biochar), enchanting/runes (inks, mana fluids), habitats (feed,
  lures), medicine/food, civ trade.
- `links.ron` gets one `alchemy` row; the link check proves each of the above.

## 5. Decisions for the design conversation

1. **Aspects:** declared tags per material (Thaumcraft-like) or derived from
   real sim columns (melt, boil, density, conductivity, flammability) plus the
   five hedron elements? Derived avoids a second table and keeps B-TC-01's
   warning; declared gives more authored flavour.
2. **Pillar:** is real chemistry in the always-on craft domain (works in
   science-only) with transmutation under Magic? This split fits §6.3 cleanly.
3. **Seeded secret recipes:** per-world recipes from a grammar (GAME_DESIGN §8,
   owner tag G06) or fixed recipes discoverable by experiment?
4. **Executor:** extend the `processes.ron` row shape with conditions/heat/risk
   (one executor for machines and apparatus), or make apparatus factory Kinds?
   C-MC4 requires one of these, not a new executor.
5. **Touch of gold:** move its table to data and give it a mass/mana cost under
   the conservation rule. Changing outcomes affects existing play; owner call.
6. Names for apparatus, reagents and any new items.

## 6. Dev-tool preparation

Present and usable now:
- `catalog-cli list`: catalogs are fabrication, spells, tool-parts,
  tool-materials, reactions. **Brewing, processes, runes, enchantments and
  mana_sources are not registered catalogs** and have no `check_*` validator
  example.
- `factory-lab-cli`: headless process tests; reads `processes.ron` from disk
  each run; scenarios in `tools/axiom-dev/scenarios/*.json` with parity tags.
- `alloy-cli` / `material-progression-cli`: inspect/preview/apply(/revert)
  over RON tables through `publishBundle`: the template for an authoring CLI.
- `food-workflow-cli`: new item + recipe in one bundle (potions are `FoodKind`).
- `parity-cli describe <ID>`: rows B-TC-01, -06, -07, -08, A14-022, B-BOT-07,
  B-BM-10, A13-01, A13-05.
- `engine-cli`: live world for apparatus prototyping.

Proposed, once the design is approved (each per `CODING_TOOLS.md` "Extend
without a parallel editor": one semantic module shared by panel and CLI,
registered in `tool-capabilities.mjs`, schema stays in the native catalog):
1. Register `brewing` and `processes` as catalogs with `check_brewing` /
   `check_processes` validator examples.
2. `alchemy-cli.mjs` + `alchemy.mjs`: `inspect`, `preview`, `apply`, `revert`
   for apparatus rows and reagents.
3. `alchemy-cli cycles`: build the conversion graph across processes,
   fabrication, brewing, reactions, transmutation and MM conversions, and fail
   on any cycle whose product of efficiencies is ≥ 1 (C-MC1), or any cycle
   that creates mass or mana.
4. Factory-lab scenarios: lime cycle (calcination then carbonation nets zero
   carbon minus losses), distillation by boiling point, one transmutation with
   its mana and pressure cost.
5. `links.ron` row and `links-cli check` coverage (living-world design §11).
