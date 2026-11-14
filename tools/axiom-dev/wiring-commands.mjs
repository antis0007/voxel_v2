import { addEntry, patch, quoted } from "./syntax.mjs";
import { field, textOf } from "./idea-data.mjs";

export const recipeBlocks = {
  set: { label: "Place ingredient", fields: ["cell", "item", "count"] },
  add: { label: "Add ingredient to a free cell", fields: ["item", "count"] },
  remove: { label: "Remove ingredient", fields: ["cell"] },
  "remove-item": { label: "Remove every occurrence of an ingredient", fields: ["item"] },
  swap: { label: "Swap or move ingredients", fields: ["cell", "destination"] },
  replace: { label: "Replace ingredient everywhere", fields: ["item", "replacement"] },
  rotate: { label: "Rotate right", fields: [], shaped: true },
  mirror: { label: "Flip left to right", fields: [], shaped: true },
  output: { label: "Set output and quantity", fields: ["item", "count"] },
  describe: { label: "Write recipe description", fields: ["text"] },
};

export function applyRecipeBlocks(draft, blocks, items) {
  if (!Array.isArray(blocks) || !blocks.length || blocks.length > 32) {
    throw new Error("Add 1–32 action blocks.");
  }
  function index(value, append = false) {
    const max = draft.shaped ? 9 : Math.min(9, draft.slots.length + Number(append));
    if (!Number.isInteger(value) || value < 1 || value > max) throw new Error("Choose a cell from 1 to " + max);
    return value - 1;
  }
  function ingredient(value) {
    if (!items.has(value)) throw new Error("Choose an item from the catalog.");
    return value;
  }
  function count(value) {
    if (!Number.isSafeInteger(value) || value < 1) throw new Error("Enter a positive whole quantity.");
    return value;
  }
  for (const [step, block] of blocks.entries()) {
    try {
      if (!block || !Object.hasOwn(recipeBlocks, block.op)) throw new Error("Unknown action.");
      if (recipeBlocks[block.op].shaped && !draft.shaped) throw new Error("This action needs a shaped recipe.");
      switch (block.op) {
        case "set": {
          const cell = index(block.cell, true);
          const previous = draft.slots[cell];
          draft.slots[cell] = { code: ingredient(block.item), count: draft.shaped ? 1 : count(block.count ?? previous?.count ?? 1),
            symbol: previous?.symbol };
          break;
        }
        case "remove": {
          const cell = index(block.cell);
          if (draft.shaped) draft.slots[cell] = null;
          else draft.slots.splice(cell, 1);
          break;
        }
        case "add": {
          const code = ingredient(block.item);
          if (draft.shaped) {
            const occupied = draft.slots.flatMap((slot, cell) => slot ? [cell] : []);
            const free = draft.slots.findIndex((slot, cell) => !slot && occupied.every(other =>
              Math.abs(other % 3 - cell % 3) < draft.gridSize
              && Math.abs(Math.floor(other / 3) - Math.floor(cell / 3)) < draft.gridSize));
            if (free < 0) throw new Error("This pattern has no free cell.");
            draft.slots[free] = { code, count: 1 };
          } else {
            const prior = draft.slots.find(slot => slot?.code === code);
            const amount = count(block.count ?? 1);
            if (prior) prior.count += amount;
            else draft.slots.push({ code, count: amount });
          }
          break;
        }
        case "remove-item":
          draft.slots = draft.shaped ? draft.slots.map(slot => slot?.code === block.item ? null : slot)
            : draft.slots.filter(slot => slot?.code !== block.item);
          break;
        case "swap": {
          const from = index(block.cell), to = index(block.destination, true);
          [draft.slots[from], draft.slots[to]] = [draft.slots[to] ?? null, draft.slots[from]];
          if (!draft.shaped) draft.slots = draft.slots.filter(Boolean);
          break;
        }
        case "replace": {
          const replacement = ingredient(block.replacement);
          if (!draft.slots.some(slot => slot?.code === block.item)) throw new Error("That ingredient is absent.");
          draft.slots = draft.slots.map(slot => slot?.code === block.item ? { ...slot, code: replacement } : slot);
          break;
        }
        case "rotate":
          draft.slots = draft.slots.map((_, cell, slots) => slots[(2 - cell % 3) * 3 + Math.floor(cell / 3)]);
          break;
        case "mirror":
          draft.slots = draft.slots.map((_, cell, slots) => slots[Math.floor(cell / 3) * 3 + 2 - cell % 3]);
          break;
        case "output":
          draft.product = ingredient(block.item);
          draft.count = count(block.count ?? 1);
          break;
        case "describe":
          if (typeof block.text !== "string" || block.text.trim().length > 500) throw new Error("Use up to 500 characters.");
          draft.description = block.text.trim();
          break;
      }
    } catch (error) {
      throw new Error("Block " + (step + 1) + " · " + error.message);
    }
  }
}

export function advancementRecords(root) {
  return root.children.map(node => ({
    id: textOf(field(node, "id")), parent: textOf(field(node, "parent")?.children?.[0]), node,
  }));
}

export function reparentAdvancement(text, records, child, parent) {
  const byId = new Map();
  for (const record of records) {
    if (!record.id) continue;
    byId.set(record.id, byId.has(record.id) ? null : record);
  }
  const row = byId.get(child);
  if (!row) throw new Error("Choose a unique advancement ID.");
  const visited = new Set([child]);
  for (let id = parent; id;) {
    if (visited.has(id)) throw new Error("Connection rejected · parent cycle");
    visited.add(id);
    const ancestor = byId.get(id);
    if (!ancestor) throw new Error("Parent chain contains a missing or ambiguous ID: " + id);
    id = ancestor.parent;
  }
  if (row.parent === parent) return text;
  const node = field(row.node, "parent");
  const raw = parent ? "Some(" + quoted(parent, "ron") + ")" : "None";
  return node ? patch(text, node.start, node.end, raw) : addEntry(text, row.node, "parent: " + raw);
}
