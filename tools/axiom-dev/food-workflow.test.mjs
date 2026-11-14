import { test } from "node:test";
import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import { foodPaths, foodPlan, foodTemplates, foodIconId } from "./food-item.mjs";
import { readSheet } from "./sprite-sheet.mjs";
import { field } from "./idea-data.mjs";
import { parse } from "./syntax.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";

const source = fileURLToPath(new URL("../../", import.meta.url));
const dataPaths = [...foodPaths, "assets/recipes.ron", "assets/materials.ron", "assets/machines.ron",
  "crates/app/src/world/expedition.rs", "crates/app/src/world/inventory/food.rs", "crates/app/src/egui_ui/icons/sprites16.rs"];
const original = new Map(dataPaths.map(file => [file, readFileSync(path.join(source, file), "utf8")]));
const initialFoods = foodTemplates(original.get(foodPaths[1]));
const nextId = initialFoods.length;
const nextIcon = Math.max(...readSheet(original.get(foodPaths[2]), foodPaths[2]).entries.map(row => Number(row.id.split(" · ")[0]))) + 1;
const request = (name = "Herb Bread") => {
  while (initialFoods.some(row => row.name.toLowerCase() === name.toLowerCase())) name += " Test";
  return {
  draft: { name, template: "Bread", sprite: 60, hue: 0, hunger: 32, health: 1, saturation: 20, copyEffects: false },
  recipe: { name, station: "hand", count: 1, inputs: [{ item: "Food(Bread)", count: 1 }, { item: "Food(Egg)", count: 1 }] },
  };
};
const pureDocs = () => foodPaths.map(path => ({ path, text: original.get(path) }));

async function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "forge-food-test-"));
  t.after(() => {
    const relative = path.relative(tmpdir(), root);
    assert(!relative.startsWith("..") && !path.isAbsolute(relative));
    rmSync(root, { recursive: true, force: true });
  });
  const tools = path.join(root, "tools/axiom-dev");
  mkdirSync(tools, { recursive: true });
  for (const file of readdirSync(new URL(".", import.meta.url))) {
    if (file.endsWith(".mjs")) copyFileSync(new URL(file, import.meta.url), path.join(tools, file));
  }
  for (const [file, text] of original) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), text);
  }
  const api = await import(pathToFileURL(path.join(tools, "food-workflow.mjs")).href);
  const bundles = await import(pathToFileURL(path.join(tools, "agent-bundles.mjs")).href);
  return { root, api, bundles, get: file => readFileSync(path.join(root, file), "utf8"),
    put: (file, text) => writeFileSync(path.join(root, file), text),
    snapshot: () => dataPaths.map(file => readFileSync(path.join(root, file), "utf8")) };
}

test("next native food gets an independent icon without moving occupied slots or existing IDs", () => {
  const docs = pureDocs(), before = readSheet(docs[2].text, foodPaths[2]);
  const next = foodPlan(docs, request().draft);
  const prior = foodTemplates(docs[1].text), foods = foodTemplates(next.changes[1]);
  assert.equal(next.id, prior.length);
  assert.equal(next.iconId, Math.max(...before.entries.map(row => Number(row.id.split(" · ")[0]))) + 1);
  assert(next.id >= 25);
  assert(next.iconId > 77);
  assert.equal(foodIconId(foods.at(-1).row, next.id), next.iconId);
  assert.equal(next.changes[2].slice(0, docs[2].text.trimEnd().length), docs[2].text.trimEnd());
  for (const row of prior) {
    const after = foods.find(food => food.kind === row.kind);
    assert.equal(next.changes[1].slice(after.row.start, after.row.end), docs[1].text.slice(row.row.start, row.row.end));
  }
  assert(next.changes[0].includes(next.kind + " = " + next.id + ","));
  assert(next.changes[0].includes("ALL: [Self; " + (next.id + 1) + "]"));
  for (const id of [75, 76, 77]) assert.equal(before.entries.filter(row => row.id.startsWith(id + " · ")).length, 1);
});

test("visual tint does not mutate nutrition; variants and inherited effects remain independent", () => {
  const docs = pureDocs(), draft = { ...request().draft, template: "Trailcake", copyEffects: true };
  const a = foodPlan(docs, draft), b = foodPlan(docs, { ...draft, hue: 80 });
  assert.equal(a.changes[0], b.changes[0]);
  assert.equal(a.changes[1], b.changes[1]);
  assert.notEqual(a.changes[2], b.changes[2]);
  const base = foodTemplates(docs[1].text).find(row => row.kind === "Trailcake");
  const variant = foodTemplates(a.changes[1]).at(-1);
  assert.equal(a.changes[1].slice(field(variant.row, "effects").start, field(variant.row, "effects").end),
    docs[1].text.slice(field(base.row, "effects").start, field(base.row, "effects").end));
  const secondDocs = foodPaths.map((path, i) => ({ path, text: a.changes[i] }));
  const second = foodPlan(secondDocs, { ...draft, name: "Seed Bread", hue: -80, health: 3 });
  assert.equal(second.id, a.id + 1);
  assert.equal(second.iconId, a.iconId + 1);
  const retained = foodTemplates(second.changes[1]).find(row => row.kind === a.kind);
  assert.equal(second.changes[1].slice(retained.row.start, retained.row.end), a.changes[1].slice(variant.row.start, variant.row.end));
  assert(second.changes[2].startsWith(a.changes[2].trimEnd()));
});

test("shared service publishes item and known recipe together with exact output identity", async t => {
  const f = await fixture(t), input = request();
  const before = f.snapshot(), preview = await f.api.previewFoodWorkflow(input);
  assert.deepEqual(f.snapshot(), before);
  assert.equal(preview.recipe.output, "Food(" + preview.item.kind + ")");
  assert.equal(preview.paths.length, 4);
  assert(preview.revisions.some(row => row.path === "assets/materials.ron"));
  const result = await f.api.applyFoodWorkflow({ ...input, approval: preview.approval });
  assert.equal(result.saved.length, 4);
  assert.equal(f.api.inspectFoodWorkflow(result.bundleId).withdrawable, true);
  assert(f.get("assets/recipes.ron").includes("output: (item: " + preview.recipe.output + ", count: 1)"));
  assert.equal(f.get("assets/materials.ron"), original.get("assets/materials.ron"));
  const catalog = await f.api.foodWorkflowCatalog();
  assert(catalog.items.some(row => row.code === preview.recipe.output));
  assert.equal(catalog.templates.find(row => row.kind === preview.item.kind).iconId, nextIcon);
  const artwork = await recipeArtwork({ load: async path => ({ path, text: f.get(path) }), tree: doc => parse(doc.text, "ron") });
  assert(artwork.sprite(preview.recipe.output).entry.id.startsWith(nextIcon + " · "));
  assert(artwork.sprite("Food(Bread)").entry.id.startsWith("60 · "));
  assert(artwork.sprite("Block(ClaimStake)").entry.id.startsWith("75 · "));
});

test("two sessions cannot apply approvals against another session's appended identity", async t => {
  const f = await fixture(t), a = request(), b = request("Seed Bread");
  const [first, second] = await Promise.all([f.api.previewFoodWorkflow(a), f.api.previewFoodWorkflow(b)]);
  assert.equal(first.item.id, second.item.id);
  await f.api.applyFoodWorkflow({ ...a, approval: first.approval });
  const applied = f.snapshot();
  await assert.rejects(f.api.applyFoodWorkflow({ ...b, approval: second.approval }), /already make|stale/);
  assert.deepEqual(f.snapshot(), applied);
  delete b.recipe;
  const fresh = await f.api.previewFoodWorkflow(b);
  assert.equal(fresh.item.id, first.item.id + 1);
  await f.api.applyFoodWorkflow({ ...b, approval: fresh.approval });
  assert(f.get(foodPaths[0]).includes(first.item.kind + " = " + nextId + ","));
  assert(f.get(foodPaths[0]).includes(fresh.item.kind + " = " + (nextId + 1) + ","));
});

test("read-only catalog revisions and changed request values invalidate prior approvals", async t => {
  const f = await fixture(t), input = request(), preview = await f.api.previewFoodWorkflow(input);
  await assert.rejects(f.api.applyFoodWorkflow({ ...input, draft: { ...input.draft, health: 7 }, approval: preview.approval }), /stale/);
  f.put("assets/materials.ron", f.get("assets/materials.ron") + "\n// Concurrent catalog edit\n");
  const changed = f.snapshot();
  await assert.rejects(f.api.applyFoodWorkflow({ ...input, approval: preview.approval }), /stale/);
  assert.deepEqual(f.snapshot(), changed);
});

test("invalid quantities, unknown fields, identity collisions and orphan definitions fail before writes", async t => {
  const f = await fixture(t), before = f.snapshot();
  const cases = [
    { ...request(), draft: { ...request().draft, health: Number.NaN } },
    { ...request(), draft: { ...request().draft, copyEffects: "false" } },
    { ...request(), draft: { ...request().draft, kind: "Bread" } },
    { ...request(), draft: { ...request().draft, name: "Bread" } },
    { ...request(), recipe: { ...request().recipe, name: "Hang glider" } },
    { ...request(), recipe: { ...request().recipe, output: "Food(Bread)" } },
    { ...request(), recipe: { ...request().recipe, count: 0 } },
    { ...request(), recipe: { ...request().recipe, station: "invented" } },
    { ...request(), recipe: { ...request().recipe, inputs: [{ item: "Food(Bread)", count: -1 }] } },
    { ...request(), recipe: { ...request().recipe, inputs: [{ item: "Food(NoSuchItem)", count: 1 }] } },
  ];
  for (const input of cases) await assert.rejects(f.api.previewFoodWorkflow(input));
  assert.deepEqual(f.snapshot(), before);
  f.put(foodPaths[1], f.get(foodPaths[1]).replace("kind: Bread,", "kind: UnknownFood,"));
  await assert.rejects(f.api.previewFoodWorkflow(request()), /exactly one/);
});

test("recipe withdrawal retains allocated identity and art across the next creation", async t => {
  const f = await fixture(t), input = request(), preview = await f.api.previewFoodWorkflow(input);
  const result = await f.api.applyFoodWorkflow({ ...input, approval: preview.approval });
  assert.equal(f.bundles.inspectBundle(result.bundleId).reversible, false);
  assert.throws(() => f.bundles.revertBundle(result.bundleId), /identity is retained/);
  const retained = foodPaths.map(f.get);
  const withdrawal = f.api.previewFoodWithdrawal({ bundleId: result.bundleId });
  assert.throws(() => f.api.withdrawFoodRecipe({ bundleId: result.bundleId, approval: "old" }), /stale/);
  const withdrawn = f.api.withdrawFoodRecipe({ bundleId: result.bundleId, approval: withdrawal.approval });
  assert.equal(withdrawn.retained.kind, preview.item.kind);
  assert.equal(f.bundles.inspectBundle(withdrawn.withdrawalBundleId).reversible, true);
  assert.deepEqual(foodPaths.map(f.get), retained);
  assert.equal(f.get("assets/recipes.ron"), original.get("assets/recipes.ron"));
  assert.equal(f.api.inspectFoodWorkflow(result.bundleId).withdrawable, false);
  assert.throws(() => f.api.previewFoodWithdrawal({ bundleId: result.bundleId }), /already withdrawn/);
  const next = request("Seed Bread"), nextPreview = await f.api.previewFoodWorkflow(next);
  assert.equal(nextPreview.item.id, nextId + 1);
  assert.equal(nextPreview.item.iconId, nextIcon + 1);
});

test("withdrawal rejects later edits and no-recipe receipts without overwriting them", async t => {
  const f = await fixture(t), input = request(), preview = await f.api.previewFoodWorkflow(input);
  const result = await f.api.applyFoodWorkflow({ ...input, approval: preview.approval });
  const withdrawal = f.api.previewFoodWithdrawal({ bundleId: result.bundleId });
  f.put("assets/recipes.ron", f.get("assets/recipes.ron") + "\n// Later recipe work\n");
  const edited = f.snapshot();
  assert.throws(() => f.api.withdrawFoodRecipe({ bundleId: result.bundleId, approval: withdrawal.approval }), /sources changed/);
  assert.deepEqual(f.snapshot(), edited);
  const next = request("Seed Bread"); delete next.recipe;
  const nextPreview = await f.api.previewFoodWorkflow(next);
  const noRecipe = await f.api.applyFoodWorkflow({ ...next, approval: nextPreview.approval });
  assert.equal(f.api.inspectFoodWorkflow(noRecipe.bundleId).withdrawable, false);
});

test("CLI uses the same revision-checked service and returns JSON errors", async t => {
  const f = await fixture(t);
  const cli = (command, input) => spawnSync(process.execPath, [path.join(f.root, "tools/axiom-dev/food-workflow-cli.mjs"), command],
    { encoding: "utf8", input: JSON.stringify(input), windowsHide: true });
  const input = request(), preview = cli("preview", input);
  assert.equal(preview.status, 0, preview.stderr);
  const approved = JSON.parse(preview.stdout);
  const bad = cli("apply", input);
  assert.equal(bad.status, 1);
  assert.match(JSON.parse(bad.stderr).error, /approval/);
  const applied = cli("apply", { ...input, approval: approved.approval });
  assert.equal(applied.status, 0, applied.stderr);
  assert.equal(JSON.parse(applied.stdout).saved.length, 4);
});
