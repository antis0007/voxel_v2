import fabricationMetadata from "./fabrication-contract.mjs";
import { parse, at, patch } from "./syntax.mjs";
import { clearLinks } from "./links.mjs";
import { el, button, panel, fieldsView, graphView, isRecordList } from "./inspector.mjs";
import { mountPlaytest } from "./playtest.mjs";
import { isSheet, readSheet, cells, rgba } from "./sprite-sheet.mjs";
import { spriteView } from "./sprite-editor.mjs";
import { encodePng, stepPng, changedRegion, readTile, canvasPixels } from "./pixel-grid.mjs";
import { makeNavigation, quickOpen } from "./quick-open.mjs";
import { mountGraph } from "./graph-workspace.mjs";
import { openMaterialEditor } from "./material-editor.mjs";
import { materialPath } from "./material-properties.mjs";
import { mountCatalog, groups as catalogGroups } from "./catalog.mjs";
import { installPalette, openPalette, registerCommands } from "./command-palette.mjs";
import { mountShell, receivePanel } from "./forge-shell.mjs";
import { recipeArtwork } from "./catalog-icons.mjs";
import { recipeRows, fabricationRecipes } from "./catalog-data.mjs";
import { fabricationPath } from "./fabrication-data.mjs";
import { showContextMenu, copyText } from "./context-menu.mjs";
import { dragSource, dropTarget } from "./slot-tools.mjs";
import { mountAgent, sourceFromDocument } from "./agent-panel.mjs";
import { mountBacklog } from "./backlog-panel.mjs";
import { canAssist } from "./agent-contract.mjs";
import { createFood } from "./food-creator.mjs";
import { foodPaths } from "./food-item.mjs";
import { advancementPath, field, textOf, recipePath } from "./idea-data.mjs";
import { address, resolve } from "./targets.mjs";
import { mountRecipeWiring } from "./recipe-wiring.mjs";
import { mountAdvancementWiring } from "./advancement-wiring.mjs";
import { mountPortalLayout } from "./portal-panel.mjs";
import { editSparkSpeed, sparkFields } from "./forge-spark.mjs";
import { previewSpellBlocks, readSpells, spellChoices, spellFields, spellPath } from "./spell-wiring.mjs";
import { createIdea } from "./idea-creator.mjs";

const $ = id => document.getElementById(id);
const token = location.hash.slice(1);
const fabricationVersion = crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(fabricationMetadata)))
  .then(bytes => [...new Uint8Array(bytes)].map(byte => byte.toString(16).padStart(2, "0")).join(""));
let index = [];
// Per-browser view memory; storage can be unavailable in private windows.
const memory = {
  get(key, fallback) { try { return localStorage.getItem("axiom-dev." + key) ?? fallback; } catch { return fallback; } },
  set(key, value) { try { localStorage.setItem("axiom-dev." + key, value); } catch { /* private window */ } },
};
let category = memory.get("files.category", "Data");
let current;
let activeTab = "catalog";
let graphMode = "wiring";
let selection = 0;
let imageUrl;
let usageSelection = 0;
let dialogAction;
const drafts = new Map();
const loading = new Map();

async function api(route, path, payload, binary = false) {
  const url = new URL(route, location.origin);
  if (path) url.searchParams.set("path", path);
  const headers = { "X-Axiom-Session": token, "X-Axiom-Fabrication-Schema": await fabricationVersion };
  const upload = payload instanceof Blob;
  const body = payload === undefined || upload ? payload : JSON.stringify(payload);
  const options = body === undefined ? { headers } : {
    method: "POST", headers: { ...headers, "Content-Type": upload ? "application/octet-stream" : "application/json" }, body,
    keepalive: route === "/api/sprite-draft" && new TextEncoder().encode(body).length <= 48_000,
  };
  const response = await fetch(url, options);
  if (!response.ok) throw new Error((await response.json()).error ?? response.statusText);
  return binary ? response.blob() : response.json();
}

const navigation = makeNavigation(api);

function note(message, error = false) {
  syncDraftCount();
  $("notice").textContent = message;
  $("notice").className = error ? "error" : "";
}

function prompt(title, content, confirm, action) {
  $("dialog-title").textContent = title;
  $("dialog-content").replaceChildren(...content);
  $("dialog-confirm").textContent = confirm;
  dialogAction = action;
  $("dialog").showModal();
}

$("dialog-cancel").onclick = () => $("dialog").close();
$("dialog-form").onsubmit = async event => {
  event.preventDefault();
  const confirm = $("dialog-confirm");
  confirm.disabled = true;
  $("dialog-content").querySelector(".dialog-error")?.remove();
  try {
    if (await dialogAction() !== false) $("dialog").close();
  } catch (error) {
    note(error.message, true);
    $("dialog-content").append(el("p", { className: "error dialog-error" }, error.message));
  } finally {
    confirm.disabled = false;
  }
};

function dirty(doc) { return doc?.text !== doc?.original || Boolean(doc?.spriteBitmap?.changes); }

const categories = {
  Data: entry => entry.path.startsWith("assets/") && ["ron", "json"].includes(entry.kind)
    && !/^assets\/(art_direction|concepts|source)\//.test(entry.path),
  Sprites: entry => entry.sprite?.mode === "source",
  "All files": () => true,
};

const editableSprite = path => index.find(entry => entry.path === path)?.sprite?.mode === "source";

const sheetNames = {
  "effects16.txt": "Effect icons", "items16.txt": "Item icons", "machines16.txt": "Machine icons",
  "resources16.txt": "Resource icons", "sprites16.txt": "Tool masks",
};
const fileLabel = path => {
  if (path === spellPath) return "Spells";
  const name = path.split("/").at(-1);
  return isSheet(path) ? sheetNames[name] : name;
};
let visiblePaths = [];
function list() {
  const terms = $("search").value.toLowerCase().split(/\s+/).filter(Boolean);
  const matches = index.filter(entry => (category !== "Sprites" || categories.Sprites(entry)) && (terms.length
    ? terms.every(term => (entry.path + " " + fileLabel(entry.path)).toLowerCase().includes(term))
    : categories[category](entry)));
  if (terms.length) {
    const score = entry => terms.filter(term => fileLabel(entry.path).toLowerCase().includes(term)).length;
    matches.sort((a, b) => score(b) - score(a));
  } else if (category === "Sprites") matches.sort((a, b) => Number(isSheet(b.path)) - Number(isSheet(a.path)));
  $("filters").replaceChildren(...Object.keys(categories).map(label => {
    const active = category === label && (!terms.length || label === "Sprites");
    const control = button(label, () => { category = label; memory.set("files.category", label); $("search").value = ""; list(); },
      "Show " + label.toLowerCase(), active ? "active" : "");
    control.setAttribute("aria-pressed", String(active));
    return control;
  }));
  $("file-count").textContent = matches.length
    ? matches.length + (terms.length ? " matches" : " files") + (matches.length > 240 ? " · refine search" : "")
    : "No matching files";
  visiblePaths = matches.slice(0, 240).map(entry => entry.path);
  $("files").replaceChildren(...matches.slice(0, 240).map(({ path }) => {
    const file = button("", () => open(path), path, "file" + (current?.path === path ? " active" : ""));
    file.dataset.path = path;
    if (current?.path === path) file.setAttribute("aria-current", "true");
    file.append(el("strong", {}, fileLabel(path), el("span", { className: "draft-dot", hidden: !dirty(drafts.get(path)),
      title: "Unsaved changes", "aria-label": "Unsaved changes" })),
    el("small", {}, isSheet(path) ? path.split("/").at(-1) : path.slice(0, path.lastIndexOf("/")) || "Project root"));
    return file;
  }));
}

async function scan(refresh = false) {
  try {
    index = await api("/api/files" + (refresh ? "?refresh" : ""));
    if (refresh) clearLinks();
    list();
    if (!current) renderStart();
  } catch (error) { note(error.message, true); }
}

async function loadDocument(path, force = false) {
  if (!force && drafts.has(path)) return drafts.get(path);
  if (!force && loading.has(path)) return loading.get(path);
  const request = (async () => {
      const previous = drafts.get(path);
      if (force && previous && isSheet(path)) await saveRecovery(previous, true);
      const { recovery, ...loaded } = await api("/api/read", path);
      const doc = { ...loaded, path, original: loaded.text, undo: [], redo: [], trail: [],
        view: editableSprite(path) ? "sprite" : loaded.kind === "asset" ? "asset"
          : ["ron", "json"].includes(loaded.kind) ? "fields" : "source" };
      if (recovery) {
        doc.recoveryId = recovery.id;
        if (recovery.text !== loaded.text) {
          doc.recovered = recovery.version === loaded.version ? "Unsaved sprite changes recovered." :
            "Recovered draft conflicts with a newer source on disk. Export the draft before reloading.";
          doc.version = recovery.version;
          doc.original = recovery.original;
          doc.text = recovery.text;
          doc.undo.push(recovery.original);
        }
      }
      const saved = navigation.get(path);
      if (saved) {
        if (availableViews(doc).includes(saved.view)) doc.view = saved.view;
        doc.trail = saved.trail;
        if (Object.keys(saved.sprite ?? {}).length) doc.spriteSettings = saved.sprite;
      } else if (doc.view === "fields") {
        try { if (isRecordList(tree(doc))) doc.trail = [0]; }
        catch { /* The editor shows the parse error with a route to Source. */ }
      }
      drafts.set(path, doc);
      return doc;
  })();
  loading.set(path, request);
  try { return await request; }
  finally { if (loading.get(path) === request) loading.delete(path); }
}

function saveRecovery(doc, clear = false) {
  if (!isSheet(doc.path)) return Promise.resolve();
  const data = !clear && doc.text !== doc.original
    ? { version: doc.version, original: doc.original, text: doc.text } : { text: null };
  const request = (doc.recoveryPending ?? Promise.resolve()).catch(() => {}).then(async () => {
    if (data.text == null && !doc.recoveryId) return;
    const saved = await api("/api/sprite-draft", doc.path, { previous: doc.recoveryId, ...data });
    doc.recoveryId = saved.id;
  });
  doc.recoveryPending = request;
  return request;
}

function recordRecovery(doc) {
  saveRecovery(doc).catch(error => { if (doc === current) note("Sprite recovery could not be saved. " + error.message, true); });
}

async function open(path, trail, line, force = false, spriteEntry) {
  const request = ++selection;
  note("Opening " + path + "…");
  try {
    const doc = await loadDocument(path, force);
    if (request !== selection) return;
    current = doc;
    if (trail) { doc.trail = trail; doc.view = "fields"; }
    if (line) { doc.view = "source"; doc.line = line; }
    if (spriteEntry !== undefined && editableSprite(path)) {
      doc.view = "sprite";
      if (spriteEntry !== null) doc.spriteSettings = { ...doc.spriteSettings, entry: spriteEntry, key: "" };
    }
    const file = index.find(entry => entry.path === path);
    if (!$("search").value && file && !categories[category](file)) {
      category = categories.Sprites(file) ? "Sprites" : categories.Data(file) ? "Data" : "All files";
    }
    $("file-info-dialog").close();
    selectTab("workspace");
    $("editor").scrollTop = 0;
    list();
    navigation.remember(doc);
    note("");
  } catch (error) { if (request === selection) note(error.message, true); }
}

function tree(doc = current) {
  if (doc.parsedText !== doc.text || !doc.parsed) {
    doc.parsed = parse(doc.text, doc.kind);
    doc.parsedText = doc.text;
  }
  return doc.parsed;
}

function update(text, quiet = false, raw = false, doc = current) {
  if (text === doc.text) return true;
  try {
    if (!raw && ["ron", "json"].includes(doc.kind)) parse(text, doc.kind);
    const now = Date.now();
    if (!raw || !doc.rawAt || now - doc.rawAt > 500) {
      doc.undo.push(doc.text);
      const limit = Math.min(40, Math.max(2, Math.floor(8_000_000 / Math.max(text.length, 1))));
      if (doc.undo.length > limit) doc.undo.shift();
    }
    doc.rawAt = raw ? now : 0;
    doc.redo.length = 0;
    doc.text = text;
    if (isSheet(doc.path)) recordRecovery(doc);
    if (doc === current) {
      if (quiet) toolbar(); else render();
    }
    note("");
    return true;
  } catch (error) { note(error.message, true); return false; }
}

function step(from, to, doc = current) {
  if (doc?.spriteBitmap) stepPng(doc.spriteBitmap, from, to);
  else {
    if (!doc?.[from].length) return;
    doc[to].push(doc.text);
    doc.text = doc[from].pop();
    if (isSheet(doc.path)) recordRecovery(doc);
  }
  doc.rawAt = 0;
  if (doc === current) render();
  note(from === "undo" ? "Undone" : "Redone");
}

function setView(view) {
  if (!current) return;
  current.view = view;
  current.rawAt = 0;
  render();
  navigation.remember(current);
}

const ui = {
  api, open, update, tree, setView, note, dialog: prompt, load: loadDocument,
  changed() { toolbar(); note(""); },
  remember(doc) { if (current === doc) navigation.remember(doc); },
  assistField(trail, label, description) {
    const parts = address(tree(), trail);
    if (!canAssist(current.path) || !parts?.length) return;
    const task = ("Help me adjust " + label + " in " + current.path + ". "
      + "Explain its effect and change only this value when I specify the outcome I want. "
      + description).slice(0, 2050) + "\n\nDesired outcome: ";
    if (agent.focusField({ path: current.path, label, parts, valueOnly: true }, task)) {
      selectTab("agent");
      agent.focusTask();
    }
  },
  sprites() { catalog.showSprites(); selectTab("catalog"); },
  paths: () => index.map(entry => entry.path),
  get doc() { return current; },
  go(trail) {
    current.trail = trail;
    current.filter = "";
    render();
    $("editor").scrollTop = 0;
    navigation.remember(current);
  },
  replace(node, value, quiet) { return update(patch(current.text, node.start, node.end, value), quiet); },
};

function toolbar() {
  syncDraftCount();
  const changed = dirty(current);
  const history = current?.spriteBitmap ?? current;
  $("undo").disabled = !history?.undo.length;
  $("redo").disabled = !history?.redo.length;
  $("review").disabled = !changed;
  $("file-status").textContent = changed ? "Unsaved" : "";
  $("file-status").className = changed ? "dirty" : "";
  const selected = $("files").querySelector("[aria-current=true] .draft-dot");
  if (selected) selected.hidden = !changed;
}

function availableViews(doc) {
  if (doc.kind === "asset") return editableSprite(doc.path) || doc.spriteBitmap ? ["sprite", "asset"] : ["asset"];
  if (isSheet(doc.path)) return ["sprite", "source"];
  return ["ron", "json"].includes(doc.kind) ? ["fields", "graph", "source"] : ["source"];
}

function renderStart() {
  $("file-path").textContent = "";
  $("file-title").textContent = "Quick open";
  $("file-tools").hidden = $("view-bar").hidden = true;
  const paths = new Set(index.map(entry => entry.path));
  $("editor").replaceChildren(quickOpen(paths, navigation.entries, open, fileLabel, path => dirty(drafts.get(path))));
  $("editor").scrollTop = 0;
}

function render() {
  const doc = current;
  if (!doc) return;
  $("file-tools").hidden = $("view-bar").hidden = false;
  $("file-path").textContent = doc.path;
  $("file-title").textContent = fileLabel(doc.path);
  $("file-info").disabled = false;
  $("assist-file").hidden = !canAssist(doc.path);
  toolbar();
  const views = availableViews(doc);
  for (const control of $("views").children) {
    const view = control.dataset.view;
    control.hidden = !views.includes(view);
    control.classList.toggle("active", view === doc.view);
    control.setAttribute("aria-pressed", String(view === doc.view));
  }
  if (imageUrl) { URL.revokeObjectURL(imageUrl); imageUrl = undefined; }
  let content;
  if (doc.view === "sprite") content = spriteView(doc, ui);
  else if (doc.kind === "asset") content = assetView(doc);
  else if (doc.view === "source") {
    const newline = doc.original.includes("\r\n") ? "\r\n" : "\n";
    content = el("textarea", { className: "source", value: doc.text, spellcheck: false,
      "aria-label": "File source", oninput: event => update(event.target.value.replace(/\n/g, newline), true, true) });
    if (doc.line) {
      const offset = content.value.split("\n").slice(0, doc.line - 1).join("\n").length + (doc.line > 1 ? 1 : 0);
      requestAnimationFrame(() => {
        content.focus();
        content.setSelectionRange(offset, offset + (content.value.slice(offset).split("\n")[0]?.length ?? 0));
        content.scrollTop = Math.max(0, (doc.line - 5) * 20);
        doc.line = 0;
      });
    }
  } else {
    try {
      const root = tree();
      while (!at(root, doc.trail)) doc.trail.pop();
      content = doc.view === "graph" ? graphView(ui, root, doc.trail) : fieldsView(ui, root, doc.trail);
      if (doc.path === spellPath) content = el("div", { className: "stack" }, spellWiring(doc), content);
      if (doc.path === materialPath) {
        const id = textOf(field(root.children[doc.trail[0]], "id"));
        if (id) content = el("div", { className: "stack" },
          el("div", { className: "material-summary" },
            button("Material settings", () => editMaterial(id)),
            button("Make variant", () => editMaterial(id, true))), content);
      }
    } catch (error) {
      content = panel("Source needs attention", el("p", { className: "error" }, error.message),
        button("Open source", () => setView("source")));
    }
  }
  $("editor").replaceChildren(content);
  if ($("file-info-dialog").open) details(doc);
}

function spellWiring(doc) {
  const rows = readSpells(doc.text).filter(row => row.values.palette !== "Hidden");
  const spell = rows.find(row => row.index === doc.trail[0]) ?? rows[0];
  if (!spell) return panel("Spell Wiring", el("p", {}, "No spells in the catalog."));
  const picker = el("select", { "aria-label": "Spell", onchange: event => ui.go([Number(event.target.value)]) },
    rows.map(row => el("option", { value: row.index, selected: row.id === spell.id }, row.name + " · #" + row.id)));
  const fields = new Map();
  const groups = [...new Set(spellFields.map(field => field.group))];
  const nodes = groups.map(group => el("fieldset", { className: "stack" }, el("legend", {}, group),
    spellFields.filter(field => field.group === group).map(spec => {
      const { field, label, min, choice } = spec;
      const value = spell.values[field];
      const control = choice
        ? el("select", { "aria-label": label }, spellChoices(rows, field, spell.id)
          .map(option => el("option", { value: option, selected: option === value }, option)))
        : el("input", { type: "number", min, value, step: "any", required: true, "aria-label": label });
      fields.set(field, control);
      return el("label", {}, label + " ", control);
    })));
  const actionStack = () => {
    const blocks = [];
    for (const { field, choice } of spellFields) {
      const control = fields.get(field);
      if (!control.reportValidity()) return;
      const value = choice ? control.value : control.valueAsNumber;
      if (value !== spell.values[field]) blocks.push({ op: "set", field, value });
    }
    if (!blocks.length) {
      note("No spell changes.");
      return;
    }
    return { spell: spell.id, blocks };
  };
  const stage = button("Stage changes", () => {
    const request = actionStack();
    if (!request) return;
    try {
      update(previewSpellBlocks(doc.text, request.spell, request.blocks).text);
      note("Spell changes staged. Review the diff before saving.");
    } catch (error) { note(error.message, true); }
  }, "", "primary");
  const copy = button("Copy action blocks", async () => {
    const request = actionStack();
    if (!request) return;
    try {
      previewSpellBlocks(doc.text, request.spell, request.blocks);
      await copyText(JSON.stringify(request));
      note("Spell action stack copied.");
    } catch (error) { note(error.message, true); }
  });
  return panel("Spell Wiring",
    el("div", { className: "toolbar" }, picker, el("span", { className: "muted" }, spell.class), stage, copy),
    el("div", { className: "toolbar" }, nodes),
    spell.id === 0 ? el("details", {}, el("summary", {}, "Spark flight playtest"), sparkFixture(doc)) : null);
}

function sparkFixture(doc) {
  let baseline, draft;
  try {
    baseline = sparkFields(doc.original);
    draft = sparkFields(doc.text);
  } catch (error) {
    return panel("Forge · Spark", el("p", { className: "error" }, error.message));
  }
  const speed = el("input", { type: "number", min: 1, max: 120, step: "any", required: true,
    value: draft.speed.raw, "aria-label": "Spark base speed in metres per second" });
  const stage = () => {
    if (!speed.reportValidity()) return;
    try { update(editSparkSpeed(doc.text, speed.valueAsNumber)); }
    catch (error) { note(error.message, true); }
  };
  const launch = async (control, value) => {
    control.disabled = true;
    try {
      await api("/api/start-spark", undefined, { version: doc.version, speed: Number(value) });
      selectTab("playtest");
    } catch (error) { note(error.message, true); }
    finally { control.disabled = false; }
  };
  const baselineButton = button("Launch saved speed", () => launch(baselineButton, baseline.speed.raw));
  const draftButton = button("Launch draft speed", () => launch(draftButton, draft.speed.raw));
  const label = el("label", {}, "Base speed (m/s) ", speed);
  speed.onkeydown = event => { if (event.key === "Enter") { event.preventDefault(); stage(); } };
  return panel("Forge · Spark (#0)",
    el("p", {}, "Wand cast → Projectile → Spark flight"),
    el("p", { className: "muted" }, "Definition · " + draft.cost + " base mana · " + draft.damage
      + " base damage · " + draft.cooldown + " s base cooldown"),
    el("div", { className: "toolbar" }, label, button("Stage speed", stage, "", "primary"),
      button("Restore saved speed", () => update(editSparkSpeed(doc.text, Number(baseline.speed.raw))))),
    el("p", {}, "Saved " + baseline.speed.raw + " m/s → draft " + draft.speed.raw + " m/s"),
    el("div", { className: "toolbar" }, baselineButton, draftButton),
    el("p", { className: "muted" }, "Build the playtest executable first. Each launch uses a fresh solo save folder and previews only Spark’s speed. "
      + "Use the same seed and an unmodified Spark wand to compare. Use Cast Spark in Playtest. Its log shows mana paid and launch velocity."));
}

function diff(before, after) {
  const old = before.split("\n");
  const next = after.split("\n");
  let start = 0;
  while (start < Math.min(old.length, next.length) && old[start] === next[start]) start++;
  let suffix = 0;
  while (suffix < Math.min(old.length, next.length) - start && old.at(-1 - suffix) === next.at(-1 - suffix)) suffix++;
  const line = (prefix, kind) => text => el("div", { className: kind }, prefix + text);
  const removed = old.slice(start, old.length - suffix);
  const added = next.slice(start, next.length - suffix);
  return el("div", { className: "diff" },
    el("div", { className: "muted" }, "Changed region from line " + (start + 1) + " · " + removed.length + " old / " + added.length + " new"),
    old.slice(Math.max(0, start - 2), start).map(line("  ", "")),
    removed.slice(0, 130).map(line("− ", "removed")), added.slice(0, 130).map(line("+ ", "added")),
    removed.length > 130 || added.length > 130 ? el("div", { className: "muted" }, "Preview truncated") : null);
}

async function persist(doc, data, keepSpriteSelection = false) {
  const saved = await api("/api/save", doc.path, { version: doc.version, ...data });
  Object.assign(doc, saved, { original: saved.text });
  delete doc.recovered;
  if (isSheet(doc.path)) recordRecovery(doc);
  delete doc.spriteBitmap;
  if (doc.kind === "asset" && !keepSpriteSelection) delete doc.spriteSettings;
  clearLinks();
  if (current === doc) render();
  list();
  if (activeTab === "graph") activeGraph().refresh(true);
  if (activeTab === "catalog") catalog.refresh();
  note("Saved " + fileLabel(doc.path));
}

function refreshBundleFiles(saved) {
  for (const result of saved) {
    const doc = drafts.get(result.path);
    if (doc) Object.assign(doc, result, { original: result.text });
  }
  clearLinks();
  if (saved.some(doc => doc.path === current?.path)) render();
  if (activeTab === "graph") activeGraph().refresh(true);
  if (activeTab === "catalog") catalog.refresh();
  list();
}

function reviewAgentBundle(changes, runId, systemName) {
  const pending = changes.map(({ doc, text }) => ({ doc, text, path: doc.path, version: doc.version }));
  const currentDrafts = () => pending.every(({ doc, text, version }) =>
    doc.text === text && doc.version === version && doc.text !== doc.original);
  if (!currentDrafts()) return note("The staged drafts changed. Review the current drafts before saving.", true);
  prompt(systemName ? "Review " + systemName : "Review candidate bundle", pending.map(({ doc, text }) =>
    el("details", {}, el("summary", {}, doc.path), diff(doc.original, text))), "Save bundle", async () => {
    if (!currentDrafts()) throw new Error("The staged drafts changed. Review them again.");
    const { saved } = await api("/api/save-agent-bundle", undefined, {
      changes: pending.map(({ path, version, text }) => ({ path, version, text })),
      runId,
    });
    refreshBundleFiles(saved);
    note("Saved " + saved.length + (systemName ? " system files" : " candidate files"));
  });
}

async function showBundleHistory() {
  try {
    const records = await api("/api/agent-bundles");
    prompt("Candidate bundles", records.length ? records.map(record =>
      el("div", { className: "draft-row" },
        el("div", {}, el("strong", {}, new Date(record.at).toLocaleString()),
          el("small", {}, record.files.map(file => fileLabel(file.path)).join(" · "))),
        button("Inspect", () => inspectSavedBundle(record.id))))
      : [el("p", { className: "muted" }, "No saved candidate bundles")], "Done", () => true);
  } catch (error) { note(error.message, true); }
}

async function inspectSavedBundle(id) {
  try {
    const record = await api("/api/agent-bundles", id);
    const files = el("ul", {}, record.files.map(file => el("li", {}, file.path + " · " + file.status)));
    prompt("Inspect candidate bundle", [
      files,
      record.runId ? el("p", { className: "muted" }, "Run " + record.runId) : null,
    ].filter(Boolean), record.reversible ? "Revert applied files" : "Done", async () => {
      if (!record.reversible) return;
      if (record.files.some(file => dirty(drafts.get(file.path)))) {
        throw new Error("Save or undo open drafts for this bundle before reverting it.");
      }
      const restored = await api("/api/revert-agent-bundle", undefined, { id });
      refreshBundleFiles(restored);
      note("Restored " + restored.length + " candidate files");
    });
  } catch (error) { note(error.message, true); }
}

function review(doc = current) {
  if (!dirty(doc)) return;
  if (doc.spriteBitmap?.changes) {
    const bitmap = doc.spriteBitmap;
    const area = changedRegion(bitmap);
    const previews = [["Before", { width: bitmap.width, pixels: bitmap.original }], ["After", bitmap]]
      .map(([label, source]) => {
        const canvas = el("canvas", { "aria-label": label + " pixels" });
        canvasPixels(canvas, readTile(source, area), area.width, area.height);
        return el("figure", {}, canvas, el("figcaption", {}, label));
      });
    prompt("Review sprite changes", [el("p", {}, doc.path),
      el("p", {}, bitmap.changes + (bitmap.changes === 1 ? " pixel changed · " : " pixels changed · ") + bitmap.width + " × " + bitmap.height + " px"),
      el("div", { className: "pixel-diff", "aria-label": "Changed area" }, previews),
      el("p", { className: "muted" }, "Saves as 8-bit RGBA PNG. The original goes into Backups.")],
    "Save to project", async () => persist(doc, { asset: await encodePng(bitmap) }, true));
    return;
  }
  if (isSheet(doc.path)) {
    try {
      const before = readSheet(doc.original, doc.path), after = readSheet(doc.text, doc.path);
      if (!after.masks) {
        const previous = new Map(before.entries.map(entry => [entry.id, entry]));
        const signature = entry => entry && JSON.stringify([entry.rows.map(row => row.text),
          entry.palette.map(({ key, rgb }) => [key, rgb])]);
        const edits = after.entries.filter(entry => signature(entry) !== signature(previous.get(entry.id)));
        if (edits.length) {
          const previews = edits.slice(0, 8).map(entry => {
            const earlier = previous.get(entry.id);
            const pair = [["Before", earlier], ["After", entry]].filter(([, sprite]) => sprite)
              .map(([label, sprite]) => {
                const canvas = el("canvas", { "aria-label": label + " pixels" });
                const colors = new Map(sprite.palette.map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
                canvasPixels(canvas, cells(sprite), 16, 16, pixel => colors.get(pixel) ?? 0);
                return el("figure", {}, canvas, el("figcaption", {}, label));
              });
            return el("section", { className: "sprite-review" }, el("h3", {}, entry.name.replaceAll("_", " ")),
              el("div", { className: "tint-previews" }, pair));
          });
          prompt("Review sprite changes", [el("p", { className: "eyebrow" }, doc.path), ...previews,
            edits.length > previews.length ? el("p", { className: "muted" }, "Showing 8 of " + edits.length + " sprites") : null,
            el("details", {}, el("summary", {}, "View source changes"), diff(doc.original, doc.text))].filter(Boolean),
            "Save to project", () => persist(doc, { text: doc.text }));
          return;
        }
      }
    } catch (error) { return note(error.message, true); }
  }
  try { if (["ron", "json"].includes(doc.kind)) tree(doc); }
  catch (error) { return note(error.message, true); }
  prompt("Review changes", [el("p", { className: "eyebrow" }, doc.path), diff(doc.original, doc.text),
    el("p", { className: "muted save-hint" }, "The original goes into File info → Backups.")], "Save to project",
    () => persist(doc, { text: doc.text }));
}

function details(doc) {
  const relative = doc.path;
  const info = [panel("Path", el("p", { className: "mono" }, relative),
    el("p", {}, doc.size.toLocaleString() + " bytes"),
    button("Copy path", () => copyText(relative).then(() => note("Path copied")).catch(error => note(error.message, true))))];
  info.push(panel("On disk", button("Reload", () => {
    const reload = () => open(relative, undefined, undefined, true);
    if (dirty(doc)) prompt("Discard draft?", [el("p", {}, relative)], "Reload from disk", reload);
    else reload();
  }, "Read the latest version from disk")));
  if (relative.startsWith("assets/")) {
    const uses = el("div", { className: "stack" });
    const lookup = ++usageSelection;
    info.push(panel("Engine use", uses, button("Find loaders", async () => {
      uses.textContent = "Searching…";
      try {
        const results = await api("/api/usages", relative);
        if (lookup !== usageSelection) return;
        uses.replaceChildren(...results.map(entry => button(entry.path.split("/").at(-1) + ":" + entry.line,
          () => open(entry.path, undefined, entry.line), entry.path + "\n" + entry.text)));
        if (!results.length) uses.textContent = "No direct asset-path matches.";
      } catch (error) { uses.textContent = error.message; }
    })));
  }
  const backups = (doc.backups ?? []).slice(0, 12).map(id =>
    button(new Date(Number(id.split("-")[0])).toLocaleString(), async () => {
      try {
        const content = doc.kind === "asset" ? [el("p", {}, relative)]
          : [diff(doc.original, (await api("/api/history?id=" + encodeURIComponent(id), relative)).text)];
        if (dirty(doc)) content.unshift(el("p", {}, "Restoring replaces this unsaved draft."));
        prompt("Restore previous version", content, "Restore on disk", () => persist(doc, { restore: id }));
      } catch (error) { note(error.message, true); }
    }));
  info.push(panel("Backups", el("div", { className: "stack" }, backups.length ? backups : el("p", {}, "No backups yet"))));
  $("details").replaceChildren(...info);
}

function assetView(doc) {
  const image = el("img", { className: "preview-image", alt: doc.path, hidden: true });
  if (doc.image) api("/api/preview", doc.path, undefined, true).then(blob => {
    if (!image.isConnected) return;
    imageUrl = URL.createObjectURL(blob);
    image.src = imageUrl;
    image.hidden = false;
  }).catch(error => note(error.message, true));
  const file = el("input", { type: "file", "aria-label": "Select a replacement asset" });
  const replace = button("Review replacement", () => {
    const selected = file.files[0];
    if (!selected || selected.size > 24 * 1024 * 1024) return note("Select an asset up to 24 MiB.", true);
    prompt("Replace asset", [el("p", {}, doc.path + " ← " + selected.name),
      el("p", {}, selected.size.toLocaleString() + " bytes · format is checked by the game or asset tool")], "Replace on disk", async () => {
      const encoded = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result.split(",")[1]);
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(selected);
      });
      await persist(doc, { asset: encoded });
    });
  });
  return el("div", {}, image, panel("Asset file", el("p", {}, doc.version ? "Replace the file with an export from an asset editor."
    : "This asset exceeds the 24 MiB replacement limit."), doc.version ? file : null, doc.version ? replace : null));
}

const tabOrder = ["catalog", "graph", "workspace", "agent", "playtest", "backlog", "swarm"];
function selectTab(tab) {
  activeTab = tab;
  memory.set("tab", tab);
  for (const name of tabOrder) {
    $(name).hidden = name !== tab;
    $(name + "-tab").classList.toggle("active", name === tab);
  }
  if (tab === "graph") activeGraph().refresh(true);
  if (tab === "catalog") catalog.refresh();
  if (tab === "workspace" && current) render();
  if (tab === "backlog") backlog.refresh();
  if (tab === "swarm") mountSwarmFrame();
}
// The swarm supervisor runs as its own process; the panel proxies it at /swarm/ and passes its
// session token in the fragment so the frame can authorise control requests.
function mountSwarmFrame() {
  const host = $("swarm");
  if (host.firstChild) return;
  const frame = document.createElement("iframe");
  frame.title = "Swarm supervisor";
  frame.src = "/swarm/#" + location.hash.slice(1);
  host.append(frame);
}
const activeGraph = () => graphMode === "wiring" ? wiring : graphMode === "advancement" ? advancementWiring : graphMode === "portal" ? portalLayout : graph;
function showGraphMode(mode, target) {
  graphMode = mode;
  memory.set("graph-mode", mode);
  $("graph-wiring").hidden = mode !== "wiring";
  $("graph-map").hidden = mode !== "reference";
  $("graph-advancement").hidden = mode !== "advancement";
  $("graph-portals").hidden = mode !== "portal";
  $("wiring-mode").setAttribute("aria-pressed", String(mode === "wiring"));
  $("reference-mode").setAttribute("aria-pressed", String(mode === "reference"));
  $("advancement-mode").setAttribute("aria-pressed", String(mode === "advancement"));
  $("portal-mode").setAttribute("aria-pressed", String(mode === "portal"));
  if (activeTab !== "graph") selectTab("graph");
  else if (!target) activeGraph().refresh(true);
  if (target && mode !== "reference") activeGraph().open(target);
}
const isWiredRecipe = spec => spec?.path === recipePath && spec.parts?.length === 2;
function openGraph(spec) {
  if (isWiredRecipe(spec)) showGraphMode("wiring", spec);
  else if (spec?.path === advancementPath && spec.parts?.length === 1) showGraphMode("advancement", spec);
  else graph.open(spec);
}

async function openCandidateTarget(doc, offset = 0) {
  if (editableSprite(doc.path)) {
    let index;
    if (isSheet(doc.path)) {
      const rows = readSheet(doc.text, doc.path).entries;
      const hit = rows.findIndex(entry => offset >= (entry.palette[1]?.line ?? entry.rows[0].start)
        && offset <= entry.end);
      if (hit >= 0) index = hit;
    }
    return open(doc.path, undefined, undefined, false, index ?? null);
  }
  if (/^assets\/models\/factory\/[\w-]+\.vox$/.test(doc.path)) {
    selectTab("catalog");
    return catalog.setSection("Models", { path: doc.path });
  }
  if (["ron", "json"].includes(doc.kind)) {
    const root = tree(doc), trail = [];
    let node = root;
    while (node.children?.length) {
      const index = node.children.findIndex(child => offset >= (child.entry ?? child.start) && offset <= child.end);
      if (index < 0) break;
      trail.push(index);
      node = node.children[index];
      if (doc.path === recipePath && trail.length === 2) {
        return openGraph({ path: doc.path, parts: address(root, trail) });
      }
      if (doc.path === advancementPath && trail.length === 1) {
        return openGraph({ path: doc.path, parts: address(root, trail) });
      }
      if (doc.path === fabricationPath && trail.length === 1) return open(doc.path, trail);
    }
    if (trail.length) return open(doc.path, trail);
  }
  return open(doc.path, undefined, doc.text.slice(0, offset).split("\n").length);
}

$("refresh").onclick = () => scan(true);
$("quick-open").onclick = () => {
  selection++;
  current = undefined;
  if (imageUrl) { URL.revokeObjectURL(imageUrl); imageUrl = undefined; }
  renderStart();
  list();
  note("");
};
$("search").oninput = list;
$("search").onkeydown = event => {
  if (!visiblePaths.length) return;
  if (event.key === "Enter") {
    event.preventDefault();
    open(visiblePaths[0]).then(() => $("editor").focus({ preventScroll: true }));
  }
  if (event.key === "ArrowDown") { event.preventDefault(); $("files").firstElementChild?.focus(); }
};
$("files").onkeydown = event => {
  if (!event.target.matches("button.file")) return;
  const destination = {
    ArrowUp: event.target.previousElementSibling ?? $("search"),
    ArrowDown: event.target.nextElementSibling,
    Home: $("files").firstElementChild,
    End: $("files").lastElementChild,
  }[event.key];
  if (destination) { event.preventDefault(); destination.focus(); }
};
$("file-info").onclick = () => {
  if (!current) return;
  details(current);
  $("file-info-dialog").showModal();
};
$("close-file-info").onclick = () => $("file-info-dialog").close();
$("views").onclick = event => { if (event.target.dataset.view) setView(event.target.dataset.view); };
$("undo").onclick = () => step("undo", "redo");
$("redo").onclick = () => step("redo", "undo");
$("review").onclick = () => review();
$("catalog-tab").onclick = () => selectTab("catalog");
$("graph-tab").onclick = () => selectTab("graph");
$("wiring-mode").onclick = () => showGraphMode("wiring");
$("advancement-mode").onclick = () => showGraphMode("advancement");
$("portal-mode").onclick = () => showGraphMode("portal");
$("reference-mode").onclick = () => showGraphMode("reference");
$("workspace-tab").onclick = () => {
  if (!current && navigation.entries[0]) open(navigation.entries[0].path);
  else selectTab("workspace");
};
$("playtest-tab").onclick = () => selectTab("playtest");
$("backlog-tab").onclick = () => selectTab("backlog");
$("swarm-tab").onclick = () => selectTab("swarm");
$("agent-tab").onclick = () => selectTab("agent");
$("drafts-tab").onclick = showDrafts;
$("assist-file").onclick = () => {
  if (!current || !canAssist(current.path)) return;
  const text = $("editor").querySelector("textarea");
  const line = current.view === "source" && text ? current.text.slice(0, text.selectionStart).split("\n").length : undefined;
  assist(sourceFromDocument(current, line));
};
document.addEventListener("keydown", event => {
  if (document.querySelector("dialog[open]")) return;
  const typing = event.target.matches("input,textarea,select,[contenteditable]");
  if (event.altKey && !event.ctrlKey && !event.metaKey && /^[1-6]$/.test(event.key)) {
    event.preventDefault();
    $(tabOrder[Number(event.key) - 1] + "-tab").click();
    return;
  }
  if (event.key === "?" && !typing && !event.ctrlKey && !event.metaKey) { event.preventDefault(); showShortcuts(); return; }
  if (event.key === "Escape" && event.target === $("search") && $("search").value) { $("search").value = ""; list(); return; }
  if (!(event.ctrlKey || event.metaKey)) return;
  const key = event.key.toLowerCase();
  if (key === "p") { event.preventDefault(); selectTab("workspace"); $("search").focus(); $("search").select(); }
  if (key === "k") {
    event.preventDefault();
    graph.search();
  }
  if (key === "s" && ["graph", "catalog", "workspace"].includes(activeTab)) {
    event.preventDefault();
    document.activeElement?.blur();
    if (activeTab === "graph") activeGraph().review();
    else if (activeTab === "catalog") catalog.review();
    else review();
  }
  if (key === "z" && activeTab === "workspace" && !event.target.matches("input,textarea")) {
    event.preventDefault();
    if (event.shiftKey) step("redo", "undo"); else step("undo", "redo");
  }
});
window.addEventListener("beforeunload", event => {
  if ([...drafts.values()].some(dirty)) { event.preventDefault(); event.returnValue = ""; }
});

const playtest = mountPlaytest($("playtest"), api, note, () => !$("playtest").hidden);
let tray;
const trayButton = $("item-tray");
function keepItem(code, label = code) {
  tray = { code, label };
  trayButton.replaceChildren(document.createTextNode(label));
  trayButton.title = "Item Tray · " + code;
  recipeArtwork({ load: loadDocument, tree }).then(art => {
    const icon = art.icon(code);
    if (tray?.code === code && icon) trayButton.prepend(icon);
  }).catch(() => {});
  note("Item in tray · " + label);
}
async function itemContext(code, action) {
  try {
    const ctx = { load: loadDocument, tree };
    const art = await recipeArtwork(ctx);
    const sprite = art.sprite(code);
    if (action === "sprite") {
      if (!sprite) return note("This item uses generated art", true);
      return open(sprite.path, undefined, undefined, false, sprite.index);
    }
    const [crafting, fabricator] = await Promise.all([loadDocument(recipePath), loadDocument(fabricationPath)]);
    const routes = [...recipeRows(ctx, crafting), ...fabricationRecipes(ctx, fabricator)];
    const related = routes.filter(recipe => recipe.product === code).slice(0, 3);
    if (!related.length) related.push(...routes.filter(recipe => recipe.inputCodes.includes(code)).slice(0, 2));
    if (sprite) {
      related.push({ path: sprite.path, spriteId: sprite.entry.id, label: sprite.entry.name + " sprite" });
    }
    if (!related.length) return note("No editable recipe or sprite for this item", true);
    assist(related, "Refine or make a variation of " + code + " using its attached recipes and art.");
  } catch (error) { note(error.message, true); }
}
function itemActions(code) {
  const inCatalog = fn => { selectTab("catalog"); fn(); };
  return [
    { label: "Inspect Item", run: () => inCatalog(() => catalog.inspectItem(code)) },
    { label: "Recipe Tree", run: () => inCatalog(() => catalog.planItem(code)) },
    { label: "Edit Sprite", run: () => itemContext(code, "sprite") },
    { label: "Ask AI", run: () => itemContext(code, "agent") },
    { label: "Keep in Item Tray", run: () => keepItem(code) },
    { label: "Copy Item ID", run: () => copyText(code)
      .then(() => note("Item ID copied")).catch(error => note(error.message, true)) },
  ];
}
trayButton.onclick = () => {
  if (!tray) return note("Right-click an item or drop a recipe icon here to fill the tray.");
  showContextMenu(trayButton, itemActions(tray.code).filter(action => action.label !== "Keep in Item Tray"));
};
dropTarget(trayButton, ["item"], (_, code) => keepItem(code));
dragSource(trayButton, "item", () => tray?.code);

function checkMaterialDrafts(paths) {
  const blocked = paths.find(path => drafts.has(path) && dirty(drafts.get(path)));
  if (blocked) throw new Error("Review or discard the open draft first · " + blocked);
}
const materialRefresh = new Map();
async function refreshMaterials(paths) {
  const ticket = Symbol();
  paths.forEach(path => materialRefresh.set(path, ticket));
  try {
    const docs = await Promise.all(paths.map(async path => ({ ...await api("/api/read", path), path })));
    const clean = docs.filter(({ path }) => materialRefresh.get(path) === ticket &&
      (!drafts.has(path) || !dirty(drafts.get(path))));
    for (const { path } of clean) {
      const doc = drafts.get(path);
      if (doc) { doc.undo.length = 0; doc.redo.length = 0; }
    }
    refreshBundleFiles(clean);
  } catch (error) { note("Material saved. Reload the view to refresh it · " + error.message, true); }
}
function editMaterial(id, duplicate) {
  openMaterialEditor(sharedEditor, id, duplicate).catch(error => note(error.message, true));
}

const sharedEditor = {
  api, open, tree, parse, note, dirty, review, reviewAgentBundle, openCandidateTarget, scan, dialog: prompt,
  itemActions, keepItem, trayItem: () => tray?.code,
  editMaterial, refreshMaterials, checkMaterialDrafts,
  paths: ui.paths,
  spriteFiles: () => index.filter(entry => entry.sprite),
  editableSprite,
  load: loadDocument,
  edit: (doc, text, raw = false) => update(text, true, raw, doc),
  undo: doc => step("undo", "redo", doc),
  redo: doc => step("redo", "undo", doc),
};
const agent = mountAgent($("agent"), sharedEditor);
const backlog = mountBacklog($("backlog"), api, openContextTask);
function assist(spec, task) {
  const scope = (Array.isArray(spec) ? spec : [spec]).filter(source => canAssist(source?.path));
  if (!scope.length) return note("Select a source record or file", true);
  if (task?.trim()) {
    openContextTask({ task, sources: scope, origin: scope[0].label });
    return;
  }
  scope.forEach(agent.attach);
  selectTab("agent");
}
async function playRecipe(recipe) {
  try {
    const doc = await loadDocument(recipePath);
    const art = [...drafts.values()].find(candidate => editableSprite(candidate.path) && dirty(candidate));
    if (art) throw new Error("Review and save the sprite draft before trying it in Axiom.");
    const food = [...drafts.values()].find(candidate => foodPaths.includes(candidate.path) && dirty(candidate));
    if (food) throw new Error("Review and save food changes before trying them in Axiom. Open Drafts to review or undo them; only the recipe is previewed without saving.");
    await api("/api/start-recipe", undefined, { text: doc.text, rebuild: true, recipe: recipe?.label });
    selectTab("playtest");
  } catch (error) { note(error.message, true); }
}
function syncDraftCount() {
  const count = [...drafts.values()].filter(dirty).length;
  $("drafts-tab").textContent = count ? "Drafts · " + count : "Drafts";
  $("drafts-tab").classList.toggle("has-drafts", count > 0);
}
function showDrafts() {
  const changed = [...drafts.values()].filter(dirty);
  const leave = action => { $("dialog").close(); action(); };
  const rows = changed.length ? changed.map(doc => el("div", { className: "draft-row" },
    el("div", {}, el("strong", {}, fileLabel(doc.path)), el("small", {}, doc.path)),
    button("Open", () => leave(() => open(doc.path))),
    button("Review", () => leave(() => review(doc))),
    doc.path === recipePath ? button("Playtest", () => leave(playRecipe)) : null))
    : [el("p", { className: "muted" }, "No Unsaved Drafts")];
  prompt("Drafts", [...rows, button("Candidate bundle history", () => leave(showBundleHistory))], "Done", () => true);
}
const foodReviews = new Map();
async function foodWorkflow(command) {
  const mutation = ["apply", "withdraw"].includes(command.op);
  const revision = mutation ? foodReviews.get(command.request?.approval) : undefined;
  if (mutation && !revision) throw new Error("Preview this food workflow again before saving");
  const before = new Map();
  for (const row of revision ?? []) {
    const doc = drafts.get(row.path);
    if (doc && (dirty(doc) || doc.version !== row.version)) {
      throw new Error("Save or reload the open source before applying: " + row.path);
    }
    if (doc) before.set(row.path, { doc, text: doc.text, version: doc.version });
  }
  const result = await api("/api/food-workflow", undefined, command);
  if (["preview", "preview-withdraw"].includes(command.op)) {
    if (foodReviews.size >= 16) foodReviews.delete(foodReviews.keys().next().value);
    foodReviews.set(result.approval, result.revisions);
  }
  result.draftConflicts = [];
  for (const saved of result.saved ?? []) {
    const doc = drafts.get(saved.path), pinned = before.get(saved.path);
    if (!doc) continue;
    if (dirty(doc) || (pinned && (doc !== pinned.doc || doc.text !== pinned.text || doc.version !== pinned.version))) {
      result.draftConflicts.push(saved.path);
      continue;
    }
    Object.assign(doc, saved, { original: saved.text, undo: [], redo: [] });
  }
  if (mutation) {
    foodReviews.delete(command.request.approval);
    clearLinks();
    list();
  }
  return result;
}

const graph = mountGraph($("graph-map"), {
  ...sharedEditor,
  assist,
  wireRecipe: spec => showGraphMode("wiring", spec),
  wireAdvancement: spec => showGraphMode("advancement", spec),
  variantFood: async spec => {
    try {
      const doc = await loadDocument(spec.path);
      const kind = textOf(field(resolve(tree(doc), spec.parts)?.node, "kind"));
      if (!kind) return note("Select a food record", true);
      await createFood({ ...sharedEditor, foodWorkflow }, code => {
        const root = tree(doc);
        const id = code.slice(5, -1);
        const index = root.children.findIndex(row => textOf(field(row, "kind")) === id);
        if (index >= 0) graph.open({ path: doc.path, parts: address(root, [index]),
          label: textOf(field(root.children[index], "name")) });
      }, kind);
    } catch (error) { note(error.message, true); }
  },
  activate: () => showGraphMode("reference"),
  playtest: () => selectTab("playtest"),
});
const create = (path, preset) => createIdea(sharedEditor, openGraph, path, preset, true);
const wiring = mountRecipeWiring($("graph-wiring"), {
  ...sharedEditor, assist, playRecipe,
  createRecipe: () => create(recipePath),
  openSource: async spec => {
    try {
      const doc = await loadDocument(spec.path);
      const found = resolve(tree(doc), spec.parts);
      if (found) await open(doc.path, undefined, doc.text.slice(0, found.node.start).split("\n").length);
    } catch (error) { note(error.message, true); }
  },
});
const advancementWiring = mountAdvancementWiring($("graph-advancement"), {
  ...sharedEditor, assist, createAdvancement: () => create(advancementPath),
});
const portalLayout = mountPortalLayout($("graph-portals"), { api,
  recipes: () => showGraphMode("wiring"), advancements: () => showGraphMode("advancement"), catalog: () => selectTab("catalog") });
const catalog = mountCatalog($("catalog"), { ...sharedEditor, graph: openGraph, create, foodWorkflow, assist, playRecipe,
  wireRecipe: spec => showGraphMode("wiring", spec),
  openSprite: (path, entry = null) => open(path, undefined, undefined, false, entry) });
await Promise.all([navigation.load(), catalog.restore()]);
await scan();
if (activeTab === "graph") activeGraph().refresh();
if (activeTab === "catalog") catalog.refresh();

const shortcuts = [
  ["Everywhere", [["Ctrl+Shift+P / F1", "Command palette: tabs, files, sections, actions"],
    ["Alt+1 … Alt+6", "Catalog, Graph, Workspace, AI, Playtest, Backlog"], ["?", "This sheet"], ["Ctrl+S", "Review and save the active editor"]]],
  ["Catalog", [["/", "Focus catalog search"], ["Esc", "Clear search and category"], ["↓ / Enter", "Move to or open the first result"],
    ["Right-click / Shift+F10", "Entry actions"]]],
  ["Workspace", [["Ctrl+P", "Search all files"], ["Esc", "Clear file search"], ["Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"]]],
  ["Graph", [["Ctrl+K", "Search the reference map"]]],
  ["Playtest", [["/", "Filter sections"], ["Click a heading", "Collapse or expand the section"]]],
];
let shortcutSheet;
function showShortcuts() {
  if (!shortcutSheet) {
    const rows = shortcuts.flatMap(([group, keys]) => [el("tr", {}, el("th", { colSpan: 2 }, group)),
      ...keys.map(([key, text]) => el("tr", {}, el("td", {}, el("kbd", {}, key)), el("td", {}, text)))]);
    shortcutSheet = el("dialog", { className: "shortcut-sheet", "aria-label": "Keyboard shortcuts" },
      el("div", { className: "section-title" }, el("h2", {}, "Keyboard shortcuts"), button("Close", () => shortcutSheet.close())),
      el("table", {}, el("tbody", {}, rows)));
    shortcutSheet.addEventListener("click", event => { if (event.target === shortcutSheet) shortcutSheet.close(); });
    document.body.append(shortcutSheet);
  }
  shortcutSheet.showModal();
}

const tabLabels = { catalog: "Catalog", graph: "Graph", workspace: "Workspace", agent: "AI", playtest: "Playtest", backlog: "Backlog" };
tabOrder.forEach((tab, at) => {
  const control = $(tab + "-tab");
  control.title = (control.title ? control.title + " · " : "") + "Alt+" + (at + 1);
  control.append(el("kbd", { className: "tab-key", "aria-hidden": "true" }, String(at + 1)));
});
// The shared shell replaces the per-tool links that foundry-link/wand-lab-link append.
for (const id of ["foundry-link", "wand-lab-link"]) $(id)?.remove();
const shellHost = el("div", { className: "forge-inline" });
$("drafts-tab").before(shellHost);
function openContextTask(task) {
  window.focus();
  if (task.createMission === true) {
    selectTab("agent");
    agent.createMission(task);
    return;
  }
  agent.handoff(task, origin => {
    selectTab("agent");
    agent.focusTask();
    note(origin ? "Task loaded from " + origin : "Task loaded. Gather context, then run.");
  });
}
function currentAiContext() {
  if (activeTab === "catalog") return catalog.assistantContext();
  if (activeTab === "workspace" && current) return {
    label: "Workspace · " + (fileLabel(current.path) ?? current.path),
    sources: [sourceFromDocument(current)],
  };
  if (activeTab === "graph") {
    const selection = activeGraph().assistantContext?.();
    if (selection) return selection;
    const scopes = {
      wiring: ["Recipe wiring", recipePath, fabricationPath],
      advancement: ["Advancements", advancementPath],
      portal: ["Dimensions & portals", "crates/app/src/world/portals.rs", "crates/app/src/world/portals/realm.rs"],
      reference: ["Reference Map"],
    };
    const [label, ...paths] = scopes[graphMode];
    return { label, sources: paths.map(path => ({ path })) };
  }
  if (activeTab === "playtest" && playtest) return playtest.assistantContext();
  return { label: tabLabels[activeTab] };
}
mountShell({ host: shellHost, current: "/", context: currentAiContext, openTask: openContextTask,
  showWorkbench: () => selectTab("agent") });
window.addEventListener('axiom:open-mission', event => { selectTab('agent'); agent.openMission(event.detail); });
receivePanel({
  assist: openContextTask,
  "show-ai": ({ missionId }) => {
    selectTab("agent");
    if (typeof missionId === "string") agent.openMission(missionId);
  },
  open({ path, line }) {
    if (typeof path !== "string" || !index.some(entry => entry.path === path)) return note("File not in the project index: " + path, true);
    window.focus();
    open(path, undefined, Number.isInteger(line) ? line : undefined);
  },
});
const graphModes = { wiring: "Recipe wiring", advancement: "Advancements", portal: "Dimensions & portals", reference: "Reference map" };
installPalette(message => note(message, true));
registerCommands(() => [
  ...tabOrder.map((tab, at) => ({ id: "tab:" + tab, label: "Show " + tabLabels[tab], group: "Tab", keys: "Alt+" + (at + 1),
    run: () => $(tab + "-tab").click() })),
  ...Object.entries(graphModes).map(([mode, label]) => ({ id: "graph:" + mode, label: "Graph · " + label, group: "Tab",
    run: () => showGraphMode(mode) })),
  ...catalogGroups.flatMap(group => group.sections.map(name => ({ id: "catalog:" + name, label: "Catalog · " + name,
    group: "Catalog", hint: group.name, run: () => { selectTab("catalog"); catalog.setSection(name); } }))),
  ...(playtest?.sections.entries() ?? []).map(entry => ({ id: "playtest:" + entry.title, label: "Playtest · " + entry.title,
    group: "Playtest", hint: entry.category, run: () => { selectTab("playtest"); entry.reveal(); } })),
  { id: "action:rescan", label: "Rescan project files", group: "Action", run: () => scan(true) },
  { id: "action:drafts", label: "Open draft queue", group: "Action", run: showDrafts },
  { id: "action:graph-search", label: "Search reference map", group: "Action", keys: "Ctrl+K", run: () => graph.search() },
  { id: "action:find-file", label: "Find file", group: "Action", keys: "Ctrl+P",
    run: () => { selectTab("workspace"); $("search").focus(); $("search").select(); } },
  { id: "action:review", label: "Review & save current file", group: "Action", keys: "Ctrl+S",
    run: () => { if (current) { selectTab("workspace"); review(); } else note("Open a file first", true); } },
  { id: "action:shortcuts", label: "Keyboard shortcuts", group: "Action", keys: "?", run: showShortcuts },
  { id: "tool:wand-lab", label: "Open Wand Lab", group: "Tool", hint: "new tab",
    run: () => window.open("/wand-lab" + location.hash, "_blank", "noopener") },
  { id: "tool:parity", label: "Open Mod parity board", group: "Tool", hint: "Thermal, Mekanism, IE, Create…",
    run: () => window.open("/parity" + location.hash, "_blank", "noopener") },
  { id: "tool:foundry", label: "Open Foundry", group: "Tool", hint: "new tab",
    run: () => window.open("/foundry" + location.hash, "_blank", "noopener") },
  ...navigation.entries.map(({ path }) => ({ id: "recent:" + path, label: fileLabel(path) ?? path, group: "Recent", hint: path,
    run: () => open(path) })),
  ...index.map(({ path }) => ({ id: "file:" + path, label: fileLabel(path) ?? path, group: "File", hint: path,
    weight: path.startsWith("assets/") ? 60 : 0, run: () => open(path) })),
]);

// Restore the last tab and graph view from the previous visit.
const savedTab = memory.get("tab", "catalog");
const savedMode = memory.get("graph-mode", "wiring");
if (savedTab === "graph" && graphModes[savedMode]) showGraphMode(savedMode);
else if (savedTab === "workspace") $("workspace-tab").click();
else if (tabOrder.includes(savedTab) && savedTab !== activeTab) selectTab(savedTab);

