import { createHash } from "node:crypto";
import { read, history } from "./workspace.mjs";
import { inspectBundle, publishBundle } from "./agent-bundles.mjs";
import { foodPaths, foodPlan, foodTemplates, foodIconId } from "./food-item.mjs";
import { newRecipe, recipeItems, recipePath } from "./idea-data.mjs";
import { parse } from "./syntax.mjs";

export const foodNativePaths = ["crates/app/src/world/inventory/food.rs", "crates/app/src/egui_ui/icons/sprites16.rs"];
const catalogPaths = [recipePath, "assets/materials.ron", "crates/app/src/world/expedition.rs", "assets/machines.ron"];
const load = path => ({ path, ...read(path) });
const hash = value => createHash("sha256").update(JSON.stringify(value)).digest("hex");
const ctxFor = docs => ({ load: path => {
  const doc = docs.find(row => row.path === path);
  if (!doc) throw new Error("Missing workflow source: " + path);
  return doc;
}, tree: doc => parse(doc.text, doc.kind), parse });

function object(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)
      || Object.keys(value).some(key => !keys.includes(key))) throw new Error("Invalid " + label + " fields");
}

function registry(text) {
  const body = /pub enum FoodKind\s*\{([\s\S]*?)\n\}/.exec(text)?.[1];
  const rows = [...(body ?? "").matchAll(/^\s*(\w+)\s*=\s*(\d+)\s*,/gm)];
  if (!rows.length || rows.some((row, id) => Number(row[2]) !== id)
      || new Set(rows.map(row => row[1])).size !== rows.length) throw new Error("Food IDs are inconsistent");
  return rows.map(row => row[1]);
}

function sources(docs) {
  const ids = registry(docs.find(doc => doc.path === foodPaths[0]).text);
  const foods = foodTemplates(docs.find(doc => doc.path === foodPaths[1]).text);
  if (foods.length !== ids.length || ids.some(kind => foods.filter(row => row.kind === kind).length !== 1)) {
    throw new Error("Each native food ID needs exactly one food definition");
  }
  for (const row of foods) foodIconId(row.row, ids.indexOf(row.kind));
  return { ids, foods };
}

function checkedRequest(request) {
  object(request, ["draft", "recipe", "approval"], "workflow");
  object(request.draft, ["name", "template", "sprite", "hue", "hunger", "health", "saturation", "copyEffects"], "food");
  if (typeof request.draft.copyEffects !== "boolean" || !Number.isSafeInteger(request.draft.sprite)) {
    throw new Error("Food requires a sprite ID and explicit copyEffects boolean");
  }
  if (request.recipe !== undefined) {
    object(request.recipe, ["name", "station", "count", "inputs", "description"], "recipe");
    const { name, station, count, inputs, description = "" } = request.recipe;
    if (typeof name !== "string" || !name.trim() || name.trim().length > 120 || /[\x00-\x1f]/.test(name)
        || typeof station !== "string") throw new Error("Invalid recipe name or station");
    if (!Number.isSafeInteger(count) || count < 1 || count > 16) throw new Error("Food output count must be 1-16");
    if (typeof description !== "string" || description.length > 500) throw new Error("Recipe description exceeds 500 characters");
    if (!Array.isArray(inputs) || !inputs.length || inputs.length > 9) throw new Error("Use 1-9 recipe ingredients");
    for (const input of inputs) {
      object(input, ["item", "count"], "ingredient");
      if (typeof input.item !== "string" || !Number.isSafeInteger(input.count) || input.count < 1 || input.count > 16384) {
        throw new Error("Each ingredient needs a catalog item and positive whole count up to 16384");
      }
    }
  }
}

async function plan(request) {
  checkedRequest(request);
  const docs = [...foodPaths, ...foodNativePaths, ...(request.recipe ? catalogPaths : [])].map(load);
  sources(docs);
  if (!docs.find(doc => doc.path === foodNativePaths[0]).text.includes("pub icon: Option<u16>")
      || !docs.find(doc => doc.path === foodNativePaths[1]).text.includes(".icon_id()")) {
    throw new Error("Install native food icon support before publishing new foods");
  }
  const item = foodPlan(docs, request.draft);
  const changes = foodPaths.map((path, i) => ({ path, version: docs[i].version, text: item.changes[i] }));
  let recipe;
  if (request.recipe) {
    const ctx = ctxFor(docs), doc = ctx.load(recipePath);
    const items = new Map((await recipeItems(ctx, doc)).map(row => [row.code, row]));
    const output = { code: "Food(" + item.kind + ")", label: item.name, limit: 16 };
    const inputs = request.recipe.inputs.map(({ item: code, count }) => {
      if (!items.has(code)) throw new Error("Unknown ingredient: " + code);
      return { item: items.get(code), amount: count };
    });
    const draft = { ...request.recipe, name: request.recipe.name.trim(), inputs, output };
    const result = newRecipe(doc, parse(doc.text, "ron"), draft);
    changes.push({ path: doc.path, version: doc.version, text: result.text });
    recipe = { ...request.recipe, name: draft.name, output: output.code };
  }
  const revisions = docs.map(({ path, version }) => ({ path, version }));
  const approval = hash(["food-create-v1", revisions, changes]);
  const review = { approval, operation: "create", item: { id: item.id, kind: item.kind, name: item.name, iconId: item.iconId },
    recipe, paths: changes.map(change => change.path), revisions,
    provenance: { template: request.draft.template, sprite: request.draft.sprite, hue: request.draft.hue,
      copyEffects: request.draft.copyEffects, source: "Source template record and sprite remain unchanged; variant receives independent IDs" },
    behavior: { hunger: request.draft.hunger, health: request.draft.health, saturation: request.draft.saturation,
      consumption: "Ordinary food; native potion classification is not inherited" },
    runtime: "Native rebuild and playtest required; preview is source validation only",
    withdrawal: recipe ? "Recipe only; item ID, definition and icon remain allocated" : "No recipe to withdraw" };
  return { docs, changes, review };
}

function current(docs) {
  for (const doc of docs) if (read(doc.path).version !== doc.version) throw new Error("Source changed during review: " + doc.path);
}

export async function previewFoodWorkflow(request) { return (await plan(request)).review; }

export async function applyFoodWorkflow(request) {
  const next = await plan(request);
  if (request.approval !== next.review.approval) throw new Error("Preview this food workflow again; approval is missing or stale");
  current(next.docs);
  const bundle = publishBundle(next.changes, undefined, { preserveIdentity: true });
  return { ...next.review, bundleId: bundle.id, saved: bundle.saved };
}

export async function foodWorkflowCatalog() {
  const docs = [...foodPaths, ...catalogPaths].map(load);
  const { ids, foods } = sources(docs), ctx = ctxFor(docs);
  return { templates: foods.map(({ row, ...food }) => ({ ...food, id: ids.indexOf(food.kind), iconId: foodIconId(row, ids.indexOf(food.kind)) })),
    items: await recipeItems(ctx, ctx.load(recipePath)), revisions: docs.map(({ path, version }) => ({ path, version })) };
}

export function inspectFoodWorkflow(bundleId) {
  const record = inspectBundle(bundleId);
  if (!foodPaths.every(path => record.files.some(file => file.path === path))
      || record.files.some(file => ![...foodPaths, recipePath].includes(file.path))) throw new Error("Not a food creation bundle");
  const recipe = record.files.find(file => file.path === recipePath);
  return { bundleId, files: record.files, withdrawable: Boolean(recipe) && record.files.every(file => file.status === "Applied"),
    withdrawal: "Only the recipe is withdrawn. Never fully revert food creation after runtime use." };
}

function withdrawalPlan(request) {
  object(request, ["bundleId", "approval"], "withdrawal");
  const record = inspectFoodWorkflow(request.bundleId);
  if (!record.withdrawable) throw new Error("Creation sources changed, recipe already withdrawn, or no recipe exists");
  const docs = record.files.map(file => load(file.path));
  const inventory = record.files.find(file => file.path === foodPaths[0]);
  const before = registry(history(inventory.path, inventory.backupId).toString("utf8"));
  const { ids } = sources(docs);
  if (ids.length !== before.length + 1 || before.some((kind, i) => ids[i] !== kind)) throw new Error("Bundle did not append one stable food ID");
  const file = record.files.find(file => file.path === recipePath);
  const text = history(file.path, file.backupId).toString("utf8");
  const changes = [{ path: recipePath, version: file.after, text }];
  const revisions = docs.map(({ path, version }) => ({ path, version }));
  return { docs, changes, review: { operation: "withdraw-recipe", bundleId: request.bundleId,
    approval: hash(["food-withdraw-v1", request.bundleId, revisions, changes]), paths: [recipePath], revisions,
    retained: { kind: ids.at(-1), id: before.length, paths: foodPaths },
    runtime: "Rebuild required to remove the recipe from native execution; existing item saves keep their identity" } };
}

export function previewFoodWithdrawal(request) { return withdrawalPlan(request).review; }

export function withdrawFoodRecipe(request) {
  const next = withdrawalPlan(request);
  if (request.approval !== next.review.approval) throw new Error("Preview this withdrawal again; approval is missing or stale");
  current(next.docs);
  const bundle = publishBundle(next.changes);
  return { ...next.review, withdrawalBundleId: bundle.id, saved: bundle.saved };
}
