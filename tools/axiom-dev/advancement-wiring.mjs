import { el, button, findEntry } from "./inspector.mjs";
import { advancementPath, field, textOf } from "./idea-data.mjs";
import { address } from "./targets.mjs";
import { patch, quoted } from "./syntax.mjs";
import { wireCanvas } from "./wire-canvas.mjs";
import { advancementRecords, reparentAdvancement } from "./wiring-commands.mjs";

const family = ["Walked", "Swum", "Flown"];
const parentPort = id => "parent:" + id;
const childPort = id => "child:" + id;
const cap = (title, control) => el("label", {}, title, control);

export function mountAdvancementWiring(host, ctx) {
  let doc, revision, records = [], byId = new Map(), selected, generation = 0;
  const positions = new Map();
  const status = el("span", { className: "wire-status", role: "status" });
  const errors = el("p", { className: "wire-error", role: "status", hidden: true });
  const list = el("div", { className: "wire-recipe-list", "aria-label": "Advancement nodes" });
  const search = el("input", { type: "search", placeholder: "Find an advancement", "aria-label": "Find an advancement" });
  const scroll = el("div", { className: "wire-scroll", "aria-label": "Advancement wire canvas" });
  const inspector = el("aside", { className: "wire-inspector", "aria-label": "Advancement inspector" });
  const undo = button("Undo", () => history("undo"));
  const redo = button("Redo", () => history("redo"));
  const review = button("Review & Save", () => ctx.review(doc), "", "primary");
  host.append(el("header", { className: "wire-toolbar" }, el("h1", {}, "Advancements"), status,
    el("div", { className: "wire-tools" }, undo, redo, review,
      button("Reset Layout", () => { positions.clear(); renderCanvas(); }))),
  el("div", { className: "wire-layout" },
    el("aside", { className: "wire-library wire-adv-library" }, el("section", { className: "wire-library-recipes" },
      el("header", { className: "wire-section-head" }, el("h2", {}, "Progression"), button("+ New", ctx.createAdvancement)),
      search, list)),
    el("section", { className: "wire-main" },
      el("header", { className: "wire-canvas-bar" }, el("strong", {}, "Advancement Tree"),
        el("span", {}, "Parent → Advancement")), scroll, errors), inspector));
  search.oninput = renderList;
  search.onkeydown = event => { if (event.key === "Enter") list.firstElementChild?.click(); };

  function report(message = "") { errors.textContent = message; errors.hidden = !message; }
  async function open(target) {
    const request = ++generation;
    try {
      const source = await ctx.load(advancementPath);
      if (request !== generation) return;
      doc = source;
      read();
      if (target?.parts) {
        const wanted = JSON.stringify(target.parts);
        selected = records.find(row => JSON.stringify(row.parts) === wanted)?.id ?? selected;
      }
      if (!byId.has(selected)) selected = records.find(row => row.id === "place_block")?.id ?? records[0]?.id;
      report();
      render();
    } catch (error) { if (request === generation) report(error.message); }
  }
  function read() {
    const root = ctx.tree(doc);
    revision = doc.text;
    records = advancementRecords(root).map((row, i) => ({
      ...row, name: textOf(field(row.node, "name")), parts: address(root, [i]),
    })).filter(row => row.id);
    byId = new Map(records.map(row => [row.id, row]));
  }
  function edit(transform) {
    if (revision !== doc.text) { read(); render(); return report("Draft changed in another view. Try again."); }
    try {
      const next = transform(revision);
      if (next === revision) return;
      if (ctx.edit(doc, next)) { read(); report(); render(); }
    } catch (error) { report(error.message); }
  }
  function apply(node, raw) {
    if (!node) return report("This field is missing from the definition");
    edit(text => patch(text, node.start, node.end, raw));
  }
  function link(child, parent) {
    edit(text => {
      const next = reparentAdvancement(text, records, child, parent);
      selected = child;
      return next;
    });
  }
  function history(direction) {
    if (!doc?.[direction]?.length) return;
    ctx[direction](doc);
    read();
    if (!byId.has(selected)) selected = records[0]?.id;
    report();
    render();
  }
  function select(id, reveal = false) {
    selected = id;
    renderList(); renderInspector();
    for (const node of scroll.querySelectorAll(".wire-adv-node")) {
      node.classList.toggle("wire-selected", node.dataset.advancement === id);
      if (reveal && node.dataset.advancement === id) node.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }
  function renderList() {
    const query = search.value.toLowerCase().trim();
    list.replaceChildren(...records.filter(row => (row.name + " " + row.id).toLowerCase().includes(query)).map(row => {
      const control = button(row.name, () => select(row.id, true), row.id, "wire-recipe-row");
      control.setAttribute("aria-current", String(row.id === selected));
      return control;
    }));
  }
  function layout() {
    const children = new Map();
    for (const row of records) {
      const key = byId.has(row.parent) ? row.parent : "";
      if (!children.has(key)) children.set(key, []);
      children.get(key).push(row);
    }
    let next = 30, depth = 0;
    const placed = new Map(), seen = new Set();
    function visit(row, level) {
      if (seen.has(row.id)) return next;
      seen.add(row.id);
      depth = Math.max(depth, level);
      const offsets = (children.get(row.id) ?? []).map(child => visit(child, level + 1));
      const y = offsets.length ? (offsets[0] + offsets.at(-1)) / 2 : next;
      if (!offsets.length) next += 55;
      placed.set(row.id, { x: 25 + level * 192, y });
      return y;
    }
    for (const row of children.get("") ?? []) visit(row, 0);
    for (const row of records) if (!seen.has(row.id)) visit(row, 0);
    return { placed, width: Math.max(565, depth * 192 + 200), height: Math.max(425, next + 30) };
  }
  function renderCanvas() {
    const { placed, width, height } = layout();
    const surface = el("div", { className: "wire-surface wire-adv-surface", style: "min-width:" + width + "px;height:" + height + "px" });
    scroll.replaceChildren(surface);
    const canvas = wireCanvas(surface), ports = new Map();
    for (const row of records) {
      const heading = el("header", { className: "wire-node-handle" }, button(row.name, () => select(row.id), row.id, "wire-adv-name"));
      const incoming = button("", () => { select(row.id); chooseParent(row); }, "Parent of " + row.name, "wire-dot input");
      const outgoing = button("", () => select(row.id), "Connect from " + row.name, "wire-dot");
      incoming.setAttribute("aria-label", incoming.title);
      outgoing.setAttribute("aria-label", outgoing.title);
      incoming.dataset.wireTarget = childPort(row.id);
      outgoing.dataset.wireTarget = parentPort(row.id);
      const card = el("div", { className: "wire-node wire-adv-node" + (row.id === selected ? " wire-selected" : ""),
        "data-advancement": row.id }, heading, incoming, outgoing);
      surface.append(card);
      const position = positions.get(row.id) ?? { ...placed.get(row.id) };
      canvas.move(card, heading, position, (x, y) => ({
        x: Math.max(6, Math.min(surface.clientWidth - card.offsetWidth - 6, x)),
        y: Math.max(6, Math.min(height - card.offsetHeight - 6, y)),
      }), moved => positions.set(row.id, moved));
      ports.set(row.id, { incoming, outgoing });
      canvas.bind(outgoing, outgoing, to => to.startsWith("child:"), to => link(to.slice(6), row.id));
    }
    const edges = [];
    for (const row of records) {
      const { incoming } = ports.get(row.id);
      const parent = ports.get(row.parent);
      const accept = to => to.startsWith("parent:") && to !== parentPort(row.parent);
      const drop = to => link(row.id, to.slice(7));
      canvas.bind(incoming, incoming, accept, drop);
      if (parent) edges.push({ from: parent.outgoing, to: incoming, select: () => select(row.id), canDrop: accept, drop });
    }
    canvas.set(edges);
  }
  function chooseParent(row) {
    findEntry("Choose Parent", records.filter(other => other.id !== row.id)
      .map(other => ({ key: other.id, label: other.name, detail: other.id })), row.parent, parent => link(row.id, parent));
  }
  function renderInspector() {
    const row = byId.get(selected);
    const title = el("header", { className: "wire-section-head" }, el("h2", {}, "Inspector"));
    if (!row) { inspector.replaceChildren(title); return; }
    const name = el("input", { type: "text", value: row.name, maxLength: 120 });
    name.onchange = () => {
      const value = name.value.trim();
      if (value) apply(field(row.node, "name"), quoted(value, "ron"));
      else report("Enter an advancement name");
    };
    const trigger = field(row.node, "trigger"), amount = trigger?.children?.at(-1);
    const event = trigger?.prefix === "Level" ? "Level" : textOf(trigger?.children?.[0]);
    const scale = event === "PlayTime" ? 60000 : family.includes(event) ? 100 : 1;
    const unit = event === "PlayTime" ? " (min)" : family.includes(event) ? " (m)" : "";
    const quantity = el("input", { type: "number", value: Number(textOf(amount)) / scale, min: 1 / scale,
      step: "any", "aria-label": "Target" + unit });
    quantity.onchange = () => {
      const raw = Math.round(Number(quantity.value) * scale);
      if (Number.isSafeInteger(raw) && raw > 0 && (event !== "Level" || raw <= 0xffffffff)) apply(amount, String(raw));
      else report("Enter a positive target within range");
    };
    inspector.replaceChildren(title,
      el("section", { className: "wire-section" }, el("h3", {}, "Advancement"),
        cap("Name", name), el("code", {}, row.id), el("p", {}, event),
        amount ? cap("Target" + unit, quantity) : null,
        ctx.assist ? button("Ask AI", () => ctx.assist({ path: doc.path, parts: row.parts, label: row.name })) : null),
      el("section", { className: "wire-section" }, el("h3", {}, "Tree Parent"),
        el("p", {}, byId.get(row.parent)?.name ?? (row.parent || "Branch Root")),
        button(row.parent ? "Change Parent" : "Choose Parent", () => chooseParent(row)),
        row.parent ? button("Disconnect", () => link(row.id, "")) : null));
  }
  function render() {
    status.textContent = ctx.dirty(doc) ? "Draft · advancements.ron" : "Saved · advancements.ron";
    undo.disabled = !doc.undo.length; redo.disabled = !doc.redo.length; review.disabled = !ctx.dirty(doc);
    renderList(); renderCanvas(); renderInspector();
  }
  return { open, refresh: () => open(), review: () => ctx.review(doc) };
}
