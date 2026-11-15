import { el, button, panel } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";

// Realm layout editor. Every room and structure of the Gilded Cave is one
// native Layout; the running engine serves its schema (groups, bounds, units,
// options, help) so this panel holds no second copy of the fields. Edits are
// live previews: the game regenerates resident chunks. "Save to world" writes
// the world's gilded_layout.ron sidecar.
export function layoutPanel(api, note) {
  let schema = null;
  let pending = null;
  let timer = 0;
  const inputs = new Map();
  const status = el("p", { className: "muted" }, "Open a Gilded Cave world (Start fresh world → gilded), then Load layout.");
  const groups = el("div", { className: "layout-groups" });
  const filter = el("input", { type: "search", placeholder: "Filter fields", "aria-label": "Filter layout fields" });
  const live = el("input", { type: "checkbox", checked: true, "aria-label": "Apply edits live" });
  const rerollSeed = el("input", { type: "number", min: 0, max: 4294967295, value: 1, "aria-label": "Reroll seed" });
  const views = el("select", { "aria-label": "Review view" });
  const captureName = el("input", { placeholder: "Capture name", "aria-label": "Capture name" });
  const output = el("pre", { className: "log", "aria-label": "Layout state" });

  const request = async body => {
    const result = await api("/api/engine", undefined, { op: "layout", ...body });
    return result.data;
  };
  const act = fn => async () => { try { await fn(); } catch (error) { note(error.message, true); } };

  function show(data) {
    schema = data;
    status.textContent = `Seed ${data.seed} · ${data.overridden ? "edited layout" : "seed layout"} · ${data.plan}`;
    if (data.note) output.textContent = typeof data.note === "string" ? data.note : JSON.stringify(data.note, null, 2);
    if (views.options.length !== data.views.length) {
      views.replaceChildren(...data.views.map(name => el("option", { value: name }, name)));
    }
  }

  function read(field, input) {
    if (field.type === "boolean") return input.checked;
    if (field.type === "color") return input.value;
    return Number(input.value);
  }

  function queue(key, value) {
    pending = { ...(pending || {}), [key]: value };
    if (!live.checked) return;
    clearTimeout(timer);
    timer = setTimeout(act(flush), 450);
  }

  async function flush() {
    if (!pending) return;
    const values = pending;
    pending = null;
    show(await request({ action: "set", values }));
  }

  function control(field, value) {
    const wrap = el("label", { className: "layout-field", title: field.help || "" });
    const name = el("span", {}, field.label + (field.unit ? ` (${field.unit})` : ""));
    let input;
    if (field.type === "boolean") {
      input = el("input", { type: "checkbox", checked: Boolean(value) });
      input.onchange = () => queue(field.key, input.checked);
      wrap.append(input, name);
    } else if (field.type === "color") {
      input = el("input", { type: "color", value });
      input.oninput = () => queue(field.key, input.value);
      wrap.append(name, input);
    } else if (field.type === "choice") {
      input = el("select", {}, ...field.options.map((label, i) => el("option", { value: i }, label)));
      input.value = String(Math.round(value));
      input.onchange = () => queue(field.key, Number(input.value));
      wrap.append(name, input);
    } else {
      const range = el("input", { type: "range", min: field.min, max: field.max, step: field.step, value });
      input = el("input", { type: "number", min: field.min, max: field.max, step: field.step, value, className: "layout-number" });
      range.oninput = () => { input.value = range.value; queue(field.key, Number(range.value)); };
      input.onchange = () => { range.value = input.value; queue(field.key, Number(input.value)); };
      wrap.append(name, range, input);
      inputs.set(field.key + ":range", { field, input: range });
    }
    inputs.set(field.key, { field, input });
    return wrap;
  }

  function build(data) {
    show(data);
    inputs.clear();
    const byGroup = new Map();
    for (const field of data.fields) {
      if (!byGroup.has(field.group)) byGroup.set(field.group, []);
      byGroup.get(field.group).push(field);
    }
    groups.replaceChildren(...[...byGroup].map(([group, fields], index) =>
      el("details", { open: index < 2, className: "layout-group" },
        el("summary", {}, `${group} · ${fields.length}`),
        el("div", { className: "layout-grid" }, ...fields.map(field => control(field, data.values[field.key]))))));
    applyFilter();
  }

  function sync(data) {
    show(data);
    for (const [key, { field, input }] of inputs) {
      const value = data.values[key.replace(":range", "")];
      if (field.type === "boolean") input.checked = Boolean(value);
      else if (field.type === "choice") input.value = String(Math.round(value));
      else input.value = value;
    }
  }

  function applyFilter() {
    const text = filter.value.trim().toLowerCase();
    for (const details of groups.children) {
      let shown = 0;
      for (const label of details.querySelectorAll(".layout-field")) {
        const match = !text || label.textContent.toLowerCase().includes(text) || details.firstChild.textContent.toLowerCase().includes(text);
        label.hidden = !match;
        if (match) shown += 1;
      }
      details.hidden = shown === 0;
      if (text && shown) details.open = true;
    }
  }
  filter.oninput = applyFilter;

  const load = button("Load layout", act(async () => build(await request({ action: "describe" }))), "Read the schema and values from the running game");
  const apply = button("Apply", act(async () => {
    const values = Object.fromEntries([...inputs].filter(([key]) => !key.endsWith(":range")).map(([key, { field, input }]) => [key, read(field, input)]));
    pending = null;
    show(await request({ action: "set", values }));
  }), "Apply every field now and regenerate");
  const reset = button("Reset to seed", act(async () => sync(await request({ action: "reset" }))), "Drop the edits; the world uses its seed layout");
  const reroll = button("Reroll shapes", act(async () => sync(await request({ action: "reroll", seed: Number(rerollSeed.value) }))), "Take every shape from this seed; keep the light");
  const dice = button("🎲", act(async () => {
    rerollSeed.value = Math.floor(Math.random() * 4294967295);
    sync(await request({ action: "reroll", seed: Number(rerollSeed.value) }));
  }), "Reroll from a random seed");
  const save = button("Save to world", act(async () => show(await request({ action: "save" }))), "Write gilded_layout.ron beside the world save");
  const exportRon = button("Copy RON", act(async () => {
    const data = await request({ action: "export" });
    show(data);
    await copyText(data.note);
    note("Layout RON copied");
  }), "Copy every field as RON");
  const go = button("Go to view", act(async () => show(await request({ action: "view", view: views.value }))), "Move the camera to a review pose (creative or spectator)");
  const capture = button("Capture", act(async () => {
    const result = await api("/api/engine", undefined, { op: "capture", name: captureName.value || ("layout-" + views.value) });
    output.textContent = JSON.stringify(result.data, null, 2);
  }), "Save a native frame of the current view");

  return panel("Realm layout editor",
    el("p", { className: "muted" }, "Gilded Cave rooms and structures · live regenerate · Save writes the world sidecar"),
    status,
    el("div", { className: "chips" }, load, apply, reset, el("label", {}, live, " Live")),
    el("div", { className: "chips" }, el("label", {}, "Seed ", rerollSeed), reroll, dice, save, exportRon),
    el("div", { className: "chips" }, views, go, captureName, capture),
    filter,
    groups,
    output,
    el("style", {}, `
      .layout-groups{display:flex;flex-direction:column;gap:6px;margin:8px 0}
      .layout-group>summary{cursor:pointer;font-weight:600;padding:4px 0}
      .layout-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:6px 14px;padding:6px 0 10px}
      .layout-field{display:grid;grid-template-columns:1fr 120px 70px;align-items:center;gap:6px;font-size:12px}
      .layout-field:has(input[type=checkbox]){grid-template-columns:auto 1fr}
      .layout-field:has(input[type=color]),.layout-field:has(select){grid-template-columns:1fr 120px}
      .layout-number{width:70px}`));
}
