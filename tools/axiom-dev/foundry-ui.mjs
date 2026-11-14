const token = location.hash.slice(1);
const node = (tag, text, cls) => { const n = document.createElement(tag); if (text !== undefined) n.textContent = text; if (cls) n.className = cls; return n; };
const card = title => { const n = node("article", undefined, "card"); n.append(node("h3", title)); return n; };
async function api(route, binary = false) {
  const response = await fetch(route, { headers: { "X-Axiom-Session": token } });
  if (!response.ok) throw new Error((await response.json()).error || "Evidence unavailable");
  return binary ? response.blob() : response.json();
}
import { organizePanels } from "./panel-organizer.mjs";
import { installPalette, registerCommands } from "./command-palette.mjs";
import { mountShell } from "./forge-shell.mjs";

// Card filter: text, problems-only and auto-refresh pause survive the 5 s redraw.
const prefsKey = "axiom-dev.foundry.prefs";
const prefs = (() => { try { return { query: "", problems: false, paused: false, ...JSON.parse(localStorage.getItem(prefsKey) ?? "{}") }; } catch { return { query: "", problems: false, paused: false }; } })();
const savePrefs = () => { try { localStorage.setItem(prefsKey, JSON.stringify(prefs)); } catch { /* private window */ } };
const control = (tag, attrs) => Object.assign(document.createElement(tag), attrs);
const search = control("input", { type: "search", placeholder: "Filter cards (/)", value: prefs.query, ariaLabel: "Filter cards" });
const problems = control("input", { type: "checkbox", checked: prefs.problems });
const paused = control("input", { type: "checkbox", checked: prefs.paused });
const label = (input, text) => { const l = control("label", { className: "qol-check" }); l.append(input, " " + text); return l; };
const refreshNow = control("button", { type: "button", textContent: "Refresh now" });
const cardCount = control("span", { className: "qol-count" });
const problemPattern = /error|fail|unverified|incomplete|blocked|rejected|regress|unavailable/i;
function filterCards() {
  const words = prefs.query.toLowerCase().split(/\s+/).filter(Boolean);
  const cards = [...document.querySelectorAll("main .card")];
  let shown = 0;
  for (const card of cards) {
    const text = card.textContent.toLowerCase();
    const visible = words.every(word => text.includes(word)) && (!prefs.problems || problemPattern.test(text));
    card.classList.toggle("foundry-hidden", !visible);
    if (visible) shown++;
  }
  cardCount.textContent = shown === cards.length ? cards.length + " cards" : shown + " of " + cards.length + " cards";
}
search.oninput = () => { prefs.query = search.value; savePrefs(); filterCards(); };
search.onkeydown = event => { if (event.key === "Escape") { search.value = prefs.query = ""; savePrefs(); filterCards(); } };
problems.onchange = () => { prefs.problems = problems.checked; savePrefs(); filterCards(); };
paused.onchange = () => { prefs.paused = paused.checked; savePrefs(); };
refreshNow.onclick = () => refresh();
document.addEventListener("keydown", event => {
  if (event.key === "/" && !event.target.matches("input,textarea,select")) { event.preventDefault(); search.focus(); search.select(); }
});
const main = document.querySelector("main");
const sections = organizePanels(main, {
  key: "axiom-dev.foundry.sections", filter: false,
  sections: ":scope > section:not(.intro):not(#capture), :scope > .grid > section",
  heading: "h2",
  categories: [["Performance", /Workload|comparisons/i], ["Research", /experiments|Research|Reference/i]],
  extra: [search, label(problems, "Problems only"), label(paused, "Pause live refresh"), refreshNow, cardCount],
  before: main.children[1],
});
filterCards();
mountShell({ current: "/foundry" });
installPalette();
registerCommands(() => [
  ...sections.entries().map(entry => ({ id: "foundry:" + entry.title, label: "Go to " + entry.title, group: "Section", run: entry.reveal })),
  { id: "foundry:refresh", label: "Refresh evidence", group: "Action", run: () => refresh() },
  { id: "foundry:panel", label: "Open Axiom Dev Panel", group: "Tool", run: () => { location.href = "/" + location.hash; } },
  { id: "foundry:wand", label: "Open Wand Lab", group: "Tool", run: () => { location.href = "/wand-lab" + location.hash; } },
]);

let captureUrl;
async function showCapture(id, image) {
  try {
    const blob = await api("/api/foundry/artifact?receipt=" + encodeURIComponent(id) + "&path=" + encodeURIComponent(image), true);
    if (captureUrl) URL.revokeObjectURL(captureUrl);
    captureUrl = URL.createObjectURL(blob);
    document.querySelector("#capture img").src = captureUrl;
    document.getElementById("capture").hidden = false;
  } catch (error) { document.getElementById("status").textContent = error.message; }
}
async function refresh() {
  try {
    const data = await api("/api/foundry");
    document.getElementById("performance").replaceChildren(...(data.performance || []).map(r => {
      const c = card(r.id + " · " + r.status);
      if (r.error) { c.append(node("p", r.error, "error")); return c; }
      const f = r.telemetry?.frame_ms;
      if (f) c.append(node("p", "p50 " + f.p50.toFixed(2) + " ms · p95 " + f.p95.toFixed(2) + " ms · p99 " + f.p99.toFixed(2) + " ms · max " + f.max.toFixed(2) + " ms"),
        node("small", f.samples + " frames · " + r.telemetry.over_budget_frames + " above " + r.frame_budget_ms.toFixed(2) + " ms diagnostic target"));
      c.append(node("p", "Declared residents: " + r.workload?.declared_population + "; visible count " + (r.telemetry?.visible_actors ? "measured" : "unmeasured")));
      const physical = r.physical_check;
      if (physical) c.append(node("p", "Separate CPU fixture: " + physical.active_actors + " active actors; " + physical.builders_with_placements + " workers made " + physical.placements + " paid placements."),
        node("small", "Material conservation: " + physical.materials_conserved + "; save round-trip: " + physical.save_roundtrip));
      for (const finding of (r.findings || []).slice(0, 3)) c.append(node("p", (finding.metric || finding.kind) + ": p95 " + finding.p95_ms.toFixed(2) + " ms" + (finding.max_ms ? "; max " + finding.max_ms.toFixed(2) + " ms" : "")), node("small", finding.next_probe));
      c.append(node("p", (r.reliability_issues || []).slice(0, 4).join(" "), "muted"), node("code", r.binary_sha256));
      return c;
    }));
    const receipts = data.receipts.map(r => {
      const c = card(r.id);
      if (r.error) { c.append(node("p", r.error, "error")); return c; }
      c.append(node("span", r.build || "Build unverified", "badge"), node("span", r.hardwareComplete ? "Hardware identified" : "Hardware incomplete", "badge"));
      c.append(node("p", r.conclusion), node("code", r.source || "Source unavailable"));
      const table = node("table");
      const head = node("tr"); ["Pass", "State", "p95 ms", "p99 ms"].forEach(x => head.append(node("th", x))); table.append(head);
      r.runs.forEach(run => { const row = node("tr"); [run.index, run.status, run.metrics?.p95_frame_ms ?? "—", run.metrics?.p99_frame_ms ?? "—"].forEach(x => row.append(node("td", String(x)))); table.append(row); });
      c.append(table);
      if (r.images.length) { const b = node("button", "View capture"); b.onclick = () => showCapture(r.id, r.images[0]); c.append(b); }
      return c;
    });
    document.getElementById("receipts").replaceChildren(...(receipts.length ? receipts : [node("p", "No native receipts yet.")]));
    document.getElementById("research").replaceChildren(...data.research.slice(-8).reverse().map(e => {
      const c = card(e.hypothesis || "Research observation"); c.append(node("span", e.status, "badge"), node("p", e.conclusion), node("small", e.verification)); return c;
    }));
    document.getElementById("comparisons").replaceChildren(...data.comparisons.map(e => {
      const c = card(e.status); c.append(node("p", (e.reasons || []).join(" ")));
      if (e.metrics) for (const [key, m] of Object.entries(e.metrics)) c.append(node("p", key + ": " + (100 * m.mean_relative_change).toFixed(1) + "% observed mean change"));
      c.append(node("small", "Merge authorization: false")); return c;
    }));
    document.getElementById("references").replaceChildren(...data.references.references.map(r => {
      const c = card(r.name); const a = node("a", r.commit.slice(0, 12)); a.href = r.source; a.target = "_blank"; a.rel = "noopener";
      c.append(a, node("p", r.technique), node("span", r.status, "badge"), node("p", r.comparison_contract), node("small", r.blockers.join(" "))); return c;
    }));
    document.getElementById("connection").textContent = "Live · " + new Date(data.updated).toLocaleTimeString();
    document.getElementById("status").textContent = data.interpretation;
    filterCards();
  } catch (error) { document.getElementById("connection").textContent = "Unavailable"; document.getElementById("status").textContent = error.message; }
}
await refresh();
setInterval(() => { if (!document.hidden && !prefs.paused) refresh(); }, 5000);
