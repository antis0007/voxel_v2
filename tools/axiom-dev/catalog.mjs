import { el, button, findEntry } from "./inspector.mjs";
import { browse, itemRows, recipeRows, machineAssemblyRows, fabricationRecipes, spawnLinks, filename } from "./catalog-data.mjs";
import { fabricationPath, fabricationRows } from "./fabrication-data.mjs";
import { quoted } from "./syntax.mjs";
import { catalogIcon, recipeArtwork } from "./catalog-icons.mjs";
import { balanceRecipe, editCreature } from "./quick-balance.mjs";
import { voxelStudio, meshPreview, voxelThumbnail } from "./model-studio.mjs";
import { poseStudio } from "./pose-studio.mjs";
import { append, field, recipeItems, recipePath, spawnPath, textOf } from "./idea-data.mjs";
import { resolve } from "./targets.mjs";
import { canvasPixels } from "./pixel-grid.mjs";
import { cells, rgba } from "./sprite-sheet.mjs";
import { createFood } from "./food-creator.mjs";
import { biomeStudio } from "./biome-studio.mjs";
import { deposits, depositStudio } from "./deposit-studio.mjs";
import { editHabitat, habitatCards, isAnimal } from "./habitat-editor.mjs";
import { dragSource, dropTarget, slotTooltip } from "./slot-tools.mjs";
import { showContextMenu, copyText } from "./context-menu.mjs";
import { craftingIndex, recipeId } from "./craft-index.mjs";
import { recipeBook, recipeAtlas, recipePlan, recipePlate } from "./craft-workbench.mjs";
import { openCraftWeb } from "./craft-web.mjs";
import { previewRecipeBlocks } from "./recipe-wiring-data.mjs";
import { spriteUsage, emptyUsage, extendSpriteUsage } from "./sprite-usage.mjs";

export const groups = [
  { name: "Game objects", sections: ["Items", "Recipes", "Recipe Tree"] },
  { name: "World", sections: ["Biomes", "Animals", "Deposits", "Creatures", "Locations"] },
  { name: "Art", sections: ["Sprites", "Models", "Hand poses"] },
  { name: "Project", sections: ["All files"] },
];
const keyOf = spec => JSON.stringify([spec.path, spec.parts, spec.line, spec.deposit,
  ["item", "craft-output"].includes(spec.kind) ? spec.product : null, ...(spec.spriteIndex === undefined ? [] : [spec.spriteIndex])]);
const summary = (spec, items) => spec.kind === "recipe"
  ? spec.form + " · " + spec.count + " × " + (items.get(spec.product) ?? spec.product)
  : spec.detail ?? spec.form ?? "";
const quiet = text => el("p", { className: "muted" }, text);

export function mountCatalog(host, ctx) {
  let section = "Items", records = [], selected, activeDoc, page = 0, listVersion = 0, detailVersion = 0;
  let artwork, items = new Map(), itemNames = new Map(), recipes = [];
  let craft = craftingIndex([]);
  const planState = { code: "", count: 1, choices: new Map(), direction: "inputs" };
  let thumbnails;
  const spriteThumbs = new Map();
  let spriteUsers = new Map();
  let clearTooltips = [];
  let prefs = { layout: "tiles", density: "compact", font: "pixel", side: "right", width: 42, pins: [] };
  let pending = Promise.resolve();
  const pageSize = () => section === "Recipes" ? 16 : section === "Sprites" ? 48 : 96;
  const list = el("div", { className: "catalog-items", "aria-label": "Catalog entries" });
  const spaces = el("nav", { className: "catalog-sections", "aria-label": "Catalog sections" });
  const pins = el("div", { className: "catalog-pins", "aria-label": "Bookmarks" });
  const title = el("h1", {}, section);
  const filter = el("select", { "aria-label": "Filter category" });
  const artFilter = el("select", { "aria-label": "Sprite asset type", hidden: true },
    [["source", "Editable source"], ["all", "All art"], ["generated", "Generated art"], ["reference", "Reference art"],
      ["draft", "Modified"], ["pinned", "Bookmarked"]].map(([value, text]) => el("option", { value }, text)));
  const artScale = el("input", { type: "range", min: 32, max: 76, value: 44, "aria-label": "Sprite thumbnail size", hidden: true });
  const usageFilter = el("select", { "aria-label": "Sprite usage" },
    [["all", "Any usage"], ["known", "Indexed users"], ["unknown", "Usage unknown"], ["shared", "Shared art"]]
      .map(([value, text]) => el("option", { value }, text)));
  const search = el("input", { type: "search", placeholder: "Search this catalog…", "aria-label": "Search this catalog" });
  const count = el("span", { className: "catalog-count", role: "status" });
  // Sort order is a per-browser convenience, kept out of the server-validated workbench prefs.
  const sortKey = "axiom-dev.catalog.sort";
  const sort = el("select", { "aria-label": "Sort entries", title: "Sort entries" },
    [["default", "Default order"], ["name", "Name A–Z"], ["name-desc", "Name Z–A"], ["category", "Category"],
      ["pinned", "Bookmarked first"]].map(([value, text]) => el("option", { value }, text)));
  try { sort.value = localStorage.getItem(sortKey) || "default"; } catch { /* private window */ }
  if (!sort.value) sort.value = "default";
  const clearSearch = button("✕", () => { search.value = ""; filter.value = ""; changePage(0); search.focus(); },
    "Clear search and category (Esc)", "catalog-clear");
  const inspector = el("aside", { className: "catalog-detail", "aria-label": "Selected entry" });
  const extra = el("div", { className: "catalog-actions" });
  const pager = el("div", { className: "catalog-pager" });
  const grid = button("Slots", () => configure({ layout: "tiles" }));
  const rows = button("List", () => configure({ layout: "rows" }));
  const layoutSwitch = el("div", { className: "history-actions catalog-layout-switch" }, grid, rows);
  const options = button("Layout", showLayout);
  const divider = el("div", { className: "catalog-divider", role: "separator", tabIndex: 0,
    "aria-label": "Browser width", "aria-orientation": "vertical", "aria-valuemin": 30, "aria-valuemax": 65 });
  const browsePane = el("section", { className: "catalog-library", "aria-label": "Browser" },
    el("div", {}, el("div", { className: "catalog-filter" }, search, clearSearch, filter, sort, layoutSwitch),
      el("div", { className: "sprite-browser-filters", hidden: true }, artFilter, usageFilter, artScale)),
    list, el("div", { className: "catalog-browser-foot" }, count, pager));
  const panes = el("div", { className: "catalog-body" }, inspector, divider, browsePane);

  function applyLayout() {
    list.classList.toggle("as-list", prefs.layout === "rows" || section === "Recipes");
    layoutSwitch.hidden = section === "Recipes";
    artFilter.parentElement.hidden = section !== "Sprites";
    artFilter.hidden = artScale.hidden = false;
    host.dataset.density = prefs.density;
    host.dataset.side = prefs.side;
    document.documentElement.dataset.workbenchFont = prefs.font;
    panes.style.setProperty("--browse-width", prefs.width + "%");
    divider.setAttribute("aria-valuenow", String(Math.round(prefs.width)));
    grid.setAttribute("aria-pressed", String(prefs.layout === "tiles"));
    rows.setAttribute("aria-pressed", String(prefs.layout === "rows"));
  }
  function savePrefs() {
    const snapshot = structuredClone(prefs);
    pending = pending.catch(() => {}).then(() => ctx.api("/api/workbench", undefined, snapshot))
      .catch(error => ctx.note(error.message, true));
  }
  function configure(changes) {
    Object.assign(prefs, changes);
    applyLayout();
    savePrefs();
  }

  function showLayout() {
    const select = (name, key, values) => {
      const input = el("select", {}, values.map(([value, text]) => el("option", { value }, text)));
      input.value = prefs[key];
      input.onchange = () => configure({ [key]: input.value });
      return el("label", {}, name, input);
    };
    const fields = [
      select("Density", "density", [["compact", "Compact"], ["roomy", "Roomy"]]),
      select("Headings", "font", [["pixel", "Axiom Pixel"], ["plain", "System"]]),
      select("Browser Side", "side", [["left", "Left"], ["right", "Right"]]),
    ];
    ctx.dialog("Layout", [el("div", { className: "idea-fields layout-settings" }, fields)], "Done", () => true);
  }
  const resize = width => {
    prefs.width = Math.round(Math.max(30, Math.min(65, width)));
    applyLayout();
  };
  divider.onpointerdown = event => {
    if (event.button !== 0) return;
    event.preventDefault();
    divider.setPointerCapture(event.pointerId);
    divider.focus({ preventScroll: true });
  };
  divider.onpointermove = event => {
    if (!divider.hasPointerCapture(event.pointerId)) return;
    const bounds = panes.getBoundingClientRect();
    const percent = (event.clientX - bounds.left) / bounds.width * 100;
    resize(prefs.side === "left" ? percent : 100 - percent);
  };
  divider.onlostpointercapture = savePrefs;
  divider.ondblclick = () => configure({ width: 42 });
  divider.onkeydown = event => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const delta = (event.key === "ArrowLeft" ? -2 : 2) * (prefs.side === "left" ? 1 : -1);
    resize(event.key === "Home" ? 30 : event.key === "End" ? 65 : prefs.width + delta);
    savePrefs();
  };
  host.append(el("aside", { className: "catalog-rail" }, spaces, pins),
    el("section", { className: "catalog-main" },
      el("header", { className: "catalog-header" }, title, extra, options), panes));
  applyLayout();

  function categoryButton(name) {
    const tab = button("", () => setSection(name), "Browse " + name, "catalog-tab" + (name === section ? " active" : ""));
    tab.append(catalogIcon(name), name);
    tab.setAttribute("aria-pressed", String(name === section));
    return tab;
  }
  const renderTabs = () => {
    spaces.replaceChildren(...groups.map(group => el("section", {},
      el("h2", {}, group.name), ...group.sections.map(categoryButton))));
    title.textContent = section;
    host.dataset.section = section;
    search.placeholder = "Search " + section.toLowerCase() + "…";
    search.setAttribute("aria-label", search.placeholder.replace("…", ""));
  };
  renderTabs();
  renderPins();

  function pinIndex(spec) { return prefs.pins.findIndex(pin => pin.section === section && pin.key === keyOf(spec)); }
  function togglePin(spec, addOnly = false) {
    const index = pinIndex(spec);
    if (index >= 0) {
      if (addOnly) return;
      prefs.pins.splice(index, 1);
    } else {
      if (prefs.pins.length >= 20) return ctx.note("Bookmark Limit · 20");
      prefs.pins.push({ section, key: keyOf(spec), label: spec.label });
    }
    renderPins();
    savePrefs();
    inspector.querySelector(".catalog-bookmark")?.setAttribute("aria-pressed", String(Boolean(selected && pinIndex(selected) >= 0)));
  }
  function renderPins() {
    pins.replaceChildren(el("h2", {}, "Bookmarks"),
      ...prefs.pins.filter(pin => groups.some(group => group.sections.includes(pin.section))).map(pin => {
        const select = button(pin.label, () => setSection(pin.section, pin.key), pin.label);
        const remove = () => {
          prefs.pins = prefs.pins.filter(entry => entry !== pin);
          renderPins();
          savePrefs();
          inspector.querySelector(".catalog-bookmark")?.setAttribute("aria-pressed", String(Boolean(selected && pinIndex(selected) >= 0)));
        };
        select.oncontextmenu = event => {
          event.preventDefault();
          showContextMenu(select, [{ label: "Remove Bookmark", run: remove }], [event.clientX, event.clientY]);
        };
        select.onkeydown = event => { if (event.key === "Delete") { event.preventDefault(); remove(); } };
        dragSource(select, "pin", prefs.pins.indexOf(pin), pin.section === "Animals" ? { "animal-pin": pin.key } : undefined);
        dropTarget(select, pin.section === "Biomes" && section === "Animals" ? ["pin", "record"] : ["pin"], (type, raw) => {
          if (type === "record") {
            const animal = records.find(row => keyOf(row) === raw);
            if (animal && isAnimal(animal)) browse(ctx, "Biomes").then(rows => {
              const biome = rows.find(row => keyOf(row) === pin.key);
              if (biome) assignHabitat(animal, biome);
            }).catch(error => ctx.note(error.message, true));
            return;
          }
          const source = Number(raw), destination = prefs.pins.indexOf(pin);
          if (!Number.isInteger(source) || !prefs.pins[source] || source === destination) return;
          prefs.pins.splice(destination, 0, prefs.pins.splice(source, 1)[0]);
          renderPins();
          savePrefs();
        });
        return select;
      }),
      prefs.pins.length ? null : el("span", { className: "catalog-pin-empty", title: "[A] Bookmark or drop a record" }, "＋"));
  }
  dropTarget(pins, ["record"], (_, key) => {
    const spec = records.find(row => keyOf(row) === key);
    if (spec) togglePin(spec, true);
  });

  function setSection(next, focus) {
    section = next; page = 0; selected = undefined; activeDoc = undefined; detailVersion++;
    search.value = "";
    renderTabs();
    applyLayout();
    renderPins();
    refresh(true, focus);
  }

  async function createFabrication() {
    const doc = await ctx.load(fabricationPath);
    const resources = [...items.values()].flatMap(item => {
      const id = /^Resource\((\d+)\)$/.exec(item.product)?.[1];
      return id === undefined ? [] : [{ id, label: item.label }];
    }).sort((a, b) => a.label.localeCompare(b.label));
    const name = el("input", { type: "text", maxlength: 80 });
    const number = (value, min, max, step = 1) => el("input", { type: "number", value, min, max, step });
    const labeled = (label, input) => el("label", {}, el("span", {}, label), input);
    const stack = (label, placeholder) => {
      const item = el("select", {}, el("option", { value: "" }, placeholder),
        ...resources.map(resource => el("option", { value: resource.id }, resource.label)));
      const count = number(1, 1 / 64, 64, 1 / 64);
      return { item, count, fields: [labeled(label, item), labeled("Items per batch", count)] };
    };
    const inputs = [stack("Input 1", "Choose resource"), stack("Input 2", "Optional"), stack("Input 3", "Optional")];
    const output = stack("Output", "Choose resource");
    const seconds = number(10, 0.25, 3600, 0.25);
    const power = number(12, 1, 64);
    const fields = el("div", { className: "idea-fields" },
      labeled("Recipe name", name), ...inputs.flatMap(input => input.fields), ...output.fields,
      labeled("Seconds per batch", seconds), labeled("Power in kW", power));
    ctx.dialog("New Fabricator recipe", [fields], "Create draft", () => {
      const root = ctx.tree(doc);
      const stackText = ({ item, count }) => `(${item.value}, ${Number(count.value) * 64})`;
      if (!output.item.value) throw new Error("Choose an output resource");
      const entry = `(id: ${root.children.length}, name: ${quoted(name.value.trim(), "ron")}, `
        + `inputs: [${inputs.filter(input => input.item.value).map(stackText).join(", ")}], `
        + `output: ${stackText(output)}, ticks: ${Number(seconds.value) * 4}, power: ${Number(power.value)})`;
      const draft = append(doc, root, [], entry, name.value.trim());
      fabricationRows(draft.text);
      if (!ctx.edit(doc, draft.text)) return false;
      refresh(false, draft);
      ctx.note("Fabricator recipe draft created");
      return true;
    });
  }

  function headerActions() {
    const controls = [];
    if (section === "Items") {
      controls.push(button("+ Food", () => createFood(ctx, focusFood), "New food item", "primary"));
      const recipe = button("+ Recipe", () => ctx.create(recipePath), "New recipe · Item drop target");
      dropTarget(recipe, ["item"], (_, code) => { if (items.has(code)) ctx.create(recipePath, code); });
      controls.push(recipe);
    }
    if (section === "Recipes") controls.push(
      button("Book", () => { selected = undefined; activeDoc = undefined; detailVersion++; renderList(); }, "Show full recipes"),
      button("+ Recipe", () => ctx.create(recipePath), "", "primary"),
      button("+ Fabricator Recipe", () => createFabrication().catch(error => ctx.note(error.message))));
    if (section === "Recipe Tree") controls.push(button("All Outputs", () => { selected = undefined; activeDoc = undefined; detailVersion++; renderList(); }));
    if (section === "Creatures") controls.push(button("New creature", () => {
      const base = selected ?? records[0];
      if (base) editCreature(ctx, base, onEdit, true);
    }, "Start from a creature’s stats", "primary"));
    if (section === "Animals") controls.push(button("New animal", () => {
      const base = selected?.body ? selected : records.find(row => row.label === "Cow");
      if (base) editCreature(ctx, base, onEdit, true);
    }, "Start from a cow, boar, deer, or marsh fowl", "primary"));
    if (section === "Locations") controls.push(button("Build a location", openStudio,
      "Open Anchor Studio for floor plans, rooms, materials and blueprints", "primary"));
    extra.replaceChildren(...controls);
  }

  async function openStudio() {
    const link = el("a", { href: "http://127.0.0.1:4377/", target: "_blank", rel: "noopener" }, "Open Anchor Studio");
    try {
      await ctx.api("/api/anchor-studio", undefined, {});
      ctx.dialog("Building designer", [quiet("Anchor Studio is ready."), link], "Done", () => true);
    } catch (error) {
      ctx.dialog("Building designer", [quiet(error.message), link], "Close", () => true);
    }
  }

  async function refresh(reset = false, focus) {
    const generation = ++listVersion;
    const category = section;
    headerActions();
    try {
      if (reset) {
        records = [];
        list.replaceChildren(quiet("Loading " + section.toLowerCase() + "…"));
        pager.replaceChildren();
        inspector.replaceChildren();
      }
      let result;
      if (["Recipes", "Items", "Recipe Tree", "Sprites"].includes(category)) {
        const [doc, machines, fabrication] = await Promise.all([
          ctx.load(recipePath), ctx.load("assets/machines.ron"), ctx.load(fabricationPath),
        ]);
        const [icons, catalog] = await Promise.all([recipeArtwork(ctx).catch(() => null), itemRows(ctx, doc)]);
        if (generation !== listVersion) return;
        artwork = icons;
        items = new Map(catalog.map(item => [item.product, item]));
        itemNames = new Map(catalog.map(item => [item.product, item.label]));
        recipes = [...recipeRows(ctx, doc), ...machineAssemblyRows(ctx, machines), ...fabricationRecipes(ctx, fabrication)];
        craft = craftingIndex(recipes);
        spriteUsers = spriteUsage(catalog, icons, craft);
        if (category === "Sprites") {
          try { await extendSpriteUsage(ctx, spriteUsers, catalog); }
          catch (error) { ctx.note("Additional art tracing unavailable: " + error.message, true); }
        }
        result = category === "Sprites" ? await browse(ctx, category)
          : category === "Recipes" ? recipes : category === "Items" ? catalog
          : [...craft.producers.keys()].map(code => ({ ...(items.get(code) ?? {
            kind: "item", path: recipePath, product: code, label: code, category: "Other",
          }), kind: "craft-output", detail: craft.producers.get(code).length + " Recipe" +
            (craft.producers.get(code).length === 1 ? "" : "s") }));
      } else result = await browse(ctx, category);
      if (generation !== listVersion) return;
      records = result.sort((a, b) => (category === "Sprites" ? a.category.localeCompare(b.category) : 0)
        || a.label.localeCompare(b.label));
      const tags = [...new Set(records.filter(spec => category !== "Sprites" || includesArt(spec))
        .flatMap(spec => [spec.category, spec.form].filter(Boolean)))].sort();
      const current = filter.value;
      filter.replaceChildren(el("option", { value: "" }, "All categories"),
        ...tags.map(tag => el("option", { value: tag }, tag)));
      filter.value = reset || !tags.includes(current) ? "" : current;
      const target = focus ?? selected ?? records.find(row =>
        row.label === ({ Items: "Copper ingots", Biomes: "Forest", Animals: "Cow", Deposits: "Copper" })[category]);
      const matched = target && records.find(row => typeof target === "string"
        ? row.product === target || row.deposit === target || keyOf(row) === target : keyOf(row) === keyOf(target));
      const extra = category === "Recipe Tree" && items.get(typeof target === "string" ? target : target?.product);
      const currentRow = matched || (extra ? { ...extra, kind: "craft-output" } : null);
      if (focus && matched) page = Math.floor(records.indexOf(matched) / pageSize());
      if (!currentRow) selected = undefined;
      renderList();
      if (currentRow) openDetail(currentRow);
      else {
        selected = undefined;
        activeDoc = undefined;
        if (category === "Recipes" || category === "Recipe Tree") return;
        inspector.replaceChildren(el("div", { className: "catalog-empty" }, catalogIcon(section),
          el("h2", {}, "Select " + ({ Items: "an item", Recipes: "a recipe", Biomes: "a biome", Animals: "an animal",
            Deposits: "a deposit", Creatures: "a creature", Locations: "a location", Sprites: "a sprite sheet",
            Models: "a model", "Hand poses": "a grip", "All files": "a file" }[section]))));
      }
    } catch (error) {
      if (generation === listVersion) list.replaceChildren(el("p", { className: "error" }, error.message));
    }
  }

  function changePage(next) { page = next; renderList(); list.scrollTop = 0; }

  function includesArt(spec) {
    const count = usersOf(spec).length;
    const matchesUsage = usageFilter.value === "all" || (usageFilter.value === "known" && count > 0)
      || (usageFilter.value === "unknown" && count === 0) || (usageFilter.value === "shared" && count > 1);
    return matchesUsage && (artFilter.value === "all" || spec.sprite?.mode === artFilter.value
      || (artFilter.value === "draft" && spec.modified) || (artFilter.value === "pinned" && pinIndex(spec) >= 0));
  }
  function usageOf(spec) { return spriteUsers.get(spec.path + "#" + spec.spriteIndex) ?? emptyUsage; }
  function usersOf(spec) { return usageOf(spec).users; }
  function spriteGraphic(spec) {
    const graphic = el("canvas", { className: "sprite-thumb", "aria-hidden": "true" });
    if (spec.entry) {
      const colors = new Map(spec.palette.map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
      canvasPixels(graphic, cells(spec.entry), 16, 16, pixel => colors.get(pixel) ?? 0);
    }
    return graphic;
  }
  async function spriteThumbnail(spec, canvas) {
    const key = spec.path + "|" + spec.sprite.revision;
    if (!spriteThumbs.has(key)) {
      const read = ctx.api("/api/preview", spec.path, undefined, true).then(async blob => {
        const bitmap = await createImageBitmap(blob);
        try {
          const scale = Math.min(1, 96 / Math.max(bitmap.width, bitmap.height));
          const thumb = el("canvas", { width: Math.max(1, Math.round(bitmap.width * scale)),
            height: Math.max(1, Math.round(bitmap.height * scale)) });
          const paint = thumb.getContext("2d");
          paint.imageSmoothingEnabled = false;
          paint.drawImage(bitmap, 0, 0, thumb.width, thumb.height);
          return thumb;
        } finally { bitmap.close(); }
      }).catch(() => null);
      if (spriteThumbs.size >= 96) spriteThumbs.delete(spriteThumbs.keys().next().value);
      spriteThumbs.set(key, read);
    }
    const image = await spriteThumbs.get(key);
    if (!image || !canvas.isConnected) return;
    canvas.width = image.width;
    canvas.height = image.height;
    canvas.getContext("2d").drawImage(image, 0, 0);
  }

  function renderList() {
    for (const hide of clearTooltips) hide();
    clearTooltips = [];
    const words = search.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const matches = records.filter(spec => (section !== "Sprites" || includesArt(spec))
      && (!filter.value || spec.category === filter.value || spec.form === filter.value)
      && words.every(word => [spec.label, spec.path, spec.product, spec.category, spec.form, summary(spec, itemNames),
        spec.entry?.id, ...usersOf(spec).map(item => item.label),
        ...usageOf(spec).related.map(item => item.label),
        ...usageOf(spec).produces.map(recipe => recipe.label), ...usageOf(spec).consumes.map(recipe => recipe.label),
        spec.inputCodes?.map(code => itemNames.get(code) ?? code).join(" ")].join(" ").toLowerCase().includes(word)));
    const label = spec => String(spec.label ?? "").toLowerCase();
    if (sort.value === "name") matches.sort((a, b) => label(a).localeCompare(label(b)));
    else if (sort.value === "name-desc") matches.sort((a, b) => label(b).localeCompare(label(a)));
    else if (sort.value === "category") matches.sort((a, b) => String(a.category ?? a.form ?? "").localeCompare(String(b.category ?? b.form ?? ""))
      || label(a).localeCompare(label(b)));
    else if (sort.value === "pinned") {
      const pinned = spec => prefs.pins.some(pin => pin.section === section && pin.key === keyOf(spec)) ? 0 : 1;
      matches.sort((a, b) => pinned(a) - pinned(b));
    }
    clearSearch.hidden = !search.value && !filter.value;
    const take = pageSize();
    page = Math.min(page, Math.max(0, Math.ceil(matches.length/take)-1));
    count.textContent = matches.length + (matches.length === 1 ? " entry" : " entries");
    thumbnails?.disconnect();
    const imageSpecs = new WeakMap();
    thumbnails = ["Models", "Sprites"].includes(section) ? new IntersectionObserver((entries, observer) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const tile = entry.target;
        observer.unobserve(tile);
        if (imageSpecs.has(tile)) { spriteThumbnail(imageSpecs.get(tile), tile.querySelector("canvas")); continue; }
        const path = tile.dataset.voxel;
        voxelThumbnail(ctx, { path }).then(image => {
          if (tile.isConnected) tile.querySelector(".catalog-icon").replaceChildren(image);
        }).catch(() => {});
      }
    }, { root: list }) : null;
    const tiles = matches.slice(page*take, (page+1)*take).map(spec => {
      const current = selected && keyOf(spec) === keyOf(selected);
      const card = button("", () => openDetail(spec), "", "catalog-tile");
      card.setAttribute("aria-current", String(Boolean(current)));
      card.setAttribute("aria-label", spec.label);
      if (spec.kind !== "sprite") card.setAttribute("aria-haspopup", "menu");
      card.oncontextmenu = event => {
        event.preventDefault();
        showContextMenu(card, commands(spec), [event.clientX, event.clientY]);
      };
      card.onkeydown = event => {
        if (event.altKey || event.metaKey || event.ctrlKey) return;
        if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
          event.preventDefault(); showContextMenu(card, commands(spec)); return;
        }
        if (spec.kind === "sprite" && /^Arrow/.test(event.key)) {
          const columns = list.classList.contains("as-list") ? 1 : getComputedStyle(list).gridTemplateColumns.split(" ").length;
          const offset = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns }[event.key];
          const next = tiles[tiles.indexOf(card) + offset];
          if (next) { event.preventDefault(); next.focus(); }
          return;
        }
        const key = event.key.toUpperCase();
        const command = commands(spec).find(item => item.key === key);
        if (command) { event.preventDefault(); command.run(); }
      };
      card.ondblclick = () => action(spec);
      card.dataset.key = keyOf(spec);
      dragSource(card, "record", card.dataset.key, spec.kind === "item" ? { item: spec.product } : undefined);
      if (spec.kind === "biome") dropTarget(card, ["animal-pin"], (_, key) => dropAnimal(key, spec));
      clearTooltips.push(slotTooltip(card, spec.label, [spec.category, summary(spec, itemNames),
        spec.kind === "item" ? "[R] Recipes · [U] Uses · [A] Bookmark" : "[RMB] Actions"]));
      let graphic = spec.product ? artwork?.icon(spec.product) : null;
      if (spec.kind === "sprite" && spec.entry) {
        graphic = spriteGraphic(spec);
      } else if (spec.kind === "sprite" && spec.sprite.revision) {
        graphic = el("canvas", { className: "sprite-thumb", "aria-hidden": "true" });
        imageSpecs.set(card, spec);
      }
      if (spec.kind === "vox") card.dataset.voxel = spec.path;
      card.append(el("span", { className: "catalog-icon", style: spec.color ? "color:" + spec.color : null },
        graphic ?? catalogIcon(section === "Animals" ? section
          : spec.kind === "creature" && spec.category !== "Skirmisher" ? spec.category : section)),
        el("div", { className: "catalog-tile-text" }, el("strong", {}, spec.label),
          el("span", { className: "catalog-tag" }, spec.category),
          summary(spec, itemNames) ? el("small", {}, summary(spec, itemNames)) : null));
      return card;
    });
    const scrollTop = list.scrollTop;
    list.replaceChildren(...tiles);
    if (section === "Sprites") list.scrollTop = scrollTop;
    if (thumbnails) for (const tile of tiles) if (tile.dataset.voxel || imageSpecs.has(tile)) thumbnails.observe(tile);
    if (!matches.length) list.append(quiet("No matches"));
    const previous = button("Previous", () => changePage(page-1));
    const next = button("Next", () => changePage(page+1));
    previous.disabled = !page; next.disabled = (page+1)*take >= matches.length;
    pager.replaceChildren(...(matches.length > take ? [previous,
      quiet((page*take+1) + "–" + Math.min((page+1)*take, matches.length) + " of " + matches.length), next] : []));
    if (!selected && section === "Recipes") inspector.replaceChildren(recipeBook(matches.slice(page * take, (page + 1) * take), matches.length, craftUI));
    if (!selected && section === "Recipe Tree") inspector.replaceChildren(recipeAtlas(craft, matches, craftUI));
  }

  search.oninput = filter.onchange = () => changePage(0);
  sort.onchange = () => { try { localStorage.setItem(sortKey, sort.value); } catch { /* private window */ } changePage(0); };
  document.addEventListener("keydown", event => {
    if (host.hidden || document.querySelector("dialog[open]")) return;
    if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.target.matches("input,textarea,select,[contenteditable]")) return;
    event.preventDefault(); search.focus(); search.select();
  });
  artFilter.onchange = () => { page = 0; filter.value = ""; refresh(); };
  usageFilter.onchange = artFilter.onchange;
  artScale.oninput = () => list.style.setProperty("--sprite-thumbnail", artScale.value + "px");
  search.onkeydown = event => {
    if (event.key === "Enter") list.firstElementChild?.click();
    if (event.key === "Escape" && (search.value || filter.value)) { event.preventDefault(); clearSearch.click(); }
    if (event.key === "ArrowDown") { event.preventDefault(); list.firstElementChild?.focus(); }
  };

  function onEdit(spec) { selected = spec; refresh(); }
  function focusFood(code) { setSection("Items", code); }
  function assignHabitat(animal, biome) {
    if (animal.id === "42") return ctx.note("Cave Salamander · Karst Placement");
    editHabitat(ctx, animal, null, () => setSection("Biomes", keyOf(biome)), biome.biome);
  }
  async function dropAnimal(key, biome) {
    try {
      const animal = (await browse(ctx, "Animals")).find(row => keyOf(row) === key);
      if (animal) assignHabitat(animal, biome);
    } catch (error) { ctx.note(error.message, true); }
  }
  async function chooseAnimal(biome) {
    try {
      const animals = (await browse(ctx, "Animals")).filter(animal => animal.id !== "42");
      findEntry("Animal · " + biome.label, animals.map(animal =>
        ({ key: animal.id, label: animal.label, detail: animal.detail })), null,
      id => assignHabitat(animals.find(animal => animal.id === id), biome));
    } catch (error) { ctx.note(error.message, true); }
  }

  function commands(spec) {
    const result = [];
    const add = (label, run, key) => result.push({ label, run, key });
    if (spec.kind === "item") {
      add("Recipes", () => openDetail(spec, "Recipes"), "R");
      add("Uses", () => openDetail(spec, "Uses"), "U");
      add("New Recipe", () => ctx.create(recipePath, spec.product));
      if (ctx.keepItem) add("Keep in Item Tray", () => ctx.keepItem(spec.product, spec.label));
      const food = /^Food\((\w+)\)$/.exec(spec.product)?.[1];
      if (food) add("Duplicate Food", () => createFood(ctx, focusFood, food));
      const deposit = deposits.find(item => item.items.includes(spec.product));
      if (deposit) add("Deposit Rules", () => setSection("Deposits", deposit.key));
    } else if (spec.kind === "recipe") add(spec.path === recipePath ? "Balance Recipe" :
      spec.path === fabricationPath ? "Edit Process" : "Edit Assembly", () => action(spec));
    else if (spec.kind === "creature") {
      add(isAnimal(spec) ? "Edit Animal" : "Edit Creature", () => action(spec));
      if (!isAnimal(spec) || spec.body) add("Duplicate", () => editCreature(ctx, spec, onEdit, true));
    } else if (spec.kind === "biome") add("Add Animal", () => chooseAnimal(spec));
    else add("Inspect", () => openDetail(spec));
    if (spec.product) add("Recipe Tree", () => showPlan(spec.product, spec.kind === "recipe" ? spec : undefined), "T");
    if (spec.kind === "recipe") add("Copy Recipe Context", () => copyRecipe(spec));
    if (ctx.assist && spec.kind !== "craft-output") add("Ask AI", () => ctx.assist(spec));
    const sprite = spec.product && artwork?.sprite(spec.product);
    if (sprite) add("Edit Sprite", () => ctx.openSprite(sprite.path, sprite.index));
    else if (ctx.editableSprite(spec.path)) add("Edit Sprite", () => ctx.openSprite(spec.path, spec.spriteIndex ?? null));
    if (spec.kind !== "deposit") add(spec.kind === "recipe" && spec.path === recipePath ? "Wire Recipe" : "Graph", () => ctx.graph(spec));
    add("File", () => ctx.open(spec.path, undefined, spec.line));
    add(pinIndex(spec) >= 0 ? "Remove Bookmark" : "Bookmark", () => togglePin(spec), "A");
    return result;
  }

  function action(spec) {
    if (spec.kind === "sprite" && spec.sprite.mode !== "source") {
      if (spec.sprite.generator) return ctx.open(spec.sprite.generator);
      ctx.dialog(spec.label, [quiet(spec.sprite.issue ?? (spec.sprite.mode === "generated"
        ? "This image is generated. Its editable input is not registered in Forge." : "Reference artwork.")),
      button("Open File", () => { document.querySelector("dialog[open]")?.close(); ctx.open(spec.path); })], "Done", () => true);
    } else if (spec.kind === "recipe") {
      if (spec.path !== recipePath) return ctx.graph(spec);
      balanceRecipe(ctx, spec, next => { if (section === "Recipes") onEdit(next); else refresh(); });
    }
    else if (spec.kind === "creature") editCreature(ctx, spec, onEdit);
    else if (["vox","glb","hand","biome","deposit"].includes(spec.kind)) openDetail(spec);
    else if (ctx.editableSprite(spec.path)) ctx.openSprite(spec.path, spec.spriteIndex ?? null);
    else if (spec.line) ctx.open(spec.path, undefined, spec.line);
    else ctx.graph(spec);
  }

  function relation(title, specs) {
    if (!specs.length) return null;
    const rows = specs.slice(0, 24).map(spec => {
      const control = button("", () => spec.kind === "recipe" || spec.kind === "item" ? follow(spec) : ctx.graph(spec), spec.path,
        spec.kind === "recipe" ? "recipe-relation" : "");
      if (spec.kind === "recipe") {
        control.append(artwork?.icon(spec.product) ?? catalogIcon("Recipes"), el("strong", {}, spec.label),
          el("span", {}, spec.count + "× · " + spec.category));
      } else control.textContent = spec.label + (spec.detail ? " · " + spec.detail : "");
      return control;
    });
    const rest = specs.length > rows.length ? el("small", { className: "muted" }, rows.length + " / " + specs.length) : null;
    return el("section", { className: "catalog-related" }, el("h3", {}, title), el("div", {}, rows), rest);
  }

  function follow(spec) {
    const next = spec.kind === "recipe" ? "Recipes" : "Items";
    const match = Array.from(list.children).find(tile => tile.dataset.key === keyOf(spec));
    if (section === next && match) {
      openDetail(spec);
      match.scrollIntoView({ block: "nearest" });
    } else setSection(next, spec);
  }

  function showPlan(code, route) {
    if (planState.code !== code) { planState.code = code; planState.count = 1; }
    planState.direction = "inputs";
    if (route) planState.choices.set(code, recipeId(route));
    setSection("Recipe Tree", code);
  }
  const copy = text => copyText(text).then(() => ctx.note("Context copied"))
    .catch(error => ctx.note(error.message, true));
  const copyRecipe = spec => copy([spec.label + " · " + spec.category, "Source: " + spec.path,
    "Output: " + spec.count + " × " + (itemNames.get(spec.product) ?? spec.product),
    "Inputs:", ...spec.inputs.map(item => "- " + item.count + " × " + (itemNames.get(item.code) ?? item.code)),
    spec.patternSlots ? "Pattern: " + spec.patternSlots.map(row => row.join(" / ")).join(" | ") : ""].filter(Boolean).join("\n"));
  const craftUI = {
    name: code => itemNames.get(code) ?? code,
    icon: code => artwork?.icon(code),
    item: code => { const item = items.get(code); if (item) follow(item); },
    recipe: follow, edit: recipe => ctx.graph(recipe), plan: showPlan, copy, assist: ctx.assist, itemActions: ctx.itemActions,
    atlas: () => setSection("Recipe Tree"), web: showCraftWeb,
  };

  async function showCraftWeb(code) {
    try {
      const doc = await ctx.load(recipePath);
      const available = new Map((await recipeItems(ctx, doc)).map(item => [item.code, item]));
      const known = value => items.has(value) || available.has(value);
      const visual = value => artwork?.sprite(value);
      const art = value => visual(value) ? [visual(value)] : artwork?.parts(value) ?? [];
      const allRoutes = () => [...recipeRows(ctx, doc), ...recipes.filter(route => route.path !== recipePath)];
      const initial = allRoutes();
      const choices = () => [...new Set([...available.keys(), ...items.keys(),
        ...initial.flatMap(route => [route.product, ...route.inputs.map(input => input.code)])])].filter(Boolean)
        .map(key => ({ key, label: itemNames.get(key) ?? available.get(key)?.label ?? key, detail: key }));
      const root = code || initial[0]?.product || available.keys().next().value;
      if (!root) throw new Error("No crafting items are indexed.");
      function prepare(route, blocks) {
        const baseline = doc.text;
        const next = previewRecipeBlocks(ctx, doc, route, blocks, available);
        const candidate = { ...doc, text: next.text };
        const updated = [...recipeRows(ctx, candidate), ...recipes.filter(recipe => recipe.path !== recipePath)];
        const target = updated.find(recipe => recipeId(recipe) === recipeId(next.spec));
        if (!target) throw new Error("The updated route could not be resolved.");
        return { changed: next.changed, routes: updated, route: target, stage() {
          if (doc.text !== baseline) throw new Error("The recipe draft changed. Preview again.");
          if (next.changed && !ctx.edit(doc, next.text)) throw new Error("Could not stage the recipe draft.");
        } };
      }
      openCraftWeb(root, initial, {
        name: craftUI.name, icon: craftUI.icon, known, art, entries: choices, canWire: value => available.has(value),
        stackLimit: value => available.get(value)?.limit ?? 64,
        create: preset => ctx.create(recipePath, preset), review: () => ctx.review(doc), dirty: () => ctx.dirty(doc),
        canUndo: action => Boolean(doc[action].length),
        history: action => { ctx[action](doc); return allRoutes(); },
        openItem: craftUI.item, openSprite: ctx.openSprite,
        preview: prepare,
        suggest(route, audit) {
          const related = [...audit.alternatives.slice(0, 5), ...audit.downstream.slice(0, 8)];
          const codes = new Set([route.product, ...route.inputs.map(input => input.code),
            ...related.flatMap(row => [row.product, ...row.inputs.map(input => input.code)])]);
          const summary = ({ label, category, form, inputs, patternSlots, product, count, description }) =>
            ({ label, category, form, inputs, patternSlots, product, count, description });
          return ctx.api("/api/craft/suggest", undefined, { route: summary(route), issues: audit.issues,
            alternatives: audit.alternatives.slice(0, 5).map(summary),
            downstream: audit.downstream.slice(0, 8).map(summary),
            items: [...codes].filter(code => available.has(code)).slice(0, 40).map(code => available.get(code)) });
        },
        edit: route => route.path === recipePath ? ctx.wireRecipe(route) : ctx.graph(route),
        assist(route, audit, sprites) {
          const attachments = [route, ...sprites.map(sprite => ({ path: sprite.path, spriteId: sprite.entry.id,
            label: sprite.entry.name }))].slice(0, 5);
          ctx.assist(attachments, [
            "Improve the selected crafting route, description, and editable output art where warranted. Preserve authored style, item identity, and shared sprite masks.",
            "Route: " + route.label + " (" + route.category + "). Inputs: " + route.inputs.map(input =>
              input.count + " × " + craftUI.name(input.code)).join(", ") + ". Output: " + route.count + " × " + craftUI.name(route.product) + ".",
            "Checks: " + (audit.issues.join(" | ") || "None flagged") + ".",
            "Alternatives: " + (audit.alternatives.slice(0, 6).map(item => item.label).join(", ") || "none") + ".",
            "Downstream: " + (audit.downstream.slice(0, 8).map(item => item.label).join(", ") || "none") + ".",
            "Propose a small coordinated candidate. Explain changes in yield, interoperability and playtest coverage.",
          ].join("\n"));
        },
        change(route, blocks) {
          const next = prepare(route, blocks);
          next.stage();
          return next;
        },
      });
    } catch (error) { ctx.note(error.message, true); }
  }

  async function openDetail(spec, focus) {
    selected = spec;
    activeDoc = undefined;
    const generation = ++detailVersion;
    for (const tile of list.children) {
      if (tile.dataset.key) tile.setAttribute("aria-current", String(tile.dataset.key === keyOf(spec)));
    }
    if (spec.kind === "craft-output") {
      if (planState.code !== spec.product) { planState.code = spec.product; planState.count = 1; planState.direction = "inputs"; }
      inspector.replaceChildren(recipePlan(craft, spec.product, planState, craftUI));
      inspector.scrollTop = 0;
      return;
    }
    const visual = spec.product ? artwork?.icon(spec.product)
      : spec.kind === "sprite" && (spec.entry || spec.sprite.revision) ? spriteGraphic(spec) : null;
    const body = el("div", { className: "catalog-detail-body" });
    const heading = el("div", { className: "catalog-detail-title" },
      el("span", { className: "catalog-icon" }, visual ?? catalogIcon(section)),
      el("div", {}, el("span", { className: "catalog-tag" }, spec.category), el("h2", {}, spec.label)));
    const tools = el("div", { className: "catalog-detail-tools" });
    const more = button("Actions", () => showContextMenu(more, commands(spec)));
    more.setAttribute("aria-haspopup", "menu");
    const bookmark = button("◆", () => togglePin(spec), "Bookmark [A]", "catalog-bookmark");
    bookmark.setAttribute("aria-label", "Bookmark");
    bookmark.setAttribute("aria-pressed", String(pinIndex(spec) >= 0));
    heading.append(bookmark);
    if (spec.kind === "biome") dropTarget(heading, ["animal-pin"], (_, key) => dropAnimal(key, spec));
    inspector.replaceChildren(heading, tools, body);
    inspector.scrollTop = 0;
    if (spec.kind === "sprite") {
      const users = usersOf(spec);
      if (visual && !spec.entry) spriteThumbnail(spec, visual);
      tools.append(button(spec.sprite.mode === "source" ? "Paint sprite" : "Open source", () => action(spec), "", "primary"));
      if (spec.entry && spec.path === "assets/icons/items16.txt") {
        const template = users.find(item => item.product?.startsWith("Food("))?.product.slice(5, -1);
        tools.append(button("Make food variant", () => createFood(ctx, focusFood, template, Number(spec.entry.id.split(" · ")[0]))));
      }
      const usage = usageOf(spec);
      body.append(quiet(spec.detail), quiet("Source: " + spec.path + (spec.entry ? " · " + spec.entry.id : "")), relation("Used by", users)
        ?? quiet("No game-object references are indexed for this asset."));
      for (const [title, rows] of [["Recipes producing these items", usage.produces], ["Used in crafting", usage.consumes],
        ["Applies this effect", usage.related]]) {
        const links = relation(title, rows);
        if (links) body.append(links);
      }
      body.append(quiet("Recipe links show how items are made, not whether their ingredients can be found in game."),
        quiet("Coverage: item art, composed tool masks, status effects, food effects and indexed recipes. Other dynamic references remain unknown."));
      const sourceLinks = button("Find source references", async () => {
        sourceLinks.disabled = true;
        sourceLinks.textContent = "Finding references…";
        try {
          const data = await ctx.api("/api/usages", spec.path);
          if (generation !== detailVersion) return;
          body.append(relation("Direct source references", data.map(row => ({ ...row,
            label: filename(row.path) + ":" + row.line }))) ?? quiet("No direct source references found."));
        } catch (error) { ctx.note(error.message, true); sourceLinks.disabled = false; }
        sourceLinks.textContent = "Find source references";
      });
      body.append(sourceLinks);
      if (spec.entry?.palette.some(color => color.shared)) body.append(quiet("The outline swatch is shared by every icon in this sheet."));
      if (spec.path === "assets/tools/sprites16.txt") body.append(quiet("Mask edits affect these tool compositions. Material ramps are shared across every tool mask."));
      if (spec.path === "assets/icons/items16.txt" && spec.entry && /^[678] · /.test(spec.entry.id)) {
        body.append(quiet("Legacy pickaxe art. Native pickaxes use composed tool masks before this sheet."));
      }
      if (users.length > 1) body.append(quiet("Painting this shared icon updates every object listed above."));
      body.append(more);
      return;
    }
    if (spec.kind === "recipe" || spec.kind === "creature") {
      const animal = isAnimal(spec);
      tools.append(button(spec.kind === "recipe" ? spec.path === recipePath ? "Balance recipe" :
        spec.path === fabricationPath ? "Edit Process" : "Edit Assembly"
        : animal ? "Edit animal" : "Edit creature", () => action(spec), "", "primary"));
      if (spec.kind === "creature" && !animal) tools.append(button("Duplicate as variant", () => editCreature(ctx, spec, onEdit, true)));
      if (spec.body) tools.append(button("Make animal like this", () => editCreature(ctx, spec, onEdit, true)));
    }
    const sprite = spec.product && artwork?.sprite(spec.product);
    const artParts = spec.product ? artwork?.parts?.(spec.product) ?? [] : [];
    if (artParts.length) tools.append(button("Inspect composed art", () => setSection("Sprites", keyOf({
      path: artParts[0].path, spriteIndex: artParts[0].index,
    }))));
    if (sprite) tools.append(button("Inspect sprite", () => setSection("Sprites", keyOf({
      path: sprite.path, spriteIndex: sprite.index,
    })), sprite.entry.name));
    else if (ctx.editableSprite(spec.path)) tools.append(button("Edit sprite", () => ctx.openSprite(spec.path, spec.spriteIndex ?? null), "", "primary"));
    if (spec.kind === "biome") tools.append(button("+ Animal", () => chooseAnimal(spec), "Add animal habitat"));
    tools.append(more);
    if (spec.kind === "recipe" && spec.path === recipePath) tools.append(button("Wire Recipe", () => ctx.graph(spec)));
    if (spec.product) tools.append(button("Tree", () => showPlan(spec.product, spec.kind === "recipe" ? spec : undefined)));
    if (spec.kind === "recipe" && ctx.playRecipe) {
      tools.append(button("Try", () => ctx.playRecipe(spec), "Build saved objects and art with the recipe draft. Preview ingredients can be supplied; gathering and progression are not verified."));
    }
    try {
      const doc = await ctx.load(spec.path);
      if (generation !== detailVersion) return;
      activeDoc = doc;
      const save = button("Review & save", () => ctx.review(doc), spec.path);
      const undo = button("Undo", () => { ctx.undo(doc); refresh(); }, "Undo the last draft change");
      const state = el("span", { className: "catalog-count", role: "status" });
      const changed = () => {
        save.disabled = !ctx.dirty(doc); undo.disabled = !doc.undo.length;
        state.textContent = ctx.dirty(doc) ? "Unsaved draft" : "";
      };
      changed();
      tools.append(undo, save, state);
      if (spec.kind === "vox") body.append(voxelStudio(ctx, doc, changed));
      else if (spec.kind === "deposit") {
        const deposit = deposits.find(item => item.key === spec.deposit);
        body.append(depositStudio(ctx, doc, spec, changed),
          button("Browse " + deposit.label.toLowerCase() + " item", () => setSection("Items", deposit.items[0])));
      }
      else if (spec.kind === "hand" || spec.kind === "glb") {
        body.append(quiet("Loading preview…"));
        const studio = spec.kind === "hand" ? await poseStudio(ctx, doc, changed) : await meshPreview(ctx, spec);
        if (generation !== detailVersion) return;
        body.replaceChildren(studio);
      } else if (spec.kind === "recipe") {
        body.append(el("section", { className: "craft-window craft-single" },
          el("header", { className: "craft-titlebar" }, el("h3", {}, spec.category + " · " + spec.form),
            el("div", {}, button("Crafting Web", () => showCraftWeb(spec.product)), button("Copy Context", () => copyRecipe(spec)),
              ctx.assist ? button("Ask AI", () => ctx.assist(spec)) : null)),
          recipePlate(spec, craftUI), spec.description ? el("details", {}, el("summary", {}, "Recipe Note"),
            el("p", {}, spec.description)) : null));
        body.append(...[
          relation("Other Recipes", recipes.filter(row => keyOf(row) !== keyOf(spec) && row.product === spec.product)),
          relation("Uses", recipes.filter(row => row.inputCodes.includes(spec.product))),
        ].filter(Boolean));
      } else if (spec.kind === "item") {
        const block = /^Block\((\w+)\)$/.exec(spec.product)?.[1];
        if (block) tools.append(button("Material settings", () => ctx.editMaterial(block)),
          button("Make material variant", () => ctx.editMaterial(block, true)));
        const deposit = deposits.find(item => item.items.includes(spec.product));
        const madeBy = recipes.filter(row => row.product === spec.product);
        const appearance = sprite ? button("Paint icon", () => ctx.openSprite(sprite.path, sprite.index)) : null;
        const food = /^Food\((\w+)\)$/.exec(spec.product)?.[1];
        const sectionCard = (title, ...contents) => el("section", { className: "catalog-related" }, el("h3", {}, title), ...contents);
        body.append(sectionCard("Appearance", appearance ?? quiet(artParts.length
          ? "Composed from shared masks and material ramps." : "No editable icon mapping is indexed."),
          ...artParts.map(part => button("Paint " + part.entry.name.replaceAll("_", " "), () => ctx.openSprite(part.path, part.index))),
          food ? button("Make food variant", () => createFood(ctx, focusFood, food)) : null));
        const definition = food && resolve(ctx.tree(doc), spec.parts ?? [])?.node;
        if (food && definition) {
          body.append(sectionCard("Behavior", quiet(["hunger", "health", "saturation"].map(key =>
            key + " " + (textOf(field(definition, key)) || "0")).join(" · ")),
          button("Edit food definition", () => ctx.graph(spec))));
        }
        if (spec.machine) {
          const machineDoc = await ctx.load(spec.machine.path);
          if (generation !== detailVersion) return;
          const row = resolve(ctx.tree(machineDoc), spec.machine.parts)?.node;
          const labels = { generator: "Generates power", powered: "Uses power", stores_power: "Stores power",
            conduit: "Conduit", water_port: "Water port", upgradable: "Upgradeable", structural: "Structural" };
          const flags = Object.keys(labels).filter(key => textOf(field(row, key)) === "true");
          const inputs = (field(row, "accepts")?.children ?? []).map(value =>
            craftUI.name("Resource(" + textOf(value) + ")"));
          const kind = textOf(field(row, "kind"));
          const appearancePanel = body.firstElementChild;
          appearancePanel.append(button("Material and surface", () => ctx.graph(spec)),
            kind === "Furnace" ? button("Furnace mesh", () => ctx.graph({ path: "assets/models/machines/furnace.ron" }))
              : button("Native model and animation", () => ctx.open("crates/app/src/mesh/models/machine_casing.rs")));
          body.append(sectionCard("Behavior", quiet(textOf(field(row, "kind")) || spec.label),
            button("Open machine definition", () => ctx.graph(spec.machine)),
            button("Processing rules", () => ctx.open("crates/app/src/world/factory/recipes.rs"))),
          sectionCard("Connections", quiet([...flags.filter(flag => flag !== "upgradable").map(flag => labels[flag]), ...inputs].join(" · ")
            || "No connections declared in the machine definition.")),
          sectionCard("Progression", quiet(flags.includes("upgradable") ? "Has upgrade tiers" : "Assembly cost and alternatives appear under Recipes."),
            flags.includes("upgradable") ? button("Inspect upgrade costs", () => ctx.graph(spec.machine)) : null));
        }
        for (const [name, entries] of [
          ["Recipes", madeBy],
          ["Uses", recipes.filter(row => row.inputCodes.includes(spec.product))],
        ]) {
          const panel = relation(name, entries) ?? (focus === name
            ? el("section", { className: "catalog-related" }, el("h3", {}, name), quiet("None")) : null);
          if (panel) {
            body.append(panel);
            if (focus === name) {
              panel.tabIndex = -1;
              requestAnimationFrame(() => { panel.focus({ preventScroll: true }); panel.scrollIntoView({ block: "nearest" }); });
            }
          }
        }
        body.append(sectionCard("Getting this item",
          quiet(madeBy.length ? madeBy.length + (madeBy.length === 1 ? " indexed recipe makes" : " indexed recipes make") + " this item. Ingredient availability and progression are not verified."
            : "No indexed recipe makes this item. It may still come from gathering, loot or trade; these sources are not fully indexed."),
          button("Inspect ingredient plan", () => showPlan(spec.product), "See named ingredients, quantities and supplies outside the recipe plan"),
          button("Crafting Web", () => showCraftWeb(spec.product)),
          button("+ Recipe", () => ctx.create(recipePath, spec.product), "New recipe for " + spec.label),
          deposit ? button("Deposit Rules", () => setSection("Deposits", deposit.key)) : null));
        const preview = madeBy[0];
        if (ctx.playRecipe && preview) {
          body.append(sectionCard("Evidence", button("Try crafting in Axiom", () => ctx.playRecipe(preview)),
            quiet("Builds saved game objects and art with the current recipe draft. The preview can supply ingredients; it does not verify gathering or progression.")));
        }
      } else if (spec.kind === "creature" || spec.kind === "biome") {
        if (spec.kind === "creature" && spec.detail) body.append(quiet(spec.detail));
        if (spec.kind === "biome") body.append(biomeStudio(ctx, doc, spec, changed));
        const [rules, spawns] = await Promise.all([spawnLinks(ctx, spec.kind === "creature" ? spec.id : undefined, spec.biome),
          ctx.load(spawnPath)]);
        if (generation !== detailVersion) return;
        body.append(habitatCards(ctx, spawns, rules, spec.kind === "creature" ? spec : null, refresh));
      } else {
        body.append(quiet(spec.path), button(spec.line ? "Open definition" : "Open file",
          () => ctx.open(spec.path, undefined, spec.line)));
      }
      if (["vox", "glb", "hand"].includes(spec.kind)) {
        if (spec.kind === "vox") {
          const conduit = /^(?:pipe|cable|fluid)_/.test(filename(spec.path));
          body.append(relation("Model code", [{ path: conduit
            ? "crates/app/src/mesh/models/conduit.rs" : "crates/app/src/mesh/models/machine_casing.rs",
          label: conduit ? "Conduit renderer" : "Machine model renderer" }]));
        }
        if (spec.kind === "hand") {
          body.append(relation("Related art", [
            { path: "assets/runtime/viewmodels/hand_v6/hand.glb", label: "Authored hand mesh" },
            { path: "assets/runtime/viewmodels/artificer_tally/wand.glb", label: "Artificer wand mesh" },
          ]));
        }
        const connections = button("Find source links", async () => {
          connections.disabled = true;
          try {
            const data = await ctx.api("/api/usages", spec.path);
            body.append(relation("Referenced by", data.map(row => ({ ...row, label: filename(row.path) + ":" + row.line })))
              ?? quiet("No direct source references found"));
          } catch (error) { ctx.note(error.message, true); }
        });
        body.append(connections);
      }
      ctx.api("/api/notes").then(notes => {
        if (generation !== detailVersion) return;
        const parents = notes.filter(note => note.path !== spec.path && note.links.some(link =>
          link.path === spec.path && (!spec.parts || !link.parts.length || JSON.stringify(link.parts) === JSON.stringify(spec.parts))));
        const backlinks = relation("Linked notes", parents.map(note => ({ path: note.path, label: note.title })));
        if (backlinks) body.append(backlinks);
      }).catch(() => {});
    } catch (error) {
      if (generation === detailVersion) body.append(el("p", { className: "error" }, error.message));
    }
  }

  return {
    showSprites() { if (section !== "Sprites") setSection("Sprites"); },
    planItem: showPlan,
    inspectItem(code) { setSection("Items", code); },
    refresh, setSection, review: () => ctx.review(activeDoc),
    async restore() {
      try { prefs = await ctx.api("/api/workbench"); } catch { /* Defaults. */ }
      applyLayout();
      renderPins();
    },
  };
}

