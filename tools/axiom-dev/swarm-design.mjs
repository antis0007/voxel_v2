// Design stage for the swarm: prompt, parse, target choice and the notes builders receive. Nothing here spawns
// a process, so it can be checked without a model.
//
// Rows stop when nobody has written the decision they need: a scout's needs-design verdict, an owner system the
// master plan lists as design-only, an epic. A design lead (a strong read-only model) then writes the smallest
// concrete design from the tree, the docs and its own knowledge of the genre, and a builder implements it. One
// design of a parked owner system frees every row that waits on it, so owners go first.
// The lead defers only what the project keeps for the owner: story text, a new dependency, a wire-format change,
// a change to existing art direction. A missing doc or missing code is a reason to design, not to defer.
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

// Entries written by the first decide pass carry no version. Its prompt forbade save bumps and gates that the
// owner policy approves, so its deferrals are asked once more; its decisions stand.
export const DESIGN_VERSION = 2;
const PARTS = ["DESIGN", "MODEL", "RULES", "SLICE", "NEXT", "CHECK", "ASSUMED"];
const clip = (text, n) => { const s = String(text ?? "").replace(/\s+/g, " ").trim(); return s.length > n ? s.slice(0, n - 1) + "…" : s; };
export const ownerKey = id => "owner:" + id;
const fileName = key => String(key).replace(/[^\w.-]/g, "-") + ".md";

/** Saved designs: one JSON line per decision (decisions.jsonl) and the full text under designs/. */
export function createDesignStore(dir) {
  const file = path.join(dir, "decisions.jsonl"), folder = path.join(dir, "designs");
  const byKey = new Map();
  try {
    for (const line of readFileSync(file, "utf8").split("\n")) {
      if (!line.trim()) continue;
      try { const d = JSON.parse(line); if (d.forget) byKey.delete(d.id); else byKey.set(d.id, d); } catch { /* skip a torn line */ }
    }
  } catch { /* first run */ }
  return {
    get: key => byKey.get(key),
    all: () => [...byKey.values()],
    decided: key => byKey.get(key)?.verdict === "decide",
    /** True when the item should go to the design lead. `blockedAt`: when a build from the design last blocked. */
    needs(key, blockedAt = 0) {
      const d = byKey.get(key);
      if (!d) return true;
      const version = d.v ?? 1;
      if (d.verdict === "defer") return version < DESIGN_VERSION;
      // Built from the design and blocked again: one more design, with that report in hand.
      return blockedAt > d.at && (version < DESIGN_VERSION || (d.round ?? 1) < 2);
    },
    save(entry, body) {
      byKey.set(entry.id, entry);
      appendFileSync(file, JSON.stringify(entry) + "\n");
      if (body) { mkdirSync(folder, { recursive: true }); writeFileSync(path.join(folder, fileName(entry.id)), body); }
    },
    /** Drop a design so the item is designed again (the owner overruled it). */
    forget(key) { if (byKey.delete(key)) appendFileSync(file, JSON.stringify({ id: key, forget: true, at: Date.now() }) + "\n"); },
    read(key) { try { return readFileSync(path.join(folder, fileName(key)), "utf8"); } catch { return ""; } },
    pathOf: key => path.join(folder, fileName(key)),
  };
}

const RULES = `Design within these rules. They shape the design; they are not reasons to defer:
- Simulation runs in fixed steps with seeded hashes. No wall-clock time.
- Content lives in assets/*.ron tables read by typed loaders. No content tables in Rust.
- No new dependencies: use what the workspace already has.
- No new story text or proper nouns. Give every thing a plain descriptive name; a mod's own noun becomes what the thing does ("field energy", "force projector").
- Existing visuals, saves and generated terrain stay identical by default. New behaviour sits behind a world rule, preference or system switch that defaults to the old path.
- The network wire format does not change.
Already approved, so use them freely: a save-format bump with new fields appended and gated by version; a new dormant generator gate in the 600-699 range; new resources, machine kinds, modes and RON tables; a minimal owner system built in the same patch as its first user.`;

const REPLY = `Use at most 10 lookups, then answer. Reply with exactly this block and nothing else, in plain text under 2200 characters:
DESIGN: <the decision in one sentence>
MODEL: <the data: structs and fields, the RON table with one example row, save fields if any>
RULES: <how it ticks or reacts: numbers with units, order of operations, what keeps it deterministic>
SLICE: <the first compiling slice, under 300 changed lines: files and symbols to edit, the switch it sits behind, the caller that reaches it>
NEXT: <two to four later slices, one line each>
CHECK: <what the owner measures or sees to accept it>
ASSUMED: <what you took from your own knowledge instead of the tree or docs, so the owner can overrule it>
Only when the item cannot be delivered at all without new story text, a new dependency, a wire-format change or a change to existing art direction, reply with this one line instead:
DEFER: <which of those four, and the exact question for the owner>`;

const SOURCES = `Sources, in this order:
1. The tree. Grep for the nearest existing system and reuse its patterns, tables and switches.
2. The docs. docs/MASTER_PLAN.md (Grep the system or row id), docs/ARCHITECTURE.md, docs/GAME_DESIGN.md, docs/adr/, and the note under docs/research/ that the work order names. Engine rows: docs/research/ENGINE_OVERHAUL_REVIEW.md has the finding under the row id.
3. Your own knowledge of the genre and of the mod or technique the row refers to. Where the tree and docs are silent, decide it yourself and list it under ASSUMED. Missing code or a missing doc is never a reason to defer.`;

/** Prompt for one item. `item.brief` is the row's work order (JSON text) for rows and epics. */
export function designPrompt(item) {
  const head = "You are the design lead for a voxel game (Rust + wgpu, content in assets/*.ron). One item waits on a design nobody has written. Write the smallest concrete design that delivers its player-visible outcome. A builder that follows instructions literally implements it next, so name files, symbols, fields and numbers.";
  let what;
  if (item.kind === "owner") {
    const s = item.system ?? {};
    what = `Item: owner system ${item.owner} "${s.name ?? "unnamed"}"${s.milestone ? ` (milestone ${s.milestone})` : ""}. The master plan lists it as design-only${s.reuse ? `: ${clip(s.reuse, 200)}` : ""}.${s.prerequisites?.length ? ` Prerequisites: ${s.prerequisites.join(", ")}.` : ""}
Rows waiting on it:
${item.rows.slice(0, 8).map(r => `- ${r.id} ${clip(r.label, 90)}: ${clip(r.mapping, 160)}`).join("\n")}${item.rows.length > 8 ? `\n- and ${item.rows.length - 8} more` : ""}
${item.reports?.length ? "What earlier attempts reported:\n" + item.reports.map(r => `- ${r.id}: ${clip(r.summary, 220)}`).join("\n") + "\n" : ""}Design the MINIMAL owner these rows need: its data model and one tick or entry point. SLICE is that core, built together with the first row in the list.`;
  } else {
    what = `Item: ${item.kind === "epic" ? "epic row (needs a design before any slice)" : "blocked row"}.
Work order: ${String(item.brief ?? "").slice(0, 2200)}
${item.report ? `Why it stopped: ${clip(item.report, 500)}` : "No slice has been tried yet. Cut the epic so that SLICE is useful on its own."}${item.previous ? `\nAn earlier design was built and blocked again. Earlier design: ${clip(item.previous, 400)} Design around that failure; do not repeat it.` : ""}`;
  }
  return [head, what, SOURCES, RULES, REPLY].join("\n\n");
}

/** Parse the lead's reply. Returns { verdict: "decide", body, parts } | { verdict: "defer", text } | null. */
export function parseDesign(text) {
  // Models decorate headings ("**SLICE:**", "## SLICE:"); strip that so the parts are found.
  const s = String(text ?? "").replace(/\r/g, "").replace(/^[ \t]*(?:#{1,4}[ \t]*)?\*{0,2}([A-Z]{4,7})\*{0,2}[ \t]*:\*{0,2}/gm, "$1:");
  const at = s.search(/^DESIGN:/m);
  if (at >= 0) {
    const body = s.slice(at).trim().slice(0, 4000), parts = {};
    let name = null;
    for (const line of body.split("\n")) {
      const head = line.match(/^([A-Z]+):\s*(.*)$/);
      if (head && PARTS.includes(head[1])) { name = head[1]; parts[name] = head[2]; }
      else if (name) parts[name] += "\n" + line;
    }
    for (const k in parts) parts[k] = parts[k].trim();
    // 2026-11-18: designs often skipped SLICE and listed every slice under NEXT (ENG-META-13/14/19 came back as
    // "no usable answer" with a full design). The first NEXT line is the first slice; the rest stay NEXT.
    if (parts.DESIGN && !parts.SLICE && parts.NEXT) {
      const [first, ...rest] = parts.NEXT.split("\n").map(l => l.replace(/^[-*\d.)\s]+/, "").trim()).filter(Boolean);
      if (first) { parts.SLICE = first; parts.NEXT = rest.join("\n"); }
    }
    // A design without a first slice gives the builder nothing to do.
    if (parts.DESIGN && parts.SLICE) return { verdict: "decide", body, parts };
  }
  const defer = s.match(/^DEFER:\s*(.+)$/m);
  return defer ? { verdict: "defer", text: defer[1].trim().slice(0, 600) } : null;
}

/** The note a builder gets: the decision, the data and the first slice, with the full text one Read away. */
export function designNote(kind, key, parts, relFile) {
  const lead = kind === "owner"
    ? `Owner system ${key.replace(/^owner:/, "")} now has a design (follow it; build the minimal part your row needs in the same patch): `
    : "Design (decided by the design lead; follow it, do not reopen the question): ";
  return (lead + clip(parts.DESIGN, 280) + " Data: " + clip(parts.MODEL, 320) + " First slice: " + clip(parts.SLICE, 420)
    + ` Full design: ${relFile} (Read it once before editing).`).slice(0, 1300);
}

/** Note for an entry of any version (old entries have only `text`). */
export const noteOf = d => d?.note ?? (d?.text ? `Owner decision (auto, conservative; follow it, do not reopen the question): ${d.text}` : "");

/**
 * What the design lead should take next, most valuable first:
 *   1. parked owner systems (each frees every row that waits on it), most-demanded first;
 *   2. blocked rows, design blocks before other blocks, in ledger priority order;
 *   3. epics (ledger status "design") that no slice has touched, in ledger priority order.
 * Rows behind an owner that is still to be designed wait for that design instead of getting their own.
 */
export function designTargets({ ledger, finished, store, skip = () => false, scope = rows => rows, order = rows => rows, isDesignBlock = () => false, dead = new Set(), epics = true, limit = 3 }) {
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const live = r => r && !["verified", "parked"].includes(r.status) && !skip(r.id);
  const rank = new Map(order(ledger.rows.filter(live)).map((r, i) => [r.id, i]));
  const byRank = (a, b) => (rank.get(a.id) ?? 1e9) - (rank.get(b.id) ?? 1e9);
  const inScope = new Set(scope(ledger.rows).map(r => r.id));
  const blocked = [...finished.values()].filter(f => f.status === "blocked" && f.final !== false && live(byId.get(f.id)));
  const ownerPending = o => dead.has(o) && store.needs(ownerKey(o)) && !skip(ownerKey(o));

  const waiting = new Map();
  for (const row of ledger.rows) {
    if (!live(row) || !inScope.has(row.id)) continue;
    const f = finished.get(row.id);
    if (f && f.status !== "blocked") continue; // landed rows no longer wait
    for (const o of row.readiness.missing) if (ownerPending(o)) { if (!waiting.has(o)) waiting.set(o, []); waiting.get(o).push(row); }
  }
  const owners = [...waiting].sort((a, b) => b[1].length - a[1].length).map(([o, rows]) => ({
    kind: "owner", key: ownerKey(o), owner: o, system: ledger.systems.get(o), rows: rows.sort(byRank),
    reports: blocked.filter(f => byId.get(f.id).readiness.missing.includes(o)).slice(0, 4) }));

  const rows = blocked.filter(f => inScope.has(f.id) && store.needs(f.id, f.at ?? 0) && !byId.get(f.id).readiness.missing.some(ownerPending))
    .sort((a, b) => Number(isDesignBlock(b)) - Number(isDesignBlock(a)) || byRank(a, b))
    .map(f => ({ kind: "row", key: f.id, row: byId.get(f.id), report: f.summary, previous: store.decided(f.id) ? store.get(f.id).text : undefined }));

  const epicRows = !epics ? [] : ledger.rows.filter(r => r.status === "design" && live(r) && inScope.has(r.id) && !finished.has(r.id)
    && store.needs(r.id) && !r.readiness.missing.some(ownerPending)).sort(byRank).map(row => ({ kind: "epic", key: row.id, row }));

  const all = [...owners, ...rows, ...epicRows];
  return { items: all.slice(0, Math.max(0, limit)), counts: { owner: owners.length, row: rows.length, epic: epicRows.length } };
}
