import { el, button } from "./inspector.mjs";
import { recipePath, recipeItems } from "./idea-data.mjs";
import { recipeRows } from "./catalog-data.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";
import { address } from "./targets.mjs";
import { bindItemActions, dragSource, dropTarget, itemPalette, slotArt } from "./slot-tools.mjs";
import { previewRecipeBlocks, readRecipeWiring, writeRecipeWiring } from "./recipe-wiring-data.mjs";
import { openRecipeBlocks } from "./recipe-blocks.mjs";
import { wireCanvas } from "./wire-canvas.mjs";
import { canvasView } from "./canvas-view.mjs";

const slotId = i => "slot:" + i;
const isSlot = target => /^slot:[0-9]$/.test(target);
const slotIndex = target => Number(target.slice(5));
const isMaterial = target => target.startsWith("item:");
const clipboardType = "application/x-axiom-recipe";
const label = (title, control) => el("label", {}, title, control);
const section = (title, ...body) => el("section", { className: "wire-section" }, el("h3", {}, title), body);
const boardPoint = (surface, event) => {
  const bounds = surface.getBoundingClientRect();
  return { x: event.clientX - bounds.left - 18, y: Math.max(4, event.clientY - bounds.top - 16) };
};

export function mountRecipeWiring(host, ctx) {
  let doc, items, artwork, byCode, spec, state, ticket = 0, selection = "recipe", armed;
  const boards = new Map();
  let board, picker, lastPointer, renderedWidth;
  const visiblePositions = new Map();
  const title = el("h1", {}, "Recipe Wiring");
  const status = el("span", { className: "wire-status", role: "status" });
  const error = el("p", { className: "wire-error", role: "status", hidden: true });
  const search = el("input", { type: "search", placeholder: "Find a recipe", "aria-label": "Find a wiring recipe" });
  const results = el("div", { className: "wire-recipe-list", "aria-label": "Wiring recipes" });
  const palette = el("div", { className: "wire-palette" });
  const inspector = el("aside", { className: "wire-inspector", "aria-label": "Wire inspector" });
  const scroll = el("div", { className: "wire-scroll", "aria-label": "Recipe wire canvas", tabIndex: 0 });
  const view = canvasView(scroll, { onChange: () => renderBoard() });
  new ResizeObserver(() => {
    if (state && scroll.clientWidth && scroll.clientWidth !== renderedWidth) renderBoard();
  }).observe(scroll);
  const undo = button("Undo", () => history("undo"));
  const redo = button("Redo", () => history("redo"));
  const review = button("Review & Save", () => ctx.review(doc), "", "primary");
  const fit = button("Reset Layout", () => { board.positions.clear(); renderBoard(); });
  const addMaterial = button("+ Material", () => pick("board"), "Find a material and add a loose card");
  const useTray = button("Use Tray Item", () => {
    const code = ctx.trayItem?.();
    if (!state || !byCode?.has(code)) return report("Keep a supported item in the tray first.");
    armed = code;
    updateArmed();
    report();
  }, "Select a tray item, then choose a cell or the output");
  const armedLabel = el("span", { className: "wire-armed", role: "status" });
  const tools = el("div", { className: "wire-tools" }, undo, redo, review, fit);
  const actionBlocks = button("Action Blocks", compose, "Stack recipe edits and preview the result");
  actionBlocks.disabled = true;
  host.append(el("header", { className: "wire-toolbar" }, title, status, tools),
    el("div", { className: "wire-layout" },
      el("aside", { className: "wire-library", "aria-label": "Wiring library" },
        el("section", { className: "wire-library-recipes" }, el("div", { className: "wire-section-head" },
          el("h2", {}, "Recipes"), button("+ Recipe", () => ctx.createRecipe())), search, results),
        palette),
      el("section", { className: "wire-main" }, el("div", { className: "wire-canvas-bar" },
        addMaterial, useTray, actionBlocks, el("span", {}, "Drop items on cells or the board"), armedLabel), scroll, error),
      inspector));

  const item = code => byCode.get(code) ?? { code, label: code };
  const art = code => slotArt(item(code), artwork);
  const itemCode = target => board.cards.get(target)?.code ?? (isMaterial(target) ? target.slice(5) : "");
  const material = target => isMaterial(target) || board.cards.has(target);
  function report(message = "") { error.textContent = message; error.hidden = !message; }
  function updateArmed() {
    armedLabel.textContent = armed ? "Selected · " + item(armed).label : "";
  }

  async function open(target) {
    const request = ++ticket;
    try {
      const source = await ctx.load(recipePath);
      if (!items || doc?.version !== source.version) {
        const [available, icons] = await Promise.all([recipeItems(ctx, source), recipeArtwork(ctx).catch(() => null)]);
        if (request !== ticket) return;
        items = available; artwork = icons; byCode = new Map(items.map(value => [value.code, value]));
        palette.replaceChildren(itemPalette(items, artwork, value => { armed = value.code; updateArmed(); }, true));
      }
      if (request !== ticket) return;
      doc = source;
      const recipes = recipeRows(ctx, doc);
      const wanted = target ?? spec;
      const chosen = recipes.find(row => JSON.stringify(row.parts) === JSON.stringify(wanted?.parts)) ??
        recipes.find(row => row.label === "Anvil") ?? recipes[0];
      if (!chosen) throw new Error("No crafting recipes");
      const next = readRecipeWiring(ctx, doc, chosen);
      const id = next.trail.join("/");
      if (!boards.has(id)) boards.set(id, { positions: new Map(), cards: new Map(), sources: new Map(), recent: [], serial: 0 });
      if (boards.get(id) !== board) {
        board = boards.get(id);
        selection = "recipe";
        armed = undefined;
        lastPointer = undefined;
      }
      spec = chosen;
      state = next;
      report();
      render();
    } catch (error) {
      if (request === ticket) report(error.message);
    }
  }
  function renderList() {
    if (!doc) return;
    const words = search.value.trim().toLowerCase().split(/\s+/);
    const matches = recipeRows(ctx, doc).filter(row => words.every(word =>
      [row.label, row.category, row.product].join(" ").toLowerCase().includes(word)))
      .sort((a, b) => a.label.localeCompare(b.label));
    results.replaceChildren(...matches.map(row => {
      const entry = button("", () => open(row), row.label, "wire-recipe-row");
      entry.setAttribute("aria-label", row.label);
      entry.setAttribute("aria-current", String(JSON.stringify(row.parts) === JSON.stringify(spec.parts)));
      entry.append(art(row.product), el("span", {}, row.label));
      return entry;
    }));
  }
  search.oninput = renderList;
  search.onkeydown = event => { if (event.key === "Enter") results.firstElementChild?.click(); };
  function render() {
    actionBlocks.disabled = !state;
    title.textContent = state.name;
    status.textContent = ctx.dirty(doc) ? "Draft · recipes.ron" : "Saved · recipes.ron";
    undo.disabled = !doc.undo.length;
    redo.disabled = !doc.redo.length;
    review.disabled = !ctx.dirty(doc);
    updateArmed();
    renderList();
    renderBoard();
    renderInspector();
  }

  function compose() {
    if (!state) return;
    const documentAtOpen = doc, baseline = doc.text, target = spec;
    const check = () => {
      if (doc !== documentAtOpen || doc.text !== baseline || spec !== target) {
        throw new Error("The recipe changed. Reopen Action Blocks to work from the latest draft.");
      }
    };
    openRecipeBlocks(state, byCode, board.blocks ??= [], blocks => {
      check();
      return previewRecipeBlocks(ctx, doc, target, blocks, byCode);
    }, next => {
      check();
      if (!next?.changed || !ctx.edit(doc, next.text)) throw new Error("The action stack could not be staged.");
      spec = next.spec;
      state = readRecipeWiring(ctx, doc, spec);
      board.sources.clear();
      selection = "recipe";
      armed = undefined;
      report();
      render();
    });
  }

  function update(change, focus = selection, after) {
    try {
      // Re-resolve the named recipe before applying to the current draft.
      const source = readRecipeWiring(ctx, doc, spec);
      if (source.node !== state.node) {
        state = source;
        render();
        throw new Error("Recipe changed in another view. Try the edit again.");
      }
      const draft = { ...state, slots: state.slots.map(slot => slot && { ...slot }) };
      change(draft);
      const next = writeRecipeWiring(ctx, doc, state, draft, byCode);
      if (next.text !== doc.text) {
        if (!ctx.edit(doc, next.text)) return;
        spec = next.spec;
        state = readRecipeWiring(ctx, doc, spec);
      }
      after?.();
      selection = focus;
      armed = undefined;
      report();
      render();
    } catch (error) { report(error.message); }
  }
  function history(direction) {
    if (!state) return;
    const trail = state.trail;
    if (!doc[direction]?.length) return;
    ctx[direction](doc);
    try {
      spec = { ...spec, parts: address(ctx.tree(doc), trail) };
      state = readRecipeWiring(ctx, doc, spec);
      report();
      render();
    } catch { open(); }
  }
  function inspect(target) {
    selection = target;
    renderInspector();
    scroll.querySelectorAll("[data-wire-target]").forEach(node =>
      node.classList.toggle("wire-selected", node.dataset.wireTarget === selection));
  }
  function connect(code, target, source, count) {
    if (!byCode.has(code)) return;
    update(draft => {
      if (target === "output") {
        if (count !== undefined) draft.count = count;
        else if (draft.product !== code) draft.count = Math.min(draft.count, item(code).limit ?? 64);
        draft.product = code;
      }
      else if (isSlot(target)) {
        const index = slotIndex(target);
        const old = draft.slots[index];
        draft.slots[index] = { code, count: draft.shaped ? 1 : count ?? old?.count ?? 1, symbol: old?.symbol };
      }
    }, target, () => {
      if (target !== "output" && board.cards.has(source)) board.sources.set(target, source);
      else board.sources.delete(target);
      board.recent = [code, ...board.recent.filter(value => value !== code)].slice(0, 12);
    });
  }
  function sourceOf(target, code) {
    const key = board.sources.get(target);
    return board.cards.get(key)?.code === code ? key : "item:" + code;
  }
  function canMove(from, to) {
    if (to === "board") return true;
    if (material(to)) {
      const code = from === "output" ? state.product : state.slots[slotIndex(from)]?.code;
      return to !== sourceOf(from, code);
    }
    return isSlot(from) && isSlot(to) && from !== to;
  }
  function moveWire(from, to, event) {
    if (to === "board") {
      const copied = copyValue(from);
      if (copied) addCard(copied.code, boardPoint(scroll.firstElementChild, event), copied.count);
      return;
    }
    if (material(to)) return connect(itemCode(to), from, to);
    const fromSource = board.sources.get(from), toSource = board.sources.get(to);
    update(draft => {
      const a = slotIndex(from), b = slotIndex(to);
      [draft.slots[a], draft.slots[b]] = [draft.slots[b] ?? null, draft.slots[a]];
      if (!draft.shaped) draft.slots = draft.slots.filter(Boolean);
    }, !state.shaped && slotIndex(to) === state.slots.length ? slotId(state.slots.length - 1) : to, () => {
      board.sources.delete(from);
      board.sources.delete(to);
      if (toSource) board.sources.set(from, toSource);
      if (fromSource && state.shaped) board.sources.set(to, fromSource);
    });
  }
  function disconnect(target) {
    if (!isSlot(target)) return;
    update(draft => {
      const index = slotIndex(target);
      if (draft.shaped) draft.slots[index] = null;
      else draft.slots.splice(index, 1);
    }, state.shaped ? target : "recipe", () => board.sources.delete(target));
  }
  function activate(target, anchor) {
    if (armed) {
      const code = armed;
      armed = undefined;
      updateArmed();
      connect(code, target);
    } else {
      inspect(target);
      pick(target, anchor);
    }
  }

  const focusTarget = target => {
    const node = [...scroll.querySelectorAll("[data-wire-target]")].find(node =>
      node.dataset.wireTarget === target && !node.matches(".wire-rail-port,.wire-dot"));
    const control = node?.matches("button") ? node : node?.querySelector("button");
    (control ?? scroll).focus({ preventScroll: true });
  };

  function addCard(code, position, count = 1, copies = 1) {
    if (!byCode.has(code)) return;
    for (let i = 0; i < copies; i++) {
      const key = "card:" + ++board.serial;
      board.cards.set(key, { code, count });
      if (position) board.positions.set(key, { x: position.x + i * 12, y: position.y + i * 12 });
      selection = key;
    }
    armed = undefined;
    updateArmed();
    renderBoard();
    renderInspector();
    const card = scroll.querySelector('[data-wire-target="' + selection + '"] .wire-material-name');
    card?.focus({ preventScroll: true });
    card?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }

  function removeCard(key) {
    board.cards.delete(key);
    board.positions.delete(key);
    for (const [target, source] of board.sources) if (source === key) board.sources.delete(target);
    selection = "recipe";
    renderBoard();
    renderInspector();
    scroll.focus({ preventScroll: true });
  }

  function renderBoard() {
    if (!state) return;
    closePicker();
    renderedWidth = scroll.clientWidth;
    visiblePositions.clear();
    const compact = renderedWidth < 660;
    const columns = Math.max(3, Math.floor(Math.max(renderedWidth, 544) / 158));
    const sources = state.slots.flatMap((slot, i) => slot ? [sourceOf(slotId(i), slot.code)] : []);
    const codes = [...new Set(sources.filter(isMaterial))];
    const baseHeight = Math.max(320, codes.length * 44 + 32, state.shaped ? 320 : state.slots.length * 37 + 118);
    let manualBottom = 0;
    for (const position of board.positions.values()) manualBottom = Math.max(manualBottom, position.y + 76);
    const height = Math.max(scroll.clientHeight, baseHeight + 80 + Math.ceil(board.cards.size / columns) * 57, manualBottom + 12);
    const surface = el("div", { className: "wire-surface wire-recipe-surface" + (compact ? " compact" : ""),
      "data-wire-target": "board", style: "height:" + height + "px" });
    scroll.replaceChildren(surface);
    view.apply();
    const cardRegion = el("div", { className: "wire-scratch-label", style: "top:" + baseHeight + "px" },
      el("strong", {}, "Scratchpad"), el("span", {}, board.cards.size ? board.cards.size + " cards · Ctrl+C / Ctrl+V" : "Drop materials here, or use + Material"));
    surface.append(cardRegion);
    surface.onpointermove = event => {
      if (event.target === surface) lastPointer = boardPoint(surface, event);
    };
    surface.onclick = event => {
      if (event.target !== surface) return;
      lastPointer = boardPoint(surface, event);
      scroll.focus({ preventScroll: true });
      if (armed) addCard(armed, lastPointer);
      else inspect("board");
    };
    dropTarget(surface, ["item"], (_, code, event) => addCard(code, boardPoint(surface, event)));
    const canvas = wireCanvas(surface);
    const edge = [];
    const materialPorts = new Map();
    const place = (node, handle, key, x, y, done) => {
      const position = { ...(board.positions.get(key) ?? { x, y }) };
      visiblePositions.set(key, position);
      surface.append(node);
      position.x = Math.max(4, Math.min(surface.clientWidth - node.offsetWidth - 4, position.x));
      canvas.move(node, handle, position, (left, top) => ({
        x: Math.max(4, Math.min(surface.clientWidth - node.offsetWidth - 4, left)),
        y: Math.max(4, Math.min(height - node.offsetHeight - 4, top)),
      }), (final, event, released, reset) => {
        if (!released) return;
        board.positions.set(key, final);
        done?.(final, event, released, reset);
      });
    };
    function addMaterialNode(target, code, x, y, scratch) {
      const name = button(item(code).label, () => inspect(target), "Drag to a recipe cell or move on the board", "wire-material-name");
      bindItemActions(name, code, ctx.itemActions, { drag: false });
      const heading = el("header", { className: "wire-node-handle" }, art(code), name,
        scratch && scratch.count > 1 ? el("small", {}, "×" + scratch.count) : null);
      const caption = "Wire from " + item(code).label + (scratch ? " · Card " + target.slice(5) : "");
      const port = button("", () => { armed = code; updateArmed(); }, caption, "wire-dot material");
      port.setAttribute("aria-label", caption);
      const linked = sources.includes(target);
      const card = el("div", { className: "wire-node wire-material" + (scratch ? " scratch" : "") + (linked ? " linked" : ""),
        "data-wire-target": target }, heading, port);
      if (selection === target) card.classList.add("wire-selected");
      canvas.bind(port, port, to => isSlot(to) || to === "output" || to === "board", (to, event) => {
        if (to === "board") addCard(code, boardPoint(surface, event), scratch?.count ?? 1);
        else connect(code, to, target, scratch?.count > 1 ? scratch.count : undefined);
      });
      materialPorts.set(target, port);
      place(card, heading, target, x, y, (_, event, released, reset) => {
        if (!released) return;
        const drop = document.elementsFromPoint(event.clientX, event.clientY).map(node => node.closest("[data-wire-target]"))
          .find(node => node && surface.contains(node) && (isSlot(node.dataset.wireTarget) || node.dataset.wireTarget === "output"));
        if (!drop) return;
        reset();
        connect(code, drop.dataset.wireTarget, target, scratch?.count > 1 ? scratch.count : undefined);
      });
    }
    codes.forEach((target, i) => addMaterialNode(target, itemCode(target), 13, 20 + i * 44));
    [...board.cards].forEach(([target, card], i) => addMaterialNode(target, card.code,
      13 + i % columns * 158, baseHeight + 37 + Math.floor(i / columns) * 57, card));
    const recipeHeader = el("header", { className: "wire-node-handle", "data-wire-target": "recipe" },
      art(state.product), button(state.name, () => inspect("recipe"), "Recipe properties", "wire-recipe-title"));
    const count = state.shaped ? 9 : Math.min(9, state.slots.length + 1);
    const ports = [];
    const rail = [];
    for (let i = 0; i < count; i++) {
      const target = slotId(i), slot = state.slots[i];
      const text = state.shaped ? "Cell " + (i + 1) : slot ? "Input " + (i + 1) : "Add Input";
      const port = button("", () => activate(target, port), text + " · Click to choose, drag to move", "wire-input" + (slot ? " connected" : ""));
      port.dataset.wireTarget = target;
      port.setAttribute("aria-label", text + " · " + (slot ? item(slot.code).label : "Empty"));
      if (!state.shaped) port.append(el("i", { className: "wire-socket", "aria-hidden": "true" }));
      port.append(slot ? art(slot.code) : el("span", { className: "wire-empty" }, "+"),
        el("span", { className: "wire-port-caption" }, state.shaped ? i + 1 : slot ? item(slot.code).label : "+ Input"));
      if (slot && !state.shaped) port.append(el("small", {}, "×" + slot.count));
      if (selection === target) port.classList.add("wire-selected");
      dropTarget(port, ["item"], (_, code) => connect(code, target));
      let endpoint = port;
      if (state.shaped) {
        endpoint = button(String(i + 1), () => inspect(target), port.getAttribute("aria-label"),
          "wire-rail-port" + (slot ? " connected" : ""));
        endpoint.setAttribute("aria-label", "Connection · " + port.getAttribute("aria-label"));
        endpoint.dataset.wireTarget = target;
        endpoint.prepend(el("i", { className: "wire-socket", "aria-hidden": "true" }));
        if (selection === target) endpoint.classList.add("wire-selected");
        dropTarget(endpoint, ["item"], (_, code) => connect(code, target));
        rail.push(endpoint);
      }
      if (slot) {
        const from = materialPorts.get(sourceOf(target, slot.code));
        const socket = endpoint.querySelector(".wire-socket");
        const accepts = to => canMove(target, to);
        const move = (to, event) => moveWire(target, to, event);
        canvas.bind(port, from, accepts, move);
        if (endpoint !== port) canvas.bind(endpoint, from, accepts, move);
        edge.push({ from, to: socket, select: () => inspect(target), canDrop: accepts, drop: move });
      }
      port.onkeydown = event => {
        if (event.key === "Delete" && slot) { event.preventDefault(); disconnect(target); }
      };
      port.oncontextmenu = event => { event.preventDefault(); inspect(target); pick(target, port); };
      if (endpoint !== port) endpoint.onkeydown = port.onkeydown;
      ports.push(port);
    }
    const out = button("", () => activate("output", out), "Output · " + item(state.product).label, "wire-dot product");
    out.setAttribute("aria-label", out.title);
    out.dataset.wireTarget = "output";
    dropTarget(out, ["item"], (_, code) => connect(code, "output"));
    const recipe = el("div", { className: "wire-node wire-recipe" }, recipeHeader,
      el("div", { className: "wire-node-label" }, el("span", {}, state.shaped ? "Crafting Grid" : "Ingredients"),
        el("span", {}, state.table === "hand" ? "Hand Catalog" : state.table === "bench" ? "Workbench" : "Crystal Tech")),
      state.shaped ? el("div", { className: "wire-pattern-body" },
        el("div", { className: "wire-port-rail", "aria-label": "Pattern connections" }, rail),
        el("div", { className: "wire-inputs pattern", "aria-label": "Crafting pattern" }, ports))
        : el("div", { className: "wire-inputs", "aria-label": "Ingredients" }, ports), out);
    const resultTitle = el("header", { className: "wire-node-handle" }, "Output");
    const resultItem = button("", () => activate("output", resultItem), "Select output " + item(state.product).label, "wire-result-item");
    resultItem.setAttribute("aria-label", resultItem.title);
    bindItemActions(resultItem, state.product, ctx.itemActions, { drag: false });
    resultItem.dataset.wireTarget = "output";
    resultItem.append(el("i", { className: "wire-socket", "aria-hidden": "true" }),
      art(state.product), el("span", {}, item(state.product).label), el("small", {}, "×" + state.count));
    if (selection === "output") resultItem.classList.add("wire-selected");
    dropTarget(resultItem, ["item"], (_, code) => connect(code, "output"));
    const result = el("div", { className: "wire-node wire-result" }, resultTitle, resultItem);
    place(recipe, recipeHeader, "recipe", compact ? 178 : 205, Math.max(29, baseHeight / 2 - (state.shaped ? 130 : 164)));
    place(result, resultTitle, "output", compact ? 414 : 482, Math.max(83, baseHeight / 2 - 51));
    const rewireOutput = to => (material(to) || to === "board") && canMove("output", to);
    canvas.bind(resultItem, out, rewireOutput, (to, event) => moveWire("output", to, event));
    edge.push({ from: out, to: resultItem.querySelector(".wire-socket"), output: true,
      select: () => inspect("output"), canDrop: rewireOutput, drop: (to, event) => moveWire("output", to, event) });
    canvas.set(edge);
  }
  function closePicker() {
    if (!picker) return;
    picker.hidePopover();
    picker.remove();
    picker = undefined;
  }

  function pick(target, anchor = addMaterial) {
    if (!state) return;
    closePicker();
    const label = target === "board" ? "Add Material" : target === "output" ? "Choose Output"
      : state.shaped ? "Choose Material · Cell " + (slotIndex(target) + 1) : "Choose Ingredient";
    const previous = target === "output" ? state.product : state.slots[slotIndex(target)]?.code;
    const used = new Set([previous, ...board.recent, ...state.slots.map(slot => slot?.code)]);
    const find = el("input", { type: "search", placeholder: "Search materials…", "aria-label": "Search materials" });
    const matches = el("div", { className: "wire-picker-items", "aria-label": "Matching materials" });
    const amount = el("input", { type: "number", min: 1, max: 1000, value: 1, "aria-label": "Number of material cards" });
    const choose = code => {
      if (target === "board") {
        const copies = Number(amount.value);
        if (!Number.isInteger(copies) || copies < 1 || copies > 1000) { amount.reportValidity(); return; }
        addCard(code, undefined, 1, copies);
      } else {
        closePicker();
        connect(code, target);
        focusTarget(target);
      }
    };
    let buttons = [];
    function filter() {
      const words = find.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
      const result = items.filter(value => words.every(word => (value.label + " " + value.code).toLowerCase().includes(word)))
        .sort((a, b) => Number(used.has(b.code)) - Number(used.has(a.code)) || a.label.localeCompare(b.label));
      buttons = result.map(value => {
        const row = button("", () => choose(value.code), value.code, "wire-picker-item");
        row.setAttribute("aria-current", String(value.code === previous));
        row.append(art(value.code), el("span", {}, value.label));
        return row;
      });
      matches.replaceChildren(...buttons);
      if (!buttons.length) matches.append(el("p", {}, "No matching materials"));
      matches.scrollTop = 0;
    }
    find.oninput = filter;
    find.onkeydown = event => {
      if (!buttons.length) return;
      if (event.key === "Enter") { event.preventDefault(); buttons[0].click(); }
      if (event.key === "ArrowDown") { event.preventDefault(); buttons[0].focus(); }
    };
    matches.onkeydown = event => {
      const index = buttons.indexOf(event.target);
      if (index < 0) return;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        (buttons[index + (event.key === "ArrowDown" ? 1 : -1)] ?? find).focus();
      }
    };
    const menu = picker = el("div", { className: "wire-picker", popover: "auto", "aria-label": label },
      el("header", {}, el("strong", {}, label), button("Close", () => { closePicker(); anchor.focus(); })),
      find, target === "board" ? el("label", { className: "wire-picker-amount" }, "Cards to add", amount) : null, matches,
      isSlot(target) && previous ? button("Clear Cell", () => { closePicker(); disconnect(target); focusTarget(target); }) : null);
    menu.oncopy = event => {
      if (event.target !== find || find.selectionStart === find.selectionEnd) copyTo(event, target);
    };
    menu.onpaste = event => {
      if (event.target === amount || target === "board") return;
      pasteFrom(event, target);
    };
    menu.addEventListener("toggle", () => { if (!menu.matches(":popover-open")) { menu.remove(); if (picker === menu) picker = undefined; } });
    document.body.append(menu);
    filter();
    menu.showPopover();
    const bounds = anchor.getBoundingClientRect();
    menu.style.left = Math.max(6, Math.min(bounds.left, window.innerWidth - menu.offsetWidth - 6)) + "px";
    menu.style.top = (bounds.bottom + menu.offsetHeight + 6 <= window.innerHeight
      ? bounds.bottom + 4 : Math.max(6, bounds.top - menu.offsetHeight - 4)) + "px";
    find.focus();
  }
  function countInput(value, name, updateValue) {
    const input = el("input", { type: "number", min: 1, max: 16384, step: 1, value, "aria-label": name });
    input.onchange = () => updateValue(Number(input.value));
    return input;
  }
  function copyValue(target) {
    if (target === "recipe") return { kind: "pattern", shaped: state.shaped,
      slots: state.slots.map(slot => slot && { code: slot.code, count: slot.count }) };
    if (target === "output") return { kind: "material", code: state.product, count: state.count };
    const slot = isSlot(target) ? state.slots[slotIndex(target)] : board.cards.get(target);
    const code = slot?.code ?? itemCode(target);
    return byCode.has(code) ? { kind: "material", code, count: slot?.count ?? 1 } : undefined;
  }

  function clipboardTarget(event) {
    const element = event.target;
    if (!host.contains(element) || element.closest("input,textarea,select,[contenteditable]")) return;
    const tile = element.closest("[data-code]");
    if (tile && palette.contains(tile)) return "item:" + tile.dataset.code;
    const node = element.closest("[data-wire-target]");
    if (node) return node.dataset.wireTarget;
    if (element === scroll) return selection;
    return inspector.contains(element) ? selection : undefined;
  }

  function copyTo(event, target) {
    if (!target) return;
    const payload = copyValue(target);
    if (!payload) return;
    event.clipboardData.setData(clipboardType, JSON.stringify(payload));
    event.clipboardData.setData("text/plain", payload.kind === "material" ? payload.code : JSON.stringify(payload));
    event.preventDefault();
    status.textContent = payload.kind === "material" ? "Copied · " + item(payload.code).label : "Copied · Recipe pattern";
  }

  function pasteFrom(event, target) {
    if (!target) return;
    const text = event.clipboardData.getData(clipboardType) || event.clipboardData.getData("text/plain").trim();
    let payload;
    try { payload = JSON.parse(text); }
    catch { payload = { kind: "material", code: text, count: 1 }; }
    if (payload?.kind === "material" && byCode.has(payload.code) && Number.isSafeInteger(payload.count)
        && payload.count >= 1 && payload.count <= 16384) {
      event.preventDefault();
      if (isSlot(target) || target === "output") {
        connect(payload.code, target, undefined, payload.count);
        focusTarget(target);
      } else {
        const card = board.cards.has(target) ? visiblePositions.get(target) : null;
        addCard(payload.code, card ? { x: card.x + 20, y: card.y + 25 } : lastPointer, payload.count);
      }
    } else if (payload?.kind === "pattern" && Array.isArray(payload.slots) && payload.slots.length <= 9 && target === "recipe") {
      event.preventDefault();
      if (state.shaped !== payload.shaped) return report("Paste into the same recipe type.");
      if (!payload.slots.every(slot => slot === null || (byCode.has(slot?.code) && Number.isSafeInteger(slot.count)))) {
        return report("The copied pattern contains an unavailable material.");
      }
      const slots = payload.slots.map(slot => slot && { code: slot.code, count: state.shaped ? 1 : slot.count });
      if (state.shaped) while (slots.length < 9) slots.push(null);
      else if (slots.some(slot => !slot)) return report("A shapeless pattern cannot have empty entries.");
      update(draft => { draft.slots = slots; }, "recipe", () => {
        for (const key of board.sources.keys()) if (isSlot(key)) board.sources.delete(key);
      });
      focusTarget("recipe");
    }
  }
  host.addEventListener("copy", event => { if (state) copyTo(event, clipboardTarget(event)); });
  host.addEventListener("paste", event => { if (state) pasteFrom(event, clipboardTarget(event)); });

  host.addEventListener("keydown", event => {
    if (!state || event.target.closest("input,textarea,select,[contenteditable]")) return;
    if (event.key === "Delete") {
      const target = clipboardTarget(event);
      if (board.cards.has(target)) { event.preventDefault(); removeCard(target); }
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") {
      event.preventDefault(); history(event.shiftKey ? "redo" : "undo");
    }
    if (event.key === "Escape" && armed) { armed = undefined; updateArmed(); }
  });

  function renderInspector() {
    if (!state) return;
    let content;
    if (selection === "board") {
      content = section("Scratchpad", el("p", {}, "Drop a material anywhere. Drag its card to a cell when you want to use it."),
        button("Add Material", () => pick("board")));
    } else if (material(selection)) {
      const key = selection, code = itemCode(key), card = board.cards.get(key);
      const location = visiblePositions.get(key) ?? { x: 13, y: 20 };
      content = section(card ? "Material Card" : "Ingredient Material", el("div", { className: "wire-item-title" }, art(code), item(code).label),
        card ? label("Quantity", countInput(card.count, "Material quantity", count => {
          if (!Number.isSafeInteger(count) || count < 1 || count > 16384) return report("Quantity must be 1–16384");
          card.count = count;
          report();
          renderBoard();
        })) : null,
        button("Duplicate Card", () => addCard(code, { x: location.x + 20, y: location.y + 25 }, card?.count ?? 1), "Ctrl+C, then Ctrl+V"),
        card ? button("Remove Card", () => removeCard(key), "Remove the card from the board · Delete") : null);
    } else if (selection === "recipe") {
      const name = el("input", { type: "text", value: state.name, maxLength: 120 });
      name.onchange = () => update(draft => { draft.name = name.value; });
      const description = el("textarea", { rows: 3, value: state.description });
      description.onchange = () => update(draft => { draft.description = description.value; });
      const hasZero = !state.shaped && state.slots.some(slot => slot?.count === 0);
      content = section("Recipe", label("Name", name),
        state.descriptionField ? label("Recipe Note", description) : null,
        hasZero ? el("p", { className: "error" }, "Zero-count ingredients can break crafting.") : null,
        hasZero ? button("Set Zero Counts to 1", () => update(draft => {
          for (const slot of draft.slots) if (slot?.count === 0) slot.count = 1;
        })) : null,
        button("Output", () => inspect("output")),
        button("Open Source", () => ctx.openSource(spec)),
        ctx.assist ? button("Ask AI", () => ctx.assist(spec)) : null,
        ctx.playRecipe ? button("Playtest Draft", ctx.playRecipe) : null);
    } else if (selection === "output") {
      content = section("Output", el("div", { className: "wire-item-title" }, art(state.product), item(state.product).label),
        label("Output count", countInput(state.count, "Output count", count => update(draft => { draft.count = count; }))),
        button("Change Item", event => pick("output", event.currentTarget)));
    } else {
      const target = selection, index = slotIndex(target), slot = state.slots[index];
      content = section(state.shaped ? "Cell " + (index + 1) : "Ingredient " + (index + 1),
        slot ? el("div", { className: "wire-item-title" }, art(slot.code), item(slot.code).label) : el("p", {}, "Empty"),
        slot && !state.shaped ? label("Ingredient count", countInput(slot.count, "Ingredient count",
          count => update(draft => { draft.slots[index].count = count; }))) : null,
        button(slot ? "Change Item" : "Choose Item", event => pick(target, event.currentTarget)),
        slot ? button("Disconnect", () => disconnect(target)) : null);
    }
    inspector.replaceChildren(el("header", { className: "wire-section-head" }, el("h2", {}, "Inspector")), content,
      section("Board Controls", el("p", {}, "Click a cell to choose its material. Drag a cell to move or swap it."),
        el("p", {}, "Ctrl+C / Ctrl+V copies cards, cells and patterns. Delete clears a cell or removes a card.")));
  }
  return { open, review: () => ctx.review(doc), refresh: () => open() };
}
