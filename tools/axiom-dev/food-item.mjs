import { addEntry, parse, patch, quoted } from "./syntax.mjs";
import { field, textOf } from "./idea-data.mjs";
import { readSheet, tintedSprite } from "./sprite-sheet.mjs";

export const foodPaths = ["crates/app/src/world/inventory.rs", "assets/foods.ron", "assets/icons/items16.txt"];

export function foodIconId(row, id) {
  if (!Number.isInteger(id) || id < 0 || id > 255) throw new Error("Invalid native food ID");
  const icon = field(row, "icon");
  if (!icon || icon.raw === "None") return id < 10 ? 10 + id : 50 + id;
  const value = Number(textOf(icon.children?.[0]));
  if (icon.prefix !== "Some" || icon.children?.length !== 1 || icon.children[0].kind !== "number"
      || !Number.isInteger(value) || value < 0 || value > 65535) {
    throw new Error("Invalid food icon reference");
  }
  return value;
}

export function foodTemplates(text) {
  const root = parse(text, "ron");
  return root.children.map(row => ({
    row, kind: textOf(field(row, "kind")), name: textOf(field(row, "name")),
    hunger: textOf(field(row, "hunger")), health: textOf(field(row, "health")),
    saturation: textOf(field(row, "saturation")),
  }));
}

const decimal = value => Number.isInteger(value) ? value + ".0" : String(value);

export function foodPlan(docs, draft) {
  const [inventory, foods, sprites] = foodPaths.map(path => docs.find(doc => doc.path === path)?.text);
  if ([inventory, foods, sprites].some(text => typeof text !== "string")) throw new Error("Food sources are unavailable");
  const { template, sprite, hue, hunger, health, saturation } = draft;
  const name = String(draft.name ?? "").trim();
  if (!/^[\p{L}\p{N}][\p{L}\p{N} '&()-]{0,59}$/u.test(name)) throw new Error("Enter a food name with up to 60 characters");
  const templates = foodTemplates(foods);
  if (templates.some(row => row.name.toLowerCase() === name.toLowerCase())) throw new Error("A food already uses that name");
  const base = templates.find(row => row.kind === template);
  if (!base) throw new Error("Choose a base food");
  for (const [label, value, min] of [["Hunger", hunger, 0], ["Health", health, -100], ["Saturation", saturation, 0]]) {
    if (!Number.isFinite(value) || value < min || value > 100) throw new Error(label + " must be between " + min + " and 100");
  }
  if (!Number.isFinite(hue) || Math.abs(hue) > 180) throw new Error("Hue shift must be between −180° and 180°");

  const kind = "Custom" + name.normalize("NFKD").replace(/[^a-z0-9 ]/gi, "")
    .split(/\s+/).map(part => part.charAt(0).toUpperCase() + part.slice(1)).join("");
  if (kind === "Custom") throw new Error("Include a Latin letter or number to generate the game ID");
  const variants = /pub enum FoodKind\s*\{([\s\S]*?)\n\}/.exec(inventory);
  const list = /impl FoodKind\s*\{\s*pub const ALL: \[Self; (\d+)\] = \[([\s\S]*?)\n    \];/.exec(inventory);
  if (!variants || !list) throw new Error("The FoodKind registry layout changed. Reload its source before adding an item.");
  const rows = [...variants[1].matchAll(/^\s*(\w+)\s*=\s*(\d+)\s*,/gm)];
  const names = [...list[2].matchAll(/\bSelf::(\w+)\s*,/g)].map(match => match[1]);
  const id = rows.length;
  if (rows.some((row, index) => Number(row[2]) !== index || row[1] !== names[index]) ||
      names.length !== id || Number(list[1]) !== id || id >= 256) {
    throw new Error("Food IDs are inconsistent or full");
  }
  if (names.includes(kind)) throw new Error("Choose a name that gives the food a unique game ID");
  const sheet = readSheet(sprites, foodPaths[2]);
  const icon = sheet.entries.find(row => Number(row.id.split(" · ")[0]) === sprite);
  const usedIcons = sheet.entries.map(row => Number(row.id.split(" · ")[0]));
  const iconId = Math.max(...usedIcons) + 1;
  if (!icon || usedIcons.some(value => !Number.isSafeInteger(value) || value > 65535)
      || new Set(usedIcons).size !== usedIcons.length || iconId > 65535) {
    throw new Error("Choose a valid source sprite in a sheet with unique, available icon slots");
  }

  const eol = inventory.includes("\r\n") ? "\r\n" : "\n";
  const all = list[0].replace("[Self; " + id + "]", "[Self; " + (id + 1) + "]")
    .replace(/\r?\n    \];$/, eol + "        Self::" + kind + "," + eol + "    ];");
  const enumEnd = variants.index + variants[0].lastIndexOf(eol + "}");
  let rust = patch(inventory, list.index, list.index + list[0].length, all);
  rust = patch(rust, enumEnd, enumEnd, eol + "    " + kind + " = " + id + ",");
  const effects = field(base.row, "effects");
  const effectSource = draft.copyEffects && effects ? foods.slice(effects.start, effects.end) : "[]";
  const food = ["(", "        kind: " + kind + ",", "        name: " + quoted(name, "ron") + ",",
    "        hunger: " + decimal(hunger) + ",", "        health: " + decimal(health) + ",",
    "        saturation: " + decimal(saturation) + ",", "        effects: " + effectSource + ",",
    "        icon: Some(" + iconId + "),", "    )"].join(foods.includes("\r\n") ? "\r\n" : "\n");
  const slug = kind.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  return { id, kind, name, iconId, changes: [rust, addEntry(foods, parse(foods, "ron"), food),
    tintedSprite(sprites, iconId + " " + slug, icon, hue)] };
}
