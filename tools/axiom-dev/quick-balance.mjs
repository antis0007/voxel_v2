import { el, button } from "./inspector.mjs";
import { addEntry, at, patch, quoted } from "./syntax.mjs";
import { address, resolve } from "./targets.mjs";
import { field, textOf, recipeItems, recipePath, spawnPath } from "./idea-data.mjs";
import { isAnimal } from "./habitat-editor.mjs";
import { bestiaryPath, recipeStack } from "./catalog-data.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";
import { dropTarget, itemPalette, slotArt } from "./slot-tools.mjs";

const label = (name, control) => el("label", {}, name, control);
const row = (...nodes) => el("div", { className: "idea-row" }, nodes);
const number = (value, min, max, step = "any") => el("input", {
  type: "number", min, max, step, value: String(value), required: true,
});
const muted = text => el("p", { className: "muted" }, text);
const q = text => quoted(text, "ron");
const recognizedRoles = ["Skirmisher", "Defender", "Flyer", "Rooted", "Grazer"];
const value = (input, type) => {
  if (!input.validity.valid || input.value === "") throw new Error("Enter a valid " + type);
  return Number(input.value);
};
const formatFloat = value => Number.isInteger(value) ? value + ".0" : String(value);

function apply(doc, edits) {
  return edits.sort((a, b) => b.node.start - a.node.start).reduce((text, edit) =>
    patch(text, edit.node.start, edit.node.end, edit.value), doc.text);
}

export async function balanceRecipe(ctx, spec, done) {
  try {
    const doc = await ctx.load(recipePath);
    const root = ctx.tree(doc);
    const found = resolve(root, spec.parts);
    if (!found?.node || !field(found.node, "output")) throw new Error("Choose a recipe row");
    const { node, trail } = found;
    const [items, artwork] = await Promise.all([recipeItems(ctx, doc), recipeArtwork(ctx).catch(() => null)]);
    const byCode = new Map(items.map(item => [item.code, item]));
    const byName = new Map(items.map(item => [item.label.toLowerCase(), item]));
    const choices = el("datalist", { id: "balance-items" }, items.map(item => el("option", { value: item.label })));
    const name = el("input", { type: "text", value: textOf(field(node, "name")), required: true, maxLength: 120 });
    const output = field(node, "output");
    let target;
    function setItem(input, item) {
      if (!item) return;
      input.value = item.label;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
    function itemBox(original) {
      const code = recipeStack(doc, original);
      const label = byCode.get(code)?.label ?? code;
      const input = el("input", { type: "text", list: choices.id, value: label, required: true, spellcheck: false });
      input.onfocus = () => { target = input; };
      dropTarget(input, ["item"], (_, code) => setItem(input, byCode.get(code)));
      return { input, current: () => byName.get(input.value.trim().toLowerCase()), source() {
        if (input.value === label) return doc.text.slice(original.start, original.end);
        const next = byName.get(input.value.trim().toLowerCase());
        if (!next) throw new Error("Choose an item from the suggestions");
        return next.code;
      } };
    }
    function displaySlot(item, fallback) {
      const slot = button("", () => item.input.focus(), "", "item-slot");
      function render() {
        slot.replaceChildren(slotArt(item.current(), artwork) ?? "?");
        slot.title = fallback + " · " + item.input.value;
        slot.setAttribute("aria-label", slot.title);
      }
      dropTarget(slot, ["item"], (_, code) => setItem(item.input, byCode.get(code)));
      item.input.addEventListener("input", render);
      render();
      return slot;
    }
    const product = itemBox(field(output, "item"));
    target = product.input;
    const quantity = number(textOf(field(output, "count")), 1, 16384, 1);
    const requirements = field(node, "ingredients")?.children ?? [];
    const pattern = field(node, "pattern")?.children ?? [];
    const keys = pattern.length ? field(node, "key").children : [];
    const amounts = requirements.map(pair => ({ pair, item: itemBox(pair.children[0]),
      count: number(textOf(pair.children[1]), 1, 16384, 1) }));
    const shaped = keys.map(pair => ({ pair, item: itemBox(pair.children[1]) }));
    const shape = el("div", { className: "idea-grid", "aria-label": "Current crafting pattern" });
    if (pattern.length) {
      for (let y = 0; y < 3; y++) for (let x = 0; x < 3; x++) {
        const symbol = textOf(pattern[y])[x]?.trim();
        const entry = shaped.find(({ pair }) => pair.children[0].raw.slice(1, -1) === symbol);
        const cell = entry ? displaySlot(entry.item, "Pattern " + symbol) : el("span", { className: "item-slot empty-slot" }, "·");
        shape.append(cell);
      }
    }
    const content = el("div", { className: "recipe-workbench" }, el("div", { className: "idea-fields" }, choices, label("Recipe name", name),
      el("div", { className: "recipe-machine" },
        el("section", { className: "recipe-inputs" }, el("h3", {}, "Inputs"), pattern.length ? shape : null,
          ...shaped.map(({ pair, item }) => label("Pattern " + pair.children[0].raw, item.input)),
          ...amounts.map(({ item, count }, index) => row(label("Ingredient " + (index + 1), item.input), label("Count", count)))),
        el("span", { className: "recipe-arrow", "aria-hidden": "true" }, "→"),
        el("section", { className: "recipe-output" }, el("h3", {}, "Output"), displaySlot(product, "Output"),
          label("Item", product.input), label("Count", quantity)))),
    itemPalette(items, artwork, item => setItem(target, item)));
    const original = doc.text;
    ctx.dialog("Balance recipe", [content], "Apply to draft", () => {
      if (doc.text !== original) throw new Error("The recipe draft changed. Reopen it to edit.");
      const newName = name.value.trim();
      if (!newName) throw new Error("Enter a recipe name");
      const another = root.children.some(table => table.children?.some(other =>
        other !== node && textOf(field(other, "name")).toLowerCase() === newName.toLowerCase()));
      if (another) throw new Error("That recipe name is already used");
      const used = new Set();
      const edits = [{ node: field(node, "name"), value: q(newName) },
        { node: field(output, "item"), value: product.source() },
        { node: field(output, "count"), value: String(value(quantity, "output count")) }];
      for (const { pair, item, count } of amounts) {
        const code = item.source();
        if (used.has(code.replace(/\s+/g, ""))) throw new Error("Combine counts for duplicate ingredients");
        used.add(code.replace(/\s+/g, ""));
        edits.push({ node: pair.children[0], value: code }, { node: pair.children[1], value: String(value(count, "ingredient count")) });
      }
      for (const { pair, item } of shaped) edits.push({ node: pair.children[1], value: item.source() });
      if (!ctx.edit(doc, apply(doc, edits))) return false;
      done({ ...spec, label: newName, parts: address(ctx.tree(doc), trail) }, doc);
      return true;
    });
  } catch (error) { ctx.note(error.message, true); }
}

const stats = [
  ["health", "Health", 1, 100000], ["speed", "Movement speed (m/s)", 0, 100],
  ["attack_range", "Attack range (m)", 0, 200], ["windup", "Windup (s)", 0, 60],
  ["cooldown", "Cooldown (s)", 0, 60], ["body_width", "Body width (m)", .01, 100],
  ["body_height", "Body height (m)", .01, 100], ["center_height", "Center height (m)", 0, 100],
];

export async function editCreature(ctx, spec, done, clone = false) {
  try {
    const doc = await ctx.load(bestiaryPath);
    const root = ctx.tree(doc);
    const found = resolve(root, spec.parts);
    const body = found?.node?.children?.[1];
    if (!body?.children) throw new Error("This creature entry moved");
    const animal = isAnimal(spec);
    const name = el("input", { type: "text", value: spec.label + (clone ? " variant" : ""), required: true, maxLength: 90 });
    const role = el("select", {}, recognizedRoles.map(value => el("option", { value }, value)));
    role.value = textOf(field(body, "role"));
    const inputs = stats.map(([key, title, min, max]) => ({ key, title,
      control: number(textOf(field(body, key)), min, max) }));
    const neutral = el("input", { type: "checkbox", checked: textOf(field(body, "neutral")) === "true" });
    const statField = ({ title, control }) => label(title, control);
    const content = el("div", { className: "idea-fields" },
      row(label(animal ? "Animal name" : "Creature name", name), animal ? null : label("Role", role)),
      el("div", { className: "catalog-form-grid" }, (animal ? inputs.slice(0, 2) : inputs).map(statField)),
      animal ? el("details", { className: "world-card" }, el("summary", {}, "Body and combat"),
        el("div", { className: "catalog-form-grid" }, inputs.slice(2).map(statField))) : label("Neutral until attacked", neutral),
      clone ? muted(animal ? "Uses the selected animal’s body, food, and drops. Add a habitat from its card."
        : "New species use Axiom’s generic creature body. The selected species supplies the starting stats.") : null);
    const original = doc.text;
    ctx.dialog(animal ? clone ? "New animal · " + spec.label : "Edit animal · " + spec.label
      : clone ? "New creature variant" : "Tune creature", [content], clone ? "Create draft" : "Apply to draft", () => {
      if (doc.text !== original) throw new Error("The bestiary draft changed. Reopen this creature.");
      const title = name.value.trim();
      if (!title) throw new Error("Enter a creature name");
      const normalized = value => value.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
      if (!normalized(title)) throw new Error("Enter a name with at least one letter or number");
      if (root.children.some(other => (clone || other !== found.node)
        && normalized(textOf(field(other.children[1], "name"))) === normalized(title))) {
        throw new Error("That creature name is already used");
      }
      const changes = new Map([["name", q(title)], ["role", role.value], ["neutral", String(animal && clone ? true : neutral.checked)],
        ...inputs.map(({ key, title, control }) => [key, formatFloat(value(control, title.toLowerCase()))])]);
      let text, trail;
      if (clone) {
        const ids = new Set(root.children.map(row => Number(textOf(row.children[0]))));
        let next = 1000;
        while (ids.has(next)) next++;
        const required = ["tall", "flying", "brawler"];
        const entries = [...changes, ...required.map(key => [key, field(body, key)?.raw ?? "false"])];
        for (const key of animal ? ["food", "drops", "shear", "lays", "melee"] : ["drops"]) {
          const node = field(body, key);
          if (node) entries.push([key, doc.text.slice(node.start, node.end)]);
        }
        if (animal) entries.push(["animal_body", "Some(" + spec.body + ")"]);
        text = addEntry(doc.text, root, "(" + next + ", (" + entries.map(([key, value]) => key + ": " + value).join(", ") + "))");
        trail = [root.children.length];
      } else {
        const edits = [...changes].flatMap(([key, value]) => {
          const node = field(body, key);
          return node ? [{ node, value }] : [];
        });
        text = apply(doc, edits);
        trail = found.trail;
        if (!field(body, "neutral") && neutral.checked) {
          const updated = at(ctx.parse(text, doc.kind), [...trail, 1]);
          text = addEntry(text, updated, "neutral: true");
        }
      }
      if (!ctx.edit(doc, text)) return false;
      done({ ...spec, label: title, parts: address(ctx.tree(doc), trail) }, doc);
      return true;
    });
  } catch (error) { ctx.note(error.message, true); }
}

