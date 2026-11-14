import { button, el } from "./inspector.mjs";
import { blockRegistryPath, materialGroups, materialPath } from "./material-properties.mjs";

const hex = channels => "#" + channels.map(value => Math.round(Math.min(1, Math.max(0, value)) * 255)
  .toString(16).padStart(2, "0")).join("");

export async function openMaterialEditor(ctx, id, duplicate = false) {
  let saved = await ctx.api("/api/material", id);
  let variant = duplicate, pending = {}, timer, writing, generating = false;
  let recoveryVersion;
  const storageKey = () => "axiom-material:" + saved.id + (variant ? ":variant" : ":edit");
  const status = el("p", { role: "status", className: "material-status" });
  const title = el("h2");
  const evidence = el("div");
  const fields = el("div", { className: "material-fields" });
  const variantId = el("input", { type: "text", maxLength: 64, "aria-label": "New block ID" });
  const identity = el("label", {}, "New block ID", variantId);
  const brief = el("textarea", { rows: 2, maxLength: 1200, placeholder: "e.g. Make a blue volcanic glass that chips easily",
    "aria-label": "Describe the material mutation" });
  const swatch = el("span", { className: "material-swatch", "aria-hidden": "true" });
  const describe = el("small");
  const current = () => ({ ...saved.values, ...pending });
  const remember = () => {
    try {
      if (!variant && !Object.keys(pending).length) localStorage.removeItem(storageKey());
      else localStorage.setItem(storageKey(), JSON.stringify({ pending, version: recoveryVersion ?? saved.version,
        variantId: variantId.value, brief: brief.value }));
    } catch { /* The open editor retains unsaved values. */ }
  };
  function recover() {
    try {
      const record = JSON.parse(localStorage.getItem(storageKey()));
      if (record) {
        pending = record.pending ?? {};
        recoveryVersion = record.version;
        variantId.value = record.variantId ?? variantId.value;
        brief.value = record.brief ?? "";
      }
    } catch { /* An unreadable recovery entry is ignored. */ }
  }
  function set(key, value) {
    pending[key] = value;
    remember();
    paint();
    if (!variant) {
      status.textContent = "Autosaving…";
      clearTimeout(timer);
      timer = setTimeout(() => flush(), 500);
    } else status.textContent = "Variant draft saved locally.";
  }
  function paint() {
    const values = current();
    title.textContent = variant ? "New variant of " + saved.values.display_name : values.display_name;
    swatch.style.backgroundColor = hex(values["render.color.base"]);
    describe.textContent = saved.state + " · " + saved.render + " · " + saved.slotsLeft + " free block IDs";
  }
  function draw() {
    identity.hidden = !variant;
    create.hidden = !variant;
    clone.hidden = variant;
    generate.textContent = variant ? "Generate variant settings" : "Generate & autosave";
    const values = current();
    fields.replaceChildren(...Object.entries(materialGroups).map(([group, rules]) =>
      el("fieldset", {}, el("legend", {}, group), Object.entries(rules).map(([key, rule]) => {
        const input = el("input", { type: { color: "color", boolean: "checkbox", number: "number", string: "text" }[rule.type],
          min: rule.min, max: rule.max, maxLength: rule.type === "string" ? 80 : undefined, step: "any",
          checked: rule.type === "boolean" ? values[key] : undefined,
          value: rule.type === "color" ? hex(values[key]) : rule.type === "boolean" ? undefined : values[key] ?? "",
          "aria-label": rule.label });
        input.oninput = () => {
          if (rule.type === "number" && !Number.isFinite(input.valueAsNumber)) {
            input.setCustomValidity("Enter a number.");
            status.textContent = rule.label + " needs a number.";
            return;
          }
          input.setCustomValidity("");
          const value = rule.type === "number" ? input.valueAsNumber : rule.type === "boolean" ? input.checked
            : rule.type === "color" ? input.value.slice(1).match(/../g).map(channel => Number.parseInt(channel, 16) / 255) : input.value;
          set(key, value);
        };
        return el("label", { className: "material-setting" }, el("span", {}, rule.label), input);
      }))));
    paint();
  }
  async function flush(createVariant = false) {
    clearTimeout(timer);
    if (writing) {
      if (!await writing) return false;
      return flush(createVariant);
    }
    if ((variant && !createVariant) || (!variant && !Object.keys(pending).length)) return true;
    if (recoveryVersion && recoveryVersion !== saved.version) {
      status.textContent = "The saved material changed since this draft. Reload the baseline to keep editing these values.";
      return false;
    }
    const changes = { ...pending };
    const source = variant ? saved.id : undefined;
    const key = storageKey();
    const target = source ? variantId.value.trim() : saved.id;
    status.textContent = source ? "Creating variant…" : "Autosaving…";
    writing = (async () => {
      try {
        ctx.checkMaterialDrafts(source ? [materialPath, blockRegistryPath] : [materialPath]);
        saved = await ctx.api("/api/material", undefined, { id: target, source, version: saved.version, changes });
        recoveryVersion = undefined;
        for (const key of Object.keys(changes)) {
          if (pending[key] === changes[key]) delete pending[key];
        }
        if (source) {
          variant = false;
          try { localStorage.removeItem(key); } catch { /* Storage is optional. */ }
          draw();
        }
        remember();
        paint();
        status.textContent = source ? "Variant created and saved. Rebuild to use its new block ID." : "Saved · rebuild to see it in game.";
        ctx.refreshMaterials(saved.paths);
        if (source && Object.keys(pending).length) timer = setTimeout(() => flush(), 500);
        return true;
      } catch (error) {
        status.textContent = error.message + " Your pending changes are kept here.";
        return false;
      }
    })();
    try { return await writing; } finally { writing = undefined; }
  }
  function startVariant() {
    variant = true;
    recoveryVersion = undefined;
    pending = { display_name: saved.values.display_name.slice(0, 72) + " Variant" };
    variantId.value = (saved.id + "Variant").slice(0, 64);
    recover();
    draw();
    remember();
    status.textContent = "Name the variant and adjust any settings before creating it.";
  }
  async function close() {
    if (!generating && await flush()) dialog.close();
  }
  const clone = button("Make variant", async () => {
    if (!generating && await flush()) startVariant();
  });
  const create = button("Create & save variant", () => { if (!generating) flush(true); }, "", "primary");
  const generate = button("Generate & autosave", async () => {
    if (generating || !brief.value.trim()) return;
    generating = true;
    generate.disabled = true;
    try {
      if (!await flush()) return;
      status.textContent = "Gathering source context and generating settings…";
      const starting = JSON.stringify(pending), version = saved.version, wasVariant = variant;
      const result = await ctx.api("/api/material/suggest", undefined,
        { id: saved.id, version, brief: brief.value.trim(), draft: pending });
      if (starting !== JSON.stringify(pending) || version !== saved.version || wasVariant !== variant) {
        throw new Error("Settings changed while AI was working. Generate again to use the latest values.");
      }
      Object.assign(pending, result.changes);
      remember();
      draw();
      evidence.replaceChildren(el("p", {}, result.rationale),
        el("details", {}, el("summary", {}, "Project context used"),
          result.sources.map(row => el("div", {}, row.path + ":" + row.line))));
      if (variant) status.textContent = "AI settings saved to this variant draft.";
      else await flush();
    } catch (error) { status.textContent = error.message; }
    finally { generating = false; generate.disabled = false; }
  });
  const reload = button("Reload saved baseline", async () => {
    if (writing || generating) return;
    clearTimeout(timer);
    try {
      saved = await ctx.api("/api/material", saved.id);
      recoveryVersion = undefined;
      draw();
      remember();
      status.textContent = "Reloaded. Pending changes are still in the editor.";
    } catch (error) { status.textContent = error.message; }
  });
  function mutate(changes) {
    for (const [key, value] of Object.entries(changes)) set(key, value);
    draw();
  }
  const dialog = el("dialog", { className: "finder material-dialog", "aria-label": "Material settings",
    oncancel: event => { event.preventDefault(); close(); }, onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, title, button("Done", close)),
    el("div", { className: "material-summary" }, swatch, describe, clone, identity, create),
    el("p", {}, "Changes autosave. Variants copy the material properties and get a new block ID."),
    el("div", { className: "material-summary" },
      button("Polish", () => mutate({ "render.surface.roughness": 0.18, "render.surface.specular": 0.7 })),
      button("Weather", () => mutate({ "render.surface.roughness": 0.88, "render.surface.detail_strength": 0.65 })),
      button("Reduce density by 25%", () => mutate({ "physics.density": current()["physics.density"] * 0.75 }))),
    status, fields,
    el("section", {}, el("h3", {}, "AI mutation"), brief, generate, evidence),
    el("div", { className: "material-summary" }, reload, button("Retry autosave", () => flush()),
      button("Discard pending changes", () => {
        if (writing || generating) return;
        clearTimeout(timer);
        if (variant) {
          try { localStorage.removeItem(storageKey()); } catch { /* Storage is optional. */ }
          startVariant();
        } else {
          pending = {};
          recoveryVersion = undefined;
          remember();
          draw();
          status.textContent = "Pending changes discarded.";
        }
      }),
      button("Close and keep pending changes", () => {
        if (writing || generating) return;
        clearTimeout(timer);
        remember();
        dialog.close();
      })));
  variantId.oninput = brief.oninput = remember;
  if (duplicate) startVariant();
  else {
    recover();
    draw();
    status.textContent = Object.keys(pending).length ? "Recovered pending changes. Retry autosave to publish them." : "All changes saved.";
  }
  document.body.append(dialog);
  dialog.showModal();
}
