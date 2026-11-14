import { button, el, findEntry } from "./inspector.mjs";
import { dragSource, dropTarget } from "./slot-tools.mjs";
import { recipeBlocks } from "./wiring-commands.mjs";

const caption = { cell: "Cell", destination: "To cell", item: "Item", replacement: "New item", count: "Quantity", text: "Description" };

export function openRecipeBlocks(state, items, blocks, preview, stage) {
  const rows = el("ol", { className: "recipe-block-stack" });
  const result = el("div", { className: "recipe-block-result", role: "status", "aria-live": "polite" });
  const apply = button("Add to draft", () => {
    try { stage(candidate); dialog.close(); }
    catch (error) { invalidate(); result.textContent = error.message; }
  }, "", "primary");
  let candidate;
  function invalidate() {
    candidate = undefined;
    apply.disabled = true;
    result.replaceChildren();
  }
  function move(from, to) {
    if (!Number.isInteger(from) || from < 0 || from >= blocks.length || to < 0 || to >= blocks.length) return;
    blocks.splice(to, 0, ...blocks.splice(from, 1));
    render();
    rows.children[to].querySelector("button").focus();
  }
  function add(op) {
    if (blocks.length >= 32) return;
    blocks.push({ op, ...Object.fromEntries(recipeBlocks[op].fields
      .filter(key => key === "cell" || key === "destination" || key === "count")
      .map(key => [key, key === "destination" ? 2 : 1])) });
    render();
    rows.lastElementChild.querySelector("button").focus();
  }
  function itemField(block, key) {
    const pick = button(items.get(block[key])?.label ?? block[key] ?? "Choose item", () =>
      findEntry(caption[key], [...items.values()].map(item => ({ ...item, key: item.code })), block[key], code => {
        block[key] = code;
        pick.textContent = items.get(code).label;
        invalidate();
      }));
    dropTarget(pick, ["recipe-block-item"], (_, code) => {
      if (!items.has(code)) return;
      block[key] = code;
      pick.textContent = items.get(code).label;
      invalidate();
    });
    return el("div", { className: "recipe-block-field" }, el("span", {}, caption[key]), pick);
  }
  function render() {
    invalidate();
    rows.replaceChildren(...blocks.map((block, index) => {
      const definition = recipeBlocks[block.op];
      const handle = button("↕ " + definition.label, () => {}, "Drag to reorder, or use Move up and Move down");
      dragSource(handle, "recipe-block-row", index);
      const up = button("↑", () => move(index, index - 1), "Move up");
      const down = button("↓", () => move(index, index + 1), "Move down");
      up.disabled = index === 0;
      down.disabled = index === blocks.length - 1;
      const row = el("li", { className: "recipe-action-block" },
        el("div", { className: "recipe-block-heading" }, handle, up, down,
          button("×", () => { blocks.splice(index, 1); render(); }, "Remove action")),
        definition.fields.filter(key => !(state.shaped && block.op === "set" && key === "count")).map(key => {
          if (key === "item" || key === "replacement") return itemField(block, key);
          if (key === "text") return el("label", { className: "recipe-block-field" }, caption[key],
            el("textarea", { rows: 2, maxLength: 500, value: block[key] ?? "",
              oninput: event => { block[key] = event.target.value; invalidate(); } }));
          return el("label", { className: "recipe-block-field" }, caption[key],
            el("input", { type: "number", min: 1, max: key === "count" ? undefined : 9, step: 1,
              value: block[key] ?? 1, oninput: event => { block[key] = event.target.valueAsNumber; invalidate(); } }));
        }));
      dropTarget(row, ["recipe-block-row", "recipe-block-add"], (type, value) => {
        if (type === "recipe-block-row") move(Number(value), index);
        else {
          if (blocks.length >= 32) return;
          add(value);
          move(blocks.length - 1, index);
        }
      });
      return row;
    }));
  }
  const palette = el("div", { className: "recipe-block-palette" }, Object.entries(recipeBlocks)
    .filter(([, definition]) => state.shaped || !definition.shaped)
    .map(([op, definition]) => {
      const control = button("+ " + definition.label, () => add(op));
      dragSource(control, "recipe-block-add", op);
      return control;
    }));
  dropTarget(rows, ["recipe-block-add"], (_, op) => add(op));
  const itemSearch = el("input", { type: "search", placeholder: "Find items to drag", "aria-label": "Find items to drag" });
  const matches = el("div", { className: "recipe-block-palette" });
  itemSearch.oninput = () => {
    const query = itemSearch.value.trim().toLowerCase();
    matches.replaceChildren(...(query ? [...items.values()].filter(item =>
      item.label.toLowerCase().includes(query) || item.code.toLowerCase().includes(query)).slice(0, 8) : [])
      .map(item => {
        const chip = el("span", { className: "recipe-item-chip" }, item.label);
        dragSource(chip, "recipe-block-item", item.code);
        return chip;
      }));
  };
  function showPreview() {
    invalidate();
    try {
      const next = preview(blocks);
      const label = code => items.get(code)?.label ?? code;
      const slots = el("ol", { className: "recipe-block-preview" + (next.state.shaped ? " shaped" : "") },
        next.state.slots.map(slot => el("li", {}, slot ? slot.count + " × " + label(slot.code) : "Empty")));
      result.append(el("strong", {}, "Produces " + next.state.count + " × " + label(next.state.product)), slots);
      if (next.state.description) result.append(el("p", {}, next.state.description));
      if (!next.changed) result.append(el("p", {}, "These actions leave the recipe unchanged."));
      candidate = next;
      apply.disabled = !next.changed;
    } catch (error) { result.textContent = error.message; }
  }
  const command = el("textarea", { rows: 5, spellcheck: false, "aria-label": "Recipe command JSON" });
  const exchange = el("details", {}, el("summary", {}, "Share with AI"),
    el("p", {}, "Export this stack as command JSON, or paste a command for this recipe and load its blocks."),
    command, el("div", { className: "recipe-block-palette" }, button("Export command", () => {
      command.value = JSON.stringify({ recipe: state.name, blocks }, null, 2);
      command.select();
    }), button("Load command", () => {
      try {
        const request = JSON.parse(command.value);
        if (request.recipe !== state.name) throw new Error("This command names a different recipe.");
        if (!Array.isArray(request.blocks) || !request.blocks.length || request.blocks.length > 32) {
          throw new Error("The command must contain 1–32 blocks.");
        }
        const imported = request.blocks.map(block => {
          if (!block || !Object.hasOwn(recipeBlocks, block.op)) throw new Error("The command contains an unknown action.");
          const definition = recipeBlocks[block.op];
          if (!state.shaped && definition.shaped) throw new Error("This action needs a shaped recipe.");
          return { op: block.op, ...Object.fromEntries(definition.fields.filter(key => block[key] !== undefined).map(key => {
            const value = block[key];
            if (typeof value !== (["item", "replacement", "text"].includes(key) ? "string" : "number")) {
              throw new Error("Invalid value for " + caption[key] + ".");
            }
            return [key, value];
          })) };
        });
        blocks.splice(0, blocks.length, ...imported);
        render();
      } catch (error) { invalidate(); result.textContent = error.message; }
    })));
  const dialog = el("dialog", { className: "finder recipe-block-dialog", "aria-label": "Recipe action blocks",
    onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, el("h2", {}, "Action blocks · " + state.name),
      button("Close", () => dialog.close())),
    el("p", {}, "Click or drag an action into the stack. Actions run from top to bottom. Cells start at 1 and count left to right."),
    palette, itemSearch, matches, rows,
    el("div", { className: "recipe-block-palette" }, button("Preview result", showPreview), apply), result, exchange);
  render();
  document.body.append(dialog);
  dialog.showModal();
}
