import { el, button } from "./inspector.mjs";

const sourcePath = "assets/tools/parts.ron";
const title = key => key.replaceAll("_", " ").replace(/^./, c => c.toUpperCase());
const labels = { blocking: "Offhand blocking", charged: "Charged mining", cuts_bamboo: "Cut bamboo", fells_trees: "Fell connected trees" };
const styles = { Dig: "Pickaxe · stone and ores", Cut: "Axe · wood", Scoop: "Shovel · loose ground" };
const stable = value => value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map(k => [k, stable(value[k])])) : value;
const equal = (a, b) => JSON.stringify(stable(a)) === JSON.stringify(stable(b));
const flatten = (value, prefix = "") => Object.fromEntries(Object.entries(value).flatMap(([key, entry]) =>
  entry && typeof entry === "object" ? Object.entries(flatten(entry, prefix + key + ".")) : [[prefix + key, entry]]));

export async function openToolBehavior(ctx, schemaId, refreshed = () => {}) {
  ctx.checkMaterialDrafts([sourcePath]);
  let data = await ctx.api("/api/tool-behavior"), selected = schemaId || data.schemas[0]?.id;
  if (!data.schemas.some(row => row.id === selected)) throw new Error("Choose an existing assembled tool.");
  let draft, candidate, busy = false, lastBundle, advancedOpen = false;
  const current = () => data.schemas.find(row => row.id === selected);
  const dirty = () => !equal(draft, current().capabilities);
  const request = () => ({ schemaId: selected, version: data.version, capabilities: structuredClone(draft) });
  const status = el("p", { className: "material-status", role: "status" });
  const controls = el("div", { className: "idea-fields" });
  const review = el("section", { className: "map-inspector-section", hidden: true, "aria-label": "Behavior change review" });
  const select = el("select", { "aria-label": "Assembled tool" }, data.schemas.map(row => el("option", { value: row.id }, row.name)));
  const summary = el("p", { className: "muted" });
  const inspect = button("Review changes", async () => run(async () => {
    ctx.checkMaterialDrafts([sourcePath]);
    candidate = await ctx.api("/api/tool-behavior/preview", undefined, request());
    review.hidden = false;
    const before = flatten(candidate.before), after = flatten(candidate.after);
    const keys = [...new Set([...Object.keys(before), ...Object.keys(after)])];
    review.replaceChildren(el("h3", {}, "Review " + candidate.name), ...keys.filter(key => before[key] !== after[key]).map(key =>
      el("p", {}, title(key.replaceAll(".", " · ")) + ": " + String(before[key] ?? "None") + " → " + String(after[key] ?? "None"))));
    status.textContent = candidate.changed ? "Native validation passed. Ready to apply." : "No behavior changes.";
  }), "Validate the exact behavior before saving");
  const apply = button("Apply changes", async () => run(async () => {
    ctx.checkMaterialDrafts([sourcePath]);
    const saved = await ctx.api("/api/tool-behavior/apply", undefined, { ...request(), approval: candidate?.approval });
    lastBundle = saved.bundle;
    await reload();
    status.textContent = "Behavior saved. Rebuild and relaunch Axiom to use it.";
  }), "Save this validated revision", "primary");
  const undo = button("Undo saved change", async () => run(async () => {
    ctx.checkMaterialDrafts([sourcePath]);
    await ctx.api("/api/tool-behavior/revert", undefined, { bundle: lastBundle });
    lastBundle = undefined;
    await reload();
    status.textContent = "Previous behavior restored. Rebuild and relaunch Axiom to use it.";
  }));
  const discard = button("Discard draft", () => { reset(); status.textContent = "Draft discarded."; });
  const refresh = button("Reload and discard draft", async () => run(async () => { await reload(); status.textContent = "Loaded current definitions."; }));
  const close = () => {
    if (busy) return;
    if (dirty()) { status.textContent = "Apply or discard the draft before closing."; return; }
    dialog.close();
  };
  const dialog = el("dialog", { className: "finder material-dialog", "aria-label": "Tool behavior",
    oncancel: event => { event.preventDefault(); close(); }, onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, el("h2", {}, "Tool behavior"), button("Close", close)),
    el("label", { className: "idea-row" }, "Assembled tool", select), summary, controls,
    status, review, el("div", { className: "system-board-actions" }, inspect, apply, discard, undo, refresh));
  function sync() {
    select.disabled = busy || dirty();
    for (const input of controls.querySelectorAll("input, select")) input.disabled = busy;
    inspect.disabled = busy || !dirty();
    apply.disabled = busy || !candidate?.changed;
    discard.disabled = busy || !dirty(); undo.disabled = busy || !lastBundle || dirty(); refresh.disabled = busy;
  }
  async function run(action) {
    if (busy) return;
    busy = true; sync();
    try { await action(); }
    catch (error) { candidate = undefined; review.hidden = true; status.textContent = error.message; }
    finally { busy = false; sync(); }
  }
  async function reload() {
    const latest = await ctx.api("/api/tool-behavior");
    if (!latest.schemas.length) throw new Error("No assembled-tool schemas are available.");
    await ctx.refreshMaterials([sourcePath]);
    data = latest;
    if (!data.schemas.some(row => row.id === selected)) { selected = data.schemas[0].id; lastBundle = undefined; }
    select.replaceChildren(...data.schemas.map(row => el("option", { value: row.id }, row.name)));
    reset(); refreshed();
  }
  function reset() { draft = structuredClone(current().capabilities); candidate = undefined; review.hidden = true; draw(); }
  function change(action) { action(); candidate = undefined; review.hidden = true; status.textContent = "Unsaved behavior draft."; draw(); }
  function toggle(key, value, action, note) {
    return el("div", { className: "tune-control" }, el("label", { className: "tune-label" },
      el("strong", {}, labels[key] || title(key)), el("input", { type: "checkbox", role: "switch", checked: value,
        "aria-label": labels[key] || title(key), onchange: event => change(() => action(event.target.checked)) })),
      note ? el("p", { className: "muted" }, note) : null);
  }
  function draw() {
    select.value = selected;
    summary.textContent = current().name + " · " + current().parts.join(" + ") + ". Changes apply to all matching assembled tools after a rebuild.";
    const basics = el("section", { className: "map-inspector-section" }, el("h3", {}, "Capabilities"),
      toggle("mining", draft.mining !== null, enabled => { draft.mining = enabled ? structuredClone(data.miningDefaults) : null; },
        "Break blocks using the assembled tool's material tier and mining speed."));
    for (const [key, value] of Object.entries(draft)) if (typeof value === "boolean") basics.append(toggle(key, value, enabled => { draft[key] = enabled; },
      key === "blocking" ? "Guard while the unbroken tool is held in the offhand." : undefined));
    const advanced = el("details", { className: "map-source-details", open: advancedOpen,
      ontoggle: event => { advancedOpen = event.currentTarget.open; } }, el("summary", {}, "Advanced behavior"));
    if (draft.mining) {
      const style = el("select", { "aria-label": "Mining style", value: draft.mining.class },
        data.miningStyles.map(id => el("option", { value: id }, styles[id] || id)));
      style.value = draft.mining.class;
      style.onchange = () => change(() => {
        draft.mining.class = style.value;
        if (style.value !== "Cut") draft.mining.fells_trees = false;
      });
      basics.append(el("label", { className: "tune-label" }, "Mining style", style));
      for (const [key, value] of Object.entries(draft.mining)) {
        if (typeof value !== "boolean" || key === "fells_trees" && draft.mining.class !== "Cut") continue;
        advanced.append(toggle(key, value, enabled => { draft.mining[key] = enabled; },
          key === "charged" ? "Hold to charge the existing crater-mining action." : undefined));
      }
    }
    const command = el("details", { className: "map-source-details" }, el("summary", {}, "AI command and source"),
      el("p", { className: "muted" }, "node tools/axiom-dev/tool-behavior-cli.mjs preview"),
      el("pre", { style: "white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px" }, JSON.stringify(request(), null, 2)),
      el("p", { className: "muted" }, sourcePath + " · Existing schema IDs and parts are retained. Native combat and new effects require engine support."));
    controls.replaceChildren(basics, ...(draft.mining ? [advanced] : []), command);
    sync();
  }
  select.onchange = () => { selected = select.value; lastBundle = undefined; reset(); status.textContent = "Saved behavior."; };
  reset(); status.textContent = "Saved behavior. Changes require a rebuild.";
  document.body.append(dialog); dialog.showModal();
}
