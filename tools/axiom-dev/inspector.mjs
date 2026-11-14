import { addEntry, at, decode, deleteEntry, patch, quoted, title } from "./syntax.mjs";
import { links } from "./links.mjs";

export function el(tag, attrs = {}, ...children) {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value == null) continue;
    if (key.startsWith("on")) element.addEventListener(key.slice(2), value);
    else if (key !== "list" && key in element) element[key] = value;
    else element.setAttribute(key, value);
  }
  element.append(...children.flat().filter(item => item != null));
  return element;
}

export function button(label, onclick, title = "", className = "") {
  return el("button", { type: "button", onclick, title, className }, label);
}

export const panel = (title, ...body) => el("section", { className: "panel" }, el("h3", {}, title), ...body);
const muted = text => el("p", { className: "muted" }, text);
const human = key => key.replace(/_/g, " ").replace(/^\p{Ll}/u, letter => letter.toUpperCase());
const caption = (node, index) => node.key === undefined ? title(node, index) : human(node.key);
export const isRecordList = node => Boolean(node?.children?.length && node.children.every(child =>
  child.key === undefined && child.children));

export function findEntry(label, entries, selected, onSelect) {
  const input = el("input", { type: "search", placeholder: "Type a name or ID…", autofocus: true,
    "aria-label": label });
  const results = el("div", { className: "finder-results", "aria-label": "Matches" });
  const status = el("span", { className: "muted", role: "status" });
  let matches = [];
  let active = 0;
  const dialog = el("dialog", { className: "finder", "aria-label": label,
    onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, el("h2", {}, label), button("Close", () => dialog.close())),
    input, results, status);

  function pick(index) {
    const entry = matches[index];
    if (!entry) return;
    dialog.close();
    onSelect(entry.key);
  }

  function select(index, scroll = false) {
    active = index;
    for (const [i, row] of [...results.children].entries()) row.classList.toggle("active", i === index);
    if (scroll) results.children[index]?.scrollIntoView({ block: "nearest" });
  }

  function update(initial = false) {
    const terms = input.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const found = entries.filter(entry => terms.every(term => (entry.label + " " + entry.detail).toLowerCase().includes(term)));
    matches = found.slice(0, 100);
    results.replaceChildren(...matches.map((entry, index) => {
      const row = button("", () => pick(index), entry.label, "finder-row");
      row.append(el("strong", {}, entry.label));
      if (entry.detail) row.append(el("small", {}, entry.detail));
      if (entry.key === selected) row.setAttribute("aria-current", "true");
      return row;
    }));
    status.textContent = found.length ? found.length + " matches" + (found.length > matches.length ? " · refine search" : "") : "No matches";
    select(initial ? Math.max(0, matches.findIndex(entry => entry.key === selected)) : 0, initial);
  }

  input.oninput = () => update();
  input.onkeydown = event => {
    if (!matches.length) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      select(Math.max(0, Math.min(matches.length - 1, active + (event.key === "ArrowDown" ? 1 : -1))), true);
    } else if (event.key === "Enter") {
      event.preventDefault();
      pick(active);
    }
  };
  document.body.append(dialog);
  dialog.showModal();
  update(true);
}

const fieldHelp = {
  id: "Stable identifier. References and saves may depend on its value or position.",
  kind: "Engine variant. Use the identifier declared by the consuming Rust type.",
  display_name: "Label shown in the game.",
  cooldown: "Time before the next action. Spell cooldowns use seconds.",
  cast_delay: "Spell delay added to the cast, in seconds.",
  recharge: "Spell delay added when the wand recharges, in seconds.",
  spread: "Spell modifier applied to aim spread.",
  hardness: "Material resistance to mining.",
  health: "Base health before difficulty or other modifiers.",
  cost: "Resource consumed by the action. Check the loader for the resource and units.",
  pattern: "Shaped recipe grid. Spaces leave slots empty. Symbols refer to key entries.",
  ingredients: "Inputs required by this recipe.",
  output: "Product granted when the recipe finishes.",
  roughness: "Material surface roughness sent to the shader.",
  occludes_light: "Whether the material blocks light propagation.",
  generator: "Whether this machine is a power source.",
};

function sourceComment(node, parent, source) {
  if (!parent) return source.slice(0, node.start).trim().slice(0, 360);
  const index = parent.children.indexOf(node);
  const prior = parent.children[index - 1];
  const start = prior ? (prior.comma ?? prior.end - 1) + 1 : parent.open;
  return source.slice(start, node.entry).split(/\r?\n/).filter(line => /^\s*\/\//.test(line))
    .map(line => line.trim().replace(/^\/\/\s*/, "")).join(" ").slice(0, 360);
}

export function hint(node, parent, source) {
  const key = node.key ?? "";
  const inferred = key.endsWith("_seconds") || key.endsWith("_secs") ? "Duration in seconds."
    : key.endsWith("_m_s") ? "Speed in metres per second."
    : key.endsWith("_per_mille") ? "Value per 1,000."
    : key.endsWith("_ms") ? "Duration in milliseconds."
    : key.endsWith("_m") ? "Distance in metres." : "";
  return [sourceComment(node, parent, source), fieldHelp[key] ?? inferred].filter(Boolean).join("\n")
    || (node.children ? "Open this group to inspect every entry." : "Edit the exact " + node.kind + " value.");
}

function pathBar(ui, root, trail) {
  return el("nav", { className: "breadcrumb", "aria-label": "Value path" },
    button(isRecordList(root) ? "All records" : "Overview", () => ui.go([])),
    trail.map((index, offset) => [el("span", {}, "›"),
      button(caption(at(root, trail.slice(0, offset + 1)), index), () => ui.go(trail.slice(0, offset + 1)))])
      .flat());
}

function scalar(ui, node, label, hintText) {
  if (node.kind === "boolean") {
    return el("input", { type: "checkbox", checked: node.raw === "true", title: hintText,
      "aria-label": label });
  }
  return el("input", { type: "text", value: node.kind === "string" ? decode(node.raw) : node.raw,
    spellcheck: false, title: hintText, "aria-label": label,
    onchange: event => {
      const input = event.currentTarget;
      const next = node.kind === "string" ? quoted(input.value, ui.doc.kind) : input.value.trim();
      const current = at(ui.tree(), input.valuePath);
      if (!current || !ui.replace(current, next, true)) input.value = current?.kind === "string" ? decode(current.raw) : current?.raw ?? "";
    },
    onkeydown: event => { if (event.key === "Enter") event.currentTarget.blur(); } });
}

function row(ui, node, parent, trail, index) {
  const label = caption(node, index);
  const description = hint(node, parent, ui.doc.text);
  if (node.children) {
    const enter = button("", () => ui.go(trail), description, "field-group");
    const summary = node.children.length > 0 && node.children.length <= 4
      && node.children.every(child => child.key === undefined && child.raw !== undefined)
      ? node.children.map(child => decode(child.raw)).join(", ")
      : (node.prefix ? node.prefix + " · " : "") + node.children.length + (isRecordList(node) ? " records" : " fields");
    enter.append(el("strong", {}, label), el("span", { className: "muted group-summary" }, summary),
      el("span", { "aria-hidden": "true", className: "chevron" }, "›"));
    return enter;
  }
  const control = scalar(ui, node, label, description);
  control.valuePath = trail;
  if (node.kind === "boolean") {
    control.onchange = () => {
      const current = at(ui.tree(), trail);
      if (!current || !ui.replace(current, String(control.checked), true)) control.checked = current?.raw === "true";
    };
  }
  const inspect = button("⋯", () => ui.go(trail), "Details for " + label, "inspect");
  inspect.setAttribute("aria-label", "Details for " + label);
  return el("div", { className: "field-row" }, el("span", { className: "field-label",
    title: (node.key ? node.key + "\n" : "") + description }, label), control, inspect);
}

function editText(ui, label, initial, commit, help) {
  const area = el("textarea", { value: initial, spellcheck: false, "aria-label": label });
  ui.dialog(label, [muted(help), area], "Apply", () => commit(area.value));
}

function recordPicker(ui, parent, trail) {
  const index = trail.at(-1);
  const base = trail.slice(0, -1);
  const go = index => ui.go([...base, index]);
  const current = caption(parent.children[index], index);
  const finder = button(current + "  ▾", () => {
    const choices = parent.children.map((child, key) => {
      const id = child.children.find(field => field.key === "id")?.raw
        ?? child.children.find(field => field.key === "kind")?.raw
        ?? child.children[0]?.raw;
      return { key, label: caption(child, key), detail: id === undefined ? "" : decode(id) };
    });
    findEntry("Find record", choices, index, go);
  }, "Find record in this file", "quick-find");
  finder.setAttribute("aria-label", "Find record. Current record " + current);
  finder.setAttribute("aria-haspopup", "dialog");
  const previous = button("‹", () => go(index - 1), "Previous record");
  const next = button("›", () => go(index + 1), "Next record");
  previous.disabled = index === 0;
  next.disabled = index === parent.children.length - 1;
  previous.setAttribute("aria-label", "Previous record");
  next.setAttribute("aria-label", "Next record");
  return el("div", { className: "record-picker" }, el("span", { className: "muted" }, "Record"), finder,
    previous, next, muted((index + 1) + " / " + parent.children.length),
    button("All records", () => ui.go(base), "Browse and filter this record list", "browse-records"));
}

export function fieldsView(ui, root, trail) {
  const node = at(root, trail);
  const parent = trail.length ? at(root, trail.slice(0, -1)) : null;
  const source = ui.doc.text;
  const index = trail.at(-1);
  const record = isRecordList(parent);
  const collection = isRecordList(node);
  const label = trail.length ? caption(node, index) : collection ? "Records" : "Fields";
  const wrapper = el("div", { className: "field-sheet" });
  if (record) wrapper.append(recordPicker(ui, parent, trail));
  else if (trail.length) wrapper.append(pathBar(ui, root, trail));
  const toolbar = el("div", { className: "field-header" });
  const actions = el("div", { className: "stack" });
  const value = () => at(ui.tree(), trail);
  const parentValue = () => at(ui.tree(), trail.slice(0, -1));
  actions.append(button("Edit raw value", () => {
    const latest = value();
    editText(ui, "Edit raw value", ui.doc.text.slice(latest.start, latest.end),
      next => ui.replace(latest, next), "Format · " + ui.doc.kind.toUpperCase());
  }, "Edit this value's source"));
  if (parent) {
    actions.append(button(record ? "Duplicate record" : "Duplicate entry", () => {
      const latest = value();
      const text = ui.doc.text;
      const snippet = text.slice(latest.entry, latest.end);
      if (latest.key !== undefined) {
        editText(ui, "Duplicate field", snippet, entry => ui.update(addEntry(text, parentValue(), entry)),
          "Give the copy a unique field name.");
      } else {
        ui.update(addEntry(text, parentValue(), snippet));
      }
    }));
    actions.append(button(record ? "Delete record" : "Delete entry", () => ui.dialog("Delete entry", [muted(label)], "Delete", () => {
      if (!ui.update(deleteEntry(ui.doc.text, parentValue(), index), true)) return false;
      ui.go(trail.slice(0, -1));
      return true;
    }), "Remove this entry from the draft", "danger"));
  }
  if (node.children) {
    const add = collection ? "Add record" : node.children.some(child => child.key !== undefined) ? "Add field" : "Add entry";
    actions.prepend(button(add, () => {
      const latest = value();
      const text = ui.doc.text;
      const example = latest.children[0];
      const initial = example ? text.slice(example.entry, example.end)
        : latest.opener === "{" ? (ui.doc.kind === "json" ? '"key": null' : '"key": None') : "";
      editText(ui, add, initial, entry => entry.trim() && ui.update(addEntry(text, latest, entry.trim())),
        "Format · " + ui.doc.kind.toUpperCase());
    }));
  }
  actions.addEventListener("click", () => { actions.parentElement.open = false; });
  const more = el("details", { className: "field-menu" }, el("summary", {}, "Actions"), actions);
  toolbar.append(el("h2", {}, label), more);
  if (trail.length && ui.assistField) {
    toolbar.append(button("Ask AI", () => ui.assistField(trail, label, hint(node, parent, source)),
      "Get help with this value and propose a draft change"));
  }
  wrapper.append(toolbar);
  const description = hint(node, parent, source);
  if (!node.children) {
    wrapper.append(row(ui, node, parent, trail, index), muted(description));
    return wrapper;
  }
  const list = el("div", { className: "field-list" });
  const filter = collection ? "Filter records" : "Filter fields";
  const search = el("input", { type: "search", placeholder: filter + "…", value: ui.doc.filter ?? "",
    "aria-label": filter, oninput: event => {
      ui.doc.filter = event.currentTarget.value;
      show(ui.doc.filter);
    } });
  const show = query => {
    const needle = query.toLowerCase().trim();
    const current = at(ui.tree(), trail);
    if (!current?.children) return;
    const results = current.children.map((child, index) => ({ child, index }))
      .filter(({ child, index }) => !needle || caption(child, index).toLowerCase().includes(needle)
        || child.raw?.toLowerCase().includes(needle));
    list.replaceChildren(...results.slice(0, 180).map(({ child, index }) => row(ui, child, current, [...trail, index], index)));
    if (!results.length) list.append(muted("No matches"));
    if (results.length > 180) list.append(muted("180 of " + results.length + " matches"));
  };
  if (collection || node.children.length > 15) toolbar.insertBefore(search, more);
  wrapper.append(list);
  const notes = sourceComment(node, parent, source) || sourceComment(root, null, source);
  if (notes) wrapper.append(el("details", { className: "source-notes" }, el("summary", {}, "Source notes"), muted(notes)));
  show(search.value);
  return wrapper;
}

function graphNode(label, detail, onclick, active = false) {
  const tag = onclick ? "button" : "div";
  return el(tag, { type: onclick ? "button" : undefined, onclick, title: detail,
    className: "graph-node" + (active ? " current" : "") }, label, el("small", {}, detail));
}

export function graphView(ui, root, trail) {
  const node = at(root, trail);
  const left = el("div", { className: "graph-column" }, el("h3", {}, "Parent / references →"));
  const middle = el("div", { className: "graph-column" }, el("h3", {}, "Selected"));
  const right = el("div", { className: "graph-column" }, el("h3", {}, "→ Fields / references"));
  if (trail.length) {
    const parent = trail.slice(0, -1);
    left.append(graphNode(parent.length ? title(at(root, parent), parent.at(-1)) : "Root",
      "Parent group", () => ui.go(parent)));
  }
  const label = trail.length ? title(node, trail.at(-1)) : ui.doc.path.split("/").at(-1);
  middle.append(graphNode(label, node.children ? node.children.length + " entries" : node.raw.slice(0, 180), null, true),
    button("Edit selected", () => ui.setView("fields")));
  node.children?.slice(0, 28).forEach((child, index) => {
    right.append(graphNode(title(child, index), child.children ? child.children.length + " entries" : child.raw.slice(0, 100),
      () => ui.go([...trail, index])));
  });
  if (node.children?.length > 28) right.append(muted("More entries in Fields"));
  const info = muted("Finding catalog references…");
  const layout = el("div", {}, pathBar(ui, root, trail), el("div", { className: "graph" }, left, middle, right), info);
  links(ui.api, ui.paths(), ui.doc.path, root, node, trail).then(({ incoming, outgoing }) => {
    for (const [column, entries, direction] of [[left, incoming, "Uses selected record"], [right, outgoing, "Referenced record"]]) {
      for (const entry of entries.slice(0, 24)) {
        column.append(graphNode(entry.label, direction + " · " + entry.path, () => ui.open(entry.path, entry.at)));
      }
    }
    info.textContent = incoming.length + " incoming · " + outgoing.length
      + " outgoing catalog references. Recognized IDs only. Showing up to 24 per direction.";
  }).catch(error => { info.textContent = error.message; });
  return layout;
}

