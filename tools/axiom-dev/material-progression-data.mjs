import { parse, decode, patch } from "./syntax.mjs";
import { deposits } from "./deposit-studio.mjs";

const field = (node, key) => node?.children?.find(child => child.key === key);
const scalar = node => node?.raw === undefined ? undefined : decode(node.raw);
const integer = node => {
  const value = Number(node?.raw);
  if (!/^\d+$/.test(node?.raw ?? "") || !Number.isInteger(value) || value < 0 || value > 255) throw new Error("Mining levels must be whole numbers from 0 to 255.");
  return value;
};
const unique = (rows, key, label) => {
  const values = rows.map(row => row[key]);
  if (values.some(value => typeof value !== "string" || !value) || new Set(values).size !== values.length) throw new Error("Missing or duplicate " + label + ".");
  return rows;
};
export const progressionScope = "Harvest-tier comparison only. Assumes an unbroken, unmodified tool with this material in every mining-level part. Mixed materials, modifiers, break speed, acquisition routes, special block behavior and world generation are not simulated.";

export function materialRows(text) {
  return unique(parse(text, "ron").children.map(node => ({ id: scalar(field(node, "id")),
    name: scalar(field(node, "display_name")), miningLevel: integer(field(field(node, "props"), "mining_level")), node })), "id", "material ID");
}

export function miningSchemas(text) {
  const rows = field(parse(text, "ron"), "schemas");
  if (rows?.opener !== "[") throw new Error("Native tool schemas are unavailable.");
  return unique(rows.children.map(node => {
    const mining = field(field(node, "capabilities"), "mining");
    if (!mining || !["Some", "None"].includes(mining.prefix ?? mining.raw)) throw new Error("Tool mining capabilities changed shape.");
    const miningClass = mining.prefix === "Some" ? scalar(field(mining.children[0], "class")) : null;
    if (miningClass !== null && !["Dig", "Cut", "Scoop"].includes(miningClass)) throw new Error("Unknown native mining class.");
    const parts = field(node, "parts");
    if (parts?.opener !== "[") throw new Error("Tool parts changed shape.");
    const levelParts = parts.children.filter(part => field(field(part, "weights"), "mining_level")?.raw === "true")
      .map(part => scalar(field(part, "kind")));
    return { id: scalar(field(node, "id")), name: scalar(field(node, "display_name")), miningClass, levelParts };
  }), "id", "schema ID");
}

export function harvestGates(miningText, blockText) {
  const blocks = new Map(parse(blockText, "ron").children.map(row => [scalar(field(row, "id")), scalar(field(row, "display_name"))]));
  const rules = field(parse(miningText, "ron"), "blocks");
  if (rules?.opener !== "[") throw new Error("Native mining block rules changed shape.");
  const gates = new Map();
  for (const rule of rules.children) {
    const miningClass = scalar(field(rule, "class")), tier = integer(field(rule, "tier")), list = field(rule, "blocks");
    if (!["Dig", "Cut", "Scoop"].includes(miningClass) || list?.opener !== "[") throw new Error("Invalid mining block rule.");
    for (const node of list.children) {
      const block = scalar(node);
      if (!blocks.has(block) || gates.has(block)) throw new Error("Unknown or duplicate mining block: " + block);
      gates.set(block, { block, name: blocks.get(block), miningClass, tier, source: "assets/mining.ron" });
    }
  }
  for (const deposit of deposits) for (const item of deposit.items) {
    const block = /^Block\((\w+)\)$/.exec(item)?.[1];
    if (!block) continue;
    if (!blocks.has(block)) throw new Error("Forge deposit names an unknown block: " + block);
    if (!gates.has(block)) gates.set(block, { block, name: blocks.get(block), miningClass: "Dig", tier: 0, source: "native default (Dig, tier 0)" });
    gates.get(block).deposit = deposit.label;
  }
  return [...gates.values()].sort((a, b) => a.tier - b.tier || a.block.localeCompare(b.block));
}

export function compareHarvest(gates, schema, before, after = before) {
  const eligibility = (gate, level) => (schema.miningClass === gate.miningClass && schema.levelParts.length ? level : 0) >= gate.tier;
  const rows = gates.map(gate => ({ ...gate, before: eligibility(gate, before), after: eligibility(gate, after) }));
  return { schema, materialAffectsMiningLevel: Boolean(schema.miningClass && schema.levelParts.length), beforeLevel: before, afterLevel: after,
    gained: rows.filter(row => !row.before && row.after), lost: rows.filter(row => row.before && !row.after),
    eligible: rows.filter(row => row.after), blocked: rows.filter(row => !row.after), scope: progressionScope };
}

export function reviseMiningLevel(text, materialId, miningLevel) {
  if (!Number.isInteger(miningLevel) || miningLevel < 0 || miningLevel > 255) throw new Error("Mining level must be a whole number from 0 to 255.");
  const row = materialRows(text).find(row => row.id === materialId);
  if (!row) throw new Error("Choose an existing material ID from inspect.");
  const node = field(field(row.node, "props"), "mining_level");
  return { text: row.miningLevel === miningLevel ? text : patch(text, node.start, node.end, String(miningLevel)),
    material: { id: row.id, name: row.name }, before: row.miningLevel, after: miningLevel };
}
