import { el, button } from "./inspector.mjs";
import { applyPortalOperations, dimensions, portalPath } from "./portal-operations.mjs";

export function mountPortalLayout(host, ctx) {
  let base, graph, operations = [], undo = [], redo = [], selected, linking, filter = "Surface", busy = false;
  let zoom = 1, pan = [24, 36];
  const toolbar = el("header", { className: "portal-toolbar" });
  const status = el("p", { className: "portal-status", role: "status", "aria-live": "polite" });
  const canvas = el("div", { className: "portal-canvas", "aria-label": "Portal layout canvas" });
  const stage = el("div", { className: "portal-stage" });
  const inspector = el("aside", { className: "portal-inspector", "aria-label": "Portal settings" });
  const dialog = el("dialog", { className: "portal-dialog", "aria-label": "Review portal layout" });
  canvas.append(stage);
  host.append(toolbar, status, el("div", { className: "portal-body" }, canvas, inspector), dialog);
  const message = (text, error = false) => { status.textContent = text; status.classList.toggle("error", error); };
  const act = callback => async () => { try { await callback(); } catch (error) { message(error.message, true); } };
  function snapshot() { return { graph: structuredClone(graph), operations: structuredClone(operations), selected }; }
  function edit(batch, fieldEdit = false) {
    const next = applyPortalOperations(graph, batch);
    if (operations.length + batch.length > 128) throw new Error("Publish this revision before adding more changes.");
    undo.push(snapshot()); redo = []; graph = next; operations.push(...batch); linking = null;
    if (fieldEdit) { refreshFields(); refreshButtons(); } else render();
    message("Draft updated. Review to validate and publish.");
  }
  function restore(from, to) {
    if (!from.length) return;
    to.push(snapshot()); const state = from.pop();
    graph = state.graph; operations = state.operations; selected = state.selected; linking = null; render();
    message(operations.length ? "Draft restored." : "Back to the saved layout.");
  }
  function unique(prefix, rows) { let n = 1; while (rows.some(row => row.id === prefix + n)) n++; return prefix + n; }
  function addOpening() {
    const id = unique("opening_", graph.nodes), n = graph.nodes.length;
    selected = { kind: "node", id };
    edit([{ op: "add_node", node: { id, label: "Opening " + id.split("_").at(-1), dimension: filter,
      position: [n * 12, 3, 0], yaw_degrees: 0, half_size: [1.5, 2], canvas: [60 + (n % 3) * 360, 70 + Math.floor(n / 3) * 175] } }]);
  }
  function connect(id) {
    if (!linking) { linking = id; selected = { kind: "node", id }; render(); message("Choose the destination opening's link handle. Escape cancels."); return; }
    if (linking === id) { linking = null; render(); return; }
    const linkId = unique("fold_", graph.links);
    edit([{ op: "connect", link: { id: linkId, kind: "Fold", from: linking, to: id, two_way: true, enabled: true } }]);
    selected = { kind: "link", id: linkId }; render();
  }
  function transform() { stage.style.transform = `translate(${pan[0]}px, ${pan[1]}px) scale(${zoom})`; }
  function fit() {
    const nodes = graph?.nodes.filter(node => node.dimension === filter) ?? [];
    if (!nodes.length) { zoom = 1; pan = [24, 36]; transform(); return; }
    const minX = Math.min(...nodes.map(node => node.canvas[0])), minY = Math.min(...nodes.map(node => node.canvas[1]));
    const width = Math.max(...nodes.map(node => node.canvas[0] + 236)) - minX;
    const height = Math.max(...nodes.map(node => node.canvas[1] + 128)) - minY;
    zoom = Math.max(.2, Math.min(1, (canvas.clientWidth - 80) / width, (canvas.clientHeight - 90) / height));
    pan = [40 - minX * zoom, 50 - minY * zoom]; transform();
  }
  function drag(event, node) {
    if (event.button !== 0 || event.target.closest(".portal-handle")) return;
    node = graph.nodes.find(row => row.id === node.id);
    const origin = [...node.canvas], start = [event.clientX, event.clientY];
    const target = event.currentTarget; let position = origin;
    target.setPointerCapture(event.pointerId);
    const move = e => {
      position = [0, 1].map((axis) => Math.max(-10000, Math.min(10000, Math.round(origin[axis] + ([e.clientX, e.clientY][axis] - start[axis]) / zoom))));
      const card = target.closest(".portal-node"); card.style.left = position[0] + "px"; card.style.top = position[1] + "px";
    };
    const stop = () => {
      target.removeEventListener("pointermove", move); target.removeEventListener("pointerup", stop); target.removeEventListener("pointercancel", cancel);
      if (position.some((n, i) => n !== origin[i])) {
        try { edit([{ op: "update_node", id: node.id, changes: { canvas: position } }]); } catch (error) { render(); message(error.message, true); }
      }
    };
    const cancel = () => { position = origin; stop(); render(); };
    target.addEventListener("pointermove", move); target.addEventListener("pointerup", stop); target.addEventListener("pointercancel", cancel);
  }
  canvas.addEventListener("pointerdown", event => {
    if (event.button !== 0 || event.target.closest("button,.portal-node")) return;
    const start = [event.clientX, event.clientY], origin = [...pan]; canvas.setPointerCapture(event.pointerId);
    const move = e => { pan = [origin[0] + e.clientX - start[0], origin[1] + e.clientY - start[1]]; transform(); };
    const stop = () => { canvas.removeEventListener("pointermove", move); canvas.removeEventListener("pointerup", stop); canvas.removeEventListener("pointercancel", stop); };
    canvas.addEventListener("pointermove", move); canvas.addEventListener("pointerup", stop); canvas.addEventListener("pointercancel", stop);
  });
  host.addEventListener("keydown", event => {
    if (event.key === "Escape") { linking = null; render(); }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" && !event.target.matches("input,textarea")) {
      event.preventDefault(); if (event.shiftKey) restore(redo, undo); else restore(undo, redo);
    }
  });
  window.addEventListener("beforeunload", event => { if (operations.length) { event.preventDefault(); event.returnValue = ""; } });

  function render() {
    if (!graph) return;
    const realm = el("select", { "aria-label": "Dimension", onchange: event => { filter = event.target.value; selected = null; linking = null; render(); fit(); } },
      dimensions.map(value => el("option", { value, selected: value === filter }, value)));
    toolbar.replaceChildren(el("strong", {}, "Dimensions & portals"), realm,
      button("Add opening", act(addOpening), "Place an opening in the selected dimension", "primary"),
      button("Undo", () => restore(undo, redo)), button("Redo", () => restore(redo, undo)),
      button("Fit", fit), button("−", () => { zoom = Math.max(.2, zoom / 1.2); transform(); }, "Zoom out"),
      button("+", () => { zoom = Math.min(2, zoom * 1.2); transform(); }, "Zoom in"),
      button("Review & publish", act(review), "Validate the draft and inspect changes", "primary"),
      button("Reload", act(() => reload()), "Reload the saved layout; asks before discarding a draft"));
    refreshButtons();
    draw(); inspect();
  }
  function refreshButtons() {
    [...toolbar.querySelectorAll("button")].forEach(btn => {
      btn.disabled = busy || (btn.textContent === "Undo" && !undo.length) || (btn.textContent === "Redo" && !redo.length)
        || (btn.textContent === "Review & publish" && !operations.length);
    });
  }
  function wireGeometry(a, b) {
    const right = b.canvas[0] >= a.canvas[0], x1 = a.canvas[0] + (right ? 236 : 0), x2 = b.canvas[0] + (right ? 0 : 236);
    const y1 = a.canvas[1] + 68, y2 = b.canvas[1] + 68, bend = Math.max(60, Math.abs(x2 - x1) * .4) * (right ? 1 : -1);
    return { d: `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`, left: (x1 + x2) / 2 - 35, top: (y1 + y2) / 2 - 15 };
  }
  // Keep controls and click targets mounted when a field commits on blur.
  // Replacing the inspector here would swallow the next click or reset focus.
  function refreshFields() {
    for (const card of stage.querySelectorAll(".portal-node")) {
      const node = graph.nodes.find(row => row.id === card.dataset.nodeId);
      card.style.left = node.canvas[0] + "px"; card.style.top = node.canvas[1] + "px";
      const title = card.querySelector(".portal-node-title"); title.textContent = node.label; title.setAttribute("aria-label", "Select " + node.label);
      card.querySelector("small").textContent = node.position.map(n => Number(n.toFixed(2))).join(", ") + " m";
      const handle = card.querySelector(".portal-handle"); handle.textContent = "Link"; handle.setAttribute("aria-label", "Link from " + node.label);
      card.classList.remove("connecting");
    }
    for (const path of stage.querySelectorAll(".portal-wire")) {
      const link = graph.links.find(row => row.id === path.dataset.linkId);
      const a = graph.nodes.find(row => row.id === link.from), b = graph.nodes.find(row => row.id === link.to), geometry = wireGeometry(a, b);
      path.setAttribute("d", geometry.d);
      const label = stage.querySelector(`.portal-edge[data-link-id="${link.id}"]`);
      label.style.left = geometry.left + "px"; label.style.top = geometry.top + "px"; label.title = a.label + " to " + b.label;
    }
    if (selected?.kind === "node") inspector.querySelector("h2").textContent = graph.nodes.find(row => row.id === selected.id).label;
  }
  function draw() {
    stage.replaceChildren(); transform();
    const nodes = graph.nodes.filter(node => node.dimension === filter);
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "portal-wires"); svg.setAttribute("width", "20000"); svg.setAttribute("height", "20000");
    svg.setAttribute("viewBox", "-10000 -10000 20000 20000");
    const defs = document.createElementNS(svg.namespaceURI, "defs"), marker = document.createElementNS(svg.namespaceURI, "marker");
    for (const [key, value] of Object.entries({ id: "portal-arrow", viewBox: "0 0 10 10", refX: "9", refY: "5", markerWidth: "6", markerHeight: "6", orient: "auto-start-reverse" })) marker.setAttribute(key, value);
    const arrow = document.createElementNS(svg.namespaceURI, "path"); arrow.setAttribute("d", "M 0 0 L 10 5 L 0 10 z"); arrow.setAttribute("fill", "#83d9d4");
    marker.append(arrow); defs.append(marker); svg.append(defs); stage.append(svg);
    for (const link of graph.links) {
      const a = nodes.find(node => node.id === link.from), b = nodes.find(node => node.id === link.to);
      if (!a || !b) continue;
      const geometry = wireGeometry(a, b);
      const path = document.createElementNS(svg.namespaceURI, "path");
      path.setAttribute("d", geometry.d); path.dataset.linkId = link.id;
      path.setAttribute("class", "portal-wire" + (link.enabled ? "" : " disabled"));
      path.setAttribute("marker-end", "url(#portal-arrow)"); if (link.two_way) path.setAttribute("marker-start", "url(#portal-arrow)");
      svg.append(path);
      const label = button((link.enabled ? "Fold" : "Disabled") + (link.two_way ? " ↔" : " →"), () => { selected = { kind: "link", id: link.id }; render(); },
        a.label + " to " + b.label, "portal-edge" + (selected?.id === link.id && selected?.kind === "link" ? " selected" : ""));
      label.dataset.linkId = link.id; label.setAttribute("aria-label", "Edit link " + link.id); label.style.left = geometry.left + "px"; label.style.top = geometry.top + "px"; stage.append(label);
    }
    for (const node of nodes) {
      const card = el("div", { className: "portal-node" + (selected?.kind === "node" && selected.id === node.id ? " selected" : "") + (linking === node.id ? " connecting" : ""),
        style: `left:${node.canvas[0]}px;top:${node.canvas[1]}px` });
      card.dataset.nodeId = node.id;
      const head = button(node.label, () => { selected = { kind: "node", id: node.id }; render(); }, "Select or drag opening", "portal-node-title");
      head.setAttribute("aria-label", "Select " + node.label); head.addEventListener("pointerdown", event => drag(event, node));
      const handle = button(linking === node.id ? "Cancel" : linking ? "Link here" : "Link", act(() => connect(node.id)), "Connect this opening", "portal-handle");
      handle.setAttribute("aria-label", (linking ? "Link to " : "Link from ") + node.label);
      card.append(el("span", { className: "portal-dimension" }, node.dimension), head,
        el("small", {}, node.position.map(n => Number(n.toFixed(2))).join(", ") + " m"), handle); stage.append(card);
    }
    if (!nodes.length) stage.append(el("div", { className: "portal-empty" }, el("h2", {}, "Connect places in " + filter),
      el("p", {}, "Add two openings. Choose Link on the first, then Link here on the second."),
      button("Add first opening", act(addOpening), "Create a portal opening", "primary"),
      el("small", {}, "The layout starts empty. Your existing gates and saved worlds keep their behavior.")));
  }
  function input(label, value, change, attrs = {}) {
    let applied = String(value);
    const commit = event => {
      const raw = event.target.value;
      if (raw === applied) return;
      try {
        const next = attrs.type === "number" ? (raw.trim() === "" ? NaN : Number(raw)) : raw;
        if (attrs.type === "number" && !Number.isFinite(next)) throw new Error("Enter a finite number for " + label + ".");
        change(next); applied = raw;
      } catch (error) { message(error.message, true); }
    };
    const control = el("input", { value, ...attrs, "aria-label": label, onchange: commit, onblur: commit,
      onkeydown: event => { if (event.key === "Enter") { event.preventDefault(); commit(event); } } });
    return el("label", { className: "portal-field" }, el("span", {}, label), control);
  }
  function details(label, ...contents) { return el("details", {}, el("summary", {}, label), ...contents); }
  function inspect() {
    const expanded = new Set([...inspector.querySelectorAll("details[open]")].map(row => row.querySelector("summary")?.textContent));
    inspector.replaceChildren();
    const node = selected?.kind === "node" && graph.nodes.find(row => row.id === selected.id);
    const link = selected?.kind === "link" && graph.links.find(row => row.id === selected.id);
    if (node) {
      const current = () => graph.nodes.find(row => row.id === node.id);
      const update = changes => edit([{ op: "update_node", id: node.id, changes }], true);
      inspector.append(el("span", { className: "portal-kicker" }, "OPENING · " + node.dimension), el("h2", {}, node.label),
        input("Name", node.label, label => update({ label }), { maxLength: 80 }),
        el("p", { className: "muted" }, "Centre of the opening in metres. Changing these values moves the portal in the game."),
        el("div", { className: "portal-coordinates" }, ...["X", "Y", "Z"].map((axis, i) => input(axis + " (m)", node.position[i], n => {
          const position = [...current().position]; position[i] = n; update({ position }); }, { type: "number", step: .5, min: -1000000, max: 1000000 }))),
        input("Facing (degrees)", node.yaw_degrees, yaw_degrees => update({ yaw_degrees }), { type: "number", step: 15, min: -360, max: 360 }),
        button(linking ? "Cancel linking" : "Connect to another opening", act(() => connect(node.id)), "Choose a destination on the graph"),
        details("Opening size", el("p", { className: "muted" }, "Connected openings must match. The player keeps their size."),
          ...["Width (m)", "Height (m)"].map((label, i) => input(label, node.half_size[i] * 2, size => {
            const half_size = [...current().half_size]; half_size[i] = size / 2; update({ half_size }); }, { type: "number", step: .5, min: .5, max: 64 }))),
        details("Graph placement & identity", el("code", {}, node.id), ...["Graph X", "Graph Y"].map((label, i) => input(label, node.canvas[i], n => {
          const canvas = [...current().canvas]; canvas[i] = n; update({ canvas }); }, { type: "number", min: -10000, max: 10000 }))),
        button("Remove opening", act(() => { edit([{ op: "remove_node", id: node.id }]); selected = null; render(); })));
    } else if (link) {
      const a = graph.nodes.find(node => node.id === link.from), b = graph.nodes.find(node => node.id === link.to);
      inspector.append(el("span", { className: "portal-kicker" }, "FOLD LINK"), el("h2", {}, a.label + " → " + b.label),
        el("p", { className: "muted" }, "Live view and player passage within " + a.dimension + ". Facing and momentum follow the exit transform."));
      for (const [key, label] of [["two_way", "Allow return through this link"], ["enabled", "Enabled in the next build"]]) {
        inspector.append(el("label", { className: "portal-check" }, el("input", { type: "checkbox", checked: link[key], onchange: event => {
          edit([{ op: "update_link", id: link.id, changes: { [key]: event.target.checked } }]); } }), label));
      }
      if (!link.two_way) inspector.append(el("p", { className: "portal-warning" }, "One-way. Review checks whether another portal route leads back."));
      inspector.append(el("p", {}, "Travel preview: " + a.label + (link.two_way ? " ↔ " : " → ") + b.label + (link.enabled ? "" : " (disabled)")),
        el("small", { className: "muted" }, "This is a route preview. In-game passage still requires a clear, loaded exit and a body that fits."),
        button("Select destination", () => { selected = { kind: "node", id: b.id }; render(); }),
        button("Disconnect", act(() => { edit([{ op: "remove_link", id: link.id }]); selected = null; render(); })));
    } else {
      inspector.append(el("span", { className: "portal-kicker" }, "LAYOUT"), el("h2", {}, filter),
        el("p", {}, "Place openings, make a link, then tune the selected opening or route."),
        el("p", { className: "muted" }, "Drag a card to arrange the graph. Drag the background to pan. Use Fit to see the whole dimension."),
        details("Dimension & traversal support", el("p", {}, "Surface, Hell and Manyfold are existing world families. This editor creates portal openings within one family."),
          el("p", {}, "Hell's built-in realm-transfer gates keep their current save and inventory behavior. Custom cross-dimension links, new world generators, and entity or projectile passage are not available here."),
          el("p", {}, "Surface and Hell folds use the upright player controller. Manyfold uses its radial controller. Openings do not carve a doorway or build a frame.")),
        details("Other asset flows", el("p", {}, "Recipes and advancements have their own typed wiring. Materials, tools, sprites and models keep their existing catalog editors."),
          button("Recipe wiring", ctx.recipes), button("Advancement wiring", ctx.advancements), button("Open asset catalog", ctx.catalog)));
    }
    inspector.append(details("Saved revisions", ...(base?.revisions?.length ? base.revisions.map(revision => button(
      "Restore before " + new Date(revision.at).toLocaleString(), act(() => revert(revision.id)), "Refuses if the file changed after this revision")) : [el("p", { className: "muted" }, "No portal revisions yet.")])));
    inspector.append(el("div", { className: "portal-activation" }, "Publish saves the catalog. Rebuild and relaunch the game and all multiplayer peers to activate it. Applies to all seeds of this world family."));
    for (const row of inspector.querySelectorAll("details")) row.open = expanded.has(row.querySelector("summary")?.textContent);
  }
  async function review() {
    if (!operations.length) return;
    busy = true; render();
    try {
      const request = { version: base.version, operations: structuredClone(operations) };
      const result = await ctx.api("/api/portal-layout/preview", undefined, request);
      const close = button("Keep editing", () => dialog.close());
      const errorMessage = el("p", { className: "portal-warning", role: "alert", hidden: true });
      const save = button("Publish layout", act(async () => {
        save.disabled = true;
        try { const saved = await ctx.api("/api/portal-layout/apply", undefined, { ...request, approval: result.approval });
          load(saved); dialog.close(); message("Layout published. Rebuild Axiom to activate. Saved revisions can restore it.");
        } catch (error) { save.disabled = false; errorMessage.textContent = error.message; errorMessage.hidden = false; }
      }), "Save this exact validated revision", "primary");
      save.disabled = !result.changed;
      const summaries = Object.entries(result.changes).map(([kind, changes]) => el("p", {},
        `${kind}: ${changes.added.length} added · ${changes.changed.length} changed · ${changes.removed.length} removed`));
      dialog.replaceChildren(el("span", { className: "portal-kicker" }, "NATIVE VALIDATION PASSED"), el("h2", {}, "Review portal layout"),
        ...summaries, el("h3", {}, "Routes"), ...result.graph.links.map(link => el("p", {}, `${link.from} ${link.two_way ? "↔" : "→"} ${link.to} · ${link.enabled ? "enabled" : "disabled"}`)),
        ...(result.warnings.length ? result.warnings.map(warning => el("p", { className: "portal-warning" }, warning.target + ": " + warning.message)) : [el("p", {}, "No broken links or missing return paths.")]),
        el("p", { className: "muted" }, result.activation), errorMessage, el("div", { className: "toolbar" }, close, save));
      dialog.showModal();
    } finally { busy = false; render(); }
  }
  async function revert(bundle) {
    if (operations.length) throw new Error("Publish or reload your draft before restoring a saved revision.");
    const saved = await ctx.api("/api/portal-layout/revert", undefined, { bundle });
    load(saved); message("Previous catalog restored. Rebuild to activate it.");
  }
  function load(saved) { base = saved; graph = structuredClone(saved.graph); operations = []; undo = []; redo = []; linking = null; render(); }
  async function reload() {
    if (operations.length) {
      dialog.replaceChildren(el("h2", {}, "Discard this draft?"), el("p", {}, "Your published layout will be reloaded. Unsaved graph changes will be discarded."),
        button("Keep draft", () => dialog.close()), button("Discard draft and reload", act(async () => { load(await ctx.api("/api/portal-layout")); dialog.close(); fit(); message("Saved layout reloaded."); })));
      dialog.showModal(); return;
    }
    load(await ctx.api("/api/portal-layout")); fit(); message("Saved layout loaded. Add an opening to begin.");
  }
  return { async refresh() { if (!base) { try { await reload(); } catch (error) { message(error.message, true); } } }, open() { return this.refresh(); } };
}
