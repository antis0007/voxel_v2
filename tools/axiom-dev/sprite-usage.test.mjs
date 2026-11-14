import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { spriteUsage, extendSpriteUsage } from "./sprite-usage.mjs";
import { craftingIndex } from "./craft-index.mjs";
import { itemRows, recipeRows, machineAssemblyRows, fabricationRecipes } from "./catalog-data.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";
import { parse } from "./syntax.mjs";
import { readSheet, masksPath, paintInk, artSource, rgba } from "./sprite-sheet.mjs";
import { paletteCandidates, varyColor, adjustColor } from "./sprite-color.mjs";

test("shared art traces recipes once while keeping independent sprites separate", () => {
  const items = ["a", "b", "c"].map(product => ({ product }));
  const recipe = { path: "recipes", parts: [0], product: "a", count: 1,
    inputs: [{ code: "a", count: 1 }, { code: "b", count: 2 }] };
  const art = { sprite: code => ({ path: "icons", index: code === "c" ? 2 : 1 }) };
  const index = spriteUsage(items, art, craftingIndex([recipe]));
  assert.deepEqual(index.get("icons#1").users, items.slice(0, 2));
  assert.deepEqual(index.get("icons#1").produces, [recipe]);
  assert.deepEqual(index.get("icons#1").consumes, [recipe]);
  assert.deepEqual(index.get("icons#2").consumes, []);
  assert.equal(spriteUsage(items, null, craftingIndex([])).size, 0);
});

test("native tool precedence, effect consumers and mask schemas have real sources", async () => {
  const ctx = {
    load: async path => ({ path, text: await readFile(new URL("../../" + path, import.meta.url), "utf8") }),
    tree: doc => parse(doc.text, "ron"),
  };
  const recipes = await ctx.load("assets/recipes.ron");
  const items = await itemRows(ctx, recipes), artwork = await recipeArtwork(ctx);
  for (const code of ["Tool(6)", "Tool(7)", "Tool(8)", "Weapon(1)"]) {
    assert.equal(artwork.sprite(code), undefined, "Native composition precedes legacy item art");
    assert.ok(artwork.parts(code).length >= 3);
  }
  const index = spriteUsage(items, artwork, craftingIndex(recipeRows(ctx, recipes)));
  await extendSpriteUsage(ctx, index, items);
  const masks = readSheet((await ctx.load(masksPath)).text, masksPath);
  const head = index.get(masksPath + "#" + masks.entries.findIndex(entry => entry.name === "pick_head"));
  assert.ok(head.users.some(item => item.product === "Tool(6)"));
  assert.ok(head.users.some(item => item.detail === "Composed tool schema" && item.path === "assets/tools/parts.ron"));
  const effectSheet = "assets/icons/effects16.txt";
  const effects = readSheet((await ctx.load(effectSheet)).text, effectSheet);
  const regen = index.get(effectSheet + "#" + effects.entries.findIndex(entry => entry.name === "regeneration"));
  assert.ok(regen.users.some(user => user.path.endsWith("effects.rs") && user.line > 1));
  assert.ok(regen.related.some(item => item.product === "Food(Trailcake)"));
});

test("paint alpha lock protects silhouette and preserves partial alpha", () => {
  assert.equal(paintInk(46, 97, true, true), 46);
  assert.equal(paintInk(97, 46, true, true), 97);
  assert.equal(paintInk(97, 98, true, true), 98);
  assert.equal(paintInk(0, rgba("ff0000"), false, true), 0);
  assert.equal(paintInk(rgba("111111", 83), rgba("ff0000"), false, true), rgba("ff0000", 83));
  assert.equal(paintInk(rgba("111111", 83), 0, false, true), rgba("111111", 83));
  assert.equal(paintInk(0, rgba("ff0000"), false, false), rgba("ff0000"));
});

test("palette exploration is reproducible and filters preserve alpha", () => {
  assert.deepEqual(paletteCandidates(17, 100), paletteCandidates(17, 100));
  assert.notDeepEqual(paletteCandidates(17, 100), paletteCandidates(18, 100));
  const options = { offset: 120, mode: "ramp", strength: .85, shadow: -28, highlight: 32, chromaLimit: .32, protectNeutrals: true };
  assert.equal(varyColor(rgba("808080"), options), rgba("808080"));
  assert.equal(varyColor(rgba("aa4400", 79), options) & 255, 79);
  assert.equal(varyColor(0, options), 0);
  for (const filter of ["None", "Grayscale", "Sepia", "Invert", "Posterize"]) {
    assert.equal(adjustColor(rgba("aa4400", 79), { hue: 20, saturation: 1, contrast: 1, filter }) & 255, 79);
  }
  assert.equal(artSource("assets/source/textures/grass.png").mode, "source");
  assert.equal(artSource("assets/source/reference/grass.png").mode, "reference");
  assert.equal(artSource("assets/source/textures/clean_modded_candidate/kit.png").mode, "generated");
  assert.equal(artSource("assets/source/textures/clean_modded_candidate/comparison_current_left_candidate_right.png").mode, "reference");
});

test("actual bread icon resolves to food definition and listed recipe", async () => {
  const ctx = {
    load: async path => ({ path, text: await readFile(new URL("../../" + path, import.meta.url), "utf8") }),
    tree: doc => parse(doc.text, "ron"),
  };
  const recipes = await ctx.load("assets/recipes.ron");
  const machines = await ctx.load("assets/machines.ron");
  const items = await itemRows(ctx, recipes);
  const artwork = await recipeArtwork(ctx);
  const fabrication = await ctx.load("assets/fabrication.ron");
  const processes = fabricationRecipes(ctx, fabrication);
  const craft = craftingIndex([...recipeRows(ctx, recipes), ...machineAssemblyRows(ctx, machines), ...processes]);
  const bread = items.find(item => item.product === "Food(Bread)");
  assert.equal(bread.path, "assets/foods.ron");
  const sprite = artwork.sprite(bread.product);
  const usage = spriteUsage(items, artwork, craft).get(sprite.path + "#" + sprite.index);
  assert.ok(usage.users.includes(bread));
  assert.ok(usage.produces.some(recipe => recipe.product === bread.product));
  const usageIndex = spriteUsage(items, artwork, craft);
  for (const process of processes) {
    const icon = artwork.sprite(process.product);
    if (icon) assert.ok(usageIndex.get(icon.path + "#" + icon.index).produces.includes(process));
  }
  for (const row of items) {
    const icon = artwork.sprite(row.product);
    if (!icon) continue;
    const entry = spriteUsage([row], artwork, craft).get(icon.path + "#" + icon.index);
    assert.ok(entry.produces.every(recipe => recipe.product === row.product));
    assert.ok(entry.consumes.every(recipe => recipe.inputs.some(input => input.code === row.product && input.count > 0)));
  }
});
