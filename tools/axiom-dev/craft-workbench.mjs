import { el, button } from "./inspector.mjs";
import { catalogIcon } from "./catalog-icons.mjs";
import { recipeId, planCraft } from "./craft-index.mjs";
import { bindItemActions } from "./slot-tools.mjs";

const format = new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 });
const num = value => format.format(value);
const stat = (label, value) => el("div", { className: "craft-stat" }, el("span", {}, label), el("strong", {}, String(value)));
const pict = (ui, code) => ui.icon(code) ?? catalogIcon("Items");
const supplyHeading = "Items to source separately";
const supplyCoverage = "Recipe plan only. Supplies listed here are assumed available, not verified in game. Gathering, loot, trade and progression are outside this plan.";
const statusLabel = status => status === "No Recipe" ? "No indexed recipe" : status === "Supply" ? "Source separately" : status;

function stack(ui, code, count) {
  if (!code) return el("span", { className: "item-slot empty-slot", "aria-label": "Empty" }, "·");
  const name = ui.name(code);
  const label = (count !== 1 ? num(count) + " × " : "") + name;
  const control = button("", () => ui.item(code), label, "item-slot craft-ingredient");
  control.setAttribute("aria-label", label);
  control.append(pict(ui, code));
  if (count !== 1) control.append(el("small", { className: "slot-count" }, num(count)));
  return bindItemActions(control, code, ui.itemActions);
}

export function recipePlate(recipe, ui, compact = false) {
  const width = recipe.gridSize ?? 3;
  const slots = recipe.patternSlots
    ? Array.from({ length: width * width }, (_, i) => stack(ui, recipe.patternSlots[Math.floor(i / width)]?.[i % width], 1))
    : recipe.inputs.map(input => stack(ui, input.code, input.count));
  const count = ({ code, count }) => el("li", {}, pict(ui, code), el("span", {}, ui.name(code)), el("b", {}, num(count)));
  return el("div", { className: "craft-plate" + (compact ? " compact" : "") },
    el("div", { className: "craft-inputs" },
      el("div", { className: "craft-grid", style: "--craft-columns:" + (recipe.patternSlots ? width : Math.max(1, Math.min(3, slots.length))),
        "aria-label": recipe.form + " inputs" }, slots),
      el("ul", { className: "craft-counts", "aria-label": "Ingredient quantities" }, recipe.inputs.map(count))),
    el("span", { className: "recipe-arrow", "aria-hidden": "true" }, "→"),
    el("div", { className: "craft-output" }, stack(ui, recipe.product, recipe.count),
      el("strong", {}, ui.name(recipe.product)), el("small", {}, num(recipe.count) + " per Batch")));
}

function titlebar(text, actions = []) {
  return el("header", { className: "craft-titlebar" }, el("h2", {}, text), el("div", {}, actions));
}

export function recipeBook(rows, total, ui) {
  return el("div", { className: "craft-window" },
    titlebar("Recipe Book", [el("span", {}, rows.length + " / " + total)]),
    rows.length ? el("div", { className: "craft-book" }, rows.map(recipe => {
      const name = button(recipe.label, () => ui.recipe(recipe), recipe.category, "craft-recipe-name");
      return el("article", { className: "craft-book-entry" },
        el("header", {}, name, el("span", {}, recipe.category), button("Edit", () => ui.edit(recipe))),
        recipePlate(recipe, ui, true));
    })) : el("p", { className: "craft-empty" }, "No Recipes"));
}

export function recipeAtlas(index, outputs, ui) {
  const groups = new Map();
  for (const item of outputs) {
    if (!groups.has(item.category)) groups.set(item.category, []);
    groups.get(item.category).push(item);
  }
  const windows = [...groups.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([category, records]) => {
    const shell = el("details", { className: "craft-group" },
      el("summary", {}, category, el("span", {}, String(records.length))));
    shell.ontoggle = () => {
      if (!shell.open || shell.children.length > 1) return;
      shell.append(el("div", { className: "craft-group-rows" }, records.map(item => {
        const routes = index.producers.get(item.product);
        const branch = el("details", { className: "craft-output-branch" },
          el("summary", {}, pict(ui, item.product), el("strong", {}, item.label),
            el("span", {}, routes.length + (routes.length === 1 ? " Recipe" : " Recipes"))));
        branch.ontoggle = () => {
          if (!branch.open || branch.children.length > 1) return;
          branch.append(el("div", { className: "craft-routes" }, routes.map(route =>
            el("div", { className: "craft-atlas-route" },
              button(route.label, () => ui.recipe(route)), el("span", {}, route.category),
              button("Tree", () => ui.plan(item.product, route)), button("Edit", () => ui.edit(route))))));
        };
        return branch;
      })));
    };
    return shell;
  });
  const recipeCount = outputs.reduce((sum, item) => sum + (index.producers.get(item.product)?.length ?? 0), 0);
  const expand = button("Expand Groups", () => {
    const opened = windows.some(group => !group.open);
    for (const group of windows) group.open = opened;
    expand.textContent = opened ? "Collapse Groups" : "Expand Groups";
  });
  if (windows.length === 1) windows[0].open = true;
  return el("div", { className: "craft-window" }, titlebar("All Outputs", [button("Crafting Web", () => ui.web()), ...(windows.length ? [expand] : [])]),
    el("div", { className: "craft-stats" }, stat("Outputs", outputs.length), stat("Recipes", recipeCount)),
    el("div", { className: "craft-atlas" }, windows.length ? windows : el("p", { className: "craft-empty" }, "No Outputs")));
}

function disclosure(row, label, render, open = false) {
  if (!render) return row;
  const child = el("div", { className: "craft-branches", hidden: !open });
  let loaded = false;
  const expand = button(open ? "−" : "+", () => {
    child.hidden = !child.hidden;
    if (!child.hidden && !loaded) { child.append(...render()); loaded = true; }
    expand.textContent = child.hidden ? "+" : "−";
    expand.setAttribute("aria-expanded", String(!child.hidden));
  }, label, "craft-expand");
  expand.setAttribute("aria-label", label);
  expand.setAttribute("aria-expanded", String(open));
  row.prepend(expand);
  if (open) { child.append(...render()); loaded = true; }
  return el("div", { className: "craft-branch" }, row, child);
}

export function recipePlan(index, code, state, ui) {
  const content = el("div", { className: "craft-plan-content" });
  const inputs = button("Inputs", () => { state.direction = "inputs"; render(); });
  const uses = button("Uses", () => { state.direction = "uses"; render(); });
  const quantity = el("input", { type: "number", min: 1, max: 99999, step: 1, value: state.count, "aria-label": "Target quantity" });
  quantity.oninput = () => { if (quantity.value && quantity.validity.valid) { state.count = Number(quantity.value); render(); } };
  const countField = el("label", {}, "Target", quantity);
  const display = el("div", { className: "craft-plan-tools" }, inputs, uses, countField);
  const copy = button("Copy Context", () => ui.copy(contextText()));
  const output = el("div", { className: "craft-window" },
    titlebar(ui.name(code), [button("All Outputs", ui.atlas), button("Crafting Web", () => ui.web(code)), copy, ui.assist ? button("Ask AI", delegate) : null]),
    display, content);
  let current;

  function nameSlot(item, amount) {
    const control = button("", () => ui.item(item), "Inspect " + ui.name(item), "craft-node-item");
    control.append(pict(ui, item), el("strong", {}, ui.name(item)), amount ? el("b", {}, num(amount)) : document.createTextNode(""));
    return control;
  }
  function branch(node, depth) {
    const row = el("div", { className: "craft-node" }, nameSlot(node.item, node.need));
    if (node.recipe) {
      row.append(el("span", { className: "craft-node-station" }, num(node.batches) + "× " + node.recipe.category),
        button("Open", () => ui.recipe(node.recipe)), button("Edit", () => ui.edit(node.recipe)));
    } else row.append(el("span", { className: "craft-node-state", "data-status": node.status }, statusLabel(node.status)));
    if (node.routes.length && !["Cycle","Limit","Surplus"].includes(node.status)) {
      const choice = el("select", { "aria-label": "Recipe for " + ui.name(node.item) }, node.routes.map(route =>
        el("option", { value: recipeId(route) }, route.label)), el("option", { value: "supply" }, "Source separately"));
      choice.value = node.recipe ? recipeId(node.recipe) : "supply";
      choice.onchange = () => { state.choices.set(node.item, choice.value); render(node.item); };
      row.append(choice);
    }
    return disclosure(row, "Inputs for " + ui.name(node.item), node.children.length
      ? () => node.children.map(child => branch(child, depth + 1)) : null, depth < 2);
  }
  let useNodes = 0;
  function used(item, path) {
    if (path.length > 22 || ++useNodes > 1000) return [el("div", { className: "craft-node-state" }, "Display Limit")];
    if (path.includes(item)) return [el("div", { className: "craft-node-state", "data-status": "Cycle" }, "Cycle · " + ui.name(item))];
    const routes = index.consumers.get(item) ?? [];
    return routes.map(route => {
      const row = el("div", { className: "craft-node" }, nameSlot(route.product, route.count),
        el("span", { className: "craft-node-station" }, route.label + " · " + route.category),
        button("Open", () => ui.recipe(route)), button("Edit", () => ui.edit(route)));
      return disclosure(row, "Uses of " + ui.name(route.product), index.consumers.has(route.product)
        ? () => used(route.product, [...path, item]) : null);
    });
  }
  function list(title, values) {
    return el("section", { className: "craft-supply" }, el("h3", {}, title),
      values.size ? [...values].sort(([a], [b]) => ui.name(a).localeCompare(ui.name(b)))
        .map(([item, amount]) => nameSlot(item, amount)) : el("p", {}, "None"));
  }
  function render(focus) {
    const reverse = state.direction === "uses";
    inputs.setAttribute("aria-pressed", String(!reverse));
    uses.setAttribute("aria-pressed", String(reverse));
    countField.hidden = reverse;
    if (reverse) {
      useNodes = 0;
      const rows = used(code, []);
      content.replaceChildren(rows.length ? el("div", { className: "craft-tree" }, rows) : el("p", { className: "craft-empty" }, "No Uses"));
      return;
    }
    current = planCraft(index, code, state.count, state.choices);
    content.replaceChildren(el("div", { className: "craft-tree" }, branch(current.root, 0)),
      list(supplyHeading, current.supply), el("p", { className: "muted" }, supplyCoverage),
      current.loops.size ? list("Circular Requirements", current.loops) : "",
      current.limited ? el("p", { className: "error" }, "Plan Limit · Partial Totals") : "");
    if (focus) Array.from(content.querySelectorAll("select")).find(control =>
      control.getAttribute("aria-label") === "Recipe for " + ui.name(focus))?.focus({ preventScroll: true });
  }
  function contextText() {
    const header = "Axiom crafting · " + ui.name(code);
    if (state.direction === "uses") return [header, "Direct uses:",
      ...(index.consumers.get(code) ?? []).map(recipe => "- " + recipe.label + " · " + recipe.category + " · " + recipe.path)].join("\n");
    const lines = [header, "Target: " + num(state.count), "Plan:"];
    function walk(node, depth) {
      if (lines.length >= 170) return;
      lines.push("  ".repeat(depth) + "- " + num(node.need) + " × " + ui.name(node.item) +
        (node.recipe ? " · " + node.recipe.label + " · " + node.recipe.category + " · " + node.recipe.path : " · " + statusLabel(node.status)));
      node.children.forEach(child => walk(child, depth + 1));
    }
    walk(current.root, 0);
    if (lines.length >= 170 || current.limited) lines.push("Partial branch listing.");
    lines.push(supplyHeading + ":", ...[...current.supply].map(([item, amount]) => "- " + num(amount) + " × " + ui.name(item)), supplyCoverage);
    return lines.join("\n");
  }
  function delegate() {
    const context = new Map();
    const add = recipe => { if (recipe && context.size < 5) context.set(recipeId(recipe), recipe); };
    function gather(node) { add(node.recipe); if (context.size < 5) node.children.forEach(gather); }
    if (state.direction === "uses") (index.consumers.get(code) ?? []).forEach(add);
    else if (current) gather(current.root);
    if (!context.size) (index.producers.get(code) ?? []).forEach(add);
    ui.assist([...context.values()], "Review the " + ui.name(code) + " crafting " +
      (state.direction === "uses" ? "uses" : "plan for " + state.count) + ". Identify one improvement to prototype.");
  }
  render();
  return output;
}
