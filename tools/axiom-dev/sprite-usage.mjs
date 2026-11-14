import { recipeId } from "./craft-index.mjs";
import { readSheet, masksPath } from "./sprite-sheet.mjs";
import { field, textOf } from "./idea-data.mjs";
import { address } from "./targets.mjs";

const blank = () => ({ users: [], related: [], produces: [], consumes: [] });
const usageAt = (index, key) => {
  if (!index.has(key)) index.set(key, blank());
  return index.get(key);
};

export function spriteUsage(items, artwork, craft) {
  const index = new Map();
  for (const item of items) {
    const sprite = artwork?.sprite(item.product);
    const sources = sprite ? [sprite] : artwork?.parts?.(item.product) ?? [];
    for (const part of sources) {
      const usage = usageAt(index, part.path + "#" + part.index);
      if (!usage.users.includes(item)) usage.users.push(item);
    }
  }
  for (const usage of index.values()) {
    for (const [field, routes] of [["produces", craft.producers], ["consumes", craft.consumers]]) {
      const unique = new Map();
      for (const item of usage.users) for (const recipe of routes.get(item.product) ?? []) {
        unique.set(recipeId(recipe), recipe);
      }
      usage[field] = [...unique.values()];
    }
  }
  return index;
}

export const emptyUsage = Object.freeze(blank());

export async function extendSpriteUsage(ctx, index, items) {
  const effectPath = "crates/app/src/world/progression/effects.rs";
  const effectSheetPath = "assets/icons/effects16.txt";
  const partsPath = "assets/tools/parts.ron";
  const [effects, icons, masks, parts, foods] = await Promise.all([
    ctx.load(effectPath), ctx.load(effectSheetPath), ctx.load(masksPath), ctx.load(partsPath), ctx.load("assets/foods.ron"),
  ]);
  const sheet = readSheet(icons.text, effectSheetPath);
  const enumText = /pub enum Kind\s*\{([^}]+)\}/.exec(effects.text);
  const effectsById = new Map([...(enumText?.[1] ?? "").matchAll(/(\w+)\s*=\s*(\d+)/g)]
    .map(match => [Number(match[2]), match[1]]));
  const foodItems = new Map(items.map(item => [item.product, item]));
  for (const [i, entry] of sheet.entries.entries()) {
    const id = Number(entry.id.split(" · ")[0]);
    const effect = effectsById.get(id);
    if (!effect) continue;
    const declaration = new RegExp("\\b" + effect + "\\s*=\\s*" + id + "\\b").exec(effects.text);
    const usage = usageAt(index, effectSheetPath + "#" + i);
    usage.users.push({ kind: "file", path: effectPath, line: effects.text.slice(0, declaration.index).split("\n").length,
      label: effect.replace(/([a-z])([A-Z])/g, "$1 $2") + " status effect", detail: "Badge #" + id });
    for (const food of ctx.tree(foods).children) {
      if (!(field(food, "effects")?.children ?? []).some(row => textOf(field(row, "kind")) === effect)) continue;
      const item = foodItems.get("Food(" + textOf(field(food, "kind")) + ")");
      if (item) usage.related.push(item);
    }
  }
  const maskSheet = readSheet(masks.text, masksPath);
  const root = ctx.tree(parts), schemas = field(root, "schemas");
  for (const [i, entry] of maskSheet.entries.entries()) {
    const usage = usageAt(index, masksPath + "#" + i);
    for (const tool of maskSheet.tools.filter(tool => tool.layers.includes(entry.name))) {
      const schemaIndex = schemas.children.findIndex(row => textOf(field(row, "id")) === tool.name);
      if (schemaIndex < 0) continue;
      const row = schemas.children[schemaIndex];
      usage.users.push({ kind: "file", path: partsPath, parts: address(root, [root.children.indexOf(schemas), schemaIndex]),
        label: textOf(field(row, "display_name")) || tool.name, detail: "Composed tool schema" });
    }
  }
}
