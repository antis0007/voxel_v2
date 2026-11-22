// Mod parity board: coverage map, filterable ledger, AI handoff, live engine tests.
import { installPalette, registerCommands } from "./command-palette.mjs";
import { mountShell, openAiWorkbench, openInPanel, sendToAi } from "./forge-shell.mjs";

const token = location.hash.slice(1);
const $ = id => document.getElementById(id);
const children = kids => kids.flat().filter(kid => kid != null && kid !== false);
const el = (tag, { dataset, ...props } = {}, ...kids) => {
  const node = Object.assign(document.createElement(tag), props);
  if (dataset) Object.assign(node.dataset, dataset);
  node.append(...children(kids));
  return node;
};
const replace = (host, ...kids) => host.replaceChildren(...children(kids));
const byId = (a, b) => a.id.localeCompare(b.id, undefined, { numeric: true });
const INDUSTRIAL = ["thermal", "mekanism", "ie", "create"];
const PREFS = "axiom-dev.parity.prefs";
const defaults = { families: [], statuses: [], milestones: [], ready: "", query: "", group: "section",
  sort: "priority", view: "list", map: false, evidenceGap: false, history: [] };
let prefs = { ...defaults };
try { prefs = { ...defaults, ...JSON.parse(localStorage.getItem(PREFS) ?? "{}") }; } catch { /* private window */ }
const save = () => { try { localStorage.setItem(PREFS, JSON.stringify(prefs)); } catch { /* private window */ } };

let ledger = { rows: [], statuses: [], families: {} }, visible = [], selected = null, brief = null, live = false;
let loadError = null;
const boardLimits = new Map();
const livePhases = new Set(["Queued", "Running", "Planning"]);

async function api(route, payload) {
  const response = await fetch(route, { method: payload ? "POST" : "GET", headers: { "X-Axiom-Session": token,
    ...(payload ? { "Content-Type": "application/json" } : {}) }, body: payload ? JSON.stringify(payload) : undefined });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw Object.assign(new Error(data.error || response.statusText), { status: response.status });
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
const evidenceGap = row => row.status === "verified" && !row.verification?.currentNative;

function matches(row, except) {
  const words = prefs.query.toLowerCase().split(/\s+/).filter(Boolean);
  return (except === "families" || !prefs.families.length || row.families.some(f => prefs.families.includes(f)))
    && (!prefs.evidenceGap || evidenceGap(row))
    && (except === "statuses" || !prefs.statuses.length || prefs.statuses.includes(row.status))
    && (except === "milestones" || !prefs.milestones.length || prefs.milestones.includes(row.milestone))
    && (!prefs.ready || (prefs.ready === "ready") === row.ready)
    && words.every(word => [row.id, row.label, row.mapping, row.family, row.notes, row.note ?? "", ...(row.packJars ?? [])].join(" ").toLowerCase().includes(word));
}

function sorted(rows) {
  const rank = Object.fromEntries(ledger.statuses.map((s, i) => [s, i]));
  const sorters = {
    priority: (a, b) => (b.priority?.score ?? 0) - (a.priority?.score ?? 0) || byId(a, b),
    id: byId,
    milestone: (a, b) => a.milestone.localeCompare(b.milestone) || byId(a, b),
    risk: (a, b) => a.risks.length - b.risks.length || byId(a, b),
    status: (a, b) => rank[a.status] - rank[b.status] || byId(a, b),
  };
  return rows.sort(sorters[prefs.sort] ?? byId);
}

// ---------- coverage map ----------
function renderLegend() {
  $("legend").replaceChildren(el("span", { title: "Open row with code listed for every owner" }, el("i", { className: "swatch ready-open" }), "open, owner code listed"),
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
    const share = status => rows.filter(row => row.status === status).length / rows.length * 100;
    return el("div", { className: "lane", dataset: { active: String(active) } },
      el("div", { className: "lane-head" },
        el("button", { type: "button", className: "lane-name", ariaPressed: String(active), title: active ? "Show every family" : "Show only " + familyLabel(key),
          onclick: () => { prefs.families = active ? [] : [key]; render(); } }, familyLabel(key)),
        el("span", { className: "lane-score", title: `${done} of ${rows.length} recorded verified` + (moving ? ` · ${moving} building or playable` : "") },
          el("b", {}, String(done)), "/" + rows.length + (moving ? ` · ${moving} moving` : ""))),
      el("div", { className: "lane-bar", role: "img", ariaLabel: "Status mix" },
        ledger.statuses.map(status => share(status) ? el("i", { dataset: { status }, style: `width:${share(status)}%` }) : null)),
      el("div", { className: "lane-cells", role: "group", ariaLabel: familyLabel(key) }, rows.map(row => el("button", {
        type: "button", className: "cell" + (row.ready ? " ready" : "") + (shown.has(row.id) ? "" : " dim"),
        dataset: { status: row.status, id: row.id, running: String(livePhases.has(row.mission?.phase) || Boolean(row.work?.running)) },
        ariaCurrent: String(row.id === selected),
        title: [`${row.id} · ${row.label}`, `${row.status}${row.ready ? " · owner code listed" : ""} · ${row.milestone}`, ...runLabels(row)].join(String.fromCharCode(10)),
        onclick: () => select(row.id, true) }))));
  }));
}

// ---------- rail ----------
function facet(host, entries, active, onToggle) {
  host.replaceChildren(...entries.map(([value, label, count, status]) => el("button", {
    type: "button", ariaPressed: String(active.includes(value)), onclick: () => { onToggle(value); render(); } },
  status ? el("i", { className: "swatch", dataset: { status } }) : null, label, el("small", {}, String(count)))));
}
function renderRail() {
  const counts = (filter, key) => {
    const values = new Map();
    for (const row of ledger.rows) {
      if (!matches(row, filter)) continue;
      for (const value of [].concat(row[key])) values.set(value, (values.get(value) ?? 0) + 1);
    }
    return value => values.get(value) ?? 0;
  };
  const familyCount = counts("families", "families");
  const statusCount = counts("statuses", "status");
  const milestoneCount = counts("milestones", "milestone");
  const families = Object.keys(ledger.families);
  families.sort((a, b) => (INDUSTRIAL.includes(b) - INDUSTRIAL.includes(a)) || familyLabel(a).localeCompare(familyLabel(b)));
  facet($("families"), families.map(key => [key, familyLabel(key), familyCount(key)]), prefs.families,
    value => { prefs.families = toggle(prefs.families, value); });
  facet($("statuses"), ledger.statuses.map(s => [s, s, statusCount(s), s]), prefs.statuses, value => { prefs.statuses = toggle(prefs.statuses, value); });
  const milestones = [...new Set(ledger.rows.map(row => row.milestone))].sort();
  facet($("milestones"), milestones.map(m => [m, m, milestoneCount(m)]), prefs.milestones, value => { prefs.milestones = toggle(prefs.milestones, value); });
  for (const button of $("readiness").children) button.setAttribute("aria-checked", String(button.dataset.ready === prefs.ready));
  for (const button of $("view").children) button.setAttribute("aria-checked", String(button.dataset.view === prefs.view));
  $("group").value = prefs.group; $("sort").value = prefs.sort;
  $("group").parentElement.hidden = prefs.view === "board";
  $("all-families").setAttribute("aria-pressed", String(prefs.families.length === 0));
  $("industrial-families").setAttribute("aria-pressed", String(prefs.families.length === INDUSTRIAL.length
    && INDUSTRIAL.every(family => prefs.families.includes(family))));
  if (document.activeElement !== $("query")) $("query").value = prefs.query;
}

function renderActiveFilters() {
  const host = $("active-filters");
  const chips = [];
  const chip = (label, remove) => chips.push(el("button", { type: "button", ariaLabel: "Remove filter " + label,
    onclick: () => { remove(); render(); $("query").focus(); } }, label, " ×"));
  for (const [key, label] of [["families", familyLabel], ["statuses", v => v], ["milestones", v => v]]) {
    for (const value of prefs[key]) chip(label(value), () => { prefs[key] = toggle(prefs[key], value); });
  }
  if (prefs.ready) chip(prefs.ready === "ready" ? "Code listed" : "Owner gaps", () => { prefs.ready = ""; });
  if (prefs.evidenceGap) chip("Verification evidence gaps", () => { prefs.evidenceGap = false; });
  if (prefs.query) chip(prefs.query, () => { prefs.query = ""; });
  host.hidden = chips.length === 0;
  host.replaceChildren(...chips);
}

// ---------- results ----------
function verificationRecovery(row) {
  if (!evidenceGap(row)) return "";
  const work = row.work;
  if (work?.phase === "Stopped") return "Automatic verification stopped";
  if (!row.verification?.registered) return "No registered native check";
  if (work?.running) return "";
  if (work?.automatic && work.retryAt) {
    return work.retryable ? "Automatic retry scheduled" : "Waiting for updated source";
  }
  return "Automatic verification pending";
}

function runLabels(row) {
  const recovery = verificationRecovery(row);
  return [evidenceGap(row) && row.verification?.evidence, row.mission && "AI · " + row.mission.phase,
    recovery,
    row.work && !recovery && (row.work.running || !row.verification?.currentNative) &&
      (row.work.automatic ? "Verification · " : "Factory · ") + row.work.phase].filter(Boolean);
}

function rowButton(row) {
  return el("button", { type: "button", className: "row", dataset: { id: row.id }, ariaCurrent: String(row.id === selected),
    ariaLabel: [row.id, row.label, row.status, row.milestone, row.ready ? "Owner code listed" : "Owner gaps", ...runLabels(row)].join(", "),
    onclick: () => select(row.id), title: row.mapping },
  el("i", { className: "swatch", dataset: { status: row.status }, title: row.status }), el("code", {}, row.id),
  el("span", { className: "label" }, row.label),
  el("span", { className: "ready-dot", title: row.ready ? "Code listed for every owner" : "No code listed for " + row.missing.join(", ") },
    row.ready ? "code listed" : "owner gaps"),
  el("span", { className: "milestone" }, row.milestone),
  runLabels(row).length ? el("span", { className: "run-labels row-activity" },
    runLabels(row).map(status => el("small", { className: "row-run" }, status))) : null);
}
function card(row) {
  const node = el("button", { type: "button", className: "card", draggable: true, dataset: { id: row.id }, ariaCurrent: String(row.id === selected),
    onclick: () => select(row.id), title: "Drag to another column to change status" },
  el("code", {}, row.id + " · " + row.milestone + (row.ready ? " · owner code listed" : "")), el("span", {}, row.label),
  runLabels(row).length ? el("div", { className: "run-labels" }, runLabels(row).map(label => el("small", { className: "row-run" }, label))) : null);
  node.addEventListener("dragstart", event => { event.dataTransfer.setData("text/plain", row.id); event.dataTransfer.effectAllowed = "move"; });
  return node;
}
function renderResults() {
  if (loadError) {
    $("count").textContent = "Ledger unavailable";
    const sessionExpired = loadError.status === 403;
    const message = sessionExpired
      ? "Open Axiom Dev Panel from the launcher to get a fresh session, then choose Mod Parity."
      : "Try loading the ledger again. If the problem continues, restart the Dev Panel.";
    $("rows").replaceChildren(el("section", { className: "detail-empty", role: "alert" },
      el("h3", {}, sessionExpired ? "This tab’s Dev Panel session has expired" : "Couldn’t load the parity ledger"),
      el("p", {}, message),
      sessionExpired ? null : el("button", { type: "button", className: "ghost", onclick: load }, "Retry loading")));
    return;
  }
  const total = ledger.rows.length;
  $("count").textContent = `${visible.length} of ${total} rows · ${visible.filter(row => row.ready).length} with owner code listed`;
  if (!visible.length) {
    $("rows").replaceChildren(el("p", { className: "detail-empty" }, "No rows match these filters. ", el("button", { type: "button", className: "ghost", onclick: () => $("reset").click() }, "Clear filters")));
    return;
  }
  if (prefs.view === "board") {
    $("rows").replaceChildren(el("div", { className: "board" }, ledger.statuses.map(status => {
      const rows = visible.filter(row => row.status === status);
      const limit = boardLimits.get(status) ?? 80;
      const column = el("section", { className: "column", dataset: { status }, ariaLabel: status },
        el("h3", {}, status, el("small", {}, String(rows.length))), rows.slice(0, limit).map(card),
        rows.length > limit ? el("button", { type: "button", className: "ghost column-more",
          onclick: () => { boardLimits.set(status, limit + 80); renderResults();
            document.querySelector(`.column[data-status="${CSS.escape(status)}"] .card:nth-of-type(${limit + 1})`)?.focus(); } },
        `Show ${Math.min(80, rows.length - limit)} more`) : null);
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
  visible = sorted(ledger.rows.filter(row => matches(row)));
  renderRail();
  renderActiveFilters();
  renderLanes();
  renderResults();
  const status = ledger.statuses.map(s => [s, ledger.rows.filter(row => row.status === s).length]).filter(([, n]) => n);
  $("totals").replaceChildren(el("strong", {}, String(ledger.rows.length)), " feature rows · ",
    el("strong", {}, String(ledger.rows.filter(row => row.ready).length)), " with owner code listed · recorded status ",
    ...status.flatMap(([s, n], i) => [i ? ", " : "", el("strong", {}, String(n)), " " + s]));
  const gaps = ledger.rows.filter(evidenceGap).length;
  const audit = $("verification-gaps");
  audit.hidden = !gaps && !prefs.evidenceGap;
  audit.textContent = gaps + " verification claims without a current native receipt";
  audit.setAttribute("aria-pressed", String(prefs.evidenceGap));
}

// ---------- detail ----------
let selectionRequest = 0, paintFactory, paintMission;
async function select(id, focusDetail = false) {
  const request = ++selectionRequest;
  brief = null;
  paintFactory = undefined;
  paintMission = undefined;
  selected = id;
  for (const node of document.querySelectorAll("[data-id]")) node.setAttribute("aria-current", String(node.dataset.id === id));
  document.querySelector(`.rows [data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "nearest" });
  const row = ledger.rows.find(entry => entry.id === id);
  if (!row) return;
  const host = $("detail");
  host.replaceChildren(el("div", { className: "detail-title" }, el("code", {}, row.id + " · " + row.section), el("h2", {}, row.label)),
    el("p", { className: "hint" }, "Loading brief…"));
  try {
    const loaded = await api("/api/parity/describe?id=" + encodeURIComponent(id));
    if (request !== selectionRequest) return;
    brief = loaded;
  } catch (error) {
    if (request === selectionRequest) host.lastChild.textContent = error.message;
    return;
  }
  renderDetail(row);
  void refreshWork();
  void refreshEvidence();
  if (focusDetail) host.focus({ preventScroll: true });
}

function renderDetail(row) {
  const b = brief;
  const statusSelect = el("select", { id: "status-select", ariaLabel: "Status" }, ledger.statuses.map(s => el("option", { value: s, selected: s === row.status }, s)));
  const evidence = el("input", { placeholder: "Evidence: path#line or commit:sha, comma separated", ariaLabel: "Evidence" });
  const note = el("input", { id: "status-note", placeholder: "Note for the next session", value: row.note ?? "", ariaLabel: "Note" });
  const hintFiles = b.hints.files;
  $("detail").replaceChildren(
    el("div", { className: "detail-title" }, el("code", {}, b.id + " · " + b.section), el("h2", {}, b.label)),
    el("div", { className: "facts" },
      el("span", { id: "selected-status", className: "fact", dataset: { status: row.status } }, el("i", { className: "swatch", dataset: { status: row.status } }), row.status),
      el("span", { className: "fact" }, b.ready ? "Owner code listed" : "Owner code unlisted for " + b.contract.missingOwners.join(", ")),
      el("span", { className: "fact" }, b.milestone), el("span", { className: "fact", title: "Cost class from the matrix" }, "Cost " + b.cost),
      b.risks.length ? el("span", { className: "fact", title: "Risk codes, see matrix legend" }, b.risks.join(" ")) : null),
    el("section", { className: "parity-studio", ariaLabel: "Parity prototyping" },
      el("header", { className: "studio-head" }, el("h3", {}, "Build & verify"),
        el("p", {}, b.contract.outcome)),
      workCard(b)),
    el("div", { className: "actions detail-actions" },
      el("button", { type: "button", title: "Load this brief and likely sources into the AI workbench (A)",
        onclick: () => handoff() }, "Open brief in AI workbench"),
      el("button", { type: "button", onclick: () => copy(JSON.stringify(b, null, 2), "Brief copied as JSON") }, "Copy brief"),
      el("button", { type: "button", onclick: () => copy(`node tools/axiom-dev/parity-cli.mjs describe ${b.id}`, "CLI command copied") }, "Copy CLI command")),
    el("p", { id: "selected-verification", className: "hint", hidden: !row.verification },
      [row.verification?.evidence, verificationRecovery(row)].filter(Boolean).join(" · ")),
    el("section", { id: "live-parity-evidence", className: "block" }, el("h3", {}, "Live evidence"),
      el("p", { className: "hint" }, "Checking current evidence…")),
    el("section", { className: "block" }, el("h3", {}, "Mechanism"), el("p", {}, b.mapping), b.notes ? el("p", { className: "hint" }, b.notes) : null),
    el("section", { className: "block" }, el("h3", {}, "Reference"), el("p", {}, b.sources), el("p", { className: "hint" }, b.contract.reference),
      b.packSources?.length ? el("details", {}, el("summary", {}, "October 2020 pack evidence"),
        el("ul", {}, b.packSources.map(({ jar, note }) => el("li", {}, el("strong", {}, jar), el("p", {}, note))))) : null,
      b.sourceNote ? el("button", { type: "button", className: "ghost", onclick: () => openInPanel(b.sourceNote) }, "Open source note") : null,
      b.contract.missingSourceNote ? el("p", { className: "hint" }, "Missing source note · " + b.contract.missingSourceNote) : null),
    el("details", {}, el("summary", {}, "Acceptance checks"), el("ul", {}, b.contract.checks.filter(check => typeof check === "string" && check.trim() && !/^[-–—]+$/.test(check.trim())).map(check => el("li", {}, check)))),
    el("section", { className: "block" }, el("h3", {}, "Owner systems"), el("ul", { className: "owners" }, b.owners.map(owner => el("li", {},
      el("code", {}, owner.id), el("span", {}, owner.name, el("span", { className: "owner-state " + owner.substrate },
        owner.substrate === "legacy" ? "has code" : owner.substrate === "design-only" ? "design only" : "unknown")),
      el("span", { className: "reuse" }, (owner.milestone ? owner.milestone + " · " : "") + (owner.reuse || "No inventory entry")))))),
    el("section", { className: "block" }, el("h3", {}, "Likely related code"),
      hintFiles.length ? el("ul", { className: "files" }, hintFiles.map(file => el("li", {}, el("button", { type: "button",
        title: "Open in the panel's Workspace", onclick: () => openInPanel(file) }, file))))
        : el("p", { className: "hint" }, "No matches in content tables or factory code."),
      el("p", { className: "hint" }, (b.hints.words.length ? "Matched " + b.hints.words.join(", ") : "")
        + (b.hints.absent?.length ? (b.hints.words.length ? ". " : "") + "No matches in searched files for " + b.hints.absent.join(", ") : ""))),
    liveConsole(),
    el("form", { className: "record", onsubmit: event => {
      event.preventDefault();
      record(b.id, { status: statusSelect.value, note: note.value, evidence: evidence.value.split(",").map(item => item.trim()).filter(Boolean) });
    } }, el("h3", {}, "Record progress"), el("div", { className: "row-inline" }, statusSelect, el("button", { type: "submit", className: "primary" }, "Save")),
    evidence, note,
    el("p", { id: "parity-updated", className: "hint", hidden: !row.updated }, row.updated ? "Updated " + row.updated : "")),
    el("details", {}, el("summary", {}, "Rules the agent receives"), el("p", { className: "hint" }, b.rules)));
  // Keep work visible; retain reference material and expert tools on demand.
  const host = $("detail");
  const fold = (label, nodes) => {
    const group = el("details", { className: "feature-disclosure" }, el("summary", {}, label));
    for (const node of nodes) if (node) group.append(node);
    return group;
  };
  const sections = [...host.children];
  const titled = title => sections.find(node => node.querySelector("h3")?.textContent === title);
  const references = fold("Design & acceptance", [titled("Mechanism"), titled("Reference"),
    sections.find(node => node.tagName === "DETAILS" && node.querySelector("summary")?.textContent === "Acceptance checks")]);
  const source = fold("Source suggestions · not verified owners", [titled("Owner systems"), titled("Likely related code")]);
  const tools = fold("Developer tools", [host.querySelector(".detail-actions"), host.querySelector(".console"),
    sections.find(node => node.tagName === "DETAILS" && node.querySelector("summary")?.textContent === "Rules the agent receives")]);
  const recording = fold("Update recorded status", [host.querySelector(".record")]);
  const studio = host.querySelector(".parity-studio");
  const missionStatus = el("p", {});
  const configure = el("button", { type: "button", className: "primary", onclick: () => handoff(true) }, "Set up AI mission");
  const previous = el("button", { type: "button", className: "ghost" }, "Open mission");
  const next = el("section", { className: "feature-next", ariaLabel: "AI mission" },
    el("h3", {}, "AI mission"), missionStatus, el("div", { className: "actions" }, configure, previous));
  paintMission = mission => {
    const current = ledger.rows.find(item => item.id === b.id);
    next.hidden = !mission && current?.status === "verified" && Boolean(b.workChecks.length);
    configure.hidden = livePhases.has(mission?.phase);
    previous.hidden = !mission;
    previous.onclick = mission ? () => openAiWorkbench(mission.id) : null;
    const text = mission ? mission.phase + (mission.total ? ` · ${mission.done}/${mission.total} steps` : "")
      : "Review the outcome, source scope, AI provider and build settings before queueing.";
    if (missionStatus.textContent !== text) missionStatus.textContent = text;
  };
  paintMission(row.mission);
  host.insertBefore(next, studio);
  if (!b.workChecks.length) {
    studio.hidden = true;
  } else {
    const checks = fold("Build & verification", [studio]);
    checks.open = row.status === "verified" && evidenceGap(row);
    host.append(checks);
  }
  host.append(references, source, tools, recording);
  const acceptance = references.querySelector("ul");
  if (acceptance && !acceptance.children.length) acceptance.replaceWith(el("p", { className: "hint" }, "No concrete acceptance checks recorded yet."));

}

const field = (label, input) => el("label", { className: "studio-field" }, label, input);
const providerInput = () => el("select", {}, [["auto", "Claude Code + Codex"], ["claude", "Claude Code"], ["codex", "Codex"], ["ollama", "Ollama"], ["lmstudio", "LM Studio"]]
  .map(([value, label]) => el("option", { value }, label)));
const usageLabel = usage => usage?.input || usage?.output
  ? usage.input.toLocaleString() + " input tokens · " + usage.output.toLocaleString() + " output tokens"
  : usage?.promptBytes ? (usage.promptBytes / 1024).toFixed(1) + " KB of prompt text" : "";

function workCard(b) {
  const paths = b.hints.files.filter(file => /\.(rs|ron)$/.test(file));
  const owner = el("input", { placeholder: paths[0] ?? "crates/app/src/…", ariaLabel: "Owning source file" });
  owner.setAttribute("list", "parity-work-owners");
  const line = el("input", { type: "number", min: "1", placeholder: "Optional", ariaLabel: "Owning line" });
  const provider = providerInput();
  const model = el("input", { maxLength: 100, placeholder: "Provider default", ariaLabel: "Model" });
  const attempts = el("select", { ariaLabel: "Maximum AI passes" }, [0, 1, 2, 3].map(n =>
    el("option", { value: String(n) }, String(n))));
  const recordOnPass = el("input", { type: "checkbox" });
  const peerReview = el("input", { type: "checkbox" });
  const syncRepairSettings = busy => {
    provider.disabled = busy || Number(attempts.value) === 0;
    model.disabled = provider.disabled || provider.value === "auto";
    peerReview.disabled = provider.disabled || provider.value !== "auto" || Number(attempts.value) < 2;
  };
  provider.addEventListener("change", () => syncRepairSettings(false));
  attempts.addEventListener("change", () => syncRepairSettings(false));
  syncRepairSettings(false);
  const feedback = el("textarea", { rows: 2, maxLength: 420, placeholder: "Optional feedback for the next pass", ariaLabel: "Feedback" });
  const progress = el("div", { className: "studio-status", ariaLive: "polite", hidden: true });
  const start = el("button", { type: "submit", className: "primary" }, "Queue factory checks");
  const settings = [owner, line, provider, model, attempts, feedback, recordOnPass, peerReview, start];
  let lastRun, signature, initialized = false, pending = false;
  const stop = el("button", { type: "button", hidden: true, onclick: async () => {
    stop.disabled = true;
    try { paint(await api("/api/parity/work/stop", { id: b.id, run: lastRun })); }
    catch (error) { toast(error.message, true); }
    finally { void pollLedger(); }
  } }, "Stop run");
  const mark = el("button", { type: "button", hidden: true, onclick: async () => {
    try {
      await api("/api/parity/work/record", { id: b.id, revision: ledger.revision });
      void pollLedger();
      toast("Native checks recorded for " + b.id);
    } catch (error) { toast(error.message, true); }
  } }, "Record receipt");
  const form = el("form", { className: "studio-form", onsubmit: async event => {
    event.preventDefault();
    const reviewWithBoth = !peerReview.disabled && peerReview.checked;
    const repairProvider = provider.disabled ? "auto" : provider.value;
    const repairModel = model.disabled ? "" : model.value.trim();
    pending = true;
    for (const input of settings) input.disabled = true;
    try {
      const result = await api("/api/parity/work", { id: b.id, owner: owner.value.trim() || undefined,
        line: line.value ? Number(line.value) : undefined, provider: repairProvider, model: repairModel,
        feedback: feedback.value.trim(), attempts: Number(attempts.value), recordOnPass: recordOnPass.checked, reviewWithBoth });
      pending = false;
      paint(result);
    } catch (error) {
      toast(error.message, true);
      for (const input of settings) input.disabled = false;
      syncRepairSettings(false);
    } finally { pending = false; void pollLedger(); }
  } },
  el("div", { className: "studio-controls" }, start, stop, mark),
  el("details", { className: "studio-options" }, el("summary", {}, "Repair settings"),
    field("Owning file · optional override", owner),
    el("datalist", { id: "parity-work-owners" }, paths.map(value => el("option", { value }))),
    el("div", { className: "studio-grid" }, field("Start near line", line), field("Maximum AI passes", attempts)),
    el("div", { className: "studio-grid" }, field("Provider", provider), field("Model", model)), field("Repair feedback", feedback),
    el("label", { className: "studio-toggle" }, peerReview, "Use both Codex and Claude Code on this run"),
    el("label", { className: "studio-toggle" }, recordOnPass, "Record verified if these scenarios cover the full task")));
  const card = el("section", { className: "studio-track" }, el("h4", {}, "Factory verification"),
    el("p", { className: "hint" }, "Runs the tagged scenarios on a fresh build. Failed checks can trigger focused repairs, with rollback if the run ends without a pass."),
    el("div", { className: "run-labels" }, b.workChecks.map(id => el("code", { className: "row-run" }, id))), progress, form);
  function paint(work) {
    if (!card.isConnected || pending) return;
    const recorded = ledger.rows.find(row => row.id === b.id)?.evidence.includes(work.receipt);
    const next = JSON.stringify([work, recorded]);
    if (signature === next) return;
    signature = next;
    lastRun = work.run;
    if (!initialized && work.running) {
      owner.value = work.owner ?? "";
      line.value = work.line ?? "";
      if (work.provider) provider.value = work.provider;
      model.value = work.model ?? "";
      if (work.attempts !== undefined) attempts.value = String(work.attempts);
      recordOnPass.checked = Boolean(work.recordOnPass);
      peerReview.checked = Boolean(work.reviewWithBoth);
    }
    initialized = true;
    for (const input of settings) input.disabled = Boolean(work.running);
    syncRepairSettings(Boolean(work.running));
    stop.hidden = !work.running;
    stop.disabled = Boolean(work.stop);
    mark.hidden = work.running || work.phase !== "Checks passed" || !work.receipt || recorded;
    progress.hidden = work.phase === "Idle";
    replace(progress, el("strong", { className: "studio-phase", dataset: { phase: work.phase } },
      (work.stop && work.running ? "Stopping" : work.phase) + (work.attempt ? " · AI pass " + work.attempt : "")),
      work.message ? el("p", { className: "hint" }, work.message) : null,
      work.summary ? el("p", {}, work.summary) : null,
      work.followUp?.length ? el("ul", {}, work.followUp.map(check => el("li", {}, check))) : null,
      work.lookups ? el("p", { className: "hint" }, work.providers.at(-1) + " · " + work.lookups + " source lookups") : null,
      work.commit ? el("code", {}, work.commit.slice(0, 12)) : null,
      work.checks?.length ? el("ul", {}, work.checks.map(check => el("li", {}, check.id + (check.pass ? " · passed" : " · failed"),
        check.fail.length ? el("details", {}, el("summary", {}, "Failure details"), el("pre", {}, JSON.stringify(check.fail, null, 2))) : null))) : null,
      work.buildLog ? el("button", { type: "button", className: "ghost", onclick: () => copy(work.buildLog, "Build log path copied") }, "Copy build log path") : null,
      usageLabel(work.usage) ? el("p", { className: "hint" }, usageLabel(work.usage)) : null,
      ...(work.reviews ?? []).map(review => el("details", {}, el("summary", {},
        (review.provider === "claude" ? "Claude Code" : "Codex") + " review · " + (review.elapsedMs / 1000).toFixed(1) + " s"),
        el("p", {}, review.summary), el("p", { className: "hint" }, usageLabel(review.usage)))),
      work.bundles?.length ? el("p", { className: "hint" }, work.bundles.length + " rollback bundle" + (work.bundles.length === 1 ? "" : "s")) : null);
  }
  paintFactory = paint;
  return card;
}

async function handoff(createMission = false) {
  if (!brief) return;
  const opened = await sendToAi(brief.task, brief.hints.files.filter(file => /\.(rs|ron)$/.test(file)).slice(0, 5),
    { mode: "systems", origin: "Parity", parityId: brief.id, createMission });
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
  if (document.hidden || loadError?.status === 403) return;
  try { live = Boolean((await api("/api/game")).ready); } catch { live = false; }
  const node = $("live-console");
  if (node) { node.dataset.live = String(live); $("console-state").textContent = live ? "World ready" : "No world running"; }
}

let checkingEvidence = false, checkingWork = false;
async function refreshWork() {
  if (!selected || !paintFactory || checkingWork) return;
  const id = selected;
  const paint = paintFactory;
  checkingWork = true;
  try {
    const work = await api("/api/parity/work?id=" + encodeURIComponent(id));
    if (selected === id && paintFactory === paint) paint(work);
  } catch { /* The board's next poll retries progress. */ }
  finally {
    checkingWork = false;
    if (paintFactory && paintFactory !== paint) void refreshWork();
  }
}
function evidenceItem(entry, status) {
  const [state, ...rest] = String(status).split(" · ");
  const cut = entry.lastIndexOf("/");
  const dir = cut >= 0 ? entry.slice(0, cut + 1) : "", name = cut >= 0 ? entry.slice(cut + 1) : entry;
  return el("li", { className: "evidence-item", dataset: { state: state.toLowerCase() } },
    el("button", { type: "button", title: entry, onclick: () =>
      /^(commit|https?):/.test(entry) ? copy(entry, "Copied") : openInPanel(entry.split("#")[0]) },
    el("span", { className: "ev-path" }, el("span", { className: "ev-dir" }, dir), el("b", {}, name)),
    el("span", { className: "ev-meta" }, el("i", { className: "ev-dot" }), state, rest.length ? " · " + rest.join(" · ") : "")));
}
async function refreshEvidence() {
  const host = $("live-parity-evidence");
  if (!host || checkingEvidence || !selected) return;
  const id = selected;
  checkingEvidence = true;
  try {
    const { entries, mission } = await api("/api/parity/evidence?id=" + encodeURIComponent(id));
    if (host !== $("live-parity-evidence") || selected !== id) return;
    const signature = JSON.stringify([entries, mission]);
    if (host.dataset.evidence === signature) return;
    host.dataset.evidence = signature;
    host.replaceChildren(el("h3", {}, "Live evidence"),
      entries.length ? el("ul", { className: "files" }, entries.map(({ entry, status }) =>
        evidenceItem(entry, status)))
        : el("p", { className: "hint" }, "No published evidence yet."),
      ...(mission?.evidence ?? []).map(step => el("details", {}, el("summary", {}, step.status + " · " + step.title),
        el("p", { className: "hint" }, step.freshness),
        step.summary ? el("p", {}, step.summary) : null)));
  } catch {
    if (host === $("live-parity-evidence")) {
      delete host.dataset.evidence;
      host.replaceChildren(el("h3", {}, "Live evidence"), el("p", { className: "hint" }, "Evidence unavailable · Retrying"));
    }
  } finally { checkingEvidence = false; }
}

let pollingLedger = false;
function connection(online) {
  const label = $("parity-connection");
  label.dataset.online = String(online);
  const next = online ? "Live" : "Reconnecting";
  if (label.textContent !== next) label.textContent = next;
}

async function pollLedger() {
  if (pollingLedger || document.hidden || loadError?.status === 403) return;
  pollingLedger = true;
  try {
    const previous = ledger;
    const current = await api("/api/parity" + (ledger.liveRevision ? "?since=" + encodeURIComponent(ledger.liveRevision) : ""));
    if (ledger !== previous) return;
    connection(true);
    $("totals").title = "Ledger and mission status checked just now";
    if (!current.unchanged) {
      const focused = document.activeElement.closest("#rows [data-id]")?.dataset.id;
      ledger = current;
      loadError = null;
      renderLegend();
      render();
      if (focused) $("rows").querySelector(`[data-id="${CSS.escape(focused)}"]`)?.focus({ preventScroll: true });
    }
    const detail = $("detail");
    const row = ledger.rows.find(item => item.id === selected);
    if (brief && row) {
      paintMission?.(row.mission);
      const verification = $("selected-verification");
      if (verification) {
        verification.hidden = !row.verification;
        verification.textContent = [row.verification?.evidence, verificationRecovery(row)].filter(Boolean).join(" · ");
      }
      const badge = $("selected-status");
      if (badge && badge.dataset.status !== row.status) {
        const statusSelect = $("status-select");
        if (statusSelect.value === badge.dataset.status) statusSelect.value = row.status;
        badge.dataset.status = row.status;
        badge.replaceChildren(el("i", { className: "swatch", dataset: { status: row.status } }), row.status);
      }
      const updated = $("parity-updated");
      if (updated) {
        updated.hidden = !row.updated;
        const value = row.updated ? "Updated " + row.updated : "";
        if (updated.textContent !== value) updated.textContent = value;
      }
      const note = $("status-note");
      if (note?.value === (brief.ledgerNote ?? "")) note.value = row.note ?? "";
      Object.assign(brief, { status: row.status, ledgerNote: row.note, evidence: row.evidence, revision: ledger.revision });
      brief.contract.planningRevisions.ledger = ledger.revision;
      if ((brief.contract.planningRevisions.matrix !== ledger.matrix || brief.contract.planningRevisions.owners !== ledger.owners)
        && !detail.contains(document.activeElement)) await select(selected);
    }
  } catch {
    connection(false);
    $("totals").title = "Showing the last available ledger snapshot · Retrying";
  } finally {
    pollingLedger = false;
    void refreshWork();
    void refreshEvidence();
  }
}

// ---------- wiring ----------
async function load() {
  try {
    ledger = await api("/api/parity");
    connection(true);
    loadError = null;
    renderLegend();
    render();
    const linked = new URL(location.href).searchParams.get("id");
    if (selected !== linked && ledger.rows.some(row => row.id === linked)) await select(linked, true);
    if (!selected) $("detail").replaceChildren(el("div", { className: "detail-empty" },
      el("p", {}, "Pick a row from the list, the board or the coverage map."),
      el("p", { className: "hint" }, "Keys: ", el("kbd", {}, "/"), " search, ", el("kbd", {}, "J"), " ", el("kbd", {}, "K"), " move, ",
        el("kbd", {}, "A"), " send to AI, ", el("kbd", {}, "B"), " board, ", el("kbd", {}, "M"), " map")));
  } catch (error) {
    connection(false);
    loadError = error;
    $("totals").textContent = "Ledger unavailable";
    renderResults();
  }
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
$("verification-gaps").onclick = () => { prefs.evidenceGap = !prefs.evidenceGap; render(); };
$("all-families").onclick = () => { prefs.families = []; render(); };
$("industrial-families").onclick = () => { prefs.families = [...INDUSTRIAL]; render(); };
$("reset").onclick = () => { Object.assign(prefs, { families: [], statuses: [], milestones: [], ready: "", query: "", evidenceGap: false }); $("query").value = ""; render(); $("query").focus(); };
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

mountShell({ current: "/parity", context: () => {
  const row = ledger.rows.find(entry => entry.id === selected);
  return row ? {
    label: "Parity · " + row.label,
    parityId: row.id,
    summary: [row.id, row.label, row.status, row.mapping].filter(Boolean).join(" · ").slice(0, 420),
    sources: (brief?.hints.files ?? []).filter(path => /\.(rs|ron)$/.test(path)).slice(0, 5).map(path => ({ path })),
  } : { label: "Parity" };
} });
installPalette();
registerCommands(() => [
  { id: "parity:industrial", label: "Show Thermal, Mekanism, IE and Create", group: "Filter", run: () => { prefs.families = [...INDUSTRIAL]; render(); } },
  ...Object.entries(ledger.families).map(([key, label]) => ({ id: "parity:family:" + key, label: "Show only " + label, group: "Filter",
    run: () => { prefs.families = [key]; render(); } })),
  { id: "parity:ready", label: "Show rows with owner code listed", group: "Filter", run: () => { prefs.ready = "ready"; render(); } },
  { id: "parity:clear", label: "Clear filters", group: "Filter", run: () => $("reset").click() },
  { id: "parity:board", label: "Toggle board view", group: "View", keys: "B", run: () => { prefs.view = prefs.view === "board" ? "list" : "board"; render(); } },
  { id: "parity:map", label: "Toggle coverage map", group: "View", keys: "M", run: () => { prefs.map = !prefs.map; render(); } },
  { id: "parity:send", label: "Send selected row to AI workbench", group: "Action", keys: "A", run: handoff },
  ...ledger.rows.map(row => ({ id: "parity:row:" + row.id, label: row.id + " " + row.label, group: "Row", hint: row.status, run: () => select(row.id, true) })),
]);
load();
pollEngine();
setInterval(pollEngine, 4000);
setInterval(pollLedger, 3000);
document.addEventListener("visibilitychange", () => { if (!document.hidden) void pollLedger(); });
