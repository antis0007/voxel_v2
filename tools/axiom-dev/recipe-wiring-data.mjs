import { field, textOf } from "./idea-data.mjs";
import { recipeStack } from "./catalog-data.mjs";
import { resolve, address } from "./targets.mjs";
import { addEntry, patch, quoted } from "./syntax.mjs";
import { applyRecipeBlocks } from "./wiring-commands.mjs";

const codeAt = (doc, node) => node ? recipeStack(doc, node) : "";

export function readRecipeWiring(ctx, doc, spec) {
  const root = ctx.tree(doc);
  const found = resolve(root, spec.parts);
  const node = found?.node, output = field(node, "output");
  const outputItem = field(output, "item"), outputCount = field(output, "count");
  if (!outputItem || !outputCount || found.trail.length !== 2) throw new Error("Select a crafting recipe");
  const pattern = field(node, "pattern"), key = field(node, "key"), ingredients = field(node, "ingredients");
  const shaped = Boolean(pattern?.children.length);
  const symbols = new Map((key?.children ?? []).map(pair =>
    [pair.children[0].raw.slice(1, -1), codeAt(doc, pair.children[1])]));
  const slots = shaped ? Array(9).fill(null) : (ingredients?.children ?? []).map(pair => ({
    code: codeAt(doc, pair.children[0]), count: Number(textOf(pair.children[1])),
  }));
  if (shaped) pattern.children.slice(0, 3).forEach((line, y) => [...textOf(line)].slice(0, 3).forEach((symbol, x) => {
    const code = symbols.get(symbol);
    if (code) slots[y * 3 + x] = { code, count: 1, symbol };
  }));
  const table = root.children[found.trail[0]].key;
  const bounds = extents(slots);
  const gridSize = table === "hand" && (!shaped || (bounds.width - bounds.x <= 2 && bounds.height - bounds.y <= 2)) ? 2 : 3;
  return { root, node, trail: found.trail, pattern, key, ingredients, outputItem, outputCount, shaped, slots, symbols, gridSize,
    product: codeAt(doc, outputItem), count: Number(textOf(outputCount)),
    name: textOf(field(node, "name")), nameField: field(node, "name"),
    description: textOf(field(node, "description")), descriptionField: field(node, "description"),
    table };
}

function extents(slots) {
  const used = slots.flatMap((slot, i) => slot ? [i] : []);
  return {
    width: Math.max(0, ...used.map(i => i % 3 + 1)),
    height: Math.max(0, ...used.map(i => Math.floor(i / 3) + 1)),
    x: Math.min(3, ...used.map(i => i % 3)), y: Math.min(3, ...used.map(i => Math.floor(i / 3))),
  };
}

function signature(slots, shaped) {
  if (!shaped) return slots.filter(Boolean).map(slot => slot.code).sort().join("|");
  const { width, height, x, y } = extents(slots);
  const rows = [];
  for (let r = y; r < height; r++) {
    rows.push(Array.from({ length: width - x }, (_, i) => slots[r * 3 + x + i]?.code ?? ""));
  }
  const normal = JSON.stringify(rows), mirror = JSON.stringify(rows.map(row => [...row].reverse()));
  return normal < mirror ? normal : mirror;
}

function rowSignature(doc, row) {
  const pattern = field(row, "pattern")?.children ?? [];
  if (!pattern.length) return { shaped: false, value: signature((field(row, "ingredients")?.children ?? [])
    .map(pair => ({ code: codeAt(doc, pair.children[0]) })), false) };
  const keys = new Map((field(row, "key")?.children ?? []).map(pair =>
    [pair.children[0].raw.slice(1, -1), codeAt(doc, pair.children[1])]));
  const slots = Array(9).fill(null);
  pattern.slice(0, 3).forEach((line, y) => [...textOf(line)].slice(0, 3).forEach((char, x) => {
    if (keys.has(char)) slots[y * 3 + x] = { code: keys.get(char) };
  }));
  return { shaped: true, value: signature(slots, true) };
}

function shapeFields(state, slots) {
  const { width, height, x: left, y: top } = extents(slots);
  const occupied = slots.filter(Boolean);
  const keys = new Map();
  // Reserve surviving keys first so one changed cell never steals a shared key.
  for (const slot of occupied) if (state.symbols.get(slot.symbol) === slot.code) keys.set(slot.symbol, slot.code);
  const codes = new Map([...keys].map(([symbol, code]) => [code, symbol]));
  function symbolFor(slot) {
    if (keys.get(slot.symbol) === slot.code) return slot.symbol;
    if (!codes.has(slot.code)) {
      const symbol = [..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"].find(value => !keys.has(value));
      keys.set(symbol, slot.code);
      codes.set(slot.code, symbol);
    }
    return codes.get(slot.code);
  }
  const rows = Array.from({ length: height - top }, (_, y) => Array.from({ length: width - left }, (_, x) => {
    const slot = slots[(y + top) * 3 + x + left];
    return slot ? symbolFor(slot) : " ";
  }).join(""));
  return { rows, keys };
}

export function writeRecipeWiring(ctx, doc, state, draft, items) {
  const occupied = draft.slots.filter(Boolean);
  const name = draft.name.trim();
  if (!name || name.length > 120) throw new Error("Recipe name must contain 1–120 characters");
  if (!occupied.length) throw new Error("A recipe needs at least one input");
  const quantity = (count, limit, label) => {
    if (!Number.isSafeInteger(count) || count < 1 || count > limit) throw new Error(label + " must be 1–" + limit);
  };
  const known = (code, old) => items.has(code) || code === old;
  if (!known(draft.product, state.product)) throw new Error("Select a known output item");
  if (draft.product !== state.product || draft.count !== state.count) {
    quantity(draft.count, items.get(draft.product)?.limit ?? 64, "Output count");
  }
  if (draft.product !== state.product && state.gridSize === 2 && items.get(draft.product)?.machine) {
    throw new Error("Machine outputs need a workbench.");
  }
  const changedSlots = draft.slots.some((slot, i) =>
    slot?.code !== state.slots[i]?.code || slot?.count !== state.slots[i]?.count) || draft.slots.length !== state.slots.length;
  if (changedSlots) {
    for (const slot of occupied) {
      if (!items.has(slot.code) && !state.slots.some(old => old?.code === slot.code)) throw new Error("Select a known ingredient");
      quantity(slot.count, 16384, "Ingredient count");
    }
    if (state.shaped) {
      const { width, height, x, y } = extents(draft.slots);
      if (width - x > state.gridSize || height - y > state.gridSize) {
        throw new Error("The pattern must fit this " + state.gridSize + " × " + state.gridSize + " station.");
      }
    } else {
      if (new Set(occupied.map(slot => slot.code)).size !== occupied.length) throw new Error("That item is already connected. Increase its count instead.");
      const slots = values => values.reduce((sum, slot) => sum + Math.ceil(slot.count / (items.get(slot.code)?.limit ?? 64)), 0);
      const budget = Math.min(9, Math.max(state.gridSize ** 2, slots(state.slots.filter(Boolean))));
      if (slots(occupied) > budget) throw new Error("Ingredients exceed " + budget + " crafting slots.");
    }
  }
  const before = signature(state.slots, state.shaped);
  const after = signature(draft.slots, state.shaped);
  for (const row of state.root.children.flatMap(group => group.children ?? [])) {
    if (row === state.node) continue;
    const label = textOf(field(row, "name"));
    if (name !== state.name && label.toLowerCase() === name.toLowerCase()) throw new Error("Recipe name already used");
    if (before === after) continue;
    const comparable = rowSignature(doc, row);
    if (comparable.shaped === state.shaped && comparable.value === after) throw new Error("Those inputs already make " + label);
  }
  const edits = [];
  const replace = (node, value) => { if (node) edits.push({ node, value }); };
  if (name !== state.name) replace(state.nameField, quoted(name, "ron"));
  if (draft.description !== state.description) replace(state.descriptionField, quoted(draft.description, "ron"));
  if (draft.product !== state.product) replace(state.outputItem, draft.product);
  if (draft.count !== state.count) replace(state.outputCount, String(draft.count));
  if (changedSlots) {
    if (state.shaped) {
      const next = shapeFields(state, draft.slots);
      if (state.pattern.children.length === next.rows.length) {
        state.pattern.children.forEach((row, i) => {
          if (textOf(row) !== next.rows[i]) replace(row, quoted(next.rows[i], "ron"));
        });
      } else replace(state.pattern, "[" + next.rows.map(row => quoted(row, "ron")).join(", ") + "]");
      if (state.symbols.size === next.keys.size && [...state.symbols.keys()].every(symbol => next.keys.has(symbol))) {
        for (const pair of state.key.children) {
          const symbol = pair.children[0].raw.slice(1, -1);
          const code = next.keys.get(symbol);
          if (code !== state.symbols.get(symbol)) replace(pair.children[1], code);
        }
      } else replace(state.key, "[" + [...next.keys].map(([symbol, code]) => "('" + symbol + "', " + code + ")").join(", ") + "]");
    } else if (state.ingredients.children.length === occupied.length) {
      state.ingredients.children.forEach((pair, i) => {
        const next = occupied[i], before = state.slots[i];
        if (next.code !== before.code) replace(pair.children[0], next.code);
        if (next.count !== before.count) replace(pair.children[1], String(next.count));
      });
    } else {
      replace(state.ingredients, "[" + occupied.map(slot => "(" + slot.code + ", " + slot.count + ")").join(", ") + "]");
    }
  }
  let text = edits.sort((a, b) => b.node.start - a.node.start).reduce((source, edit) =>
    patch(source, edit.node.start, edit.node.end, edit.value), doc.text);
  if (!state.descriptionField && draft.description !== state.description) {
    const parsed = ctx.parse(text, "ron");
    const [table, row] = state.trail;
    text = addEntry(text, parsed.children[table].children[row], "description: " + quoted(draft.description, "ron"));
  }
  return { text, spec: { path: doc.path, kind: "record", label: name,
    parts: address(ctx.parse(text, "ron"), state.trail) } };
}

export function previewRecipeBlocks(ctx, doc, spec, blocks, items) {
  const source = readRecipeWiring(ctx, doc, spec);
  const draft = { ...source, slots: source.slots.map(slot => slot && { ...slot }) };
  applyRecipeBlocks(draft, blocks, items);
  const next = writeRecipeWiring(ctx, doc, source, draft, items);
  const nextDoc = { ...doc, text: next.text };
  return { ...next, changed: next.text !== doc.text,
    state: readRecipeWiring(ctx, nextDoc, next.spec) };
}
