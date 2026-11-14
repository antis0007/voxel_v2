// Mod-parity ledger: the canonical feature matrix (read-only planning rows)
// joined with the master-plan system inventory and a small status overlay.
// The matrix stays the authority for IDs and routing; status lives in
// docs/research/mod_parity_status.json so rows never get rewritten.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, renameSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const matrixPath = "docs/research/MOD_FEATURE_MATRIX.md";
export const planPath = "docs/MASTER_PLAN.md";
export const statusPath = "docs/research/mod_parity_status.json";

/** Ordered delivery states. `verified` needs evidence a reviewer can rerun. */
export const STATUSES = ["open", "design", "building", "playable", "verified", "blocked", "parked"];

/** Short names an agent or the board can use instead of family text. */
export const FAMILIES = {
  thermal: { label: "Thermal Series", match: /\bThermal\b|Redstone Arsenal|Simply Jetpacks/i, ids: /^A09-/ },
  mekanism: { label: "Mekanism", match: /\bMekanism\b/i, ids: /^A13-/ },
  create: { label: "Create", match: /\bCreate\b(?![ds])/, ids: /^B-CREATE-/ },
  ie: { label: "Immersive Engineering", match: /Immersive Engineering/i, ids: /^B-IE-/ },
  gregtech: { label: "GregTech", match: /GregTech/i, ids: /^B-GT-/ },
  enderio: { label: "Ender IO", match: /Ender ?IO/i, ids: /^A03-/ },
  rftools: { label: "RFTools", match: /RFTools/i, ids: /^A04-/ },
  ic2: { label: "IC2 / BuildCraft / quarries", match: /IndustrialCraft|\bIC2\b|BuildCraft|Quarr/i, ids: /^A12-/ },
  factorio: { label: "Factorio", match: /Factorio/i, ids: /^A10-/ },
  ae2: { label: "Applied Energistics 2", match: /Applied Energistics|\bAE2\b/i, ids: /^B-AE2-/ },
  nuclear: { label: "Nuclear / accelerators", match: /Atomic Science|reactor/i, ids: /^A08-/ },
  tinkers: { label: "Tinkers / gear", match: /Tinkers/i, ids: /^A02-/ },
  botania: { label: "Botania", match: /Botania/i, ids: /^B-BOT-/ },
  space: { label: "Space family", match: /Galacticraft|Ad Astra/i, ids: /^A05-/ },
};
/** The industrial set the user asked the agent to work through first. */
export const INDUSTRIAL = ["thermal", "mekanism", "ie", "create"];

const cells = line => line.slice(1, line.endsWith("|") ? -1 : undefined).split(/(?<!\\)\|/).map(cell => cell.trim());
const listOf = text => text.split(/[,\s]+/).map(part => part.trim()).filter(part => part && part !== "—");
const read = relative => readFileSync(path.join(root, relative), "utf8");
const digest = text => createHash("sha256").update(text).digest("hex").slice(0, 16);

export function parseMatrix(text) {
  const rows = [];
  let section = "";
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("## ")) { section = line.slice(3).trim(); continue; }
    if (!line.startsWith("| ")) continue;
    const parts = cells(line);
    const head = parts[0]?.match(/^([A-Z][A-Z0-9]*(?:-[A-Z0-9]+)*):\s*(.+)$/);
    if (!head || parts.length < 10) continue;
    const note = parts[1].match(/\[source note\]\(([^)]+)\)/)?.[1];
    rows.push({
      id: head[1], label: head[2].replace(/`\[unverified\]`/g, "").trim(), section,
      sources: parts[1].replace(/\[([^\]]+)\]\([^)]+\)/g, "$1"),
      family: parts[1].split(";")[0].trim(),
      sourceNote: note ? "docs/research/" + note : null,
      mapping: parts[2], depends: listOf(parts[3]), cost: parts[4],
      pillars: listOf(parts[5]), paths: listOf(parts[6]), milestone: parts[7],
      risks: listOf(parts[8]), notes: parts[9] ?? "",
    });
  }
  return rows;
}

export function parseSystems(text) {
  const systems = new Map();
  for (const line of text.split(/\r?\n/)) {
    if (!/^\| [A-Z]\d{2} \|/.test(line)) continue;
    const [id, name, tier, milestone, prerequisites, reuse] = cells(line);
    systems.set(id, { id, name, tier, milestone, prerequisites: listOf(prerequisites ?? ""),
      reuse: reuse ?? "", substrate: /^L:/.test(reuse ?? "") ? "legacy" : /^P:/.test(reuse ?? "") ? "design-only" : "unknown" });
  }
  return systems;
}

export function readStatus() {
  const file = path.join(root, statusPath);
  const text = existsSync(file) ? readFileSync(file, "utf8") : "";
  const data = text ? JSON.parse(text) : { version: 1, rows: {} };
  return { data, revision: digest(text || "{}") };
}

/** Revision-checked, atomic status write. Unknown IDs and states are refused. */
export function writeStatus(id, change, revision, known) {
  const { data, revision: current } = readStatus();
  if (revision !== current) throw new Error(`Status file changed (revision ${current}); reload before writing`);
  if (!known.has(id)) throw new Error("Unknown feature ID " + id);
  if (change.status && !STATUSES.includes(change.status)) throw new Error("Status must be one of " + STATUSES.join(", "));
  const before = data.rows[id] ?? { status: "open", evidence: [] };
  const evidence = [...new Set([...(before.evidence ?? []), ...(change.evidence ?? [])])];
  for (const entry of evidence) {
    const file = entry.split("#")[0];
    if (!/^(?:https?:|commit:)/.test(entry) && !existsSync(path.join(root, file))) throw new Error("Evidence path not found: " + entry);
  }
  if (change.status === "verified" && !evidence.length) throw new Error("verified needs at least one evidence path or commit:");
  const next = { ...before, ...change, evidence, updated: new Date().toISOString().slice(0, 10) };
  if (!next.note) delete next.note;
  data.rows[id] = next;
  data.rows = Object.fromEntries(Object.entries(data.rows).sort(([a], [b]) => a.localeCompare(b)));
  const file = path.join(root, statusPath), temporary = file + ".tmp";
  writeFileSync(temporary, JSON.stringify(data, null, 2) + "\n");
  renameSync(temporary, file);
  return { id, before, after: next, revision: readStatus().revision };
}

export function familiesOf(row) {
  return Object.entries(FAMILIES).filter(([, family]) => family.ids.test(row.id) || family.match.test(row.sources)).map(([key]) => key);
}

/** Owner readiness: legacy substrate on every owner means work can start now. */
export function readiness(row, systems) {
  const owners = row.depends.map(id => systems.get(id) ?? { id, name: "unknown system", substrate: "unknown" });
  const missing = owners.filter(owner => owner.substrate !== "legacy").map(owner => owner.id);
  return { owners, missing, ready: !missing.length };
}

export function loadLedger() {
  const matrixText = read(matrixPath);
  const rows = parseMatrix(matrixText);
  const systems = parseSystems(read(planPath));
  const { data, revision } = readStatus();
  const ledger = rows.map(row => {
    const state = data.rows[row.id] ?? { status: "open", evidence: [] };
    return { ...row, families: familiesOf(row), status: state.status, evidence: state.evidence ?? [], note: state.note,
      updated: state.updated, readiness: readiness(row, systems) };
  });
  return { rows: ledger, systems, revision, matrixRevision: digest(matrixText) };
}

export function filterRows(rows, { family, status, milestone, ready, query, section } = {}) {
  const words = (query ?? "").toLowerCase().split(/\s+/).filter(Boolean);
  const families = family ? family.split(",").flatMap(name => name === "industrial" ? INDUSTRIAL : [name]) : [];
  return rows.filter(row => (!families.length || row.families.some(name => families.includes(name)))
    && (!status || status.split(",").includes(row.status))
    && (!milestone || row.milestone.split(/[ /]+/).includes(milestone))
    && (ready === undefined || row.readiness.ready === ready)
    && (!section || row.section.toLowerCase().includes(section.toLowerCase()))
    && words.every(word => [row.id, row.label, row.sources, row.mapping, row.notes].join(" ").toLowerCase().includes(word)));
}

/** Suggested next rows: open, owners already have substrate, earliest milestone, fewest risks. */
export function nextRows(rows, limit = 5) {
  const rank = row => Number(row.milestone.match(/M(\d+)/)?.[1] ?? 9) * 10 + row.risks.length;
  return rows.filter(row => row.status === "open" && row.readiness.ready).sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id)).slice(0, limit);
}

const STOP = new Set("with from that this into their shared each other where which about using under over more than only also have when these those across without between through".split(" "));
/** Where the repo may already implement part of a row: bounded keyword search over content tables and factory code. */
const SCOPE = ["assets", "crates/app/src/world/factory", "crates/app/src/world/inventory.rs", "crates/content/src"];
const GENERIC = 30; // a word found in more files than this is too common to locate anything
export function codeHints(row, max = 10) {
  const terms = (text, weight) => (text.toLowerCase().match(/[a-z][a-z-]{4,}/g) ?? [])
    .filter(word => !STOP.has(word)).map(word => [word.replace(/s$/, ""), weight]);
  const weights = new Map();
  for (const [word, weight] of [...terms(row.label, 2), ...terms(row.mapping, 1)]) weights.set(word, Math.max(weights.get(word) ?? 0, weight));
  const words = [...weights.keys()].slice(0, 8);
  if (!words.length) return { words: [], absent: [], files: [] };
  // One pass: every "path:match" line tells which words each file contains.
  let out = "";
  try {
    out = execFileSync("rg", ["-i", "-o", "-F", "--no-line-number", "--with-filename", "-g", "*.rs", "-g", "*.ron",
      "-g", "!assets/art_direction/**", "-g", "!assets/source/**", ...words.flatMap(word => ["-e", word]), ...SCOPE],
    { cwd: root, encoding: "utf8", timeout: 6000, maxBuffer: 16 << 20 });
  } catch (error) { out = error.stdout ?? ""; }
  const filesOf = new Map(words.map(word => [word, new Set()]));
  for (const line of out.split(/\r?\n/)) {
    const at = line.lastIndexOf(":");
    if (at > 0) filesOf.get(line.slice(at + 1).toLowerCase())?.add(line.slice(0, at).replaceAll("\\", "/"));
  }
  const score = new Map();
  const used = words.filter(word => filesOf.get(word).size && filesOf.get(word).size <= GENERIC);
  for (const word of used) for (const file of filesOf.get(word)) {
    // Factory code and machine/recipe tables beat unrelated content tables and tests.
    const place = /_tests\.rs$|\/tests(\.rs|\/)/.test(file) ? 0.4
      : /^crates\/app\/src\/world\/factory|^assets\/(machines|recipes|reactions|materials|fabrication)\.ron/.test(file) ? 1.5
      : file.startsWith("assets/") ? 0.6 : 1;
    score.set(file, (score.get(file) ?? 0) + weights.get(word) * place);
  }
  const files = [...score].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, max).map(([file]) => file);
  return { words: used, absent: words.filter(word => !filesOf.get(word).size), files };
}

/**
 * Gather Context hook: briefs for feature IDs named in a task (at most 3).
 * Never throws; a missing or malformed matrix yields no parity context.
 */
export function parityContext(task) {
  const named = [...new Set(String(task).toUpperCase().match(/\b(?:L\d{3}|A\d{2}-\d{2}|A14-\d{3}|A15-\d{2}|B-[A-Z0-9]+-\d{2})\b/g) ?? [])];
  if (!named.length) return undefined;
  try {
    const ledger = loadLedger();
    const rows = named.map(id => ledger.rows.find(row => row.id === id)).filter(Boolean).slice(0, 3)
      .map(row => brief(row, ledger.systems));
    return rows.length ? { rows, rules: RULES, scope: "planning rows, not acceptance evidence" } : undefined;
  } catch { return undefined; }
}

/** Stated once per CLI help / context packet instead of in every brief. */
export const RULES = "Build Axiom's mechanism from `mapping`; never copy mod code, art, names or numbers. Prefer a mode/field on an existing machine kind (new kind/resource = save bump). Content in assets/*.ron. Verify headless: `factory-lab-cli recipes|test`, add a scenario tagged with the ID, then `parity-cli verify <ID>` (runs it and records evidence). R1: resolve the source note first.";

const clip = (text, max) => text.length > max ? text.slice(0, max - 1) + "…" : text;
const present = object => Object.fromEntries(Object.entries(object).filter(([, v]) =>
  v !== undefined && v !== null && v !== "" && !(Array.isArray(v) && !v.length)));

let switchRows;
/**
 * The world-launch system (assets/systems.ron) whose `parity` prefixes own
 * this row id, or undefined for always-on core features. A feature built for
 * a row must sit behind that switch: `world.rules.systems.enabled(id)`.
 */
export function worldSystem(rowId) {
  if (!switchRows) {
    const file = path.join(root, "assets", "systems.ron");
    const text = existsSync(file) ? readFileSync(file, "utf8").replace(/\/\/.*$/gm, "") : "";
    switchRows = [...text.matchAll(/id:\s*"([^"]+)"[\s\S]*?parity:\s*\[([^\]]*)\]/g)]
      .map(([, id, list]) => ({ id, prefixes: [...list.matchAll(/"([^"]+)"/g)].map(m => m[1]) }));
  }
  return switchRows.find(s => s.prefixes.some(prefix => rowId.startsWith(`${prefix}-`)))?.id;
}

/**
 * Work order for one row. Compact (default) is for agents: empty fields
 * omitted, owners as one string each. `full` keeps the structured form for the board.
 */
export function brief(row, systems, full = false) {
  const note = row.sourceNote && existsSync(path.join(root, row.sourceNote)) ? row.sourceNote : null;
  const hints = codeHints(row);
  if (full) {
    return { id: row.id, label: row.label, section: row.section, family: row.family, status: row.status,
      mapping: row.mapping, milestone: row.milestone, cost: row.cost, risks: row.risks, notes: row.notes,
      owners: row.readiness.owners.map(({ id, name, tier, milestone, substrate, reuse }) => ({ id, name, tier, milestone, substrate, reuse })),
      ready: row.readiness.ready, evidence: row.evidence, sourceNote: note, hints, rules: RULES, system: worldSystem(row.id) };
  }
  const state = { legacy: "code", "design-only": "design", unknown: "?" };
  return present({
    id: row.id, label: row.label, status: row.status === "open" ? undefined : row.status,
    ready: row.readiness.ready || undefined, ms: row.milestone, cost: row.cost, risks: row.risks.join(" "),
    mapping: row.mapping, notes: row.notes, system: worldSystem(row.id),
    owners: row.readiness.owners.map(o => `${o.id} ${o.name} [${state[o.substrate]}] ${clip((o.reuse ?? "").replace(/^[LP]:\s*/, ""), 70)}`.trim()),
    files: hints.files.slice(0, 6), absent: hints.absent.join(" ") || undefined,
    note, evidence: row.evidence,
  });
}
