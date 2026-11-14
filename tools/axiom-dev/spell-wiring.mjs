import { parse, patch } from "./syntax.mjs";

export const spellPath = "assets/spells.ron";

// The editor and command line build the same set blocks from this schema.
export const spellFields = [
  { field: "cost", label: "Base mana", group: "Cast", min: 0 },
  { field: "cooldown", label: "Cooldown (s)", group: "Cast", min: 0 },
  { field: "cast_delay", label: "Cast delay adjustment (s)", group: "Cast" },
  { field: "recharge", label: "Recharge adjustment (s)", group: "Cast" },
  { field: "spread", label: "Spread adjustment (°)", group: "Cast" },
  { field: "damage", label: "Base damage", group: "Delivery", min: 0 },
  { field: "speed", label: "Base speed (m/s)", group: "Delivery", positive: true },
  { field: "palette", label: "Gear category", group: "Library", choice: true },
  { field: "unlock", label: "Unlock", group: "Library", choice: true },
];

export function readSpells(text) {
  const root = parse(text, "ron");
  if (root.opener !== "[") throw new Error("Expected a spell table.");
  const ids = new Set();
  return root.children.map((row, index) => {
    const nodes = new Map(row.children?.map(node => [node.key, node]));
    const id = Number(nodes.get("id")?.raw);
    if (!Number.isSafeInteger(id) || id < 0 || ids.has(id)) throw new Error("Invalid or duplicate spell ID.");
    ids.add(id);
    const values = Object.fromEntries(spellFields.map(({ field, choice }) => {
      const raw = nodes.get(field)?.raw;
      const value = choice ? raw : Number(raw);
      if (choice ? !/^[A-Za-z]+$/.test(raw ?? "") : !Number.isFinite(value)) {
        throw new Error("Invalid " + field + " in spell " + id + ".");
      }
      return [field, value];
    }));
    return { id, index, name: JSON.parse(nodes.get("name").raw), class: nodes.get("class").raw, values, nodes };
  });
}

export function spellChoices(rows, field, id) {
  const hidden = field === "palette" ? "Hidden" : "Never";
  return [...new Set(rows.map(row => row.values[field]))]
    .filter(value => value !== hidden && (value !== "RareBit" || id < 32));
}

export function spellSummary({ id, name, class: kind, values }) {
  return { id, name, class: kind, ...values };
}

export function previewSpellBlocks(text, id, blocks) {
  const rows = readSpells(text);
  const spell = rows.find(row => row.id === id);
  if (!spell || spell.values.palette === "Hidden") throw new Error("Choose a spell ID from the spells command.");
  if (!Array.isArray(blocks) || !blocks.length || blocks.length > 32) throw new Error("Add 1–32 action blocks.");
  const values = { ...spell.values };
  for (const [index, block] of blocks.entries()) {
    const spec = spellFields.find(field => field.field === block?.field);
    const value = block?.value;
    const valid = spec?.choice
      ? spellChoices(rows, spec.field, id).includes(value)
      : typeof value === "number" && Number.isFinite(Math.fround(value))
        && (spec?.min === undefined || value >= spec.min)
        && (!spec?.positive || Math.fround(value) > 0);
    if (block?.op !== "set" || !spec || !valid) {
      throw new Error("Invalid spell field or value in block " + (index + 1) + ".");
    }
    values[spec.field] = value;
  }
  const changes = spellFields.filter(({ field }) => values[field] !== spell.values[field])
    .map(({ field, choice }) => {
      const value = values[field];
      const literal = String(value);
      return { node: spell.nodes.get(field), literal: choice || /[.e]/i.test(literal) ? literal : literal + ".0" };
    })
    .sort((a, b) => b.node.start - a.node.start);
  for (const { node, literal } of changes) text = patch(text, node.start, node.end, literal);
  return { text, before: spellSummary(spell), after: spellSummary({ ...spell, values }) };
}
