import { resourceCatalog } from "./workshop-planning.mjs";
import { addEntry, decode, parse, quoted } from "./syntax.mjs";
import { address } from "./targets.mjs";

export const recipePath = "assets/recipes.ron";
export const spawnPath = "assets/spawns.ron";
export const habitatRealms = [
  ["Surface", "Surface"], ["Underground", "Underground"], ["Nether", "Nether"], ["Shell", "Shell realms"],
  ["Gloaming", "Gloaming"], ["Highfold", "Highfold"],
];
export const advancementPath = "assets/advancements.ron";
export const field = (node, key) => node?.children?.find(child => child.key === key);
export const textOf = node => node?.raw === undefined ? "" : decode(node.raw);

export function zeroRecipeIngredient(root) {
  for (const table of root.children ?? []) {
    for (const recipe of table.children ?? []) {
      if (field(recipe, "pattern")?.children?.length) continue;
      const inputs = field(recipe, "ingredients")?.children ?? [];
      const index = inputs.findIndex(pair => {
        const count = pair.children?.[1]?.raw;
        return count !== undefined && Number(count.replaceAll("_", "").replace(/u32$/, "")) === 0;
      });
      if (index >= 0) return { name: textOf(field(recipe, "name")), index };
    }
  }
}

const q = value => quoted(value, "ron");
const shapeKey = items => Array.from({ length: 9 }, (_, i) => items[i]?.trim() ?? "").join("\x1f");
const setKey = items => [...new Set(items)].sort().join("\x1f");
const compact = value => value.replace(/\s+/g, "");
const nameKey = text => text.toLowerCase().replace(/[^a-z0-9]/g, "");

export async function recipeItems(ctx, recipes) {
  const [materials, foods, machines] = await Promise.all([
    ctx.load("assets/materials.ron"), ctx.load("assets/foods.ron"),
    ctx.load("assets/machines.ron"),
  ]);
  const catalog = new Map();
  const machineIds = new Set(ctx.tree(machines).children.map(row => textOf(field(row, "block"))));
  function add(code, label, limit = 64, machine = false) {
    if (code && !catalog.has(code)) catalog.set(code, { code, label, limit, machine });
  }
  for (const row of ctx.tree(materials).children) {
    const id = textOf(field(row, "id"));
    const state = textOf(field(field(row, "physics"), "matter_state"));
    if (!/^[A-Za-z][A-Za-z0-9]*$/.test(id) || /^(Air|Fire|Smoke|Steam|SnowLayer)$/.test(id) || state === "Liquid") continue;
    const limit = textOf(field(field(row, "render"), "kind")) === "Model" ? 16 : 64;
    add("Block(" + id + ")", textOf(field(row, "display_name")) || id, limit, machineIds.has(id));
  }
  for (const row of ctx.tree(foods).children) {
    add("Food(" + textOf(field(row, "kind")) + ")", textOf(field(row, "name")), 16);
  }
  resourceCatalog().forEach(({ id, name }) => add("Resource(" + id + ")", name));
  const source = recipes.text;
  for (const table of ctx.tree(recipes).children) {
    for (const row of table.children ?? []) {
      const output = field(field(row, "output"), "item");
      if (!output) continue;
      const code = compact(source.slice(output.start, output.end));
      if (!/^(?:Armor|Weapon|Tool|Bucket|Cast)\([A-Za-z0-9_,]+\)$/.test(code)) continue;
      add(code, textOf(field(row, "name")) || code, 1);
    }
  }
  const groups = new Map();
  for (const item of catalog.values()) {
    const key = item.label.toLowerCase();
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(item);
  }
  const result = [];
  for (const group of groups.values()) {
    const canonical = group.filter(item => {
      const id = /^Block\((\w+)\)$/.exec(item.code)?.[1];
      return id && nameKey(id) === nameKey(item.label);
    });
    for (const item of group) {
      const id = /^Block\((\w+)\)$/.exec(item.code)?.[1];
      const variant = id?.replace(/([a-z0-9])([A-Z])/g, "$1 $2") ?? item.code;
      const useName = group.length === 1 || (canonical.length === 1 && canonical[0] === item);
      result.push({ ...item, label: useName ? item.label : item.label + " · " + variant });
    }
  }
  return result.sort((a, b) => a.label.localeCompare(b.label));
}

export function newRecipe(doc, root, draft) {
  const { name, station, output, count, inputs, cells, description = "" } = draft;
  const table = field(root, station);
  if (!table?.children) throw new Error("This station is missing from recipes.ron");
  const width = station === "hand" ? 2 : 3;
  if (output.machine && station === "hand") throw new Error("Machine outputs need a workbench");
  if (count > output.limit) throw new Error("Use at most " + output.limit + " items per output stack");
  const rows = root.children.flatMap(group => group.children ?? []);
  if (rows.some(row => textOf(field(row, "name")).toLowerCase() === name.toLowerCase())) {
    throw new Error("A recipe named " + name + " already exists");
  }
  let requirements;
  if (cells) {
    const filled = cells.map((item, index) => item ? index : -1).filter(index => index >= 0);
    if (!filled.length) throw new Error("Place an ingredient on the grid");
    const minX = Math.min(...filled.map(i => i % 3)), minY = Math.min(...filled.map(i => Math.floor(i / 3)));
    const maxX = Math.max(...filled.map(i => i % 3)), maxY = Math.max(...filled.map(i => Math.floor(i / 3)));
    if (maxX - minX + 1 > width || maxY - minY + 1 > width) throw new Error("Use a " + width + " × " + width + " shape for this station");
    const codes = new Map();
    const pattern = [], normalized = [];
    for (let y = minY; y <= maxY; y++) {
      let row = "";
      for (let x = minX; x <= maxX; x++) {
        const item = cells[y * 3 + x];
        if (item && !codes.has(item.code)) codes.set(item.code, String.fromCharCode(65 + codes.size));
        row += item ? codes.get(item.code) : " ";
        normalized[(y - minY) * 3 + (x - minX)] = item?.code;
      }
      pattern.push(q(row));
    }
    const duplicate = rows.find(row => {
      const key = field(row, "key");
      const prior = field(row, "pattern");
      if (!key?.children || !prior?.children) return false;
      const byKey = new Map(key.children.map(entry => {
        const [symbol, item] = entry.children;
        return [symbol.raw?.slice(1, -1), compact(doc.text.slice(item.start, item.end))];
      }));
      const comparable = [];
      prior.children.forEach((line, y) => [...textOf(line)].forEach((symbol, x) => { comparable[y * 3 + x] = byKey.get(symbol); }));
      return shapeKey(comparable) === shapeKey(normalized);
    });
    if (duplicate) throw new Error("That pattern already makes " + textOf(field(duplicate, "name")));
    requirements = "pattern: [" + pattern.join(", ") + "], key: [" +
      [...codes].map(([code, key]) => "('" + key + "', " + code + ")").join(", ") + "]";
  } else {
    const distinct = new Map();
    for (const { item, amount } of inputs) {
      const previous = distinct.get(item.code);
      distinct.set(item.code, { item, amount: amount + (previous?.amount ?? 0) });
    }
    const stacks = [...distinct.values()].reduce((total, { item, amount }) => total + Math.ceil(amount / item.limit), 0);
    if (!stacks || stacks > width * width) throw new Error("Ingredients must fit in the " + width + " × " + width + " grid");
    const duplicate = rows.find(row => {
      if (field(row, "pattern")?.children?.length) return false;
      const keys = field(row, "ingredients")?.children.map(entry => compact(doc.text.slice(entry.children[0].start, entry.children[0].end)));
      return keys?.length && setKey(keys) === setKey([...distinct.keys()]);
    });
    if (duplicate) throw new Error("These ingredients already make " + textOf(field(duplicate, "name")) + ". Try a shaped recipe.");
    requirements = "ingredients: [" + [...distinct.values()].map(({ item, amount }) => "(" + item.code + ", " + amount + ")").join(", ") + "]";
  }
  const entry = "(name: " + q(name) + ", output: (item: " + output.code + ", count: " + count + "), " +
    requirements + ", description: " + q(description) + ")";
  return append(doc, table, [root.children.indexOf(table)], entry, name);
}

export function append(doc, group, trail, entry, label) {
  const text = addEntry(doc.text, group, entry);
  const parts = address(parse(text, "ron"), [...trail, group.children.length]);
  return { text, label, parts, path: doc.path, kind: "record" };
}

export function spawnRow(kind, weight, realm, time, biome) {
  return "(" + kind + ", (weight: " + weight + ", realms: [" + realm + "], time: " + time
    + (biome ? ", biomes: [" + q(biome) + "]" : "") + "))";
}

export function advancementRow(id, name, parent, trigger) {
  return "(id: " + q(id) + ", name: " + q(name) + ", parent: " + (parent ? "Some(" + q(parent) + ")" : "None")
    + ", trigger: " + trigger + ")";
}

