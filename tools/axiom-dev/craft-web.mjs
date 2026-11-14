import { button, el, findEntry } from "./inspector.mjs";
import { dragSource, dropTarget } from "./slot-tools.mjs";
import { wireCanvas } from "./wire-canvas.mjs";
import { craftingIndex, recipeId } from "./craft-index.mjs";
import { checkRoute, webNeighborhood } from "./craft-web-data.mjs";
import { recipePath } from "./idea-data.mjs";

import { showContextMenu, copyText } from "./context-menu.mjs";
import { canvasView } from "./canvas-view.mjs";

const list = values => el("ul", {}, values.map(value => el("li", {}, value)));
// Survives closing the dialog, so a copied recipe can be pasted after a paste or reopen.
let clipboard;

export function openCraftWeb(start, initialRoutes, ui) {
  let root = start, routes = initialRoutes, index = craftingIndex(routes), selected, auditTicket = 0;
  const positions = new Map(), nodeByKey = new Map(), handles = new Map();
  let canvas, web, hover = null;
  const litKey = () => hover ?? selected?.key ?? null;
  const scroll = el("div", { className: "wire-scroll craft-web-scroll", tabIndex: 0, "aria-label": "Crafting graph" });
  const zoomLabel = button("100%", () => view.reset(), "Reset zoom (0)");
  const view = canvasView(scroll, { nodeSelector: ".craft-web-node", onChange: zoom => {
    zoomLabel.textContent = Math.round(zoom * 100) + "%";
    canvas?.redraw();
  } });
  const sidebar = el("aside", { className: "craft-web-inspector", "aria-label": "Crafting inspector" });
  const status = el("p", { role: "status", className: "muted" });
  const title = el("h2");
  const direction = el("select", { "aria-label": "Crafting direction", onchange: () => relayout() },
    [["both", "Inputs and uses"], ["inputs", "Inputs only"], ["uses", "Uses only"]]
      .map(([value, name]) => el("option", { value }, name)));
  const depth = el("select", { "aria-label": "Number of recipe hops", onchange: () => relayout() },
    [1, 2, 3, 4, 5].map(value => el("option", { value }, value + (value === 1 ? " hop" : " hops"))));
  depth.value = "2";
  const leave = action => { dialog.close(); action(); };
  const create = preset => leave(() => ui.create(preset));
  const find = (label, choose, accept = () => true) => findEntry(label, ui.entries().filter(row => accept(row.key)), root, choose);
  const newRoute = button("+ New route", () => create(root), "Click for a new recipe or drop an item to use it as an ingredient");
  dropTarget(newRoute, ["craft-web-item"], (_, code) => create({ product: root === code ? undefined : root, inputs: [{ code, count: 1 }] }));
  const paste = button("Paste as branch", () => { if (clipboard) create(clipboard); });
  paste.disabled = !clipboard;
  const review = button("Review recipe draft", () => leave(ui.review), "", "primary");
  const undo = button("Undo", () => history("undo"));
  const redo = button("Redo", () => history("redo"));
  const dialog = el("dialog", { className: "finder craft-web-dialog", "aria-label": "Crafting Web", onclose: () => { view.destroy(); dialog.remove(); },
    onkeydown: event => {
      if (event.target.closest("input,textarea,select")) return;
      if (!(event.ctrlKey || event.metaKey)) return plainKey(event);
      const key = event.key.toLowerCase();
      if (key === "c" && selected?.route && !window.getSelection()?.toString()) {
        event.preventDefault();
        copyRoute(selected.route);
      } else if (key === "v" && clipboard) {
        event.preventDefault();
        create(clipboard);
      }
    },
    // Only reached when no recipe is held in-graph: a material copied from Wiring or the palette.
    onpaste: event => {
      if (event.target.closest("input,textarea,select")) return;
      const text = (event.clipboardData?.getData("application/x-axiom-recipe") || event.clipboardData?.getData("text/plain") || "").trim();
      let code = text;
      try { const payload = JSON.parse(text); if (payload?.kind === "material") code = payload.code; } catch { /* bare code */ }
      if (typeof code !== "string" || !ui.canWire(code)) { status.textContent = "Clipboard holds no usable item."; return; }
      event.preventDefault();
      create({ inputs: [{ code, count: 1 }] });
    } },
    el("header", { className: "section-title" }, title, button("Close", () => dialog.close())),
    el("div", { className: "craft-web-toolbar" }, button("Find item", () => find("Focus crafting item", focus)),
      direction, depth, newRoute, paste, button("Audit all routes", auditAll), button("Reset layout", () => relayout()), undo, redo, review),
    el("div", { className: "craft-web-toolbar craft-web-zoom", role: "group", "aria-label": "View" },
      button("−", () => view.zoomBy(1 / 1.2), "Zoom out (-)"), zoomLabel, button("+", () => view.zoomBy(1.2), "Zoom in (+)"),
      button("Fit", () => view.fit(), "Fit all nodes (F)"),
      el("small", { className: "muted" }, "Drag empty space or hold Space to pan · Ctrl+wheel zooms · arrow keys move between linked nodes · / finds an item")),
    status, el("div", { className: "craft-web-body" }, scroll, sidebar));

  function plainKey(event) {
    const k = event.key;
    if (k === "/") { event.preventDefault(); find("Focus crafting item", focus); }
    else if (k === "+" || k === "=") { event.preventDefault(); view.zoomBy(1.2); }
    else if (k === "-") { event.preventDefault(); view.zoomBy(1 / 1.2); }
    else if (k === "0") { event.preventDefault(); view.reset(); }
    else if (k.toLowerCase() === "f" && !event.altKey) { event.preventDefault(); view.fit(); }
    else if (k.startsWith("Arrow") && event.target.dataset?.nodeKey) {
      const next = neighbor(event.target.dataset.nodeKey, k);
      const handle = next && handles.get(next);
      if (!handle) return;
      event.preventDefault();
      handle.focus({ preventScroll: true });
      handle.scrollIntoView({ block: "nearest", inline: "nearest" });
      inspect(nodeByKey.get(next));
    }
  }
  function neighbor(key, direction) {
    if (direction === "ArrowRight") return web.edges.find(edge => edge.from === key)?.to;
    if (direction === "ArrowLeft") return web.edges.find(edge => edge.to === key)?.from;
    const me = nodeByKey.get(key);
    if (!me) return;
    const column = web.nodes.filter(node => node.level === me.level)
      .sort((a, b) => positions.get(a.key).y - positions.get(b.key).y);
    return column[column.indexOf(me) + (direction === "ArrowDown" ? 1 : direction === "ArrowUp" ? -1 : 0)]?.key;
  }
  function canvasMenu(event) {
    if (!view.isEmptyTarget(event.target)) return;
    event.preventDefault();
    const actions = [
      { label: "Fit view", key: "F", run: () => view.fit() },
      { label: "Reset zoom", key: "0", run: () => view.reset() },
      { label: "Reset layout", run: () => relayout() },
      { label: "Find item", key: "/", run: () => find("Focus crafting item", focus) },
      { label: "New route", run: () => create(root) },
    ];
    if (clipboard) actions.push({ label: "Paste as branch", key: "Ctrl+V", run: () => create(clipboard) });
    showContextMenu(scroll, actions, [event.clientX, event.clientY]);
  }
  scroll.addEventListener("contextmenu", canvasMenu);
  function focus(code) {
    root = code;
    selected = undefined;
    relayout();
  }
  function relayout() {
    positions.clear();
    render();
    center();
  }
  function center() {
    const position = positions.get("item:" + root);
    if (position) requestAnimationFrame(() => {
      const k = view.zoom();
      scroll.scrollLeft = Math.max(0, (position.x + 85) * k - scroll.clientWidth / 2);
      scroll.scrollTop = Math.max(0, (position.y - 48) * k);
    });
  }
  const editable = route => route.path === recipePath;
  function history(action) {
    try {
      routes = ui.history(action);
      index = craftingIndex(routes);
      selected = undefined;
      render();
    } catch (error) { status.textContent = error.message; }
  }
  async function change(route, blocks, nextRoot) {
    try {
      const next = await ui.change(route, blocks);
      if (!dialog.open) return;
      adopt(next, nextRoot);
    } catch (error) { status.textContent = error.message; }
  }
  function adopt(next, nextRoot) {
    if (!next.changed) { status.textContent = "That change leaves the recipe unchanged."; return; }
    routes = next.routes;
    index = craftingIndex(routes);
    selected = { route: next.route };
    if (nextRoot && root !== nextRoot) { root = nextRoot; positions.clear(); }
    render();
    if (nextRoot) center();
    status.textContent = "Recipe draft updated. Review it before saving.";
  }
  function aiProposal(spec, audit) {
    const details = el("div", { role: "status", className: "craft-web-proposal" });
    const run = button("Generate AI balance proposal", async () => {
      run.disabled = true;
      details.replaceChildren(el("p", {}, "Analyzing this route and its neighbors…"));
      const baseline = routes;
      try {
        const result = await ui.suggest(spec.route, audit);
        if (!dialog.open || !details.isConnected) return;
        if (routes !== baseline) throw new Error("The recipe draft changed while AI was working.");
        details.replaceChildren(el("p", {}, result.rationale));
        if (!result.blocks.length) return;
        const next = ui.preview(spec.route, result.blocks);
        if (!next.changed) return;
        const describe = route => route.inputs.map(input => input.count + " × " + ui.name(input.code)).join(" + ")
          + " → " + route.count + " × " + ui.name(route.product);
        const findings = checkRoute(craftingIndex(next.routes), next.route, ui).issues;
        details.append(el("strong", {}, "Current"), el("p", {}, describe(spec.route)),
          el("strong", {}, "Proposed"), el("p", {}, describe(next.route)),
          next.route.description ? el("p", {}, next.route.description) : "",
          findings.length ? list(findings) : el("p", {}, "No findings in the proposed route."),
          button("Add proposal to draft", () => {
            try { next.stage(); adopt(next); } catch (error) { details.textContent = error.message; }
          }, "", "primary"));
      } catch (error) { details.textContent = error.message; }
      finally { run.disabled = false; }
    });
    return el("section", {}, run, details);
  }
  function rewire(edge, code) {
    if (edge.output) return change(edge.route, [{ op: "output", item: code,
      count: Math.min(edge.route.count, ui.stackLimit(code)) }], code);
    return change(edge.route, [{ op: "replace", item: edge.ingredient, replacement: code }]);
  }
  function menuFor({ route, code }) {
    const items = route ? [
      { label: "Copy recipe", key: "Ctrl+C", run: () => copyRoute(route) },
      { label: "Branch route", run: () => create(route) },
      { label: editable(route) ? "Open Wiring" : "Open station definition", run: () => leave(() => ui.edit(route)) },
      { label: "Focus product", run: () => focus(route.product) },
      { label: "Copy tree", run: () => copyTree(route.product) },
      { label: "Swap ingredient in tree…", run: () => swapInTree(route.product) },
    ] : [
      { label: "Focus item", run: () => focus(code) },
      { label: "New recipe for this item", run: () => create(code) },
      { label: "Use in a new recipe", run: () => create({ inputs: [{ code, count: 1 }] }) },
      { label: "Open item", run: () => leave(() => ui.openItem(code)) },
      { label: "Copy tree", run: () => copyTree(code) },
      { label: "Swap ingredient in tree…", run: () => swapInTree(code) },
    ];
    if (clipboard) items.push({ label: "Paste as branch", key: "Ctrl+V", run: () => create(clipboard) });
    return items;
  }
  /** Default production routes below an item, breadth-limited so a huge tree stays cheap. */
  function treeRoutes(code, limit = 40) {
    const seen = new Set(), found = [];
    const visit = (item, depth) => {
      const route = index.defaults.get(item);
      if (!route || depth > 6 || found.length >= limit || seen.has(recipeId(route))) return;
      seen.add(recipeId(route));
      found.push(route);
      for (const input of route.inputs) visit(input.code, depth + 1);
    };
    visit(code, 0);
    return found;
  }
  const treeText = code => treeRoutes(code).map(route => route.count + " × " + ui.name(route.product) + " ← " +
    route.inputs.map(input => input.count + " × " + ui.name(input.code)).join(" + ")).join("\n");
  function copyTree(code) {
    const text = treeText(code);
    if (!text) { status.textContent = ui.name(code) + " has no recipe tree to copy."; return; }
    copyText(text).then(() => { status.textContent = "Copied the " + ui.name(code) + " tree (" + text.split("\n").length + " recipes)."; })
      .catch(error => { status.textContent = error.message; });
  }
  /** Pick an ingredient used somewhere in the tree and a replacement, then review before applying. */
  function swapInTree(code) {
    const tree = treeRoutes(code);
    const used = new Set(tree.flatMap(route => route.inputs.map(input => input.code)).filter(Boolean));
    if (!tree.some(editable)) { status.textContent = "No editable recipes in this tree."; return; }
    find("Replace which ingredient?", from => find("Replace with", to => reviewSwap(code, tree, from, to), ui.canWire), key => used.has(key));
  }
  function reviewSwap(code, tree, from, to) {
    const affected = tree.filter(route => editable(route) && route.inputs.some(input => input.code === from));
    const lines = affected.map(route => route.label + " · " + route.inputs.map(input =>
      input.count + " × " + ui.name(input.code === from ? to : input.code)).join(" + "));
    auditTicket++;
    selected = undefined;
    sidebar.replaceChildren(el("h3", {}, "Swap in " + ui.name(code) + " tree"),
      el("p", {}, ui.name(from) + " → " + ui.name(to) + " in " + affected.length + " recipe" + (affected.length === 1 ? "" : "s") + "."),
      affected.length ? list(lines) : el("p", {}, "Nothing in this tree uses that ingredient."),
      affected.length ? button("Apply to " + affected.length + " recipes", () => applySwap(affected.map(recipeId), from, to), "Each recipe is one undo step", "primary") : null,
      button("Cancel", () => inspect(undefined)));
  }
  async function applySwap(ids, from, to) {
    let latest, failure;
    for (const id of ids) {
      const route = (latest?.routes ?? routes).find(row => recipeId(row) === id);
      if (!route) continue;
      try {
        const next = await ui.change(route, [{ op: "replace", item: from, replacement: to }]);
        if (next.changed) latest = next;
      } catch (error) { failure = error; break; }
    }
    if (!dialog.open) return;
    if (latest) adopt(latest);
    if (failure) status.textContent = "Stopped: " + failure.message + (latest ? " Earlier recipes changed; use Undo to revert." : "");
    else if (latest) status.textContent = "Updated " + ids.length + " recipes. Use Undo to step back one at a time; review before saving.";
  }
  function copyRoute(route) {
    clipboard = route;
    paste.disabled = false;
    // Wiring pastes this as a material card or slot; failure leaves the in-graph clipboard intact.
    copyText(JSON.stringify({ kind: "material", code: route.product, count: route.count })).catch(() => {});
    status.textContent = "Recipe copied. Paste as branch to change its inputs or pattern.";
  }
  async function auditAll() {
    const ticket = ++auditTicket, catalog = routes, current = index;
    const flagged = [];
    status.textContent = "Auditing " + catalog.length + " indexed recipes…";
    for (const [i, route] of catalog.entries()) {
      if (i && i % 30 === 0) await new Promise(resolve => setTimeout(resolve, 0));
      if (ticket !== auditTicket || !dialog.open || catalog !== routes) return;
      const audit = checkRoute(current, route, ui);
      const actionable = audit.issues.filter(issue => !issue.startsWith("External supply") &&
        !issue.includes("may use generated art"));
      if (actionable.length) flagged.push({ route, actionable, priority: audit.ceiling !== null ? 2 : actionable.length > 1 ? 1 : 0 });
    }
    flagged.sort((a, b) => b.priority - a.priority || a.route.label.localeCompare(b.route.label));
    selected = undefined;
    status.textContent = "Audited " + catalog.length + " routes · " + flagged.length + " with findings.";
    sidebar.replaceChildren(el("h3", {}, "Catalog audit"),
      el("p", {}, "Conversion checks compare item quantities. Inspect station, power, and progression costs in each route."),
      el("p", {}, "Showing " + Math.min(50, flagged.length) + " of " + flagged.length + " routes with findings."),
      ...flagged.slice(0, 50).map(({ route, actionable }) => el("div", { className: "craft-web-audit-row" },
        button(route.label, () => {
          focus(route.product);
          inspect({ route });
        }), el("small", {}, actionable[0]))));
  }
  function inspect(spec) {
    auditTicket++;
    selected = spec;
    canvas?.highlight(litKey());
    sidebar.replaceChildren();
    if (!spec?.route) {
      const code = spec?.code ?? root;
      const made = index.producers.get(code)?.length ?? 0;
      const used = index.consumers.get(code)?.length ?? 0;
      sidebar.append(el("h3", {}, ui.name(code)), el("p", {}, made + " ways to make it · " + used + " recipes use it"),
        button("Focus item", () => focus(code)),
        button("New recipe for this item", () => create(code)),
        button("Use in a new recipe", () => create({ inputs: [{ code, count: 1 }] })),
        button("Open item", () => leave(() => ui.openItem(code))),
        button("Copy tree", () => copyTree(code), "Copy this item's production tree as compact text"),
        button("Swap ingredient in tree…", () => swapInTree(code), "Replace one ingredient across every editable recipe below this item"));
      return;
    }
    const { route, edge } = spec;
    const audit = checkRoute(index, route, ui);
    sidebar.append(el("h3", {}, route.label),
      el("p", {}, route.category + " · " + route.count + " × " + ui.name(route.product)),
      el("div", { className: "craft-web-toolbar" },
        button(editable(route) ? "Open Wiring" : "Open station definition", () => leave(() => ui.edit(route))),
        button("Copy recipe", () => copyRoute(route)),
        button("Branch route", () => create(route)),
        button("Copy tree", () => copyTree(route.product)),
        button("Swap ingredient in tree…", () => swapInTree(route.product))));
    if (edge && editable(route)) sidebar.append(button(edge.output ? "Change output" : "Replace " + ui.name(edge.ingredient),
      () => find("Reconnect to item", code => rewire(edge, code), code => ui.canWire(code))));
    if (editable(route)) {
      sidebar.append(el("div", { className: "craft-web-toolbar" },
        button("+ Ingredient", () => find("Add ingredient", code => change(route, [{ op: "add", item: code }]), ui.canWire)),
        button("Change output", () => find("Change output", code => rewire({ route, output: true }, code), ui.canWire)),
        edge?.ingredient ? button("Remove connection", () =>
          change(route, [{ op: "remove-item", item: edge.ingredient }])) : null));
      sidebar.append(el("details", {}, el("summary", {}, "Ingredient connections"), list(route.inputs.map(input =>
        button(input.count + " × " + ui.name(input.code), () => inspect({ route, edge: { route, ingredient: input.code } }))))));
    }
    sidebar.append(el("h4", {}, "Balance and integration"),
      el("p", {}, audit.alternatives.length + " alternatives · " + audit.downstream.length + " downstream recipes"),
      audit.issues.length ? list(audit.issues) : el("p", {}, "No flagged issues in this route."),
      ...[audit.ceiling > 0 && editable(route) && ui.canWire(route.product) ? button("Cap loop yield at " + audit.ceiling, () =>
        change(route, [{ op: "output", item: route.product, count: audit.ceiling }])) : null,
      !route.description?.trim() && editable(route) ? button("Generate factual description", () =>
        change(route, [{ op: "describe", text: "Makes " + route.count + " × " + ui.name(route.product)
          + " from " + route.inputs.map(input => input.count + " × " + ui.name(input.code)).join(", ") + "." }])) : null].filter(Boolean));
    const artwork = ui.art(route.product);
    if (editable(route)) sidebar.append(aiProposal(spec, audit));
    sidebar.append(el("h4", {}, "Output artwork"),
      ...artwork.map((sprite, i) => button(artwork.length > 1 ? "Open sprite part " + (i + 1) : "Open output sprite",
        () => leave(() => ui.openSprite(sprite.path, sprite.index)))),
      button("AI improve recipe & art", () => leave(() => ui.assist(route, audit, artwork))));
  }
  function render() {
    title.textContent = "Crafting Web · " + ui.name(root);
    review.disabled = !ui.dirty();
    undo.disabled = !ui.canUndo("undo");
    redo.disabled = !ui.canUndo("redo");
    web = webNeighborhood(index, root, direction.value, Number(depth.value));
    nodeByKey.clear();
    handles.clear();
    hover = null;
    status.textContent = web.nodes.length + " nodes · " + web.edges.length + " wires"
      + (web.clipped ? " · Display limit reached. Focus a neighboring item to continue." : "")
      + " · Drag card headers to move them. Drag a wire to reconnect it.";
    const surface = el("div", { className: "craft-web-surface" });
    scroll.replaceChildren(surface);
    view.apply();
    canvas = wireCanvas(surface);
    const columns = new Map(), anchors = new Map();
    const min = Math.min(...web.nodes.map(node => node.level)), max = Math.max(...web.nodes.map(node => node.level));
    const width = (max - min + 1) * 225 + 45;
    const largest = Math.max(...web.nodes.map(node => web.nodes.filter(other => other.level === node.level).length));
    const height = Math.max(400, largest * 98 + 54);
    surface.style.width = width + "px";
    surface.style.height = height + "px";
    for (const node of web.nodes) {
      const { route, code, key, level } = node;
      const y = columns.get(level) ?? 0;
      columns.set(level, y + 1);
      if (!positions.has(key)) positions.set(key, { x: (level - min) * 225 + 20, y: y * 98 + 24 });
      const handle = button("", () => inspect(node), "Inspect or drag to arrange", "craft-web-node-title");
      handle.dataset.nodeKey = key;
      handles.set(key, handle);
      nodeByKey.set(key, node);
      const icon = ui.icon(route?.product ?? code);
      if (icon) handle.append(icon);
      handle.append(el("strong", {}, route?.label ?? ui.name(code)));
      const card = el("article", { className: "craft-web-node " + (route ? "route" : "item") + (code === root ? " focus" : "") },
        handle, el("small", {}, route ? route.category + " · " + route.count + " out" :
          (index.producers.get(code)?.length ?? 0) + " recipes in · " + (index.consumers.get(code)?.length ?? 0) + " uses"));
      const lightOn = () => { if (scroll.querySelector(".wire-ready")) return; hover = key; canvas.highlight(litKey()); };
      const lightOff = () => { hover = null; canvas.highlight(litKey()); };
      card.addEventListener("pointerenter", lightOn);
      card.addEventListener("pointerleave", lightOff);
      card.addEventListener("focusin", lightOn);
      card.addEventListener("focusout", lightOff);
      const openMenu = point => { inspect(node); showContextMenu(handle, menuFor(node), point); };
      card.oncontextmenu = event => { event.preventDefault(); openMenu([event.clientX, event.clientY]); };
      handle.onkeydown = event => {
        if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) { event.preventDefault(); openMenu(); }
      };
      if (code) {
        card.dataset.wireTarget = key;
        card.ondblclick = () => focus(code);
        const grip = button("↗", () => focus(code), "Focus item or drag to New route");
        grip.className = "craft-web-item-link";
        dragSource(grip, "craft-web-item", code);
        card.append(grip);
      }
      if (route && editable(route)) dropTarget(card, ["craft-web-item"], (_, ingredient) => {
        if (ui.canWire(ingredient)) change(route, [{ op: "add", item: ingredient }]);
      });
      surface.append(card);
      const left = el("span", { className: "craft-web-port input" }), right = el("span", { className: "craft-web-port output" });
      card.append(left, right);
      anchors.set(key, { left, right });
      canvas.move(card, handle, positions.get(key), (x, y) =>
        ({ x: Math.max(4, Math.min(width - 200, Math.round(x / 8) * 8)), y: Math.max(4, Math.min(height - 68, Math.round(y / 8) * 8)) }));
    }
    canvas.set(web.edges.map(edge => ({
      ends: [edge.from, edge.to],
      from: anchors.get(edge.from).right, to: anchors.get(edge.to).left, output: edge.output,
      select: () => inspect({ route: edge.route, edge }),
      canDrop: target => target.startsWith("item:") && target !== (edge.output ? edge.to : edge.from) && ui.canWire(target.slice(5)),
      drop: editable(edge.route) ? target => rewire(edge, target.slice(5)) : undefined,
    })));
    inspect(selected);
  }
  document.body.append(dialog);
  dialog.showModal();
  render();
  center();
}
