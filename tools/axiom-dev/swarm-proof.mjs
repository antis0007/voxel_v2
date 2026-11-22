// Live engine proof for the parity swarm: pure decisions, no processes. swarm.mjs supplies the build,
// the headless run and the ledger effects, so every rule here is testable without an engine.
//
// Agents may not write under tools/ (the supervisor imports code from there), so a builder returns
// its scenario inside RESULT and the supervisor validates it and writes the file itself.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const OP = /^[a-z][a-z_]{1,39}$/;
const plain = v => v !== null && typeof v === "object" && !Array.isArray(v);

// Test names factory-lab's check() understands; anything else fails only after a full engine build.
const TEST_NAMES = new Set(["eq", "ne", "lt", "le", "gt", "ge", "has", "absent"]);

/**
 * Op names the headless engine accepts, read from the workbench's own `operations` list so the check never
 * drifts from the engine. Returns null when the list cannot be found (then ops are not checked).
 */
export function engineOps(root) {
  try {
    const src = readFileSync(path.join(root, "crates/app/src/dev_panel/engine.rs"), "utf8");
    const list = src.match(/"operations"\s*:\s*\[([^\]]+)\]/);
    if (!list) return null;
    const ops = new Set([...list[1].matchAll(/"([a-z_]+)"/g)].map(m => m[1]));
    return ops.size ? ops : null;
  } catch { return null; }
}

/**
 * Returns null when the agent's scenario is safe to store, otherwise the reason it is rejected. With `ops`
 * (a Set from engineOps) an unknown op is rejected at once, so the builder fixes it in the same turn instead
 * of after the next 40-minute engine build.
 */
/**
 * Agents return the scenario in several shapes: {"steps":[...]}, a bare array of steps (a third of builder
 * results on 2026-11-19, each rejected as "must be an object" and then paid for again as a proof pass), a single
 * step object, or any of those as a JSON string. One shape leaves here; anything else is returned unchanged
 * for validateScenario to reject.
 */
export function normalizeScenario(scenario) {
  let s = scenario;
  if (typeof s === "string") { try { s = JSON.parse(s); } catch { return scenario; } }
  if (Array.isArray(s)) return { steps: s };
  if (plain(s) && !Array.isArray(s.steps) && typeof s.op === "string") return { steps: [s] };
  return s;
}

export function validateScenario(scenario, ops = null) {
  if (!plain(scenario)) return "scenario must be an object";
  const { steps } = scenario;
  if (!Array.isArray(steps) || !steps.length) return "scenario needs at least one step";
  if (steps.length > 12) return "scenario has more than 12 steps";
  let expects = 0;
  for (const [i, step] of steps.entries()) {
    if (!plain(step)) return `step ${i} is not an object`;
    if (typeof step.op !== "string" || !OP.test(step.op)) return `step ${i} has no valid op`;
    if (ops && !ops.has(step.op)) return `step ${i} op "${step.op}" is not an engine op (known: ${[...ops].join(", ")})`;
    if (step.expect !== undefined) {
      if (!plain(step.expect) || !Object.keys(step.expect).length) return `step ${i} expect must be a non-empty object`;
      for (const [key, rule] of Object.entries(step.expect)) {
        if (plain(rule)) for (const test of Object.keys(rule)) if (!TEST_NAMES.has(test)) return `step ${i} expect "${key}" uses unknown test "${test}" (use ${[...TEST_NAMES].join(", ")})`;
      }
      expects++;
    }
    if (step.ticks !== undefined && !(Number.isFinite(step.ticks) && step.ticks >= 0 && step.ticks <= 200_000)) return `step ${i} ticks out of range`;
  }
  if (!expects) return "scenario asserts nothing: add an expect to a step";
  if (JSON.stringify(scenario).length > 6000) return "scenario is larger than 6000 characters";
  return null;
}

/** File name and body the supervisor stores for a row's scenario. One file per row; a retry overwrites it. */
export function scenarioFile(rowId, scenario, note = "") {
  const slug = String(rowId).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const id = "swarm-" + slug;
  return { name: id + ".json", body: JSON.stringify({ id, parity: [String(rowId)], note: String(note).slice(0, 200), steps: scenario.steps }, null, 1) + "\n" };
}

/** Reads every scenario file; one unreadable file is reported instead of failing the whole proof run. */
export function tolerantScenarios(dir, rel = "tools/axiom-dev/scenarios") {
  const scenarios = [], invalid = [];
  if (!existsSync(dir)) return { scenarios, invalid };
  for (const name of readdirSync(dir).filter(n => n.endsWith(".json")).sort()) {
    try {
      const s = JSON.parse(readFileSync(path.join(dir, name), "utf8"));
      if (!Array.isArray(s.steps) || !s.steps.length) throw new Error("steps required");
      scenarios.push({ ...s, id: s.id ?? name.replace(/\.json$/, ""), file: `${rel}/${name}` });
    } catch (error) { invalid.push({ file: `${rel}/${name}`, error: String(error.message).slice(0, 120) }); }
  }
  return { scenarios, invalid };
}

/** Landed, not yet proven rows that have at least one tagged scenario: Map id -> scenarios. */
export function pendingProof(landed, scenarios, proven) {
  const out = new Map();
  for (const entry of landed) {
    if (!/^(building|playable)$/.test(entry.status ?? "") || proven.has(entry.id)) continue;
    const mine = scenarios.filter(s => (s.parity ?? []).includes(entry.id));
    if (mine.length) out.set(entry.id, mine);
  }
  return out;
}

/**
 * Turns run results into one action per row. A row is proven only when every one of its scenarios
 * passed; a first failure is sent back once with the observed values; a second one blocks the row.
 * A scenario the engine never reported on counts as a failure, never as a pass.
 */
export function settleProof(pending, results, retried) {
  const byId = new Map(results.map(r => [r.id, r]));
  const actions = [];
  for (const [id, scenarios] of pending) {
    const files = scenarios.map(s => s.file);
    const why = [], seen = [];
    // "scenario" while every failure is the scenario's own mistake; one wrong value makes it "behaviour".
    let fault = "scenario";
    for (const s of scenarios) {
      const r = byId.get(s.id);
      if (!r) { why.push(`${s.id}: the engine returned no result`); fault = "behaviour"; continue; }
      if (r.pass) continue;
      // fail rows are [step, path, test, want, got]
      for (const f of (r.fail ?? []).slice(0, 3)) why.push(`${s.id} step ${f[0]} ${f[1]}: want ${f[2] && f[2] !== "eq" ? f[2] + " " : ""}${JSON.stringify(f[3])}, engine gave ${JSON.stringify(f[4])}`);
      // The machine's own end state (status, progress, ticks run) for each failed step: without it agents
      // guessed "output appears in a later step" when the batch simply had not finished.
      for (const [step, text] of Object.entries(r.seen ?? {})) seen.push(`step ${step}: ${text}`);
      if (!(r.fail ?? []).length) { why.push(`${s.id}: failed without detail`); fault = "behaviour"; }
      if (!(r.fail ?? []).every(scenarioFault)) fault = "behaviour";
    }
    if (!why.length) { actions.push({ id, kind: "proven", files, summary: `${scenarios.length} scenario(s) passed in the built engine` }); continue; }
    const detail = why.join("; ").slice(0, 280) + (seen.length ? " | engine saw: " + seen.join("; ").slice(0, 420) : "");
    actions.push(retried.has(id)
      ? { id, kind: "blocked", fault, files, summary: "live proof failed twice: " + detail }
      : { id, kind: "retry", fault, files, summary: "live proof failed, sent back: " + detail,
          note: "Your earlier change compiled but FAILED in the running engine. Fix the behaviour, or the scenario if its expectation is wrong. " + detail });
  }
  return actions;
}

/**
 * True when one failure row [step, path, test, want, got] is the scenario's mistake, not the landed code's:
 * the engine refused the request (unknown argument, too many sections: `ok` came back as an error text), or the
 * expected path is not in the reply at all and the scenario did not expect a number there (absent numbers read
 * as 0, so a missing number can be real behaviour). Most blocked proofs on 2026-11-19 were this kind (a
 * "position_m" that is really "feet_m", too many inspect sections), and each had been sent to a builder twice
 * to "fix" code that was not wrong.
 */
export function scenarioFault(f) {
  const [, key, test, want, got] = f;
  if (got === "unknown test") return true;
  if (key === "ok") return typeof got === "string";
  // "has" on an object can never pass (it reads strings and arrays): {"player":{"has":"position_m"}}.
  if (test === "has" && got !== null && typeof got === "object" && !Array.isArray(got)) return true;
  return (got === null || got === undefined) && typeof want !== "number" && test !== "absent";
}

/**
 * The engine refuses to start when a content table is inconsistent ("processes: CokeOven recipe 2: ...").
 * Names the table and the unproven landed rows that edited it, so the error goes to its likely author
 * instead of failing every proof. No row touched it: ids is empty and the caller reports a stale build.
 */
export function blameContent(message, landed, proven) {
  const table = (String(message).match(/^\s*([a-z_]{3,30}):\s/) ?? [])[1] ?? null;
  if (!table) return { table: null, ids: [] };
  const file = `assets/${table}.ron`;
  const ids = [...landed].filter(e => /^(building|playable)$/.test(e.status ?? "") && !proven.has(e.id) && (e.files ?? []).some(f => f.replace(/\\/g, "/") === file)).map(e => e.id);
  return { table, ids };
}

/** Prove now, or wait for more rows? Building the engine pauses the agents, so small batches wait. */
export function shouldProve({ pending, batch, manual, idle }) {
  if (!pending) return false;
  return manual || idle || pending >= Math.max(1, batch);
}
