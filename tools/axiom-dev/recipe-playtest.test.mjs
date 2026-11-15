import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { foodPaths } from "./food-item.mjs";
import { recipePath } from "./idea-data.mjs";

// Exercise the actual UI handler without booting a browser, server, compiler or game.
const source = readFileSync(process.env.AXIOM_RECIPE_PLAYTEST_SOURCE ?? new URL("./app.mjs", import.meta.url), "utf8");
const start = source.indexOf("async function playRecipe(recipe) {");
const end = source.indexOf("\nfunction syncDraftCount()", start);
assert(start >= 0 && end > start, "native recipe handoff handler must be present");
const handler = source.slice(start, end);
const dirtySource = source.match(/function dirty\(doc\) \{[^\n]+\}/)?.[0];
assert(dirtySource, "use the same dirty-state predicate as the app");
const doc = (path, changed = false) => ({ path, original: "saved", text: changed ? "draft" : "saved" });

async function attempt(documents, sprites = [], failLoad = false) {
  const recipe = documents.find(row => row.path === recipePath) ?? doc(recipePath, true);
  const calls = [], notes = [], tabs = [];
  const context = vm.createContext({ foodPaths, drafts: new Map(documents.map(row => [row.path, row])),
    recipePath, editableSprite: path => sprites.includes(path),
    loadDocument: async () => { if (failLoad) throw new Error("recipe unavailable"); return recipe; },
    api: async (...args) => { calls.push(JSON.parse(JSON.stringify(args))); },
    note: (...args) => notes.push(args), selectTab: tab => tabs.push(tab) });
  vm.runInContext(dirtySource + "\n" + handler + "\nthis.run = playRecipe;", context);
  await context.run({ label: "Herb Bread" });
  return { calls, notes, tabs, recipe };
}

for (const path of foodPaths) {
  test("unsaved food source blocks native build: " + path, async () => {
    const result = await attempt([doc(path, true)]);
    assert.equal(result.calls.length, 0, "never launch a stale saved definition");
    assert.equal(result.tabs.length, 0);
    assert.equal(result.notes.length, 1);
    assert.match(result.notes[0][0], /save food changes.*Open Drafts/);
    assert.equal(result.notes[0][1], true);
  });
}

test("existing sprite draft guard still blocks bitmap-only edits", async () => {
  const image = { ...doc("assets/custom.png"), spriteBitmap: { changes: true } };
  const result = await attempt([image], [image.path]);
  assert.equal(result.calls.length, 0);
  assert.match(result.notes[0][0], /save the sprite draft/);
});

test("recipe draft remains the sole intentionally unsaved native overlay", async () => {
  const recipe = doc(recipePath, true);
  const result = await attempt([recipe, ...foodPaths.map(path => doc(path))]);
  assert.deepEqual(result.calls, [["/api/start-recipe", null, { text: "draft", rebuild: true, recipe: "Herb Bread" }]]);
  assert.deepEqual(result.tabs, ["playtest"]);
  assert.equal(result.notes.length, 0);
  assert.equal(recipe.text, "draft", "playtest must not save or erase the recipe draft");
});

test("saving or undoing food edits unblocks the handoff without blocking unrelated notes", async () => {
  const food = doc(foodPaths[1], true);
  assert.equal((await attempt([food])).calls.length, 0);
  food.text = food.original;
  const result = await attempt([food, doc("docs/personal-note.md", true)]);
  assert.equal(result.calls.length, 1);
  assert.equal(result.notes.length, 0);
});

test("failed recipe loading cannot launch a build", async () => {
  const result = await attempt([], [], true);
  assert.equal(result.calls.length, 0);
  assert.equal(result.notes[0][0], "recipe unavailable");
});
