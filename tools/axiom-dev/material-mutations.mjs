import { addEntry, parse, patch, quoted } from "./syntax.mjs";
import { field, textOf } from "./idea-data.mjs";
import { materialProperties } from "./material-properties.mjs";

const idOf = row => textOf(field(row, "id"));
// sim_traits.rs asserts that the block count stays below the NONE sentinel (255).
const maxBlocks = 254;
const nodeAt = (row, path) => path.split(".").reduce(field, row);
const decimal = value => Number.isInteger(value) ? value + ".0" : String(value);
const genericSimulation = [
  "sim: (", "    cohesion: 1.0,", "    thermal: (", "        conductivity: 1.0,",
  "        capacity: 800.0,", "        melt: None,", "        boil: None,", "        ignite: None,",
  "        cool: None,", "        fuel: 0.0,", "    ),", "    electric_conductivity: 0.0,",
  "    melts_into: None,", "    boils_into: None,", "    burns_into: None,", "    cools_into: None,", "),",
];
const renderValue = (rule, value) => {
  if (rule.type === "string" && typeof value === "string" && value.trim() && value.trim().length <= 80
      && !/[\x00-\x1f]/.test(value)) return quoted(value.trim(), "ron");
  if (rule.type === "boolean" && typeof value === "boolean") return String(value);
  if (rule.type === "number" && Number.isFinite(value) && value >= rule.min && value <= rule.max) return decimal(value);
  if (rule.type === "color" && Array.isArray(value) && value.length === 3
      && value.every(channel => Number.isFinite(channel) && channel >= 0 && channel <= 1)) {
    return "(" + value.map(decimal).join(", ") + ")";
  }
  throw new Error("Invalid " + rule.label + (rule.type === "number" ? " · use " + rule.min + "–" + rule.max : ""));
};

function writeField(text, row, path, raw) {
  const existing = nodeAt(row, path);
  if (existing) return patch(text, existing.start, existing.end, raw);
  if (path.startsWith("sim.") && !field(row, "sim") && field(row, "tags")) {
    const expanded = insertBefore(text, field(row, "tags"), genericSimulation);
    return writeField(expanded, parse(expanded, "ron"), path, raw);
  }
  if (path === "sim.thermal.fusion" && nodeAt(row, "sim.thermal.melt")) {
    return insertBefore(text, nodeAt(row, "sim.thermal.melt"), ["fusion: " + raw + ","]);
  }
  const keys = path.split(".");
  let parent = row;
  while (keys.length > 1 && field(parent, keys[0])) parent = field(parent, keys.shift());
  const key = keys.shift();
  const value = keys.reduceRight((inner, part) => "(" + part + ": " + inner + ")", raw);
  return addEntry(text, parent, key + ": " + value);
}

function insertBefore(text, node, lines) {
  const start = text.lastIndexOf("\n", node.entry - 1) + 1;
  const indent = text.slice(start, node.entry);
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const separator = /^[ \t]*$/.test(indent) ? eol + indent : " ";
  return patch(text, node.entry, node.entry, lines.join(separator) + separator);
}

function valuesOf(row) {
  return Object.fromEntries(Object.entries(materialProperties).map(([key, rule]) => {
    const node = nodeAt(row, key);
    const value = !node ? rule.fallback : rule.type === "number" ? Number(textOf(node).replaceAll("_", ""))
      : rule.type === "boolean" ? node.raw === "true" : rule.type === "color" ? node.children.map(channel => Number(textOf(channel)))
        : textOf(node);
    return [key, value];
  }));
}

export function materialSnapshot(text, id) {
  const rows = parse(text, "ron").children;
  const row = rows.find(candidate => idOf(candidate) === id);
  if (!row) throw new Error("Material is missing · " + id);
  return { id, values: valuesOf(row), index: rows.indexOf(row), state: textOf(nodeAt(row, "physics.matter_state")),
    render: textOf(nodeAt(row, "render.kind")), slotsLeft: Math.max(0, maxBlocks - rows.length),
    line: text.slice(0, row.start).split("\n").length };
}

export function changeMaterial(text, id, changes) {
  if (!changes || typeof changes !== "object" || Array.isArray(changes)) throw new Error("Expected material properties");
  const root = parse(text, "ron");
  const index = root.children.findIndex(row => idOf(row) === id);
  if (index < 0) throw new Error("Material is missing · " + id);
  let rowText = text.slice(root.children[index].start, root.children[index].end);
  for (const [key, value] of Object.entries(changes)) {
    if (!Object.hasOwn(materialProperties, key)) throw new Error("Property is unavailable · " + key);
    const row = parse(rowText, "ron");
    const current = valuesOf(row)[key];
    if (JSON.stringify(current) === JSON.stringify(value)) continue;
    rowText = writeField(rowText, row, key, renderValue(materialProperties[key], value));
  }
  const result = parse(rowText, "ron"), values = valuesOf(result);
  const density = Math.round(Math.fround(Math.fround(values["physics.density"]) * 100));
  const capacity = Math.round(Math.fround(values["sim.thermal.capacity"]));
  if (Math.floor(density * capacity / 1024) > Math.floor(0xffffffff / 64)) {
    throw new Error("Density and specific heat together exceed the simulation limit.");
  }
  if (textOf(nodeAt(result, "physics.matter_state")) === "Liquid" && !(values["physics.fluid.viscosity"] > 0)) {
    throw new Error("Liquid viscosity must be above zero.");
  }
  const row = root.children[index];
  return patch(text, row.start, row.end, rowText);
}

export function appendMaterialVariant(text, registry, sourceId, id, changes) {
  if (typeof id !== "string" || !/^[A-Z][A-Za-z0-9]{1,63}$/.test(id)) throw new Error("Use a PascalCase block ID, such as BlueBasalt.");
  const macro = /^block_ids!\s*\{([^}]*)\}/m.exec(registry);
  if (!macro) throw new Error("The native block registry layout changed.");
  const entries = macro[1].replace(/\/\/[^\r\n]*/g, "").trim();
  if (!/^(?:[A-Z][A-Za-z0-9_]*\s*,\s*)+$/.test(entries)) throw new Error("The native block registry layout changed.");
  const ids = entries.split(",").map(value => value.trim()).filter(Boolean);
  const root = parse(text, "ron"), rows = root.children;
  if (rows.length !== ids.length || rows.some((row, i) => idOf(row) !== ids[i])) {
    throw new Error("The material table and native registry must have the same IDs in the same order.");
  }
  if (ids.includes(id)) throw new Error("That block ID already exists.");
  if (ids.length >= maxBlocks) throw new Error("The native material registry has no free block IDs.");
  const row = rows[ids.indexOf(sourceId)];
  if (!row || sourceId === "Air" || textOf(nodeAt(row, "render.kind")) === "Invisible") {
    throw new Error("Choose a visible source material.");
  }
  let clone = text.slice(row.start, row.end);
  for (const [key, raw] of [["id", id], ["tags.hotbar_order", "None"], ["tags.selectable", "true"]]) {
    clone = writeField(clone, parse(clone, "ron"), key, raw);
  }
  const eol = registry.includes("\r\n") ? "\r\n" : "\n";
  const end = macro.index + macro[0].lastIndexOf("}");
  const materials = changeMaterial(addEntry(text, root, clone), id, changes);
  return { materials, registry: patch(registry, end, end, "    " + id + "," + eol) };
}
