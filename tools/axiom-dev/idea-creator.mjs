import { el, button } from "./inspector.mjs";
import { slug } from "./targets.mjs";
import { recipePath, spawnPath, advancementPath, field, textOf, recipeItems, newRecipe, append,
  spawnRow, advancementRow } from "./idea-data.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";
import { dragSource, dropTarget, itemPalette, slotArt } from "./slot-tools.mjs";

const label = (title, element) => el("label", {}, title, element);
const select = options => el("select", {}, options.map(([value, name]) => el("option", { value: String(value) }, name)));
const number = (value, max) => el("input", { type: "number", min: 1, max, step: 1, value: String(value), required: true });
const line = (...nodes) => el("div", { className: "idea-row" }, nodes);
const small = text => el("p", { className: "muted" }, text);
const count = input => {
  const value = Number(input.value);
  if (!input.validity.valid || !Number.isSafeInteger(value) || value < 1) throw new Error("Enter a positive whole number");
  return value;
};

async function recipe(ctx, preset) {
  const doc = await ctx.load(recipePath);
  const [items, artwork] = await Promise.all([recipeItems(ctx, doc), recipeArtwork(ctx).catch(() => null)]);
  const options = el("datalist", { id: "idea-items" }, items.map(item => el("option", { value: item.label })));
  const itemLookup = new Map(items.map(item => [item.label.toLowerCase(), item]));
  const byCode = new Map(items.map(item => [item.code, item]));
  const itemInput = required => el("input", { type: "text", list: options.id, placeholder: "Find an item…", required,
    autocomplete: "off", spellcheck: false });
  const itemOf = (input, action) => {
    const item = itemLookup.get(input.value.trim().toLowerCase());
    if (!item) throw new Error("Choose an item from the suggestions for " + action);
    return item;
  };
  const name = el("input", { type: "text", placeholder: "Defaults to the output item", maxLength: 110 });
  const description = el("textarea", { rows: 2, maxLength: 500, placeholder: "Optional recipe description" });
  const station = select([["hand", "Hand · 2 × 2"], ["bench", "Workbench · 3 × 3"]]);
  station.value = "bench";
  const mode = select([["shapeless", "Any arrangement"], ["shaped", "Draw a pattern"]]);
  const output = itemInput(true);
  output.value = byCode.get(typeof preset === "string" ? preset : preset?.product)?.label ?? "";
  const outputCount = number(1, 64);
  const summary = el("div", { className: "idea-summary", role: "status" });
  const held = el("div", { className: "workbench-held", role: "status" });
  let brush;
  function setInput(input, item) {
    if (!item) return;
    input.value = item.label;
    describe();
  }
  const setOutput = button("", () => brush ? setInput(output, brush) : output.focus(), "Set output from the item palette", "item-slot output-slot");
  dropTarget(setOutput, ["item"], (_, code) => setInput(output, byCode.get(code)));
  dropTarget(output, ["item"], (_, code) => setInput(output, byCode.get(code)));
  const ingredientRows = [];
  const ingredients = el("div", { className: "idea-ingredients" });
  const add = button("+ Ingredient", () => brush ? putIngredient(brush) : addIngredient());
  function addIngredient(item, amount = 1) {
    if (ingredientRows.length >= (station.value === "hand" ? 4 : 9)) return;
    const input = itemInput(true);
    const quantity = number(amount, 16384);
    if (item) input.value = item.label;
    dropTarget(input, ["item"], (_, code) => setInput(input, byCode.get(code)));
    const entry = { input, quantity };
    const remove = button("×", () => {
      ingredientRows.splice(ingredientRows.indexOf(entry), 1);
      entry.row.remove();
      updateIngredients();
    }, "Remove ingredient");
    remove.setAttribute("aria-label", "Remove ingredient");
    entry.remove = remove;
    entry.row = line(label("Ingredient", input), label("Count", quantity), remove);
    ingredientRows.push(entry);
    ingredients.insertBefore(entry.row, add);
    updateIngredients();
    if (item) describe();
  }
  function updateIngredients() {
    add.disabled = ingredientRows.length >= (station.value === "hand" ? 4 : 9);
    ingredientRows.forEach(entry => { entry.remove.disabled = ingredientRows.length === 1; });
  }
  ingredients.append(add);
  if (!preset?.inputs?.length || preset.patternSlots) addIngredient();
  function putIngredient(item) {
    if (!item) return;
    const empty = ingredientRows.find(entry => !entry.input.value.trim());
    if (empty) setInput(empty.input, item);
    else addIngredient(item);
  }
  dropTarget(ingredients, ["item"], (_, code) => putIngredient(byCode.get(code)));

  const cells = Array(9).fill(null);
  const cellButtons = cells.map((_, i) => button("+", event => {
    if (event.shiftKey) cells[i] = null;
    else if (brush) cells[i] = brush;
    else if (cells[i]) pick(cells[i]);
    renderCells();
  }, "Slot " + (i + 1), "item-slot craft-slot"));
  const grid = el("div", { className: "idea-grid", "aria-label": "Crafting pattern" }, cellButtons);
  const shape = el("div", { className: "idea-shape" }, grid,
    el("span", { className: "muted slot-keys" }, "[RMB] Clear Slot"));
  function pick(item) {
    brush = item;
    held.replaceChildren(slotArt(item, artwork), el("span", {}, item.label));
  }
  cellButtons.forEach((cell, i) => {
    cell.oncontextmenu = event => { event.preventDefault(); cells[i] = null; renderCells(); };
    cell.onkeydown = event => {
      if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault(); cells[i] = null; renderCells();
      }
    };
    dragSource(cell, "craft-slot", i);
    dropTarget(cell, ["item", "craft-slot"], (type, value) => {
      if (type === "item") {
        const item = byCode.get(value);
        if (!item) return;
        cells[i] = item;
      } else {
        const from = Number(value);
        if (!Number.isInteger(from) || !cells[from] || cellButtons[from].disabled || i === from) return;
        [cells[from], cells[i]] = [cells[i], cells[from]];
      }
      renderCells();
    });
  });
  function renderCells() {
    grid.style.setProperty("--craft-columns", station.value === "hand" ? 2 : 3);
    cellButtons.forEach((cell, i) => {
      cell.disabled = station.value === "hand" && (i % 3 > 1 || i >= 6);
      cell.hidden = cell.disabled;
      cell.dataset.draggable = String(!cell.disabled && Boolean(cells[i]));
      cell.replaceChildren(slotArt(cells[i], artwork) ?? "+");
      cell.classList.toggle("filled", !cell.disabled && Boolean(cells[i]));
      cell.setAttribute("aria-label", "Slot " + (i + 1) + " · " + (cells[i]?.label ?? "Empty"));
      cell.title = (cells[i]?.label ?? "Empty slot") + " · [RMB] Clear";
    });
    describe();
  }

  const requirements = el("div");
  function showMode() {
    requirements.replaceChildren(mode.value === "shaped" ? shape : ingredients);
    describe();
  }
  function describe() {
    const chosen = itemLookup.get(output.value.trim().toLowerCase());
    setOutput.replaceChildren(slotArt(chosen, artwork) ?? "→");
    setOutput.setAttribute("aria-label", "Output slot · " + (chosen?.label ?? "Empty"));
    const used = mode.value === "shaped" ? cells.filter((item, index) => item && !cellButtons[index].disabled).length
      : ingredientRows.filter(entry => itemLookup.has(entry.input.value.trim().toLowerCase())).length;
    summary.textContent = chosen
      ? (mode.value === "shaped" ? used + " filled slots" : used + " ingredients") + " → " + outputCount.value + " × " + chosen.label
      : "No Output";
  }
  mode.onchange = showMode;
  station.onchange = () => { updateIngredients(); renderCells(); };
  output.oninput = outputCount.oninput = describe;
  ingredients.addEventListener("input", describe);
  if (preset && typeof preset === "object") {
    name.value = preset.label ? (preset.label + " alternative").slice(0, 110) : "";
    description.value = preset.description ?? "";
    station.value = preset.category === "Hand" ? "hand" : "bench";
    outputCount.value = Math.min(preset.count ?? 1, byCode.get(preset.product)?.limit ?? 64);
    if (preset.patternSlots) {
      mode.value = "shaped";
      preset.patternSlots.slice(0, 3).forEach((row, y) => row.slice(0, 3).forEach((code, x) => {
        cells[y * 3 + x] = byCode.get(code) ?? null;
      }));
    } else {
      for (const input of preset.inputs ?? []) addIngredient(byCode.get(input.code), input.count);
    }
  }
  showMode();
  renderCells();
  return {
    content: el("div", { className: "recipe-workbench" }, el("div", { className: "idea-fields" }, options, label("Recipe name", name),
      line(label("Station", station), label("Pattern", mode)), label("Description", description),
      el("div", { className: "recipe-machine" },
        el("section", { className: "recipe-inputs" }, el("h3", {}, "Inputs"), requirements),
        el("span", { className: "recipe-arrow", "aria-hidden": "true" }, "→"),
        el("section", { className: "recipe-output" }, el("h3", {}, "Output"), setOutput,
          label("Item", output), label("Count", outputCount))),
      held, summary), itemPalette(items, artwork, pick)),
    make() {
      const product = itemOf(output, "the output");
      return newRecipe(doc, ctx.tree(doc), {
        name: name.value.trim() || "Make " + product.label, description: description.value.trim(), station: station.value, output: product,
        count: count(outputCount), cells: mode.value === "shaped"
          ? cells.map((item, index) => cellButtons[index].disabled ? null : item) : null,
        inputs: mode.value === "shaped" ? [] : ingredientRows.map(row =>
          ({ item: itemOf(row.input, "each ingredient"), amount: count(row.quantity) })),
      });
    },
  };
}

async function spawn(ctx, preset) {
  const [doc, creatures, biomeDoc] = await Promise.all([ctx.load(spawnPath), ctx.load("assets/bestiary.ron"), ctx.load("assets/biomes.ron")]);
  const names = new Map(ctx.tree(creatures).children.map(row =>
    [textOf(row.children[0]), textOf(field(row.children[1], "name"))]).filter(([id, name]) => /^\d+$/.test(id) && name));
  const habitats = ctx.tree(doc);
  const biomes = new Set();
  for (const row of habitats.children) {
    for (const biome of field(row.children[1], "biomes")?.children ?? []) biomes.add(textOf(biome));
  }
  for (const row of field(ctx.tree(biomeDoc), "families").children) biomes.add(textOf(field(row, "name")));
  const creature = select([...names].sort((a, b) => a[1].localeCompare(b[1])));
  if (names.has(preset)) creature.value = preset;
  const realm = select([["Surface", "Surface"], ["Underground", "Underground"], ["Nether", "Nether"], ["Shell", "Shell"]]);
  const time = select([["Any", "Any time"], ["Day", "Day"], ["Night", "Night"]]);
  const biome = select([["", "Any biome"], ...[...biomes].sort().map(value => [value, value])]);
  if (biomes.has(preset?.biome)) biome.value = preset.biome;
  const weight = number(3, 255);
  realm.onchange = () => { biome.disabled = ["Nether", "Shell"].includes(realm.value); };
  return {
    content: el("div", { className: "idea-fields" }, label("Creature", creature),
      line(label("Realm", realm), label("Time", time)), line(label("Biome", biome), label("Spawn weight", weight)),
      small("Weight is relative to other creatures that can spawn here.")),
    make() {
      const name = names.get(creature.value);
      if (!name) throw new Error("Choose a creature");
      return append(doc, ctx.tree(doc), [], spawnRow(creature.value, count(weight), realm.value, time.value,
        biome.disabled ? "" : biome.value), name + " · " + realm.value);
    },
  };
}

async function advancement(ctx) {
  const doc = await ctx.load(advancementPath);
  const root = ctx.tree(doc);
  const parents = root.children.map(row => [textOf(field(row, "id")), textOf(field(row, "name"))]).filter(([id, name]) => id && name);
  const triggers = [["Mined", "Blocks mined"], ["Placed", "Blocks placed"], ["Crafted", "Items crafted"],
    ["Kills", "Creatures defeated"], ["Deaths", "Deaths"], ["Walked", "Distance walked (m)"],
    ["Swum", "Distance swum (m)"], ["Flown", "Distance flown (m)"], ["PlayTime", "Time played (min)"], ["Level", "Player level"]];
  const name = el("input", { type: "text", placeholder: "e.g. Quarry Foreman", required: true, maxLength: 90 });
  const parent = select([["", "Start a new branch"], ...parents]);
  const event = select(triggers);
  const target = number(100, 1_000_000_000);
  return {
    content: el("div", { className: "idea-fields" }, label("Advancement name", name),
      line(label("Trigger", event), label("Target", target)), label("Tree Parent", parent)),
    make() {
      const title = name.value.trim();
      const id = slug(title).replace(/-/g, "_");
      if (!id) throw new Error("Enter a name with at least one letter or number");
      const root = ctx.tree(doc);
      if (root.children.length >= 64) throw new Error("All 64 advancement save slots are in use");
      if (root.children.some(row => textOf(field(row, "id")) === id)) throw new Error("An advancement already uses that name");
      if (parent.value && !root.children.some(row => textOf(field(row, "id")) === parent.value)) {
        throw new Error("The parent advancement changed");
      }
      const units = event.value === "PlayTime" ? 60000 : ["Walked", "Swum", "Flown"].includes(event.value) ? 100 : 1;
      const value = count(target) * units;
      const trigger = event.value === "Level" ? "Level(" + value + ")" : "Stat(" + event.value + ", " + value + ")";
      return append(doc, root, [], advancementRow(id, title, parent.value, trigger), title);
    },
  };
}

export function createIdea(ctx, go, initialPath, preset, direct = false) {
  const templates = [
    ["Recipe", recipePath, recipe], ["Spawn rule", spawnPath, spawn], ["Advancement", advancementPath, advancement],
  ];
  const display = el("div", { className: "idea-body" });
  const tabs = el("div", { className: "chips idea-tabs" });
  const loaded = new Map();
  let active, version = 0;
  async function show(index) {
    const request = ++version;
    active = undefined;
    [...tabs.children].forEach((control, i) => {
      control.classList.toggle("active", index === i);
      control.setAttribute("aria-pressed", String(index === i));
    });
    display.replaceChildren(small("Loading catalog…"));
    try {
      if (!loaded.has(index)) loaded.set(index, templates[index][2](ctx, preset));
      const template = await loaded.get(index);
      if (request !== version) return;
      active = template;
      display.replaceChildren(template.content);
      display.querySelector("input")?.focus({ preventScroll: true });
    } catch (error) {
      loaded.delete(index);
      if (request === version) display.replaceChildren(el("p", { className: "error" }, error.message));
    }
  }
  const first = Math.max(0, templates.findIndex(([, path]) => path === initialPath));
  if (!direct) tabs.append(...templates.map(([title], index) => button(title, () => show(index))));
  ctx.dialog(direct ? "New " + templates[first][0] : "Create",
    direct ? [display] : [tabs, display], "Create draft", async () => {
    if (!active) return false;
    const { text, ...node } = active.make();
    const doc = await ctx.load(node.path);
    if (!ctx.edit(doc, text)) return false;
    go(node);
    ctx.note("Draft created");
    return true;
  });
  show(first);
}

