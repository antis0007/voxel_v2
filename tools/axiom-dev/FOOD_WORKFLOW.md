# Food variants and recipe withdrawal

The next food originally collided with the claim-stake artwork: FoodKind 25
implied item-sheet slot 75, already used by the claim stake. Slots 76 and 77
also contain authored objects. New foods now carry an explicit `icon: Some(id)`
in `assets/foods.ron`. Forge appends a new sprite after the highest occupied
slot. Existing food definitions omit this field and retain their old mapping.
No existing native item ID or artwork slot moves.

`Food.icon` is optional with `serde(default, skip_serializing_if = "Option::is_none")`.
It changes the authoring schema only; inventory/save encodings remain unchanged.
New FoodKind values append to the real enum and its ALL list. Applied IDs must
remain allocated, including when their crafting recipe is withdrawn.

## Workflow

1. Choose an existing project food and sprite. Set nutrition explicitly and
   decide whether to copy its status effects. Hue changes only the new artwork.
2. Preview through `previewFoodWorkflow`. An optional shapeless recipe uses
   known catalog ingredients and its output is always the newly allocated food.
3. Review item identity, independent icon slot, stats, dependencies, provenance
   and the source-only verification status. No files change at preview.
4. Send the same request and returned approval to `applyFoodWorkflow`.
   Every source revision is rechecked, including read-only recipe catalog
   inputs. `publishBundle` saves the item/registry/icon/optional recipe together.
5. Rebuild and test in the native game. Nothing here claims live hot reload,
   complete engine validation, a rendered result, or a world-save round trip.

The source template record and source sprite remain intact; the variant is an
independent copy with new native and sprite identities. Later template edits
do not propagate into it. There is no imported mod art or new licensing grant.
Template provenance and source hashes appear in the preview/apply result;
the existing bundle ledger retains hashes and backups, not template metadata.
Save the result alongside any external review if that metadata must persist.

New variants are ordinary edible foods. Copying effects from a potion does
not copy native potion classification. This slice does not add material masks,
new behaviors, shaped recipe creation, or a general item-registry abstraction.
Existing recipe/node tools can edit a created recipe after publication.

## CLI

Run `node tools/axiom-dev/food-workflow-cli.mjs help` for the exact JSON shape.
`catalog` lists current templates and ingredient codes. `preview` and `apply`
read JSON from stdin; add `approval` from the preview for apply. No provider
invocation or network request is involved.

```json
{
  "draft": {
    "name": "Herb Bread", "template": "Bread", "sprite": 60, "hue": 0,
    "hunger": 32, "health": 1, "saturation": 20, "copyEffects": false
  },
  "recipe": {
    "name": "Herb Bread", "station": "hand", "count": 1,
    "inputs": [{"item": "Food(Bread)", "count": 1}, {"item": "Food(Egg)", "count": 1}]
  }
}
```

Recipe station keys come from the real recipes document (`hand`, `bench`,
`crystal_tech` in the current project). The request cannot choose or overwrite
the new item's native ID, output code, or allocated sprite slot.

`inspect <bundleId>` reports receipt status. `preview-withdraw` takes
`{"bundleId":"..."}`; `withdraw` repeats that request with its approval.
Withdrawal restores only the recipe document. Native ID, food definition and
sprite remain, and the next variant receives the next ID. Any later change to
the receipt's files blocks withdrawal; reload/review those edits separately.
Creation bundles carry an immutable `retain-food-identity` policy. The generic
bundle history disables full revert and its server command rejects it. Recipe
withdrawal remains reversible without removing an allocated food identity.

## Browser integration

`food-creator.mjs` now requires `ctx.foodWorkflow(command)` and has no direct-save
fallback. The form keeps name and template visible; nutrition/effects/art are
expandable. Preview compares server revisions with loaded documents so stale
local art cannot approve a different server-side sprite. The review shows
source status and dependency paths. Combined recipe creation remains available
through the shared API/CLI; the existing recipe editor remains the browser path.

The authenticated POST `/api/food-workflow` dispatcher uses the existing local
server token/origin/body-limit checks:

| Command `op` | Export from `food-workflow.mjs` | Argument |
| --- | --- | --- |
| `catalog` | `foodWorkflowCatalog` | none |
| `preview` | `previewFoodWorkflow` | `command.request` |
| `apply` | `applyFoodWorkflow` | `command.request` |
| `inspect` | `inspectFoodWorkflow` | `command.bundleId` |
| `preview-withdraw` | `previewFoodWithdrawal` | `command.request` |
| `withdraw` | `withdrawFoodRecipe` | `command.request` |

Apply returns `item`, `bundleId`, and `saved` documents with path/text/version.
The app adapter rejects dirty dependency documents before submit, refreshes
saved documents afterward, and preserves a local draft changed during a response.
The legacy `/api/create-food` write bypass is removed. Both creation actions and
catalog variants use the shared adapter. Restart Forge after updating its server.
The full native game must be rebuilt to execute newly authored foods.

## Checks

`node --test tools/axiom-dev/food-workflow.test.mjs` covers the occupied-slot
regression, existing records, independent variants/effects, visual-only tint,
recipe output identity, catalog artwork mapping, invalid inputs, stale approvals,
two sessions, CLI parity, and withdrawal retaining IDs and art.

An isolated offline Rust probe compiled the exact production Food module prefix,
generated FoodKind enum/impl, and extracted native effect types. It verified
legacy canonical RON round trip for all 25 foods, generated canonical round trip
for 26 foods, native lookup of ID 25, explicit icon 78, legacy mappings, nutrition
accessors, ordinary-food classification and icon range rejection. It did not
compile the full game/renderer or execute eating, crafting, or world-save reload.
The native regression in `inventory/food.rs` is included for the full suite.
