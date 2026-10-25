# Inventory and spell description contract

The user's later IV.11a amendment supersedes the earlier, longer tiers: standard
items have no prose; hidden behaviors get at most eight words; mechanics get at
most fourteen words; opt-in codex entries get at most fifty words. Numeric
properties come from gameplay accessors, never authored description text.

## Spell browser extension — 2026-10-24

The shared catalog now contains **102 entries**: the original eight inventory
entries and **94 spell records**. Those records cover 53 modifier/program cards,
40 other spells, and the empty slot. Spell ID 33 is unassigned. Every spell row
must resolve to a catalog entry when the spell table initializes; functional
copy and optional codex text use the existing tier, keyword, and numeric-prose
validator. The empty slot is Standard and has no description, stats, or cost.

All spell hovers now use the same compact layout: name/type, relevant automatic
stats, a functional sentence, and a separate cost line. Hold Alt reveals
secondary numbers, timing/spread changes, and any optional codex entry. Burning
and Wet link to the catalog's shared definitions. The old clipped hint paragraph
and duplicate `hint` field have been removed; discovery messages use the same
canonical functional sentence as hover cards. Names, colors, sounds, base values,
classification, and unlocks remain in `assets/spells.ron`.

`spell_defs/card.rs` reads projectile damage/speed, current inscription-adjusted
mana costs, current rank healing, and discovered ward upgrades from gameplay
accessors. Fractional mana costs retain a decimal rather than rounding to whole
mana. Modifier damage, speed, radius, lifetime, ricochets, wall/enemy passes, and
draw/copy counts come from an isolated neutral-deck evaluation. Elemental duration
and Wet reactions use the pure status application function. This preview is
explicitly before other deck modifiers; the equipped wand's evaluated next-cast
readout remains the authority for its actual program.

`wands/card_rules.rs` holds values moved unchanged from cast/impact code:
critical/fizzle rolls, health and mana exchanges, trigger timings/ranges,
collision/effect budgets, and utility effects. Launch and card preview now share
base-lifetime and intrinsic-modifier helpers. Poison stack capacity and the plasma
blade-direction array are shared with their effect implementations. No balancing,
spell IDs, save fields, unlocks, controls, or terrain generation are changed by
this extension. These are source-derived values, not a claim that every gameplay
parameter has migrated to RON.

The source audit corrected inaccurate old text: Lumen drill reaches 6.8 m before
speed modifiers (the old hint said 6.5 m), Blood siphon heals from base/rank/modifier
damage before resistance and critical reactions, and Orbit & ricochet does not
create the satellite bolts its old hint advertised. Touch's optional codex now
states the trigger requirement for an attached effect. Numeric mechanics formerly
embedded in hints remain in generated primary or Alt readouts.

Snapshots, hashes, and migration scripts are under
`downloads/art-direction-20261024/magic-before{,-edit}/` and its parent directory.
No tests or CI were added or run for this extension. Build, content-lint, native
capture, and visual acceptance results belong to the coordinating session record;
source review alone does not establish them. Maximum font-scale wrapping, all
status/trigger combinations, machine/relic screens, and a general hold-to-expand
codex still require separate acceptance. Functional sentences have a word budget;
the validator cannot prove that every sentence wraps to exactly two screen lines.

Native capture hook: launch an existing benchmark with
`VOXEL_GRIMOIRE_REVIEW=wands` and `VOXEL_REVIEW_SPELL_CARD=<spell id>` to place the
actual hover-card renderer in a foreground frame. Add `VOXEL_REVIEW_CODEX=1` to
exercise the same expanded path as holding Alt. Both the parsed benchmark-enabled
configuration and an existing grimoire review page are required; ordinary play
does not display this frame or force expansion. The hook does not change spell
availability, the player's deck, or cast inputs. Review-hook snapshots live in
`downloads/art-direction-20261024/magic-before-review/`.

## Original inventory slice

`assets/descriptions.ron` is a sparse catalog. Missing entries produce empty
text. `engine_content::descriptions` parses and validates the catalog on first
lookup. It rejects duplicate/empty keys, standard-tier prose, excessive word
counts, multiline/multiple-sentence functional text, numeric literals and common
number words, known filler adjectives, duplicate/unknown shared keyword names,
and missing or incorrectly capitalized definitions for referenced keywords.
The validator is deliberately not a grammar or semantic-truth checker.

The inventory, recipe output and creative item hovers share presentation code.
They omit empty description rows. Food restoration and armor defence use the
same accessors as gameplay. Base weapon damage now has a single accessor shared
by combat and presentation; its values and default case are unchanged. Packed
assemblies display their actual stored energy and upgrade level. Bore radius and
fuel cost use the same accessors/constants as excavation, preserving the choice
between the two tools. The maul description now reflects its pulse on every hit.

Blocks show material density. Hold Alt reveals any codex entry.
Density is labeled kg/m³; inventory items do not yet expose a physical volume
from which to derive mass. No invented per-item mass or flammability rating is
shown. Hardness/conductivity/ignition fields are not shown because live gameplay
does not use those authored values. Wand details and transfer controls
remain available while holding Alt; armor comparisons remain visible normally.
Existing gameplay bindings are unchanged.

Authored shared keywords render as links to the catalog's single definition.
Only Burning and Wet currently have verified definitions; Charged and Sealed
are reserved names and require a definition before use. Current inventory copy
does not need these statuses, so it shows no unused glossary labels.

## Independent critique and limits

The coordinating agent reviewed this slice before implementation and required
empty defaults, no duplicate balance constants, blank-row suppression, keyword
validation and a truthful density/mass distinction. Those constraints shaped
the implementation. An independent code review also caught lost armor/bore
comparisons, candidate-only material readouts and the maul's inaccurate condition;
these were corrected. Initial pre-edit snapshots and their SHA-256 manifest are
in `downloads/recovery/2026-10-24/descriptions-before/`; the later expedition
accessor extraction is preserved under `critic-before/` beside that directory.

At the original inventory checkpoint this was partial adoption. Spell hints and
hover cards have since migrated in the extension above; machine screens, relics,
and other codex surfaces still need migration. Their existing numbers and prose
must be reconciled with actual effect implementations before enabling this
validator for them. Resource fuel values and wand costs are not guessed
or copied into the sparse catalog; bores now show live radius and fuel cost.
Food/armor/weapon gameplay
values still live in Rust; this slice removes presentation duplication rather
than claiming all gameplay parameters are RON-authored.

The lint catches explicit filler words, not every adjective, clause, numeric
paraphrase or false claim. The word budget limits content length; exact pixel
wrapping under arbitrary font scale still requires visual review. No new tests,
test execution, CI, gameplay launch, captures or hardware timing were performed
for this slice. The consolidated offline, locked workspace library/binary check
passed with 30 warnings; the authored-content lint passed. Logs are in
`downloads/recovery/2026-10-24/{compile,description-lint}.log`.

## System design record — partial, not rubric-approved

The required fourteen sections follow. The narrow critique approved a corrective
inventory slice, not a complete tooltip-system SDD. Missing evidence is explicit.

### 1. Purpose

Show decision-relevant item behavior without duplicated balance numbers. This
implements the inventory portion of the later IV.11a amendment and supports
C-MC3's material-derived presentation. It does not satisfy C-MC3's RON-only
material/tool generation claim. No C-* gameplay claim is marked accepted.

### 2. Scope and dependencies

In: inventory/recipe/creative hovers, sparse copy, runtime stats, Alt details and
content lint. Out: spells, machines, relics, general codex, localization, VM and
text-layer integration. Dependency direction is `assets/descriptions.ron` →
`engine_content::descriptions` → inventory adapter → egui inventory helper.
Combat and item presentation read the same base-damage accessor; excavation and
tooltips share bore radius/fuel accessors. No crate edge
is added. See [architecture](../ARCHITECTURE.md); integration into the complete
system dependency graph remains open.

### 3. Data

All new data is immutable CPU content owned by `LazyLock<Catalog>` for the
process lifetime. The lint owns a temporary catalog. There are no GPU buffers,
simulation planes, save fields or cell bits. Rust layout is not a save ABI.

| Type | Fields | 64-bit inline size |
| --- | --- | --- |
| `Tier` | `Standard`, `Behavior`, `Mechanic` discriminant | 1 byte |
| `Description` | `key: String`, `tier: Tier`, `text: String`, `codex: String` | 80 bytes including padding |
| `Keyword` | `name: String`, `definition: String` | 48 bytes |
| `Catalog` | `entries: Vec<Description>`, `keywords: Vec<Keyword>` | 48 bytes |

Strings/vectors each contain three machine words on this target and own separate
allocations. The lint reports the shipped catalog at **48 inline bytes + 1,454
bytes owned capacity**, excluding allocator overhead, embedded source and
temporary parse/UI allocations. Layouts are compiler/target-dependent.

The RON example is [descriptions.ron](../../assets/descriptions.ron). Each entry
requires a key; omitted tier is Standard, omitted text/codex are empty. Unknown
fields are rejected. Missing lookup keys deliberately return no prose.

### 4. Algorithms

Load: `parse → validate rows/keywords → return catalog or errors`. For source
bytes B, entries E, keywords K and words W: parse O(B), duplicate checks expected
O(E + K), word checks O(W) against a fixed four-name vocabulary. Errors follow
input order; hash sets only serve membership, not emitted ordering.

Lookup is O(E). Hover formatting reads current values and checks keyword use
in O(KW). No simulation state, randomness, replay event or floating-point
mutation is introduced; existing gameplay floats are only formatted. Invalid
RON/copy returns diagnostics to the CLI, which exits nonzero. Invalid embedded
content panics on first lookup. Blank descriptions produce no label.

### 5. API

Public content types: `Tier`, `Description`, `Keyword`, `Catalog`. Functions:

```rust
parse(source: &str) -> Result<Catalog, Vec<String>>
validate(catalog: &Catalog) -> Vec<String>
get(key: &str) -> Option<&'static Description>
keywords(text: &str) -> impl Iterator<Item = &'static Keyword> + '_
```

Inventory adds `Item::description_entry() -> Option<&'static Description>`,
`description() -> &'static str`, `codex() -> &'static str`,
`stats_line() -> String`, `cost_line() -> String`, and
`weapon_damage(tier: u32) -> f32`. Expedition exposes
`bore_radius(tier: u32) -> f32` and `bore_fuel_cost() -> f64`.
The UI helper is private. No Actions, events or VM APIs are added.

### 6. Tick integration

No simulation tick or budget is used. Work occurs only for displayed hovers,
with catalog initialization once. Distance degradation/sleep are N/A because
no spatial simulation is scheduled. A dedicated UI timing metric is not yet
implemented. Alt reveals details without changing existing gameplay bindings.

### 7. Hardware

The catalog/formatter are GPU-optional and add no compute requirements. H0/H1/H3
use the same authored data. The measured capacity above is for this 64-bit build,
not independently verified memory on each class. Simulation tick cost is
structurally zero; startup/UI milliseconds and transient allocations remain
unmeasured. No quality-ladder knobs or new GL path are owned by this slice.

### 8. Content

Authors edit sparse RON rows; standard items need none. The example row is the
template; no scaffold generator is implemented. `just lint-descriptions` checks
the shipped file; `just lint-descriptions path/to/catalog.ron` checks another.
This offline authoring command shares `parse` with runtime loading.

Rules are listed above. Semantic truth, arbitrary adjectives/numeric paraphrases,
unknown unused entry keys and exact pixel wrapping are not validated. The
inventory enum-to-key match remains an adapter, so the full rubric's data-only
identity requirement is not met. Material stats show density used by live bodies.
Hardness/conductivity/ignition readouts are deferred until actual gameplay authority
migrates to those table fields; merely authored values are insufficient.

### 9. Tests and evidence

No tests were added/run, per the user. The authoring command `lint-descriptions`
passed: **8 entries, 2 keyword definitions**. No negative-fixture tests,
property tests, golden images, gameplay scenarios or hardware acceptance ran.
CPU/GPU equivalence and conservation are N/A for read-only UI. Consolidated
compile passed: `cargo check -q --workspace --bins --lib --offline --locked
--message-format=short` (30 warnings). This does not compile or execute tests.

### 10. Bench

No benchmark or timings exist. A future bounded measurement should compare
repeated hovered-item frames with/without Alt on H0/H1/H3 at identical font
scale, measuring UI CPU time and allocations. Numerical regression budgets
remain unset until a baseline exists. Current bounds are in section 4; measured
catalog capacity is in section 3. Full bench acceptance remains incomplete.

### 11. Migration

Save versions/material IDs are unchanged. About forty lines of duplicate item
description matches are replaced by sparse lookup. Combat damage values and
unknown-tier fallback move unchanged into a shared accessor. Standard filler
disappears; Alt retains controls and existing wand details. Armor comparison
remains visible without Alt. The inventory panel explains the Alt gesture.
Original dirty files, including `justfile`, are preserved in the snapshot
directory above. These snapshots are not a repository-wide rollback.

### 12. Risks and open questions

- False mechanics can pass lint: review each entry against effect source;
  spell migration needs a separate effect/data audit.
- Two-line wrapping and Alt discoverability are unproven: review default and
  maximum supported font-scale captures in a bounded visual follow-up.
- Typoed unused keys can remove optional copy silently: add consumer-key coverage
  validation when extending the item catalog.
- Per-item mass lacks a volume basis: keep density until volume semantics exist.
- Data-only item identities/localization would require a separate design/ADR;
  this correction does not select that long-term architecture.

### 13. Sessions

1. Current inventory slice: schema, shared UI, numeric accessors, snapshots and
   lint; estimated new code 250–400 lines. Exit: lint and workspace compile passed;
   visual acceptance and full-rubric approval remain open.
2. Visual/accessibility follow-up: inspect Alt behavior and wrapping, fix observed
   issues only; estimated 0–100 lines; exit: reviewed captures at agreed scales.
3. Spell/machine adoption: audit numeric sources before migrating a bounded family.
   LOC estimate and exit scenarios depend on that audit; not yet implementation-ready.

`just verify` was not invoked because it runs tests prohibited by the user.
This exception is recorded, not labeled full exit-criterion acceptance.

### 14. Alternatives rejected

- Keep numeric strings in item descriptions: rejected because balance edits can
  silently desynchronize them; gameplay accessors remove this duplication.
- Automatically truncate old prose: rejected because truncation can erase a
  necessary condition and misdescribe a mechanic.
- Rewrite all spell/machine effects during recovery: rejected because existing
  hints mix hardcoded numbers with mechanics across dirty files; audit first.

Rubric status: complete dependency graph integration, data-only identities,
scaffolding, class timings/captures, authorized tests and `just verify` evidence
remain incomplete. The narrow review does not establish a score of two on all
twelve full-system criteria.
