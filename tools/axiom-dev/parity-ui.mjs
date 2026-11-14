// Mod parity board: coverage map, filterable ledger, AI handoff, live engine tests.
import { installPalette, registerCommands } from "./command-palette.mjs";
import { mountShell, openInPanel, sendToAi } from "./forge-shell.mjs";

const token = location.hash.slice(1);
const $ = id => document.getElementById(id);
const el = (tag, { dataset, ...props } = {}, ...kids) => {
  const node = Object.assign(document.createElement(tag), props);
  if (dataset) Object.assign(node.dataset, dataset);
  node.append(...kids.flat().filter(kid => kid != null && kid !== false));
  return node;
};
const byId = (a, b) => a.id.localeCompare(b.id, undefined, { numeric: true });
const INDUSTRIAL = ["thermal", "mekanism", "ie", "create"];
const PREFS = "axiom-dev.parity.prefs";
const defaults = { families: [...INDUSTRIAL], statuses: [], milestones: [], ready: "", query: "", group: "section",
  sort: "id", view: "list", map: true, history: [] };
let prefs = { ...defaults };
try { prefs = { ...defaults, ...JSON.parse(localStorage.getItem(PREFS) ?? "{}") }; } catch { /* private window */ }
const save = () => { try { localStorage.setItem(PREFS, JSON.stringify(prefs)); } catch { /* private window */ } };

let ledger = { rows: [], statuses: [], families: {} }, visible = [], selected = null, brief = null, live = false;

async function api(route, payload) {
  const response = await fetch(route, { method: payload ? "POST" : "GET", headers: { "X-Axiom-Session": token,
    ...(payload ? { "Content-Type": "application/json" } : {}) }, body: payload ? JSON.stringify(payload) : undefined });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || response.statusText);
  return data;
}
let toastTimer;
function toast(text, error = false) {
  const node = $("toast");
  node.textContent = text;
  node.className = "toast show" + (error ? " error" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { node.className = "toast" + (error ? " error" : ""); }, error ? 5200 : 2600);
}
const toggle = (list, value) => list.includes(value) ? list.filter(item => item !== value) : [...list, value];
const familyLabel = key => ledger.families[key] ?? key;

function matches(row) {
  const words = prefs.query.toLowerCase().split(/\s+/).filter(Boolean);
  return (!prefs.families.length || row.families.some(f => prefs.families.includes(f)))
    && (!prefs.statuses.length || prefs.statuses.includes(row.status))
    && (!prefs.milestones.length || prefs.milestones.includes(row.milestone))
    && (!prefs.ready || (prefs.ready === "ready") === row.ready)
    && words.every(word => [row.id, row.label, row.mapping, row.family, row.notes, row.note ?? ""].join(" ").toLowerCase().includes(word));
}

function sorted(rows) {
  const rank = Object.fromEntries(ledger.statuses.map((s, i) => [s, i]));
  const sorters = {
    id: byId,
    milestone: (a, b) => a.milestone.localeCompare(b.milestone) || byId(a, b),
    risk: (a, b) => a.risks.length - b.risks.length || byId(a, b),
    status: (a, b) => rank[a.status] - rank[b.status] || byId(a, b),
  };
  return rows.sort(sorters[prefs.sort] ?? byId);
}

// ---------- coverage map ----------
function renderLegend() {
  $("legend").replaceChildren(el("span", { title: "Open, and every owner system already has code" }, el("i", { className: "swatch ready-open" }), "open, ready"),
    ...ledger.statuses.map(status => el("span", {}, el("i", { className: "swatch", dataset: { status } }), status)));
}
function renderLanes() {
  $("lanes").hidden = !prefs.map;
  $("coverage-toggle").textContent = prefs.map ? "Hide map" : "Show map";
  $("coverage-toggle").setAttribute("aria-expanded", String(prefs.map));
  if (!prefs.map) return;
  const keys = Object.keys(ledger.families).filter(key => ledger.rows.some(row => row.families.includes(key)));
  keys.sort((a, b) => (INDUSTRIAL.includes(b) - INDUSTRIAL.includes(a)) || familyLabel(a).localeCompare(familyLabel(b)));
  const shown = new Set(visible.map(row => row.id));
  $("lanes").replaceChildren(...keys.map(key => {
    const rows = ledger.rows.filter(row => row.families.includes(key)).sort(byId);
    const done = rows.filter(row => row.status === "verified").length, moving = rows.filter(row => ["building", "playable"].includes(row.status)).length;
    const active = prefs.families.length === 1 && prefs.families[0] === key;
    return el("div", { className: "lane" },
      el("button", { type: "button", className: "lane-name", ariaPressed: String(active), title: active ? "Show every family" : "Show only " + familyLabel(key),
        onclick: () => { prefs.families = active ? [] : [key]; render(); } }, familyLabel(key)),
      el("div", { className: "lane-cells", role: "group", ariaLabel: familyLabel(key) }, rows.map(row => el("button", {
        type: "button", className: "cell" + (row.ready ? " ready" : "") + (shown.has(row.id) ? "" : " dim"),
        dataset: { status: row.status, id: row.id }, ariaCurrent: String(row.id === selected),
        title: `${row.id} · ${row.label}\n${row.status}${row.ready ? " · owners ready" : ""} · ${row.milestone}`,
        onclick: () => select(row.id, true) }))),
      el("span", { className: "lane-score" }, el("b", {}, String(done)), "/" + rows.length + " verified" + (moving ? ` · ${moving} moving` : "")));
  }));
}

// ---------- rail ----------
function facet(host, entries, active, onToggle) {
  host.replaceChildren(...entries.map(([value, label, count, status]) => el("button", {
    type: "button", ariaPressed: String(active.includes(value)), onclick: () => { onToggle(value); render(); } },
  status ? el("i", { className: "swatch", dataset: { status } }) : null, label, el("small", {}, String(count)))));
}
function renderRail() {
  const count = (key, value) => ledger.rows.filter(row => [].concat(row[key]).includes(value)).length;
  const families = Object.keys(ledger.families).filter(key => count("families", key));
  families.sort((a, b) => (INDUSTRIAL.includes(b) - INDUSTRIAL.includes(a)) || familyLabel(a).localeCompare(familyLabel(b)));
  facet($("families"), families.map(key => [key, familyLabel(key), count("families", key)]), prefs.families,
    value => { prefs.families = toggle(prefs.families, value); });
  facet($("statuses"), ledger.statuses.map(s => [s, s, count("status", s), s]), prefs.statuses, value => { prefs.statuses = toggle(prefs.statuses, value); });
  const milestones = [...new Set(ledger.rows.map(row => row.milestone))].sort();
  facet($("milestones"), milestones.map(m => [m, m, count("milestone", m)]), prefs.milestones, value => { prefs.milestones = toggle(prefs.milestones, value); });
  for (const button of $("readiness").children) button.setAttribute("aria-checked", String(button.dataset.ready === prefs.ready));
  for (const button of $("view").children) button.setAttribute("aria-checked", String(button.dataset.view === prefs.view));
  $("group").value = prefs.group; $("sort").value = prefs.sort;
  $("group").parentElement.hidden = prefs.view === "board";
  if (document.activeElement !== $("query")) $("query").value = prefs.query;
}

// ---------- results ----------
function rowButton(row) {
  return el("button", { type: "button", className: "row", dataset: { id: row.id }, ariaCurrent: String(row.id === selected),
    onclick: () => select(row.id), title: row.mapping },
  el("i", { className: "swatch", dataset: { status: row.status }, title: row.status }), el("code", {}, row.id),
  el("span", { className: "label" }, row.label),
  el("span", { className: "ready-dot" + (row.ready ? "" : " waiting"), title: row.ready ? "Owner systems have code to build on" : "Waits on " + row.missing.join(", ") },
    row.ready ? "ready" : "waits"),
  el("span", { className: "milestone" }, row.milestone));
}
function card(row) {
  const node = el("button", { type: "button", className: "card", draggable: true, dataset: { id: row.id }, ariaCurrent: String(row.id === selected),
    onclick: () => select(row.id), title: "Drag to another column to change status" },
  el("code", {}, row.id + " · " + row.milestone + (row.ready ? " · ready" : "")), el("span", {}, row.label));
  node.addEventListener("dragstart", event => { event.dataTransfer.setData("text/plain", row.id); event.dataTransfer.effectAllowed = "move"; });
  return node;
}
function renderResults() {
  const total = ledger.rows.length;
  $("count").textContent = `${visible.length} of ${total} rows · ${visible.filter(row => row.ready).length} ready · ${visible.filter(row => row.status === "verified").length} verified`;
  if (!visible.length) {
    $("rows").replaceChildren(el("p", { className: "detail-empty" }, "No rows match these filters. ", el("button", { type: "button", className: "ghost", onclick: () => $("reset").click() }, "Clear filters")));
    return;
  }
  if (prefs.view === "board") {
    $("rows").replaceChildren(el("div", { className: "board" }, ledger.statuses.map(status => {
      const rows = visible.filter(row => row.status === status);
      const column = el("section", { className: "column", dataset: { status }, ariaLabel: status },
        el("h3", {}, status, el("small", {}, String(rows.length))), rows.slice(0, 80).map(card),
        rows.length > 80 ? el("p", { className: "column-more" }, `${rows.length - 80} more · narrow the filters`) : null);
      column.addEventListener("dragover", event => { event.preventDefault(); column.classList.add("drop"); });
      column.addEventListener("dragleave", () => column.classList.remove("drop"));
      column.addEventListener("drop", event => { event.preventDefault(); column.classList.remove("drop"); record(event.dataTransfer.getData("text/plain"), { status }); });
      return column;
    })));
    return;
  }
  const groups = new Map();
  for (const row of visible) {
    const key = prefs.group === "family" ? (row.families.map(familyLabel).join(" + ") || row.family)
      : prefs.group === "none" ? "" : row[prefs.group];
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(row);
  }
  $("rows").replaceChildren(...[...groups].flatMap(([name, rows]) => [
    name ? el("h3", { className: "group-title" }, name, el("small", {}, String(rows.length))) : null, ...rows.map(rowButton)]));
}

function render() {
  save();
  visible = sorted(ledger.rows.filter(matches));
  renderRail();
  renderLanes();
  renderResults();
  const status = ledger.statuses.map(s => [s, ledger.rows.filter(row => row.status === s).length]).filter(([, n]) => n);
  $("totals").replaceChildren(el("strong", {}, String(ledger.rows.length)), " feature rows · ",
    el("strong", {}, String(ledger.rows.filter(row => row.ready).length)), " ready to build · ",
    ...status.flatMap(([s, n], i) => [i ? ", " : "", el("strong", {}, String(n)), " " + s]));
}

// ---------- detail ----------
// Gather Context injects the full brief and rules for any feature ID in the task,
// so the task names the row and the goal once instead of repeating the brief.
const taskText = b => `Implement parity row ${b.id}: ${b.label}. Mechanism: ${b.mapping}. `
  + `Verify with engine-cli test, then parity-cli set ${b.id}.`;

async function select(id, focusDetail = false) {
  selected = id;
  for (const node of document.querySelectorAll("[data-id]")) node.setAttribute("aria-current", String(node.dataset.id === id));
  document.querySelector(`.rows [data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "nearest" });
  const row = ledger.rows.find(entry => entry.id === id);
  if (!row) return;
  const host = $("detail");
  host.replaceChildren(el("div", { className: "detail-title" }, el("code", {}, row.id + " · " + row.section), el("h2", {}, row.label)),
    el("p", { className: "hint" }, "Loading brief…"));
  try {
    brief = await api("/api/parity/describe?id=" + encodeURIComponent(id));
  } catch (error) { host.lastChild.textContent = error.message; return; }
  if (selected !== id) return;
  renderDetail(row);
  if (focusDetail) host.focus({ preventScroll: true });
}

function renderDetail(row) {
  const b = brief;
  const statusSelect = el("select", { id: "status-select", ariaLabel: "Status" }, ledger.statuses.map(s => el("option", { value: s, selected: s === row.status }, s)));
  const evidence = el("input", { placeholder: "Evidence: path#line or commit:sha, comma separated", ariaLabel: "Evidence" });
  const note = el("input", { placeholder: "Note for the next session", value: row.note ?? "", ariaLabel: "Note" });
  const hintFiles = b.hints.files;
  $("detail").replaceChildren(
    el("div", { className: "detail-title" }, el("code", {}, b.id + " · " + b.section), el("h2", {}, b.label)),
    el("div", { className: "facts" },
      el("span", { className: "fact" }, el("i", { className: "swatch", dataset: { status: row.status } }), row.status),
      el("span", { className: "fact", title: b.ready ? "Every owner system has code to build on" : "Waits on " + row.missing.join(", ") }, b.ready ? "Owners ready" : "Waits on " + row.missing.join(", ")),
      el("span", { className: "fact" }, b.milestone), el("span", { className: "fact", title: "Cost class from the matrix" }, "Cost " + b.cost),
      b.risks.length ? el("span", { className: "fact", title: "Risk codes, see matrix legend" }, b.risks.join(" ")) : null),
    el("div", { className: "actions" },
      el("button", { type: "button", className: "primary", title: "Load this brief and the likeliest sources into the AI workbench (A)",
        onclick: () => handoff() }, "Send to AI workbench"),
      el("button", { type: "button", onclick: () => copy(JSON.stringify(b, null, 2), "Brief copied as JSON") }, "Copy brief"),
      el("button", { type: "button", onclick: () => copy(`node tools/axiom-dev/parity-cli.mjs describe ${b.id}`, "CLI command copied") }, "Copy CLI command")),
    el("section", { className: "block" }, el("h3", {}, "Mechanism"), el("p", {}, b.mapping), b.notes ? el("p", { className: "hint" }, b.notes) : null),
    el("section", { className: "block" }, el("h3", {}, "Owner systems"), el("ul", { className: "owners" }, b.owners.map(owner => el("li", {},
      el("code", {}, owner.id), el("span", {}, owner.name, el("span", { className: "owner-state " + owner.substrate },
        owner.substrate === "legacy" ? "has code" : owner.substrate === "design-only" ? "design only" : "unknown")),
      el("span", { className: "reuse" }, (owner.milestone ? owner.milestone + " · " : "") + (owner.reuse || "No inventory entry")))))),
    el("section", { className: "block" }, el("h3", {}, "Likely related code"),
      hintFiles.length ? el("ul", { className: "files" }, hintFiles.map(file => el("li", {}, el("button", { type: "button",
        title: "Open in the panel's Workspace", onclick: () => openInPanel(file) }, file))))
        : el("p", { className: "hint" }, "No matches in content tables or factory code."),
      el("p", { className: "hint" }, (b.hints.words.length ? "Matched " + b.hints.words.join(", ") : "")
        + (b.hints.absent?.length ? (b.hints.words.length ? ". " : "") + "Not found anywhere: " + b.hints.absent.join(", ") : "")),
      b.sourceNote ? el("p", {}, el("button", { type: "button", className: "ghost", onclick: () => openInPanel(b.sourceNote) }, "Open source note")) : null),
    liveConsole(),
    el("form", { className: "record", onsubmit: event => {
      event.preventDefault();
      record(b.id, { status: statusSelect.value, note: note.value, evidence: evidence.value.split(",").map(item => item.trim()).filter(Boolean) });
    } }, el("h3", {}, "Record progress"), el("div", { className: "row-inline" }, statusSelect, el("button", { type: "submit", className: "primary" }, "Save")),
    evidence, note, row.evidence.length ? el("ul", { className: "files" }, row.evidence.map(item => el("li", {},
      el("button", { type: "button", onclick: () => /^(commit|https?):/.test(item) ? copy(item, "Copied") : openInPanel(item.split("#")[0]) }, item)))) : null,
    row.updated ? el("p", { className: "hint" }, "Updated " + row.updated) : null),
    el("details", {}, el("summary", {}, "Rules the agent receives"), el("p", { className: "hint" }, b.rules)));
}

async function handoff() {
  if (!brief) return;
  const opened = await sendToAi(taskText(brief), brief.hints.files.filter(file => /\.(rs|ron)$/.test(file)));
  toast(opened ? "Sent to the AI workbench in the open panel" : "Opening the panel with this task");
}

async function record(id, change) {
  try {
    const result = await api("/api/parity", { id, revision: ledger.revision, ...change });
    toast(`${id} saved as ${result.after.status}`);
    ledger = await api("/api/parity");
    render();
    if (selected === id) select(id);
  } catch (error) {
    toast(error.message, true);
    if (/changed/.test(error.message)) { ledger = await api("/api/parity"); render(); }
  }
}

async function copy(text, done) {
  try { await navigator.clipboard.writeText(text); toast(done); } catch { toast("Clipboard is unavailable in this window", true); }
}

// ---------- live engine console ----------
const QUICK = [
  ["Look-at machine", { op: "machine" }],
  ["Machines nearby", { op: "machines", radius_m: 32 }],
  ["Machine kinds", { op: "machine_kinds" }],
  ["Enemies nearby", { op: "enemies", radius_m: 64 }],
  ["Spawn enemy", { op: "spawn_enemy", kind: "Boar", affixes: [], elements: [] }],
  ["Clear enemies", { op: "clear_enemies" }],
  ["Player + world", { op: "inspect", sections: ["player", "world"] }],
  ["Frame times", { op: "perf" }],
];
let consoleLog = [];
function liveConsole() {
  const log = el("pre", { className: "console-log", ariaLive: "polite" });
  const input = el("textarea", { spellcheck: false, ariaLabel: "Native engine request JSON",
    value: prefs.history[0] ?? '{"op":"place_machine","position":[0,70,0],"kind":"Crusher"}' });
  let cursor = -1;
  const paint = () => {
    log.replaceChildren(...consoleLog.slice(-24).flatMap(entry => [el("span", { className: "req" }, "› " + JSON.stringify(entry.request) + "\n"),
      el("span", { className: entry.ok ? "" : "err" }, entry.text + "\n")]));
    log.scrollTop = log.scrollHeight;
  };
  async function send(request) {
    if (!live) { toast("Start a world in Panel → Playtest → Engine workbench first", true); return; }
    try {
      const response = await api("/api/engine", request);
      const ok = response.ok !== false;
      consoleLog.push({ request, ok, text: JSON.stringify(ok ? response.data ?? response : response.error, null, 1).slice(0, 4000) });
    } catch (error) { consoleLog.push({ request, ok: false, text: error.message }); }
    paint();
  }
  const submit = () => {
    let request;
    try { request = JSON.parse(input.value); } catch (error) { toast("Request is not valid JSON: " + error.message, true); return; }
    prefs.history = [input.value, ...prefs.history.filter(item => item !== input.value)].slice(0, 20);
    save();
    cursor = -1;
    send(request);
  };
  input.addEventListener("keydown", event => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) { event.preventDefault(); submit(); }
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown") && prefs.history.length) {
      event.preventDefault();
      cursor = Math.max(-1, Math.min(prefs.history.length - 1, cursor + (event.key === "ArrowUp" ? 1 : -1)));
      if (cursor >= 0) input.value = prefs.history[cursor];
    }
  });
  const state = el("span", { className: "hint", id: "console-state" }, live ? "World ready" : "No world running");
  const node = el("section", { className: "console", id: "live-console", dataset: { live: String(live) } },
    el("div", { className: "console-head" }, el("h3", {}, "Live engine"), state,
      el("a", { href: "/" + location.hash, target: "axiom-panel", className: "hint", style: "margin-left:auto" }, "Open Playtest")),
    el("div", { className: "console-quick" }, QUICK.map(([label, request]) => el("button", { type: "button", onclick: () => send(request) }, label))),
    input, el("div", { className: "row-inline", style: "display:flex;gap:8px;align-items:center" },
      el("button", { type: "button", className: "send primary", onclick: submit }, "Send request"),
      el("span", { className: "hint" }, "Ctrl+Enter sends · Alt+↑↓ history · see describe engine")), log);
  paint();
  return node;
}
async function pollEngine() {
  if (document.hidden) return;
  try { live = Boolean((await api("/api/game")).ready); } catch { live = false; }
  const node = $("live-console");
  if (node) { node.dataset.live = String(live); $("console-state").textContent = live ? "World ready" : "No world running"; }
}

// ---------- wiring ----------
async function load() {
  try {
    ledger = await api("/api/parity");
    renderLegend();
    render();
    if (!selected) $("detail").replaceChildren(el("div", { className: "detail-empty" },
      el("p", {}, "Pick a row from the list, the board or the coverage map."),
      el("p", { className: "hint" }, "Keys: ", el("kbd", {}, "/"), " search, ", el("kbd", {}, "J"), " ", el("kbd", {}, "K"), " move, ",
        el("kbd", {}, "A"), " send to AI, ", el("kbd", {}, "B"), " board, ", el("kbd", {}, "M"), " map")));
  } catch (error) { $("totals").textContent = "Ledger unavailable: " + error.message; }
}

$("query").addEventListener("input", () => { prefs.query = $("query").value; render(); });
$("query").addEventListener("keydown", event => {
  if (event.key === "Escape") { prefs.query = ""; $("query").value = ""; render(); }
  if (event.key === "Enter" && visible[0]) select(visible[0].id, true);
});
$("group").onchange = () => { prefs.group = $("group").value; render(); };
$("sort").onchange = () => { prefs.sort = $("sort").value; render(); };
$("readiness").onclick = event => { if (event.target.dataset.ready !== undefined) { prefs.ready = event.target.dataset.ready; render(); } };
$("view").onclick = event => { if (event.target.dataset.view) { prefs.view = event.target.dataset.view; render(); } };
$("coverage-toggle").onclick = () => { prefs.map = !prefs.map; render(); };
$("reset").onclick = () => { Object.assign(prefs, { families: [], statuses: [], milestones: [], ready: "", query: "" }); render(); };
document.addEventListener("keydown", event => {
  if (document.querySelector("dialog[open]") || event.ctrlKey || event.metaKey || event.altKey) return;
  if (event.target.matches("input,textarea,select,[contenteditable]")) return;
  const key = event.key.toLowerCase();
  if (key === "/") { event.preventDefault(); $("query").focus(); $("query").select(); }
  else if ((key === "j" || key === "arrowdown" || key === "k" || key === "arrowup") && visible.length) {
    event.preventDefault();
    const at = visible.findIndex(row => row.id === selected);
    select(visible[Math.max(0, Math.min(visible.length - 1, at + (key === "j" || key === "arrowdown" ? 1 : -1)))].id);
  }
  else if (key === "a" && brief) handoff();
  else if (key === "b") { prefs.view = prefs.view === "board" ? "list" : "board"; render(); }
  else if (key === "m") { prefs.map = !prefs.map; render(); }
  else if (key === "s" && $("status-select")) $("status-select").focus();
});

mountShell({ current: "/parity" });
installPalette();
registerCommands(() => [
  { id: "parity:industrial", label: "Show Thermal, Mekanism, IE and Create", group: "Filter", run: () => { prefs.families = [...INDUSTRIAL]; render(); } },
  ...Object.entries(ledger.families).map(([key, label]) => ({ id: "parity:family:" + key, label: "Show only " + label, group: "Filter",
    run: () => { prefs.families = [key]; render(); } })),
  { id: "parity:ready", label: "Show rows ready to build", group: "Filter", run: () => { prefs.ready = "ready"; render(); } },
  { id: "parity:clear", label: "Clear filters", group: "Filter", run: () => $("reset").click() },
  { id: "parity:board", label: "Toggle board view", group: "View", keys: "B", run: () => { prefs.view = prefs.view === "board" ? "list" : "board"; render(); } },
  { id: "parity:map", label: "Toggle coverage map", group: "View", keys: "M", run: () => { prefs.map = !prefs.map; render(); } },
  { id: "parity:send", label: "Send selected row to AI workbench", group: "Action", keys: "A", run: handoff },
  ...ledger.rows.map(row => ({ id: "parity:row:" + row.id, label: row.id + " " + row.label, group: "Row", hint: row.status, run: () => select(row.id, true) })),
]);
load();
pollEngine();
setInterval(pollEngine, 4000);
