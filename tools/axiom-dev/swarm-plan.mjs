// Buildout plan for the swarm page: what each agent is building now, what runs next and why it
// ranks there, what the supervisor does next (checkpoint, proof, spend cap), and how far the
// ledger is by milestone and family. Ledger columns and swarm state only: no rg, no model, so the
// page can poll it. The milestone tally is cached per ledger revision.
import { readFileSync, statSync } from "node:fs";
import path from "node:path";
import { FAMILIES, filterRows, planPath, root } from "./parity-data.mjs";

export const PLAN_STATUSES = ["verified", "playable", "building", "open", "design", "blocked", "parked"];
const CLOSED = new Set(["verified", "parked"]);
const FACTOR = {
  milestone: "milestone order", owners: "owner systems", progress: "current status",
  shared: "unblocks other rows", pack: "2020 pack feature", risk: "risk flags",
};
const clip = (text, n) => { const s = String(text ?? "").replace(/\s+/g, " ").trim(); return s.length > n ? s.slice(0, n - 1) + "…" : s; };
const msKey = ms => (String(ms ?? "").match(/M\d+(?:\.\d+)?/) ?? ["—"])[0];
const msOrder = key => key === "—" ? 99 : Number(key.slice(1));

// Milestone titles and goals from the master plan: "### M1: vertical slice" headings and
// "| M2 Bodies/mobs | goal | ..." table rows. Re-read only when the file changes.
let names = { mtime: -1, map: new Map() };
function milestoneNames() {
  const file = path.join(root, planPath);
  let mtime = 0;
  try { mtime = statSync(file).mtimeMs; } catch { return names.map; }
  if (mtime === names.mtime) return names.map;
  const map = new Map();
  try {
    const text = readFileSync(file, "utf8");
    for (const m of text.matchAll(/^###\s+(M\d+(?:\.\d+)?):\s*(.+)$/gm)) if (!map.has(m[1])) map.set(m[1], { title: m[2].trim(), goal: "" });
    for (const m of text.matchAll(/^\|\s*(M\d+(?:\.\d+)?)\s+([^|]+?)\s*\|\s*([^|]+?)\s*\|/gm)) map.set(m[1], { title: m[2].trim(), goal: clip(m[3], 160) });
  } catch { /* plan unreadable: lanes show ids only */ }
  names = { mtime, map };
  return map;
}

function tally(rows) {
  const counts = Object.fromEntries(PLAN_STATUSES.map(s => [s, 0]));
  for (const row of rows) counts[row.status] = (counts[row.status] ?? 0) + 1;
  return counts;
}

let tallyCache = { key: "", value: null };
function buildout(ledger, family) {
  const key = JSON.stringify(ledger.planningRevisions) + "|" + family;
  if (tallyCache.key === key) return tallyCache.value;
  const scope = filterRows(ledger.rows, { family: family === "all" ? undefined : family });
  const titles = milestoneNames();
  const groups = new Map();
  for (const row of scope) { const k = msKey(row.milestone); if (!groups.has(k)) groups.set(k, []); groups.get(k).push(row); }
  const milestones = [...groups].sort((a, b) => msOrder(a[0]) - msOrder(b[0])).map(([id, rows]) => ({
    id, title: titles.get(id)?.title ?? "", goal: titles.get(id)?.goal ?? "", total: rows.length, counts: tally(rows),
    ready: rows.filter(r => r.readiness.ready && !CLOSED.has(r.status) && r.status !== "design").length,
  }));
  const families = family !== "all" ? [] : Object.entries(FAMILIES).filter(([k]) => !k.startsWith("eng"))
    .map(([id, f]) => { const rows = scope.filter(r => r.families.includes(id)); return { id, label: f.label, total: rows.length, counts: tally(rows) }; })
    .filter(f => f.total).sort((a, b) => b.total - a.total);
  // Owner systems that open rows wait on, most-demanded first: building one unblocks the most rows.
  const demand = new Map();
  for (const row of scope) if (!CLOSED.has(row.status)) for (const id of row.readiness.missing) demand.set(id, (demand.get(id) ?? 0) + 1);
  const gaps = [...demand].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([id, rows]) => {
    const sys = ledger.systems.get(id);
    return { id, name: sys?.name ?? "unknown system", substrate: sys?.substrate ?? "unknown", milestone: sys?.milestone ?? "", rows };
  });
  const value = { scope: family, total: scope.length, open: scope.filter(r => !CLOSED.has(r.status)).length, counts: tally(scope), milestones, families, gaps };
  tallyCache = { key, value };
  return value;
}

function describe(row, ledger) {
  const titles = milestoneNames();
  const ms = msKey(row.milestone);
  const why = Object.entries(row.priority?.factors ?? {}).filter(([, v]) => v)
    .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1])).map(([key, value]) => ({ key, label: FACTOR[key] ?? key, value }));
  return {
    id: row.id, label: row.label, section: row.section, status: row.status, mapping: clip(row.mapping, 240),
    notes: clip(row.notes, 280), ms, msTitle: titles.get(ms)?.title ?? "", cost: row.cost, risks: row.risks,
    serves: row.paths, ready: row.readiness.ready,
    owners: row.readiness.owners.map(o => ({ id: o.id, name: o.name, substrate: o.substrate })),
    missing: row.readiness.missing, score: row.priority?.score ?? 0, why,
    evidence: (row.evidence ?? []).length, ledgerNote: clip(row.note, 200),
  };
}

// What the tier ladder will do with a queued row, in order.
function ladder(item, cfg) {
  if (/scout/.test(item.tier)) return ["haiku scout triages (read-only)", "sonnet builder", cfg.prove === false ? null : "engine proof if it adds a scenario"].filter(Boolean);
  if (/opus/.test(item.tier)) return ["opus retry (final tier)"];
  if (/fable/.test(item.tier)) return ["fable builder (no escalation)"];
  if (/^designed: /.test(item.label ?? "")) return ["design lead wrote the design", "sonnet builder follows it", "one opus escalation if it fails"];
  return ["sonnet builder", "one opus escalation if it fails"];
}

/** Supervisor actions that are due soon, so the page can say what happens next. */
function upcoming(state, design) {
  const cfg = state.cfg ?? {}, t = state.totals ?? {}, out = [];
  if (state.paused) out.push({ kind: "paused", text: String(state.paused), at: state.resumeAt ?? null });
  // Design stage: what the lead is designing, what waits for it and how much of the hourly cap is used.
  if (design) {
    const w = design.waiting ?? {}, waiting = (w.owner ?? 0) + (w.row ?? 0) + (w.epic ?? 0);
    const kinds = [w.owner && `${w.owner} owner system${w.owner === 1 ? "" : "s"}`, w.row && `${w.row} blocked row${w.row === 1 ? "" : "s"}`, w.epic && `${w.epic} epic${w.epic === 1 ? "" : "s"}`].filter(Boolean).join(", ");
    if (design.running?.length) out.push({ kind: "design", text: "Designing now: " + design.running.map(d => d.key).join(", ") });
    if (!design.on) { if (waiting) out.push({ kind: "paused", text: `Design stage is off: ${kinds} wait for the owner. Turn it on in Settings.` }); }
    else if (waiting) out.push({ kind: "design", text: `Waiting for a design: ${kinds}. ${design.hour} of ${design.cap} designs used this hour.` });
  }
  if (state.checkpointing) out.push({ kind: "checkpoint", text: "Running now: " + state.checkpointing });
  else if (cfg.checkpointEvery > 0) {
    const left = cfg.checkpointEvery - ((t.done ?? 0) % cfg.checkpointEvery);
    out.push({ kind: "checkpoint", text: `Checkpoint (cargo check of the whole tree) after ${left} more landed row${left === 1 ? "" : "s"}` });
  }
  // Owner systems the supervisor parked after they blocked 2+ rows ("M10×7"): their rows are skipped.
  if ((state.deadOwners ?? []).length) out.push({ kind: "paused", text: "Rows wait on owner systems that have no design yet: " + state.deadOwners.slice(0, 8).join(", ")
    + (state.deadOwners.length > 8 ? ` and ${state.deadOwners.length - 8} more` : "") + (design?.on ? ". The design stage takes the most-demanded first." : "") });
  if (state.health && !state.health.ok) out.push({ kind: "red", text: "Tree does not compile; builders wait for green. " + clip((state.health.errors ?? [])[0], 160) });
  if (cfg.budget > 0) {
    const hrs = (Date.now() - (state.startedAt ?? Date.now())) / 36e5, rate = hrs > 0.02 ? (t.cost ?? 0) / hrs : 0;
    // The cap counts every run since the last reset (state.spend.total); t.cost is this process only.
    const spent = state.spend?.total ?? t.cost ?? 0;
    out.push({ kind: "budget", text: `Spend cap $${cfg.budget}: $${spent.toFixed(2)} used` + (rate > 0 ? `, about ${Math.max(0, (cfg.budget - spent) / rate).toFixed(1)} h left at the current rate` : "") });
  }
  if (cfg.prove !== false && state.lastProof) out.push({ kind: "proof", text: "Last engine proof " + (state.lastProof.ok ? "passed" : "failed") + (state.lastProof.note ? ": " + clip(state.lastProof.note, 120) : ""), at: state.lastProof.at });
  return out;
}

export function planView({ ledger, state, queue, family = "all", design, idle }) {
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const cfg = state.cfg ?? {};
  // `status` is the row's ledger status; `state` is the worker's (working, waiting for green build, ...).
  const now = state.workers.filter(w => w.row && byId.has(w.row)).map(w => ({
    worker: w.id, state: w.status, phase: w.phase ?? "", tier: w.tier ?? "", startedAt: w.startedAt ?? null, turns: w.turns ?? 0,
    files: (w.files ?? []).slice(0, 6), likely: (w.likely ?? []).slice(0, 4), lastActivity: clip(w.activity?.at?.(-1)?.text, 160),
    ...describe(byId.get(w.row), ledger),
  }));
  const next = (queue ?? []).slice(0, 8).filter(q => byId.has(q.id)).map((q, i) => ({
    position: i + 1, tier: q.tier, pinned: !!q.pinned, steps: ladder(q, cfg), ...describe(byId.get(q.id), ledger),
  }));
  const plan = buildout(ledger, family ?? "all");
  const hrs = (Date.now() - (state.startedAt ?? Date.now())) / 36e5;
  const finishedRows = (state.totals?.done ?? 0) + (state.totals?.blocked ?? 0);
  const rate = hrs > 0.05 ? finishedRows / hrs : 0;
  // "Open" in the ledger means "not verified", which counts every landed row. The queue estimate uses only what
  // has not been started: 516 "open" rows beside an empty queue read as a hang, and "39 h of queue" was wrong.
  const todo = (plan.counts.open ?? 0) + (plan.counts.design ?? 0);
  return {
    at: Date.now(), family: family ?? "all", idle: state.workers.filter(w => w.status === "idle").length,
    idleWhy: idle?.text ?? null,
    now, next, upcoming: upcoming(state, design), buildout: plan,
    pace: { rowsPerHour: rate, hoursToOpen: rate > 0 && (queue ?? []).length ? todo / rate : null },
  };
}
