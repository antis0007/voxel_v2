# Field study and production progression

Implemented source slice of `MOD_PARITY_DELIVERY.md`'s ordinary play loops and
engineer/mage/hybrid routes. This does not close the 422-feature matrix or claim
modpack parity.

## Play loop

Craft a Bookcase (existing recipe), open the Field Guide's study page and focus
the physical bookcase. Each of eight projects accepts either earned XP or its
complete supply batch. Supplies are consumed; no XP is charged on that route.
Missing supplies, missing prerequisites, stale purchase intents and distant or
occluded bookcases cannot debit inventory. Existing magic XP prices remain.

After learning a project, submit another supply batch for research XP: one
quarter of the lesson's XP price, rounded down, minimum one. This is a renewable
resource sink, not a free periodic reward. Farming/cooking, refined metals and
arcane manufacturing can therefore fund further study and enchanting. Repeat
research has a distinct request intent from first-time study.

| New specialization | Prerequisite | Permanent effect |
|---|---|---|
| Prospecting | None | +1 XP on harvests that already award mining XP |
| Provisioning | Horticulture | +25% food saturation, capped by current hunger; potions excluded |
| Wayfaring | Threshold Survey | 15% less hunger drain, before saturation absorbs it |

The five existing lessons gain supply alternatives. All three new lessons can
also use either payment route. Five new advancements cover first study, four
and eight learned disciplines, first research and 25 research batches. Existing
lessons count toward the new total on the next statistics step.

## Owning files and tooling

- `assets/field_studies.json`: XP prices, supply batches and prerequisites.
- `world/progression/study.rs` and `study/curriculum.rs`: authoritative purchase,
  research and bounded catalog loading. The three passive effects live in
  `world/progression.rs`.
- `egui_ui/recipe_book.rs`: supply icons with owned/required counts, both payment
  actions, prerequisites and repeat research. The magic guide links the Bookcase
  recipe and exposes the same study UI.
- `tools/axiom-dev/field-study-cli.mjs`: inspect, preview and apply existing
  project quantities through the dev panel's revision checks and backups.
  Identity, item types and prerequisite changes are outside this CLI's scope.

The final Prospecting price (35 XP) was published through this CLI. The game
embeds the catalog and needs rebuilding; edits are not live overrides.

## Persistence and authority

Specializations use the three remaining lesson bits in the existing progression
sidecar; Horticulture retains its separate boolean and conduction retains bit 4.
Old saves default the new bits to zero. Older executables do not understand the
new lesson bits; keep matching builds. No world-body layout or item ID changes.
Statistics append two codes to the existing counted sidecar format. Multiplayer
uses ordered host requests and mounted player inventory/progression, including
fresh bookcase and payment checks. Supply payment stages a clone before commit.

## Evidence and remaining work

Source implementation and actual dev-tool preview/apply completed. The final
`cargo check -p minecraft_style_voxel_prototype --offline --quiet` passed with
default multiplayer enabled; `target/field-studies-check.log` records that run.
Earlier attempts encountered concurrent settlement and terrain edits, which
were resolved by their ongoing work without changes from this slice. The CLI
syntax check and final catalog inspection also succeeded. No tests were added
or run, per workspace instructions. No native playthrough, save/reload exercise
or multiplayer acceptance is claimed.

Balance needs play evidence, especially renewable food-to-XP throughput and the
relative value of supply versus XP payments. Further slices should connect
kinetic processing, request logistics, multi-stage chemistry and expedition
rewards to these paths; they are not implemented by this slice.

## Inspiration references

- [Enigmatica 2: Expert](https://www.curseforge.com/minecraft/modpacks/enigmatica2expert):
  1.12 expert quest progression; source for cross-discipline progression inspiration.
- [Create: Above and Beyond](https://www.curseforge.com/minecraft/modpacks/create-above-and-beyond):
  production-led technological journey toward space.
- [MC Eternal](https://www.curseforge.com/minecraft/modpacks/minecraft-eternal):
  broad technology/adventure variety with exploration-based gates.

These inspire original Axiom systems; their code, assets and numerical balance
were not imported. See the existing feature matrix for the much broader backlog.
