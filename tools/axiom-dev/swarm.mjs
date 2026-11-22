// Parity swarm: N concurrent headless Claude Code agents, each implementing one ledger row
// end to end (investigate, edit, cargo check) in the shared tree, plus a live supervisor UI.
//
//   node tools/axiom-dev/swarm.mjs [--workers 3] [--family f] [--port 4377] [--max-rows 0]
//        [--sonnet sonnet] [--opus opus] [--haiku haiku] [--fable fable] [--fable-rows id,id|none] [--no-scout] [--explore-turns 15] [--reopen] [--ready-only]
//        [--timeout-min 25] [--dry] [--start-paused] [--budget-usd 0] [--checkpoint-every 8] [--no-context]
// - Context: each row gets context-cli evidence from its likely files (no model) in the scout and
//   builder prompts. UI: live Settings (agents, cap, family, toggles), per-row Log/Diff/Retry/Park,
//   Do next / Park on the queue, Lessons and Parked tabs; settings persist only for this process.
//
// - Rows are chosen by the same ranking as `parity-cli next`; two active agents never share a
//   likely file; ready rows rank first, then rows whose owner system the agent builds (POLICY),
//   unless --ready-only. Cargo's own build lock serialises `cargo check`; reading/editing runs in parallel.
// - Cost ladder: a read-only haiku scout (15 turns, $0.20) settles already-implemented and
//   design-blocked rows and hands the builder owner files + plan; builder is sonnet/low (32 turns,
//   $1); ONE escalation to opus/medium unless the failure is a design/spec block. A builder with no
//   edit after --explore-turns turns is stopped. Blocked rows are not reopened on restart without --reopen.
// - Live engine proof: builders of machine rows return a headless scenario in RESULT; the supervisor
//   validates and stores it (agents cannot write under tools/). After a checkpoint compiles and
//   --prove-batch rows wait (or the swarm is idle, or Prove now is pressed), it builds the engine once,
//   runs the scenarios in it and sends any failure back to the row as ONE retry with the observed
//   values. A content-table error goes to the rows that edited that table. No model cost. --no-prove: off.
// - Robustness: one swarm per tree (lock file), atomic state writes, and a circuit breaker that pauses
//   after --breaker rows in a row fail to land (0 = off). Never stuck: low disk first reclaims stale cargo
//   incremental caches and old logs; a red tree triggers a checkpoint repair and builders start anyway
//   after 20 minutes; checkpoints drain only agents with a live process.
// - Tool contract: agents get the exact Bash forms the sandbox allows (denials used to burn ~1 turn in 6);
//   denied calls do not count toward the no-edit cap. Killed runs report an estimated cost from stream usage.
//   The supervisor, not the agent, appends changes.md from RESULT (incl. its "tests" field).
// - Agents never touch the ledger or git. The supervisor records `building` with the files touched.
// - A usage limit pauses the swarm until the provider's stated reset (scout, builder and design runs alike).
// - Design stage (swarm-design.mjs): rows that wait on a decision nobody wrote (needs-design verdicts, owner
//   systems the plan lists as design-only, epics) go to a read-only design lead that designs from the tree, the
//   docs and its own knowledge, inside the project rules. Designs are saved under <state>/designs and handed to
//   the builder. --design-model m (default: the opus model), --decide-max N designs an hour (default 24),
//   --no-design-epics leaves epics to the owner, --no-decide turns the stage off.
// UI: http://127.0.0.1:<port>
import { spawn, spawnSync } from "node:child_process";
import { appendFileSync, closeSync, existsSync, fstatSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, renameSync, rmSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import path from "node:path";
import { FAMILIES, brief, filterRows, isEngineRow, loadLedger, nextRows, writeStatus } from "./parity-data.mjs";
import { root, buildCommand, runScenarios } from "./factory-lab.mjs";
import { blameContent, engineOps, normalizeScenario, pendingProof, scenarioFile, settleProof, shouldProve, tolerantScenarios, validateScenario } from "./swarm-proof.mjs";
// Read once at start; a restart picks up ops the engine gained since.
const ENGINE_OPS = engineOps(root);
import { createBrain } from "./swarm-brain.mjs";
import { APP, killChecks, runCheck, runTables, settleOnce, treeKill, validPackage } from "./swarm-check.mjs";
import { planView } from "./swarm-plan.mjs";
import { PERF_IDS, ensureOwnerRows, ensurePerfRows, feedChores, feedPerf, perfNextAt } from "./swarm-feeder.mjs";
import { DESIGN_VERSION, createDesignStore, designNote, designPrompt, designTargets, noteOf, ownerKey, parseDesign } from "./swarm-design.mjs";

const argv = process.argv.slice(2);
const opt = (name, fallback) => { const i = argv.indexOf("--" + name); return i < 0 ? fallback : argv[i + 1]; };
const flag = name => argv.includes("--" + name);
const family = opt("family");
if (family && !(family in FAMILIES)) { console.error("Unknown family " + family); process.exit(2); }
const cfg = { workers: Math.min(16, Math.max(1, Number(opt("workers", 5)))), port: Number(opt("port", 4377)),
  maxRows: Number(opt("max-rows", 0)), timeoutMs: Number(opt("timeout-min", 25)) * 60_000,
  sonnet: opt("sonnet", "sonnet"), opus: opt("opus", "opus"), haiku: opt("haiku", "haiku"), family: family ?? "all",
  scout: !flag("no-scout"), readyOnly: flag("ready-only"), budget: Number(opt("budget-usd", 0)),
  checkpointEvery: Number(opt("checkpoint-every", 8)), context: !flag("no-context"), prove: !flag("no-prove"), proveBatch: Number(opt("prove-batch", 3)), breaker: Number(opt("breaker", 6)), exploreTurns: Number(opt("explore-turns", 15)), wire: !flag("no-wire"), maxCont: Number(opt("max-cont", 4)), maxContDry: Number(opt("max-cont-dry", 8)), familyFallback: !flag("no-family-fallback"),
  chores: !flag("no-chores"), perfFeed: !flag("no-perf-feed"), decide: !flag("no-decide"), decideMax: Number(opt("decide-max", 24)), designEpics: !flag("no-design-epics"), designParallel: 3, proofPerHour: Number(opt("proof-per-hour", 12)), proofDebt: Number(opt("proof-debt", 40)),
  budgetWindowH: Math.max(0, Number(opt("budget-window-h", 0)) || 0) };
// Live settings outlive a restart: a value changed in Settings is reloaded unless its flag is on the command line.
// Each key names the flag that overrides it.
const SETTING_FLAGS = { workers: "workers", family: "family", budget: "budget-usd", scout: "no-scout", readyOnly: "ready-only", context: "no-context",
  prove: "no-prove", wire: "no-wire", decide: "no-decide", designEpics: "no-design-epics", decideMax: "decide-max", exploreTurns: "explore-turns",
  checkpointEvery: "checkpoint-every", proveBatch: "prove-batch", breaker: "breaker", proofPerHour: "proof-per-hour", budgetWindowH: "budget-window-h" };
// Cost ladder (history: failed runs cost as much as successes, and turns drive cost because every
// turn re-reads context). A haiku scout triages first; the builder starts cheap with a tight turn
// cap; ONE escalation to opus, only for failures a stronger model can fix.
const tiers = [
  // Caps bound the tail only: sonnet runs average 8.8 turns, landed opus runs 21-28.
  { name: "sonnet·low", model: cfg.sonnet, effort: "low", maxTurns: 24, budget: 0.5 },
  // Opus escalation landed 0 of 3 (2026-11-18); opt in with --escalate.
  ...(flag("escalate") ? [{ name: "opus·medium", model: cfg.opus, effort: "medium", maxTurns: 40, budget: 1.2 }] : []),
];
// Fable rows: determinism-, layout- or save-sensitive rows where a wrong guess costs more than the model.
// Listed rows skip the sonnet start and the opus escalation and build on Fable directly; every other row
// stays on the cheap ladder. --fable-rows id,id replaces the list; --fable-rows none turns it off.
// Owner 2026-11-18: Fable only for determinism/thread-safety rows; capped low so a row costs <= $1.2.
const FABLE_DEFAULT = "ENG-SIM-16,ENG-SIM-11,ENG-JOB-10";
cfg.fable = opt("fable", "fable");
cfg.fableRows = new Set(String(opt("fable-rows", FABLE_DEFAULT)).split(",").map(s => s.trim()).filter(s => s && s !== "none"));
const fableTier = { name: "fable·low", model: cfg.fable, effort: "low", maxTurns: 25, budget: 1.2 };
// Owner-system rows (ENG-OWN-*, swarm-feeder ensureOwnerRows) build a whole subsystem from a design: owner
// 2026-11-19 asked for the most capable model there, so they run on Fable at medium effort with room to finish.
// --no-owner-fable sends owner rows down the cheap sonnet ladder instead (2026-11-20: owner rows were ~95% of spend).
cfg.ownerFable = !flag("no-owner-fable");
const isOwnerRow = id => cfg.ownerFable && /^ENG-OWN-/.test(id);
// exploreTurns: a subsystem build reads its design and several owner files before the first edit; the 15-call
// cap (tool calls, and Fable issues them in parallel) stopped all 9 owner rows on 2026-11-19 within 35 s.
const ownerTier = { name: "fable·owner", model: cfg.fable, effort: "medium", maxTurns: 60, budget: 4, exploreTurns: 45 };
const isFableRow = id => cfg.fableRows.has(id) || isOwnerRow(id);
const fableLimited = () => (state?.fableUntil ?? 0) > Date.now();
cfg.fableRepair = !flag("no-fable-repair");
// Wire passes add one call site to code that already compiles: haiku (~$0.004 a pass vs ~$0.09 on sonnet).
// --wire-model sonnet uses sonnet instead.
const wireTier = { name: "haiku·wire", model: cfg.haiku, effort: "low", maxTurns: 16, budget: 0.2 };
// Data-only rows (the scout names only assets/*.ron files) start on haiku; anything that does not land goes
// to a normal sonnet run with haiku's report, so the worst case adds ~$0.005. --no-haiku-data turns it off.
const dataTier = { name: "haiku·data", model: cfg.haiku, effort: "low", maxTurns: 16, budget: 0.2 };
cfg.haikuData = !flag("no-haiku-data");
const dataOnly = files => files?.length > 0 && files.every(f => /^assets\/[\w./-]+\.ron$/.test(f));
// Haiku wire passes wired 0 of 4 (2026-11-18): sonnet is the default; --wire-model haiku opts back in.
cfg.wireModel = opt("wire-model", "sonnet") === "haiku" ? "haiku" : "sonnet";
const scoutTier = { name: "haiku·scout", model: cfg.haiku, effort: "low", maxTurns: 10, budget: 0.15, tools: "Read,Grep,Glob", scout: true };
// Design lead: read-only, the strongest judgment in the ladder, because one design steers every build that
// follows it. The 48 earlier decisions cost $0.044 each on this model; a design reads a little more.
cfg.designModel = opt("design-model", cfg.opus);
const designTier = { name: (String(cfg.designModel).match(/[a-z]+/)?.[0] ?? "opus") + "·design", model: cfg.designModel, effort: "medium", maxTurns: 14, budget: 0.8, tools: "Read,Grep,Glob", scout: true };
// A stronger model cannot invent a missing spec, owner decision or protocol: those stop at once.
const designBlock = /\b(spec|design decision|owner decision|needs? (a |an )?(decision|design|approval)|protocol|lore|proper noun)\b/i;
// Scout needs-design verdicts quote "[design]" owners, which designBlock misses; Requeue used to pay a builder for each.
const isDesignBlock = r => r?.why === "design" || /^scout:/.test(r?.summary ?? "") || designBlock.test(r?.summary ?? "");

// --state-dir lets a test instance run beside the live swarm without sharing its results.
const dir = opt("state-dir") ? path.resolve(opt("state-dir")) : path.join(root, "target", "axiom-dev", "swarm");
mkdirSync(path.join(dir, "logs"), { recursive: true });
const resultsFile = path.join(dir, "results.jsonl");
const stateFile = path.join(dir, "state.json");
const settingsFile = path.join(dir, "settings.json");
const spendFile = path.join(dir, "spend.json");
const readJson = (file, fallback) => { try { return JSON.parse(readFileSync(file, "utf8")); } catch { return fallback; } };
{
  const saved = readJson(settingsFile, {});
  for (const [key, flagName] of Object.entries(SETTING_FLAGS)) {
    if (!(key in saved) || argv.includes("--" + flagName)) continue;
    const v = saved[key];
    if (key === "family" ? (v === "all" || v in FAMILIES) : typeof v === typeof cfg[key]) cfg[key] = v;
  }
  cfg.workers = Math.min(16, Math.max(1, Number(cfg.workers) || 1));
}
const saveSettings = () => { try { writeFileSync(settingsFile, JSON.stringify(Object.fromEntries(Object.keys(SETTING_FLAGS).map(k => [k, cfg[k]])))); } catch { /* best effort */ } };
// The spend cap is cumulative across restarts: totals.cost is this process only, spend.total is every run since
// the last reset (Settings "reset spend"). A restart used to zero the counter and silently double the cap.
const spend = { total: 0, since: Date.now(), ...readJson(spendFile, {}) };
const saveSpend = () => { try { writeFileSync(spendFile + ".tmp", JSON.stringify(spend)); renameSync(spendFile + ".tmp", spendFile); } catch { /* best effort */ } };
// Optional rolling window (--budget-window-h N or Settings budgetWindowH): the cap then applies per N hours, so an
// unattended swarm pauses at the cap and resumes by itself when the window rolls. 0 keeps it cumulative until
// "reset spend". The window starts at spend.since, which survives restarts like the total does.
const budgetWindowMs = () => cfg.budgetWindowH > 0 ? cfg.budgetWindowH * 3600_000 : 0;
function rollSpend() {
  const win = budgetWindowMs();
  if (!win || Date.now() - spend.since < win) return;
  event(`budget window rolled: $${spend.total.toFixed(2)} spent in the last ${cfg.budgetWindowH} h`);
  spend.total = 0; spend.since = Date.now(); saveSpend();
  if (state.paused?.startsWith("budget cap")) state.paused = null;
}
const overBudget = () => cfg.budget > 0 && spend.total >= cfg.budget;
const finished = new Map(); // id -> last result (only terminal ones block re-pick)
if (existsSync(resultsFile)) for (const line of readFileSync(resultsFile, "utf8").split("\n").filter(Boolean)) {
  try {
    const r = JSON.parse(line);
    if (!r.final) continue;
    // Rows blocked only because their proof scenario asked the engine the wrong question (a reply field that
    // does not exist, a request the engine refused) compiled and landed; before 2026-11-19 such a mismatch was
    // sent to a builder twice and then recorded as blocked. Keep the landed result and let the read-only proof
    // pass write one new scenario with the engine's answer in hand (rescript() does the same for new runs).
    const landed = finished.get(r.id);
    const mismatch = r.tier === "engine·proof" && r.status === "blocked" && /^live proof failed twice: /.test(r.summary ?? "")
      && /step \d+ ok: want true, engine gave "|engine gave null/.test(r.summary) && !/: want (?:\w+ )?-?\d/.test(r.summary);
    if (mismatch && landed && /^(building|playable)$/.test(landed.status ?? "")) {
      if (!landed.proofRescripted) {
        const slug = String(r.id).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
        if (!flag("dry")) { try { rmSync(path.join(root, "tools/axiom-dev/scenarios", `swarm-${slug}.json`), { force: true }); } catch { /* gone */ } }
        finished.set(r.id, { ...landed, scenario: undefined, proofPass: undefined, proofRetried: undefined, needsProof: true, proofRescripted: true,
          proofWhy: "it ran in the built engine and the reply did not have what it expected: " + r.summary.replace(/^live proof failed twice: /, "").slice(0, 260) });
      }
      continue;
    }
    finished.set(r.id, r);
  } catch { /* skip */ }
}

// --allow-save-bump: rows earlier blocked only by the save-format rule get ONE serialized retry on
// the stronger tier. Rows needing a new spec, a vertex/WGSL field or a network protocol stay blocked.
const allowBump = !flag("no-save-bump"); // owner policy: save bumps are approved by default
const bumpRows = new Set();
const reopened = new Set();
const reopenedRows = new Map(); // id -> last terminal result, for the reopen decision below
const POLICY = [
  "OWNER POLICY (standing approval): a missing owner is never a reason to stop; build the minimal owner in the same patch.",
  "BUILD ORDER: keep the tree compiling. When you add a struct field, enum variant or fn parameter, rg every constructor, match and call site first and edit them in consecutive Edit calls BEFORE running cargo check, so peers never see a half-added field. Run cargo check once at the end of the slice, not after each edit.",
  "SAVE FORMAT: save bumps are approved. Re-read SAVE_VERSION in source; if a peer already raised it this session (git diff shows the constant changed) reuse that number, else bump by exactly one. Append new fields at the END of their records gated by version; old saves must decode with defaults; never renumber or reorder ids. Prefer a mode/field on an existing kind when it fits.",
  "GENERATOR GATES: never edit a historical gate. New worldgen behaviour gets a new pub const gate in the 600-699 range reserved for swarm work: rg the whole repo (crates, docs, branches via git grep) for the number first, add its row to docs/worldgen/GENERATOR_GATES.md, and leave it dormant (not CURRENT_GENERATOR_VERSION).",
  "SHADERS: a vertex, uniform or bind-group change edits the Rust struct and the WGSL declaration in the same patch; check indices, sizes and that new .wgsl files are listed in shader_sources.rs. Any look change is opt-in or identical to the current output by default; the owner has rejected unverified visual changes, so never alter existing art, palettes or shading silently.",
  "ART/MESH CONFLICTS: when a feature fights greedy merging or existing art, add the data, flag or variant that keeps the existing output byte-identical and apply the new behaviour only where it is switched on.",
  "NAMES: use plain descriptive names; invent no new lore, proper nouns or player-visible story text.",
  "SCOPE: land the first playable slice (up to 300 changed lines) and put the remainder in the summary. Never touch files another agent is editing right now beyond your own hunks; never revert a peer.",
  "RED TREE: other agents edit live, so cargo check can fail in code you never touched. If every error is outside your hunks, wait 60 s and re-run up to 6 times; if it stays red, report your slice as building with the exact foreign error quoted, not blocked.",
  "ALREADY-IMPLEMENTED needs evidence: name the file and symbol and what in it satisfies the row, or build the missing part instead.",
  "LOG: do not write any log file; the supervisor writes target/axiom-dev/swarm/changes.md from your RESULT. Put the tests the owner should run in RESULT \"tests\" (required for save, gate, shader-contract or new-kind changes).",
  "WIRING: a slice counts only when a player action, a tick or a loaded table reaches it. Before reporting, wire the smallest real caller (one call site or one data row read). Say 'not wired' in the summary only when no caller was possible, because unwired scaffolding is not counted as landed value.",
  "HONESTY: only cargo check is run here. State plainly what was not tested; never claim a feature works in game.",
  "Report blocked ONLY if cargo check still fails after two fix attempts on your own hunks, quoting the exact compiler error."
].join(" ");
const BUMP_NOTE = "OWNER APPROVED a save-format change for this row (up to 3 bump agents run at once and share ONE unreleased version). Follow .claude/rules/saves.md: re-read the current SAVE_VERSION in source; if another agent already raised it this session (git diff shows the constant changed) reuse that exact number and do NOT bump again, otherwise bump it by exactly one; append new fields/kinds at the END of their records gated by version so every older save still decodes with defaults, never reorder or renumber existing ids, claim nothing already used. Prefer a mode/field on an existing kind when it works; bump only when it does not. Keep it under 250 changed lines. Cargo check is your only verification; say in the summary exactly which save files and version you changed so the owner can run the migration tests.";
if (allowBump) for (const [id, r] of [...finished]) {
  if (r.status === "blocked" || r.status === "failed") { finished.delete(id); reopened.add(id); reopenedRows.set(id, r); if (/save|bump|migration/i.test(r.summary)) bumpRows.add(id); }
}
const history = [...finished.values()].slice(-60).reverse();
const state = { startedAt: Date.now(), cfg, paused: null, stop: false, workers: [], results: history, totals: { in: 0, out: 0, cost: 0, done: 0, blocked: 0, cached: 0, proven: 0 }, tierStats: {}, spend };
// A pause outlives a dashboard restart: state.json is never read back, so without this a paused swarm resumed
// spending as soon as the new code started.
const keepPausedFile = path.join(dir, "restart-paused.json");
try { if (existsSync(keepPausedFile)) { state.paused = String(JSON.parse(readFileSync(keepPausedFile, "utf8")).paused || "paused") + " (kept across restart)"; unlinkSync(keepPausedFile); } } catch { /* start unpaused */ }
// A breaker cooldown timer dies with the process: re-arm one (30 min) for a breaker pause carried across a restart,
// or the restarted swarm would wait for Resume. A red-tree breaker pause is left to the health gate.
if (/^circuit breaker\b/.test(state.paused ?? "") && !/does not compile/.test(state.paused)) setTimeout(() => {
  if (!/^circuit breaker\b/.test(state.paused ?? "")) return;
  state.paused = null; event("circuit breaker: cooldown over after restart; resumed");
}, 30 * 60_000);
// Every model cost goes through here so the per-process total and the cumulative cap ledger never drift.
// Restarts used to show "0 proven" although proven results are on disk.
state.totals.proven = [...finished.values()].filter(r => r.status === "proven").length;
// Where landed rows stand on proof, so "0 proven" is never mistaken for "proof is broken".
Object.defineProperty(state, "proof", { enumerable: true, get: () => { try {
  const n = { landed: 0, scenario: 0, waiting: 0, none: 0, proven: 0, hour: proofsThisHour(), cap: cfg.proofPerHour };
  for (const r of finished.values()) {
    if (r.status === "proven") { n.proven++; continue; }
    if (!/^(building|playable)$/.test(r.status)) continue;
    n.landed++;
    if (r.scenario || taggedIds().has(r.id)) n.scenario++; else if (r.noProof) n.none++; else n.waiting++;
  }
  return n;
  } catch { return null; } // read before the proof helpers below are initialised (early persist)
} });
const addCost = c => { const v = Number(c) || 0; if (!v) return v; state.totals.cost += v; spend.total += v; saveSpend(); return v; };
const retry = new Map(); // id -> next tier
const wireStats = { n: 0, wired: 0 };
let sinceCheckpoint = 0; // landed rows + wire passes since the last auto checkpoint
const forced = new Set(); // rows already given their one narrower sonnet retry after a zero-edit failure
let failStreak = 0, idleProved = false, breakerTrips = 0, breakerTimer = null;
const lastFail = new Map(); // id -> previous failure summary; an identical repeat is deterministic, so do not pay to retry it
const sameFailure = (a, b) => { const x = new Set(String(a).toLowerCase().match(/[a-z_]{4,}/g) ?? []), y = new Set(String(b).toLowerCase().match(/[a-z_]{4,}/g) ?? []); let h = 0; for (const w of x) if (y.has(w)) h++; return h / Math.max(1, Math.min(x.size, y.size)) > 0.8; };
const parkedFile = path.join(dir, "parked.json");
const parked = new Set((() => { try { return JSON.parse(readFileSync(parkedFile, "utf8")); } catch { return []; } })());
const saveParked = () => { try { writeFileSync(parkedFile, JSON.stringify([...parked])); } catch { /* best effort */ } };
const priority = []; // row ids the supervisor pushed to the front
ensurePerfRows({ root });
if (!flag("no-perf-first")) priority.push(...PERF_IDS); // measured performance rows lead the queue (--no-perf-first to skip)
const familyOf = () => (cfg.family === "all" ? undefined : cfg.family);
// POLICY is already in every prompt; the note carries only the reason. Retries start on the cheap tier.
const requeue = (id, tier, why) => { finished.delete(id); retry.set(id, tier); retry.set(id + ":note", why ?? ""); };
// Startup no longer reopens every blocked row onto opus (that re-bought each failure on every restart).
// Pass --reopen to retry blocked rows that were not design blocks; the Requeue button does the same.
for (const id of reopened) {
  const r = reopenedRows.get(id);
  if (flag("reopen") && !bumpRows.has(id) && !isDesignBlock(r)) requeue(id, 0, "Earlier attempt reported blocked: " + String(r?.summary ?? "").slice(0, 200));
  else { finished.set(id, r ?? { id, status: "blocked", final: true }); reopened.delete(id); }
}
// Save-bump retries (opus) also run only with --reopen; POLICY is already in the prompt.
for (const id of bumpRows) {
  if (flag("reopen")) { retry.set(id, 1); retry.set(id + ":note", BUMP_NOTE); }
  else { finished.set(id, reopenedRows.get(id) ?? { id, status: "blocked", final: true }); }
}
if (!flag("reopen")) bumpRows.clear();
// In-flight work outlives a restart: rows waiting to retry (with their scout plan or failure note) and rows a
// worker held when the process stopped come back first, with what that worker had done. A restart used to drop
// both, and the rows started from scratch (scout and exploration paid again).
const inflightFile = path.join(dir, "inflight.json");
const saveInflight = () => {
  try {
    const held = state.workers.filter(w => w.row && w.status !== "idle" && !finished.has(w.row)).map(w => ({ id: w.row, tier: w.tier,
      files: (w.files ?? []).slice(0, 8), last: (w.activity ?? []).slice(-4).map(a => String(a.text).slice(0, 160)) }));
    const waiting = [...retry.entries()].filter(([k]) => !k.endsWith(":note")).map(([id, tier]) => ({ id, tier, note: String(retry.get(id + ":note") ?? "").slice(0, 1500) }));
    writeFileSync(inflightFile + ".tmp", JSON.stringify({ at: Date.now(), held, waiting, resumeAt: state.resumeAt ?? null }));
    renameSync(inflightFile + ".tmp", inflightFile);
  } catch { /* best effort */ }
};
{
  const saved = readJson(inflightFile, null);
  let n = 0;
  for (const w of saved?.waiting ?? []) {
    if (finished.has(w.id) || retry.has(w.id)) continue;
    retry.set(w.id, Number(w.tier) || 0); retry.set(w.id + ":note", w.note ?? ""); n++;
  }
  for (const h of saved?.held ?? []) {
    if (finished.has(h.id) || retry.has(h.id)) continue;
    requeue(h.id, 0, (`Resumed after a supervisor restart; the earlier run stopped mid-row.${h.files?.length ? " It had edited " + h.files.join(", ") + ": read those hunks first and finish them, do not redo them." : ""}${h.last?.length ? " Its last notes: " + h.last.join(" | ") : ""}`).slice(0, 1200));
    if (!priority.includes(h.id)) priority.unshift(h.id);
    n++;
  }
  // A provider usage limit outlives the restart too; agents launched before it lifts only fail.
  if (saved?.fableUntil > Date.now()) state.fableUntil = saved.fableUntil;
  if (saved?.resumeAt && saved.resumeAt > Date.now()) {
    state.paused = "usage limit hit; resumes " + new Date(saved.resumeAt).toLocaleString(); state.resumeAt = saved.resumeAt;
    setTimeout(() => { if (String(state.paused ?? "").startsWith("usage limit")) { state.paused = null; state.resumeAt = null; } }, Math.max(60_000, saved.resumeAt - Date.now())).unref?.();
  }
  if (n) console.log(`resumed ${n} in-flight row(s) from the last run`);
  // Workers change rows without a persist; a hard kill must still find the row it held.
  if (!flag("dry")) setInterval(saveInflight, 15_000).unref();
}
// rev: results, events, settings; act: worker activity. The broadcaster pushes both to open pages.
let rev = 0, act = 0;
const logTail = (w, text) => { act++; w.activity.push({ t: Date.now(), text }); if (w.activity.length > 60) w.activity.shift(); };
// Atomic: a crash mid-write must not leave a truncated state file for the next start.
const persist = () => { rev++; try { writeFileSync(stateFile + ".tmp", JSON.stringify(state)); renameSync(stateFile + ".tmp", stateFile); } catch { /* ui still has memory */ } try { saveInflight(); } catch { /* defined below; early persist */ } };
// One swarm per tree: two supervisors would hand the same rows to two sets of agents.
const lockFile = path.join(dir, "swarm.lock");
if (!flag("dry")) {
  let other = 0;
  try { other = Number(readFileSync(lockFile, "utf8")); } catch { /* no lock */ }
  if (other && other !== process.pid) {
    let alive = false;
    try { process.kill(other, 0); alive = true; } catch (e) { alive = e.code === "EPERM"; }
    if (alive) { console.error(`Another swarm (pid ${other}) is already working this tree. Stop it first, or delete ${lockFile} if that process is not a swarm.`); process.exit(3); }
  }
  writeFileSync(lockFile, String(process.pid));
  process.on("exit", () => { try { if (Number(readFileSync(lockFile, "utf8")) === process.pid) unlinkSync(lockFile); } catch { /* already gone */ } });
  for (const sig of ["SIGINT", "SIGTERM", "SIGBREAK"]) process.on(sig, () => process.exit(0));
}
state.events = [];
if (flag("start-paused")) state.paused = "started paused; press Resume";
const event = text => { rev++; state.events.unshift({ t: Date.now(), text }); if (state.events.length > 40) state.events.pop(); console.log(new Date().toISOString(), text); };
process.on("unhandledRejection", e => event("internal error (kept running): " + String(e?.message ?? e).slice(0, 160)));
process.on("uncaughtException", e => { event("fatal: " + String(e?.message ?? e).slice(0, 160)); setTimeout(() => process.exit(1), 300); });
for (const h of history) tierRoi(h); // the Cost panel starts from saved results, not from zero
for (const h of history) { if (/blocked|failed|escalated/.test(h.status ?? "") && !lastFail.has(h.id)) lastFail.set(h.id, h.summary ?? ""); }
const brain = createBrain({ dir, root, childEnv: () => childEnv(), event });
// Designs and owner decisions made so far (decisions.jsonl + designs/). See the design stage below.
const designs = createDesignStore(dir);
// Owner systems with a design get a build row (ENG-OWN-*), built on Fable (see ownerTier). --no-owner-rows: off.
if (!flag("no-owner-rows")) {
  try {
    const owners = [...loadLedger().systems.values()]
      .filter(s => s.substrate === "design-only" && designs.get(ownerKey(s.id))?.verdict === "decide")
      .map(s => ({ id: s.id, name: s.name, milestone: s.milestone, prerequisites: s.prerequisites ?? [],
        design: path.relative(root, designs.pathOf(ownerKey(s.id))).replaceAll("\\", "/") }));
    const added = ensureOwnerRows({ root, owners });
    if (added) event(`feeder: ${added} owner-system build row(s) added (ENG-OWN-*, Fable)`);
  } catch (e) { event("owner rows not added: " + String(e?.message ?? e).slice(0, 120)); }
}
// A design is only worth its cost once a builder has used it. A restart used to forget that a decided row was
// waiting for its build (the retry list lives in memory), so the row stayed blocked for good. Queue those again.
for (const d of designs.all()) {
  if (d.verdict !== "decide" || d.kind === "owner" || parked.has(d.id)) continue;
  const f = finished.get(d.id);
  if (f ? f.status === "blocked" && (f.at ?? 0) < d.at : d.kind === "epic") requeue(d.id, 0, noteOf(d));
}

// Bash is limited to cargo check and read-only git. Search uses the built-in Grep tool, not rg
// (rg --pre runs arbitrary programs). Build scripts and cargo config are protected because
// cargo check executes build.rs and proc macros.
// Exact commands only: a prefix rule such as `git diff:*` also admits --output / --ext-diff and `cargo check:*`
// admits --config (rustc wrapper injection). Deny rules win over allow rules, so flags that write files or run
// programs are denied outright even where a wildcard allow would otherwise match.
const SAFE_TOOLS = ["Read", "Grep", "Glob", "Edit", "Write", "Bash(cargo check)", "Bash(cargo check -p *)", "Bash(cargo check --message-format short)",
  "Bash(node tools/axiom-dev/swarm-check.mjs)", "Bash(node tools/axiom-dev/swarm-check.mjs *)",
  "Bash(git status)", "Bash(git status --short)", "Bash(git diff)", "Bash(git diff --stat)", "Bash(git diff -- *)"];
// The supervisor imports and spawns tools/axiom-dev/*.mjs (parity-data, context-cli) and git runs
// .git hooks, so agents may not write any of them. Deny both Edit and Write.
const DENY_GLOBS = ["**/build.rs", "**/Cargo.toml", "**/Cargo.lock", ".cargo/**", "**/.cargo/**", ".claude/**",
  "tools/**", ".git/**", "**/.git/**", "scripts/**", "*.cmd", "*.ps1", "**/*.ps1", "justfile", ".githooks/**",
  // cargo check also runs whatever toolchain and vendored build scripts the tree names.
  "rust-toolchain", "rust-toolchain.toml", "vendor/**"];
const DENY_TOOLS = DENY_GLOBS.flatMap(g => [`Edit(${g})`, `Write(${g})`]).concat(["Bash(git add:*)", "Bash(git commit:*)", "Bash(git push:*)", "Bash(curl:*)", "Bash(powershell:*)",
    "Bash(*--config*)", "Bash(*--output*)", "Bash(*--ext-diff*)", "Bash(*--manifest-path*)", "Bash(*--target-dir*)", "Bash(*RUSTC*)", "Bash(*&&*)", "Bash(*;*)", "Bash(*|*)"]);
// Child env: drop the parent session's CLAUDE_CODE_* wiring and anything that looks like a secret.
const childEnv = () => ({ ...Object.fromEntries(Object.entries(process.env).filter(([k]) =>
  /^ANTHROPIC_(API_KEY|AUTH_TOKEN)$/.test(k) || (!/^CLAUDE_CODE_/.test(k) && !/(TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL|PRIVATE)|_KEY$/i.test(k)))),
  RUSTC_WRAPPER: "", CARGO_BUILD_RUSTC_WRAPPER: "", RUSTC_WORKSPACE_WRAPPER: "", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: "" });

// History: ~700 tool calls across 600 logs were permission denials (rg/grep/cd/pipes through Bash, echo >> changes.md).
// Each one costs a full-context turn and counts toward the no-edit cap, so the exact allowed forms are stated up front.
const TOOL_RULES = "TOOLS: search with the Grep and Glob tools, read with Read, edit with Edit/Write; paths are relative to the repo root (already the cwd). Bash accepts ONLY these exact forms: `node tools/axiom-dev/swarm-check.mjs [package]` (the compile check: it shares one cargo run with the other agents and prints only errors, or OK; default package is the app), `git status --short`, `git diff -- <path>`. No cd, no pipes, no 2>&1, no &&/;, no cargo/rg/grep/sed/cat/echo/mkdir/python through Bash: everything else is denied or wasted. The check can take several minutes; run it once and wait for it. Every tool result is re-sent on each later turn, so Read with offset/limit around a Grep -n hit (at most 150 lines), never a whole large file, and never re-read a range you already have.";
const SYSTEM = "You are a fast Rust/wgpu voxel-game coding agent in a shared tree. Be terse; no preamble. " + TOOL_RULES + " Rules: no unwrap/expect outside tests; no content tables in Rust (use assets/*.ron); simulation uses fixed steps and seeded hashes, no wall-clock; no new dependencies; player text follows crates/app/src/egui_ui/UI_RULES.md and adds no new proper nouns; host/WGSL structs are one contract.";
// One proof rule for both build prompts (ENGINE_TASK rows never saw the old machine-only rule, so 0 engine rows
// carried a scenario). Ops come from the workbench's own list; validateScenario rejects anything else at landing.
const SCENARIO_RULE = `Make it provable: add "scenario":{"steps":[...]} to RESULT (keep RESULT on ONE line). The supervisor stores it, runs it headless in the real engine after the next build (a "reset" runs first) and returns any mismatch to you. Each step is an engine op request plus an optional "expect" {"dotted.path.in.reply.data": value | {"eq|ne|lt|le|gt|ge|has|absent": value}}; "sum:a.b+c.d" adds paths. Ops: ${ENGINE_OPS ? [...ENGINE_OPS].join(", ") : "see tools/axiom-dev/ENGINE_WORKBENCH.md"}. Machines: {"op":"machine_test","position":[0,40,0],"kind":"Crusher","set":{"energy":5000,"input":{"Tin ingots":64}},"ticks":600,"expect":{"end.out.Tin dust":64}}. UNITS: a step's "ticks" are 120 Hz engine steps, but recipe tables (factory-lab-cli recipes Kind, processes.ron) give ticks in 0.25 s factory ticks: set ticks >= recipe ticks x 30 per batch, plus margin (an unfinished batch shows status "Processing" and no "out" key). Other systems: "systems"/"command" to switch the row's world system on, then "inspect" (sections world, civilization, objectives, ...), "residents", "probe", or "replay" {"ticks":N,"sections":[...]} to check that factory/machine stepping is deterministic (expect {"same":true,"restored":true}; it steps and rewinds only factory state, so it cannot prove farming, civ or day-schedule claims). Read tools/axiom-dev/ENGINE_WORKBENCH.md for reply fields before writing expects. HEADLESS RUN (most failed proofs broke these): no window renders, so "perf", frame and "capture" ops reply "No frames measured" and "model" replies "not loaded"; no chunks stream (world.resident_chunks is 0); "inspect" takes at most nine sections; "player" reports feet_m (not position/position_m); expect paths index the reply itself ("same", "end.chemical"), never a "data." prefix; "has" works on strings and arrays only. Expect only values your change produces; at most 12 steps. Omit the key ONLY for pure refactors with no observable effect, and then say "no observable effect" in the summary. Do not write files under tools/.`;
const STATIC_TASK = `${"Implement ONE parity row end to end, fast. Never stop on a missing owner."} Steps:
1. Use the work order below; do not re-run describe.
2. Pick the smallest playable slice (<300 changed lines). Data first: rows in assets/*.ron or modes/fields on existing kinds. New resources, machine kinds, save fields, generator gates and shader fields are APPROVED: build them (see OWNER POLICY).
3. Gate behaviour under world.rules.systems.enabled("<system>") where it ticks.
4. Edit with Edit/Write only. Others edit this tree concurrently: never run git checkout/stash/reset/restore/clean, never revert or reformat, keep unrelated hunks. Do not touch wand_xmb.rs, adventure/growth.rs, the parity ledger or any test.
5. Verify once at the end with exactly: node tools/axiom-dev/swarm-check.mjs (fix errors you caused; re-run once after fixing). No tests, no cargo fmt.
6. If the behaviour already exists and works, edit nothing: report already-implemented with the owner files you verified.
7. If source context is absent after ~20 lookups, create the owner yourself per OWNER POLICY; do not stop.
8. ${SCENARIO_RULE}
Last line exactly: RESULT {"status":"building|playable|already-implemented|blocked","files":["path"],"summary":"<=200 chars","wired":true,"entry":"<file:fn or data row that reaches the change>","tests":"<owner checks, or empty>","why":"<only when not landed: no_owner|too_wide|compile|design|other>"}`;

// Engine-overhaul rows (ENG-*, --family engine|engperf|engsim|engnet|enggame|engarch) change shared
// engine code instead of adding content, so the slice rules differ: same output by default, a
// switch for the new path, and no performance claim (only cargo check runs here).
const ENGINE_TASK = `Implement ONE engine-overhaul row as its smallest safe slice. Steps:
1. Use the work order below. Its notes name the owner files in backticks; docs/research/ENGINE_OVERHAUL_REVIEW.md has the finding and the full implementation prompt under the row id (Grep the id, read only that section).
2. Confirm the finding in current source first. If the code already does what the row asks, edit nothing and report already-implemented with file + symbol.
3. Slice under 300 changed lines. Default behaviour, saves, generated terrain and rendered output stay identical: put a new code path behind a preference, world rule or existing debug switch that defaults to the old path, unless the row is a pure bug fix (panic, unbounded allocation, missing validation), which lands directly.
4. Robustness rows: replace unwrap/expect/indexing on the named path with a typed error or a bounded fallback; never change the wire or save layout unless the row says so.
5. Edit with Edit/Write only. Others edit this tree concurrently: never run git checkout/stash/reset/restore/clean, never revert or reformat, keep unrelated hunks. Do not touch wand_xmb.rs, adventure/growth.rs, the parity ledger or any test.
6. Verify once at the end with exactly: node tools/axiom-dev/swarm-check.mjs <the package you edited> (package names are in each crates/*/Cargo.toml; the app is minecraft_style_voxel_prototype). No tests, no cargo fmt, no benchmarks.
7. Summary must say what the owner has to measure (the row's acceptance check) and that it was NOT measured here.
8. ${SCENARIO_RULE} For engine rows, turn the row's acceptance check into the scenario where an op can observe it.
Last line exactly: RESULT {"status":"building|playable|already-implemented|blocked","files":["path"],"summary":"<=200 chars","wired":true,"entry":"<file:fn or data row that reaches the change>","tests":"<owner checks, or empty>","why":"<only when not landed: no_owner|too_wide|compile|design|other>"}`;

function prompt(row, tier, note, order, ctx = "") {
  return `${/^ENG-/.test(row.id) ? ENGINE_TASK : STATIC_TASK}

${POLICY}

${[brain.note(), brain.proofNote()].filter(Boolean).join("\n")}
Row: ${row.id} ${row.label}
${note ? "Notes (scout plan, retries, peers): " + note + " " : ""}
Work order: ${order}${ctx ? "\nPre-retrieved source evidence (re-read before editing):\n" + ctx : ""}`;
}

// Read-only triage on the cheapest model: settles already-implemented and design-blocked rows for
// cents, and hands the builder exact owner files + a plan so it skips its own exploration.
function scoutPrompt(row, order, ctx = "") {
  return `Read-only triage of ONE parity row in a Rust voxel game. Do not edit. Source evidence is pre-retrieved below; use at most 6 more lookups with Grep/Glob/Read (Grep first; small ranges).
Decide:
- already-implemented: the behaviour exists. Name file + symbol as evidence.
- needs-design: ONLY when a doc or the work order explicitly says the design is undecided/proposed, or it needs a network protocol or new lore/proper nouns. Quote that text in evidence. Missing code is NOT needs-design: the builder creates missing systems, so choose build.
- build: otherwise. List the 1-6 files the builder must edit (owner of the tick/table/UI) and a plan of at most 5 short steps naming symbols and RON tables.
Row: ${row.id} ${row.label}
Work order: ${order}
${ctx ? "Pre-retrieved evidence:\n" + ctx + "\n" : ""}Last line exactly: SCOUT {"verdict":"build|already-implemented|needs-design","files":["path"],"plan":"<=500 chars","evidence":"<=200 chars"}`;
}
function parseFilesFromNote(note) {
  const at = String(note ?? "").match(/says edit: ([^.]*)\./);
  return at ? at[1].split(",").map(s => s.trim()).filter(Boolean) : [];
}
function parseScout(text) {
  const hit = [...String(text ?? "").matchAll(/SCOUT\s*(\{[\s\S]*?\})\s*$/gm)].pop();
  if (!hit) return null;
  try { return JSON.parse(hit[1]); } catch { return null; }
}

function parseResult(text) {
  const hit = [...String(text ?? "").matchAll(/RESULT\s*(\{[\s\S]*?\})\s*$/gm)].pop();
  if (!hit) return null;
  // One scenario shape from here on: a bare array of steps used to be rejected and paid for again as a proof pass.
  try { const r = JSON.parse(hit[1]); if (r && r.scenario !== undefined) r.scenario = normalizeScenario(r.scenario); return r; } catch { return null; }
}

// Spend: every cache write used the 1-hour TTL (2x input price); 5-minute writes cost 1.25x. Runs take 34 s
// p50 / 122 s p90 (2 of 172 over 5 min), so the short TTL never expires mid-run. Measured on one call:
// 16k tokens written at 1h vs 0.7k at 5m (the rest read from the shared prefix). Account MCP connectors
// stay out of agents (strict MCP config) so they never enter the cached tool list.
// The Bash tool's 2-minute default was shorter than a check behind the build lock ("cargo check pending on
// lock" summaries, agents that lingered after their result); 15 minutes covers a shared check.
const LEAN_ENV = { BASH_DEFAULT_TIMEOUT_MS: "900000", BASH_MAX_TIMEOUT_MS: "1200000", FORCE_PROMPT_CACHING_5M: "1", ENABLE_CLAUDEAI_MCP_SERVERS: "false", CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1", DISABLE_TELEMETRY: "1" };
const DENIAL = /requires approval|permission to use|was blocked|denied/i;
// USD per million tokens [input, output]; cache reads bill at 0.1x input, cache writes at 1.25x. Estimates only.
// fable: placeholder at opus rates (unverified); finished runs use the CLI's reported cost, not this.
const PRICE = { fable: [5, 25], opus: [5, 25], sonnet: [3, 15], haiku: [1, 5] };
function estimateCost(model, u) {
  const [i, o] = PRICE[Object.keys(PRICE).find(k => String(model).includes(k)) ?? "sonnet"];
  return ((u.input_tokens ?? 0) * i + (u.cache_read_input_tokens ?? 0) * i * 0.1 + (u.cache_creation_input_tokens ?? 0) * i * 1.25 + (u.output_tokens ?? 0) * o) / 1e6;
}

function runAgent(worker, row, tier, note, order, ctx = "") {
  return new Promise(resolve => {
    const args = ["-p", "--model", tier.model, "--effort", tier.effort, "--output-format", "stream-json", "--verbose",
      "--no-session-persistence", "--setting-sources", "local", "--disable-slash-commands", "--exclude-dynamic-system-prompt-sections", "--strict-mcp-config",
      "--tools", tier.tools ?? "Read,Grep,Glob,Edit,Write,Bash", "--system-prompt", SYSTEM, "--permission-mode", "acceptEdits",
      "--allowedTools", SAFE_TOOLS.join(","), "--disallowedTools", DENY_TOOLS.join(","),
      "--max-turns", String(tier.maxTurns ?? 45), "--max-budget-usd", String(tier.budget ?? 1)];
    const log = path.join(dir, "logs", `${row.id}-${worker.id}-${Date.now()}.log`);
    worker.log = path.relative(root, log);
    const child = spawn(process.env.AXIOM_CLAUDE_PATH || "claude", args, { cwd: root, windowsHide: true, stdio: ["pipe", "pipe", "pipe"],
      env: { ...childEnv(), ...LEAN_ENV, ENABLE_TOOL_SEARCH: "false", AXIOM_SWARM_PORT: String(cfg.port) } });
    // claude.exe's own children (ripgrep for Grep, cargo under Bash) inherit stdout, so a plain kill left the
    // pipe open and 'close' never fired: Stop all, the explore cap, the watchdog and the timeout all waited
    // for the agent's tools to finish. Kill the whole tree, and force the pipes shut 5 s after exit.
    const plainKill = child.kill.bind(child);
    child.kill = () => {
      if (process.platform === "win32" && child.pid && child.exitCode === null) spawnSync("taskkill", ["/T", "/F", "/PID", String(child.pid)], { windowsHide: true, stdio: "ignore" });
      return plainKill();
    };
    child.once("exit", () => setTimeout(() => { child.stdout.destroy(); child.stderr.destroy(); }, 5000).unref());
    const kill = setTimeout(() => child.kill(), cfg.timeoutMs);
    worker.child = child; worker.phase = "connecting";
    let lastEvent = Date.now(), pending = 0, stalled = false, explored = false, denied = 0, turns = 0, linger;
    const cargoCalls = new Set();
    // A killed run (explore cap, watchdog, timeout) emits no result event, so its spend was recorded as $0.
    // Sum per-message usage from the stream and estimate the cost when no final total arrives.
    const seen = new Set(), usage = { input_tokens: 0, cache_read_input_tokens: 0, cache_creation_input_tokens: 0, output_tokens: 0 };
    // Silent connection (no events) for 90 s, or a tool that never returns for 15 min: kill and let the caller retry.
    const watchdog = setInterval(() => {
      const quiet = Date.now() - lastEvent;
      if (final) return; // finished; the linger timer ends the process
      if ((!pending && quiet > 90_000) || quiet > 900_000) { stalled = true; logTail(worker, "stalled " + Math.round(quiet / 1000) + "s: reconnecting"); child.kill(); }
    }, 10_000);
    let buf = "", final, err = "";
    const touched = new Set();
    child.on("error", e => { clearTimeout(kill); clearInterval(watchdog); resolve({ ok: false, error: e.code === "ENOENT" ? "claude CLI not found" : e.message }); });
    child.stderr.setEncoding("utf8").on("data", c => { err = (err + c).slice(-1500); });
    child.stdout.setEncoding("utf8").on("data", chunk => {
      appendFileSync(log, chunk);
      lastEvent = Date.now();
      buf += chunk;
      for (let n; (n = buf.indexOf("\n")) >= 0;) {
        const line = buf.slice(0, n); buf = buf.slice(n + 1);
        let ev; try { ev = JSON.parse(line); } catch { continue; }
        if (ev.type === "system" && ev.subtype === "init") worker.phase = "running";
        // The result is the end of the run. claude.exe used to linger after it (waiting on a cargo check stuck
        // behind the build lock); the watchdog then killed it as "stalled", the row was retried from scratch
        // and good work was recorded as blocked. Take the result and end the process tree shortly after.
        if (ev.type === "result") { final = ev; linger ??= setTimeout(() => child.kill(), 3000); }
        if (ev.type === "user" && Array.isArray(ev.message?.content)) {
          const results = ev.message.content.filter(b => b.type === "tool_result");
          pending = Math.max(0, pending - results.length);
          // A clean agent cargo check proves the tree is green right now: the health gate reuses it instead of
          // starting its own cargo run that competes with the agents for the build lock.
          for (const b of results) if (cargoCalls.has(b.tool_use_id)) {
            const c = typeof b.content === "string" ? b.content : JSON.stringify(b.content ?? "");
            if (/Finished/.test(c) && !/\berror(\[E\d+\])?:/.test(c)) setHealth(true, [], Date.now(), treeSignature());
          }
          for (const b of results) if (b.is_error && DENIAL.test(typeof b.content === "string" ? b.content : JSON.stringify(b.content ?? ""))) { denied++; worker.denied = (worker.denied ?? 0) + 1; }
        }
        if (ev.type === "assistant" && ev.message?.usage && !seen.has(ev.message.id)) { seen.add(ev.message.id); for (const k in usage) usage[k] += ev.message.usage[k] ?? 0; }
        if (ev.type === "assistant") for (const b of ev.message?.content ?? []) {
          if (b.type === "tool_use") {
            worker.turns++; turns++; pending++; worker.phase = "running";
            const i = b.input ?? {};
            if (b.name === "Bash" && /^cargo check\b/.test(String(i.command ?? ""))) cargoCalls.add(b.id);
            // Exploration spiral: many turns, no edit. Every extra turn re-reads the whole context.
            // Denied calls are a prompt/sandbox mismatch, not exploration, so they do not count here.
            if (!tier.scout && !explored && turns - denied >= (tier.exploreTurns ?? cfg.exploreTurns) && !touched.size && !/^(Edit|Write)$/.test(b.name)) {
              explored = true; logTail(worker, `no edit after ${worker.turns} turns: stopping`); child.kill();
            }
            if (/^(Edit|Write)$/.test(b.name) && i.file_path) { touched.add(path.relative(root, i.file_path).replaceAll("\\", "/")); logTail(worker, "✎ " + path.basename(i.file_path)); }
            else if (b.name === "Bash") logTail(worker, "$ " + String(i.command ?? "").slice(0, 90));
            else logTail(worker, b.name + " " + String(i.file_path ?? i.pattern ?? i.path ?? "").slice(-70));
          } else if (b.type === "text" && b.text.trim()) logTail(worker, "· " + b.text.trim().replace(/\s+/g, " ").slice(0, 110));
        }
        worker.files = [...touched];
      }
    });
    child.once("close", code => {
      clearTimeout(kill); clearTimeout(linger); clearInterval(watchdog);
      if (worker.child === child) worker.child = null;
      const text = explored ? `RESULT {"status":"blocked","files":[],"summary":"explored ${turns} turns without an edit; owner code not located"}` : (final?.result ?? "");
      // With a result in hand the exit code only reflects the linger kill.
      resolve({ ok: !explored && !!final && !final.is_error, code, stalled: stalled && !final, explored, noResult: !final && !explored, text, usage: final?.usage ?? usage, cost: final?.total_cost_usd ?? estimateCost(tier.model, usage), estimated: !final, denied, files: [...touched], err, error: final?.is_error ? text : (!final && code ? err : "") });
    });
    child.stdin.on("error", () => {});
    child.stdin.end(tier.prompt ?? (tier.scout ? scoutPrompt(row, order, ctx) : prompt(row, tier, note, order, ctx)));
  });
}

// Pre-retrieved source evidence (context-cli: read-only, no model). It replaces the agent's own
// first dozen Grep/Read turns, which are the most expensive part of a cold run.
const contextCache = new Map();
function rowContext(row, files) {
  if (!cfg.context) return Promise.resolve("");
  if (contextCache.has(row.id)) return Promise.resolve(contextCache.get(row.id));
  const paths = (files ?? []).filter(f => /^[\w./-]+$/.test(f) && !f.includes("..") && existsSync(path.join(root, f))).slice(0, 5);
  return new Promise(resolve => {
    let out = "", settled = false;
    const child = spawn(process.execPath, [path.join(root, "tools/axiom-dev/context-cli.mjs"), `${row.id} ${row.label}`, ...paths, "--max-chars", "6000"],
      { cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "ignore"] });
    const timer = setTimeout(() => child.kill(), 20_000);
    child.stdout.setEncoding("utf8").on("data", c => { out += c; });
    const done = () => {
      if (settled) return; settled = true;
      clearTimeout(timer);
      let text = "";
      try {
        const j = JSON.parse(out);
        text = (j.evidence ?? []).filter(e => !/test/i.test(e.kind ?? "") && !/_tests?\.rs$|\/tests?\//.test(e.path ?? ""))
          .map(e => `${e.path}:${e.line}\n${String(e.excerpt ?? "").split("\n").slice(0, 14).join("\n")}`).join("\n--\n").slice(0, 3800);
      } catch { /* no evidence */ }
      contextCache.set(row.id, text);
      resolve(text);
    };
    child.on("error", done); child.on("close", done);
  });
}

// Responsiveness. The supervisor is one thread: the page, the agents' shared compile check and every worker's
// pick share it, so nothing on the pick path may block.
// - One ledger read serves all of them for a second and a half; the swarm's own status writes drop it.
// - brief() runs a synchronous ripgrep for parity rows (up to 6 s each). pick() and the queue view asked for it
//   for every candidate on every pass, which froze the page of an idle swarm. One brief per row state, kept
//   for ten minutes.
let ledgerCache = { at: 0, value: null };
const ledgerNow = () => { if (!ledgerCache.value || Date.now() - ledgerCache.at > 1500) ledgerCache = { at: Date.now(), value: loadLedger() }; return ledgerCache.value; };
const ledgerChanged = () => { ledgerCache.at = 0; };
const briefCache = new Map();
function briefOf(row, ledger) {
  const key = `${row.status}|${(row.evidence ?? []).length}|${String(row.note ?? "").length}`;
  const hit = briefCache.get(row.id);
  if (hit && hit.key === key && Date.now() - hit.at < 600_000) return hit.value;
  const value = brief(row, ledger.systems);
  briefCache.set(row.id, { key, at: Date.now(), value });
  return value;
}
function prewarm() {
  try {
    const ledger = ledgerNow();
    for (const q of peekCached().slice(0, 3)) {
      const row = ledger.rows.find(r => r.id === q.id);
      if (row && !contextCache.has(row.id) && !active.has(row.id)) rowContext(row, briefOf(row, ledger).files ?? []);
    }
  } catch { /* best effort */ }
}
const active = new Map(); // id -> { files }
const deadOwners = new Set(); // owner systems parked by design blocks; refreshed by eligible()
// Rows a worker may take. A missing owner system that already blocked two rows parks every row that
// needs it, so no model is paid to rediscover the same gap. Pinned rows bypass the family filter.
function eligible(ledger) {
  // An epic (ledger status "design") opens once the design lead has written its design.
  const open = r => !["verified", "blocked", "parked"].includes(r.status) && (r.status !== "design" || designs.decided(r.id))
    && !parked.has(r.id) && !active.has(r.id) && (!finished.has(r.id) || retry.has(r.id));
  const blocks = new Map();
  // Only design blocks kill an owner: "explored N turns" blocks came from tool bugs, and counting them
  // parked whole owners whose rows had never really been tried.
  for (const r of ledger.rows) { const f = finished.get(r.id); if (f?.status === "blocked" && isDesignBlock(f)) for (const o of r.readiness.missing) blocks.set(o, (blocks.get(o) ?? 0) + 1); }
  const dead = new Set([...blocks].filter(([, n]) => n >= 2).map(([o]) => o));
  deadOwners.clear(); for (const o of dead) deadOwners.add(o);
  // An owner system with a design is no longer parked: its rows run and carry that design in their note.
  const stillDead = o => dead.has(o) && !designs.decided(ownerKey(o));
  state.deadOwners = [...dead].filter(stillDead).sort().map(o => `${o}×${blocks.get(o)}`);
  // A row whose own question was designed runs even when its owner system is parked.
  const usable = r => open(r) && (designs.decided(r.id) || !r.readiness.missing.some(stillDead));
  let rows = filterRows(ledger.rows, { family: familyOf(), ready: cfg.readyOnly || undefined }).filter(usable);
  // Family "all" means the parity ledger and leaves ENG-* rows out; a single family can run dry too. When the
  // chosen family has nothing left, take the rest of the ledger (engine rows included) instead of idling.
  if ((!rows.length || scopeAll) && cfg.familyFallback) {
    rows = ledger.rows.filter(r => usable(r) && (!cfg.readyOnly || r.readiness.ready));
    if (rows.length && state.fallbackFamily !== true) { state.fallbackFamily = true; event(`family ${cfg.family} has no open rows; taking ${rows.length} from the rest of the ledger (engine rows included)`); }
  }
  const pushed = priority.map(id => ledger.rows.find(r => r.id === id)).filter(r => r && open(r));
  return { rows, pushed };
}
// `dry` only answers "is there work?": it takes nothing off the pinned list and requeues nothing. The refill
// used the real pick for that test, which dropped a pinned row's place and threw the picked row away.
// Every stage (new rows, wire, proof, continue, retry) honours the chosen family. When none of them can start
// anything there, the same selection runs over the whole ledger instead of idling: 2026-11-19 family "engine"
// had every row landed while ~400 parity rows waited for proof, and five workers sat on "nothing to start".
let scopeAll = false;
const inFamily = row => scopeAll || cfg.family === "all" || filterRows([row], { family: cfg.family }).length > 0;
function pick(dry = false) {
  if (state.restartPending && !dry) return undefined; // draining for a restart: running rows finish, nothing new starts
  const own = pickIn(dry);
  if (own && !dry && !scopeAll) state.fallbackFamily = false;
  if (own || !cfg.familyFallback || cfg.family === "all" || scopeAll) return own;
  scopeAll = true;
  try {
    const wide = pickIn(dry);
    if (wide && !dry && state.fallbackFamily !== true) { state.fallbackFamily = true; event(`family ${cfg.family} has nothing to start; taking work from the rest of the ledger`); }
    return wide;
  } finally { scopeAll = false; }
}
function pickIn(dry = false) {
  const ledger = ledgerNow();
  const { rows, pushed } = eligible(ledger);
  const busy = new Set(state.workers.flatMap(w => w.row ? [...(w.likely ?? []).slice(0, 2), ...(w.files ?? [])] : []));
  // A row that just landed without a usable scenario gets its one proof pass before new work starts.
  const freshProof = proofCandidates(ledger, { fresh: true })[0];
  if (freshProof) return dry ? { row: freshProof.row } : freshProof;
  // Proof debt first. 2026-11-20: 700 landed rows had no scenario and the board read 0 playable, because new
  // rows always outranked proving old ones. Past the debt limit a worker proves landed work (cheaper than a
  // build and the only path to "playable") before it starts anything new; new rows resume once debt drops.
  const debt = [...finished.values()].filter(needsScenario).length;
  if (debt > cfg.proofDebt) {
    const owed = proofCandidates(ledger, { fresh: false, idle: true })[0];
    if (owed) return dry ? { row: owed.row } : owed;
  }
  for (const row of [...pushed, ...nextRows(rows, 80)]) {
    if (active.has(row.id) || (finished.has(row.id) && !retry.has(row.id))) continue;
    if (isFableRow(row.id) && fableLimited()) continue;
    if (bumpRows.has(row.id) && [...active.keys()].filter(id => bumpRows.has(id)).length >= 5) continue; // at most 5 save-format editors at once
    const work = briefOf(row, ledger), files = work.files ?? [];
    if (files.slice(0, 2).some(f => busy.has(f))) continue;
    if (dry) return { row };
    const at = priority.indexOf(row.id); if (at >= 0) priority.splice(at, 1);
    return { row, files, order: JSON.stringify(work).slice(0, 3500) };
  }
  // Nothing new to build: wire what already landed, then grow wired rows, instead of idling.
  const wire = wireCandidates(ledger, busy)[0];
  if (wire) return wire;
  const backfill = proofCandidates(ledger, { fresh: false })[0];
  if (backfill) return backfill;
  const cont = contCandidates(ledger, busy)[0];
  if (cont) return cont;
  // Hourly proof cap reached and nothing else to do: keep proving instead of idling.
  const idleProof = proofCandidates(ledger, { fresh: false, idle: true })[0];
  if (idleProof) return idleProof;
  // Still nothing: wired rows that used their continue passes but never reported "nothing left" get more slices.
  const deeper = cfg.maxContDry > cfg.maxCont ? contCandidates(ledger, busy, cfg.maxContDry)[0] : null;
  if (deeper) return deeper;
  const again = retryCandidate(ledger);
  if (again) {
    if (dry) return { row: again };
    retryPassIds.add(again.id);
    requeue(again.id, 0, "Earlier run stopped after exploring without an edit, mostly from tool-permission bugs fixed since. Start from the work order's owner files and make your first Edit within 8 tool calls.");
    return pick();
  }
}
// Continue pass. Once new rows and wire passes run out (every open row waiting on a design-only owner left
// all seven workers idle), a wired row that landed only its first slice gets its NEXT slice, taken from the
// remainder its own summary lists. At most cfg.maxCont passes per row; the count is saved on the result.
function contCandidates(ledger, busy = new Set(), limit = cfg.maxCont) {
  if (!limit) return [];
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const out = [];
  for (const r of [...finished.values()].sort((a, b) => (a.cont ?? 0) - (b.cont ?? 0) || (b.at ?? 0) - (a.at ?? 0))) {
    const row = byId.get(r.id);
    if (!row || row.status === "verified" || r.wired !== true || !/^(building|playable)$/.test(r.status) || (r.cont ?? 0) >= limit || (limit > cfg.maxCont && r.contDone)
      || (r.contAfter ?? 0) > Date.now() // a peer held its file last time; wait before trying again
      || active.has(r.id) || parked.has(r.id) || !inFamily(row)) continue;
    const files = (r.files ?? []).filter(f => /^[\w./-]+$/.test(f)).slice(0, 6);
    if (!files.length || files.slice(0, 2).some(f => busy.has(f))) continue;
    out.push({ row, files, cont: true, order: JSON.stringify(briefOf(row, ledger)).slice(0, 2500),
      note: `CONTINUE PASS ${(r.cont ?? 0) + 1}. Earlier slices of this row landed and are wired, in ${files.join(", ")}; the last report said: ${String(r.summary).slice(0, 300)} Build the NEXT smallest playable slice of what that report says remains, reusing the existing code and keeping it reachable (wired). Do not redo or restructure what exists. Start from those files; first Edit within 8 tool calls. If everything that remains needs an owner decision, edit nothing and report blocked with why "design".` });
    if (out.length >= 12) break;
  }
  return out;
}
// Wire pass. Most open rows wait on design-only owner systems, so the queue runs dry while ~2/3 of landed
// slices are scaffolding nothing calls (cost per wired row is ~4x cost per row). When no new row is free, a
// landed-but-unwired row goes back to sonnet once to add its smallest real caller. One pass per row; the
// result carries wirePass so restarts do not repeat it.
// Anything not explicitly wired qualifies: 133 older results had no wired field and no "unwired" keyword,
// so neither the wire pass (not unwired) nor the continue pass (not wired) ever picked them. A wire pass on
// a row that is in fact reachable reports already-implemented and marks it wired.
const isUnwired = r => r && /^(building|playable)$/.test(r.status) && !r.wirePass && (r.files ?? []).length > 0 && r.wired !== true;
function wireCandidates(ledger, busy = new Set()) {
  if (!cfg.wire) return [];
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const out = [];
  for (const r of [...finished.values()].sort((a, b) => (b.at ?? 0) - (a.at ?? 0))) {
    const row = byId.get(r.id);
    if (!row || !isUnwired(r) || active.has(r.id) || parked.has(r.id) || !inFamily(row)) continue;
    const files = (r.files ?? []).filter(f => /^[\w./-]+$/.test(f)).slice(0, 6);
    if (files.slice(0, 2).some(f => busy.has(f))) continue;
    out.push({ row, files, wire: true, order: JSON.stringify(briefOf(row, ledger)).slice(0, 2500),
      note: `WIRE PASS. An earlier slice of this row landed in ${files.join(", ")} and reported: ${String(r.summary).slice(0, 260)} Nothing reaches it yet. Do NOT add features: wire the smallest real caller so a player action, a tick or a loaded table reaches the existing code (one call site, one RON row read, or one UI line), behind the row's system switch. Start from those files; first Edit within 6 tool calls. If no caller is possible without an owner decision, edit nothing and report blocked with why "design". If a player action, tick or loaded table ALREADY reaches it, edit nothing and report already-implemented with that entry point in "entry".` });
    if (out.length >= 12) break;
  }
  return out;
}
// Retry pass: rows blocked by "explored N turns without an edit" mostly hit the Bash-denial and linger bugs
// fixed since (denials counted as turns; finished runs re-run with the old turn count). Each gets ONE fresh
// sonnet run when nothing else is queued; the result carries retryPass so it is never repeated.
const retryPassIds = new Set();
function retryCandidate(ledger) {
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  for (const r of [...finished.values()].sort((a, b) => (b.at ?? 0) - (a.at ?? 0))) {
    const row = byId.get(r.id);
    if (!row || r.status !== "blocked" || r.retryPass || isDesignBlock(r) || !/explored \d+ turns without an edit/.test(r.summary ?? "")
      || active.has(r.id) || parked.has(r.id) || row.readiness?.missing?.some(o => (state.deadOwners ?? []).some(d => d.startsWith(o + "×")))) continue;
    if (!inFamily(row)) continue;
    return row;
  }
}
// Proof pass. A landed row with no stored scenario can never reach "proven" (2026-11-18: 490 engine rows landed,
// none provable, because the engine prompt never asked for a scenario). A read-only sonnet run reads the row's
// own files and returns ONLY a scenario; it has no Edit tool, so it cannot change code. Fresh landings that came
// back without a valid scenario go first (item: send back once); older landed rows are backfilled lowest
// milestone first. At most cfg.proofPerHour passes an hour, so backfill never crowds out building. When pick()
// has nothing else for a worker (no new, wire or continue work), backfill runs uncapped (idle: true) and does not
// count against the hour: 2026-11-18 five workers sat idle with 733 rows waiting behind a 12/h cap. The spend
// cap still bounds it.
const proofTier = { name: "sonnet·proof", model: cfg.sonnet, effort: "low", maxTurns: 12, budget: 0.3, tools: "Read,Grep,Glob", scout: true };
const proofRuns = []; // start times of proof passes, pruned to the last hour
const proofsThisHour = () => { const cut = Date.now() - 3600_000; while (proofRuns.length && proofRuns[0] < cut) proofRuns.shift(); return proofRuns.length; };
const milestoneOf = row => Number(String(row?.milestone ?? "").match(/M(\d+(?:\.\d+)?)/)?.[1] ?? 9);
// Rows that already have a tagged scenario file (older machine scenarios carry no `scenario` field on the result).
let taggedCache = { at: 0, ids: new Set() };
const taggedIds = () => {
  if (Date.now() - taggedCache.at > 60_000) taggedCache = { at: Date.now(), ids: new Set(tolerantScenarios(scenarioPath).scenarios.flatMap(s => s.parity ?? [])) };
  return taggedCache.ids;
};
// Unwrap/TODO chores (ENG-ARCH-C*) change error paths only; their proof passes came back "no observable effect"
// almost every time, so they are not offered one (the builder may still attach a scenario itself).
const isChore = id => /^ENG-ARCH-C[0-9A-F]/.test(id);
// deadCode: the proof pass found the landed code has no caller. A wire pass runs first; once it lands, the row gets
// one fresh proof pass (its earlier "no op reaches it" verdict described unreachable code, not the feature).
const rewired = r => r?.deadCode && r.wirePass && r.wired === true;
const needsScenario = r => r && /^(building|playable)$/.test(r.status) && !r.scenario && (!r.proofPass || rewired(r)) && (!r.noProof || rewired(r)) && (r.files ?? []).length > 0 && !taggedIds().has(r.id) && !isChore(r.id);
// A proof that failed only on the answer's shape (no scenario, wrong JSON shape, no expect) gets ONE more pass when
// workers would otherwise idle: 58 rows lost their only pass that way before normalizeScenario accepted every shape.
const FORMAT_FAIL = /^(no scenario|scenario must be an object|scenario needs at least one step|scenario asserts nothing)/;
const needsReproof = r => r && /^(building|playable)$/.test(r.status) && r.proofPass && !r.scenario && !r.reproof
  && FORMAT_FAIL.test(String(r.noProof ?? "")) && (r.files ?? []).length > 0 && !taggedIds().has(r.id)
  && !isChore(r.id);
function proofCandidates(ledger, { fresh, idle = false }) {
  if (!cfg.prove || !(cfg.proofPerHour > 0) || (!idle && proofsThisHour() >= cfg.proofPerHour)) return [];
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const out = [];
  for (const r of finished.values()) {
    const again = idle && !fresh && needsReproof(r);
    // Idle backfill takes rows flagged needsProof too: those are only offered on the fresh path, which the hourly
    // cap closes, so on 2026-11-19 130 flagged rows waited while five workers sat on "No row can start".
    if (!again && (!needsScenario(r) || (!idle && !!r.needsProof !== fresh))) continue;
    const row = byId.get(r.id);
    if (!row || row.status === "verified" || active.has(r.id) || parked.has(r.id) || !inFamily(row)) continue;
    // Read-only: a file a builder holds does not block it.
    out.push({ row, files: (r.files ?? []).filter(f => /^[\w./-]+$/.test(f)).slice(0, 6), proof: true, idle, rank: milestoneOf(row), at: r.at ?? 0,
      order: JSON.stringify(briefOf(row, ledger)).slice(0, 2000) });
  }
  return out.sort((a, b) => a.rank - b.rank || b.at - a.at).slice(0, 12);
}
function proofPrompt(row, files, prior) {
  return `PROOF PASS for ONE row that already landed in a Rust voxel game. Do NOT edit anything: you only have Read, Grep and Glob. Your only output is a headless engine scenario that proves the landed behaviour.
Row: ${row.id} ${row.label}
It landed in: ${files.join(", ") || "(files unknown)"}. Its report said: ${String(prior?.summary ?? "").slice(0, 300)}
${prior?.proofWhy ? "The last attempt was rejected: " + String(prior.proofWhy).slice(0, 300) + ". Fix exactly that.\n" : ""}Steps (at most 10 lookups, Grep first, small Read ranges):
1. Find the landed hunks in those files and the world system switch (world.rules.systems.enabled("...")) they are gated on, if any.
2. Read tools/axiom-dev/ENGINE_WORKBENCH.md for the request and reply fields of the ops you use; expects must name fields the reply really has.
3. Write the smallest scenario that observes the change: switch the system on if needed, act, then assert.
${SCENARIO_RULE}
${brain.proofNote()}
If no op can observe it (pure refactor, render-only, or nothing an op reaches), report status "blocked" with why "no_observable" and name the op that is missing in the summary.
Last line exactly: RESULT {"status":"building|blocked","files":[],"summary":"<=200 chars: what the scenario checks","scenario":{"steps":[...]},"why":"<only when blocked>"}`;
}
async function proofPass(worker, row, next, prior) {
  if (!next.idle) proofRuns.push(Date.now());
  const tier = { ...proofTier, prompt: proofPrompt(row, next.files, prior) };
  Object.assign(worker, { tier: tier.name, startedAt: Date.now(), activity: [], turns: 0, files: [], likely: next.files, phase: "proof" });
  logTail(worker, `proof pass ${row.id}`);
  const out = await runAgent(worker, row, tier, "", next.order, "");
  active.delete(row.id); worker.child = null;
  const cost = addCost(out.cost);
  state.totals.in += (out.usage?.input_tokens ?? 0) + (out.usage?.cache_read_input_tokens ?? 0) + (out.usage?.cache_creation_input_tokens ?? 0);
  state.totals.out += out.usage?.output_tokens ?? 0; state.totals.cached += out.usage?.cache_read_input_tokens ?? 0;
  if (limit.test((out.error || "") + " " + (out.text || ""))) { if (!next.idle) proofRuns.pop(); pauseForLimit((out.error || "") + " " + (out.text || ""), "proof"); return; }
  if (state.stop && !out.ok) return;
  const parsed = parseResult(out.text);
  // Models write why as "no_observable" or "no_observable: needs an op that ..."; keep the trailing detail.
  const whyText = String(parsed?.why ?? "");
  const noObservable = /^\s*no_observable\b/i.test(whyText);
  const whyDetail = whyText.replace(/^\s*no_observable\b[\s:;,-]*/i, "");
  let bad = parsed?.scenario ? validateScenario(parsed.scenario, ENGINE_OPS) : noObservable ? null : "no scenario returned";
  const kept = { ...prior, needsProof: undefined, final: true, reproof: prior?.proofPass ? true : prior?.reproof };
  if (parsed?.scenario && !bad) {
    try {
      const f = scenarioFile(row.id, parsed.scenario, parsed.summary ?? prior?.summary);
      mkdirSync(scenarioPath, { recursive: true });
      writeFileSync(path.join(scenarioPath, f.name), f.body);
      kept.scenario = "tools/axiom-dev/scenarios/" + f.name; kept.proofPass = true; kept.noProof = undefined; idleProved = false; taggedCache.at = 0;
      state.totals.scenarios = (state.totals.scenarios ?? 0) + 1;
    } catch (error) { bad = "not stored: " + String(error.message).slice(0, 80); }
  }
  kept.deadCode = undefined; // the post-wire proof is a dead-code row's one extra pass
  if (!kept.scenario) {
    // One retry for a scenario that came back but failed validation (unknown op or test): the reason goes into
    // the next prompt. A missing scenario or a "no_observable" verdict is final for the row.
    if (bad && parsed?.scenario && !prior?.proofRetried) Object.assign(kept, { needsProof: true, proofRetried: true, proofWhy: bad });
    else Object.assign(kept, { proofPass: true, noProof: String(noObservable ? [parsed?.summary, whyDetail].filter(Boolean).join(" ") || "no observable effect" : bad ?? "no scenario").slice(0, 200) });
  }
  finished.set(row.id, kept);
  appendFileSync(resultsFile, JSON.stringify(kept) + "\n");
  rev++;
  state.results.unshift({ id: row.id, label: row.label, tier: tier.name, at: Date.now(), ms: Date.now() - worker.startedAt, files: kept.scenario ? [kept.scenario] : [], log: worker.log,
    cost, turns: worker.turns, status: kept.scenario ? "building" : "blocked", final: false,
    summary: kept.scenario ? "proof pass: scenario stored: " + String(parsed?.summary ?? "").slice(0, 160) : `proof pass: no scenario (${kept.noProof ?? kept.proofWhy})` });
  if (state.results.length > 200) state.results.pop();
  persist();
}
let resumeTimer;
// "resets 2pm" or "resets 2:30 pm": next occurrence of that local time; otherwise retry in 30 minutes.
// A weekly limit names a date ("resets Nov 23, 2pm"). Read as "no time given" it retried every 30 minutes for
// days, launching every worker into the same refusal.
function limitResetAt(message, now = Date.now()) {
  const dated = String(message).match(/resets?\s+(?:on\s+)?([A-Za-z]{3,9})\.?\s+(\d{1,2})(?:st|nd|rd|th)?,?\s*(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*([ap]m)\b/i);
  const month = dated ? ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].indexOf(dated[1].slice(0, 3).toLowerCase()) : -1;
  if (month >= 0) {
    const when = new Date(now);
    when.setMonth(month, Number(dated[2]));
    when.setHours(Number(dated[3]) % 12 + (dated[5].toLowerCase() === "pm" ? 12 : 0), Number(dated[4] ?? 0), 0, 0);
    if (when.getTime() <= now) when.setFullYear(when.getFullYear() + 1);
    return when.getTime();
  }
  const at = String(message).match(/resets?\s+(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*([ap]m)\b/i);
  if (!at) return now + 30 * 60_000;
  const when = new Date(now);
  when.setHours(Number(at[1]) % 12 + (at[3].toLowerCase() === "pm" ? 12 : 0), Number(at[2] ?? 0), 0, 0);
  if (when.getTime() <= now) when.setDate(when.getDate() + 1);
  return when.getTime();
}
const startTier = () => 0;
const limit = /weekly limit|usage limit|rate limit|out of credits|credit balance|hit your [\w -]{0,20}limit|\d+-hour limit|limit resets/i;
// One pause for every kind of run. The scout path used to set the pause with no timer, so a limit that arrived
// during a scout held the swarm until someone pressed Resume. The timer lifts only a limit pause: a pause the
// supervisor or the owner set in the meantime stays.
function pauseForLimit(text, where) {
  const until = limitResetAt(text);
  state.paused = "usage limit hit; resumes " + new Date(until).toLocaleString();
  state.resumeAt = until;
  clearTimeout(resumeTimer);
  resumeTimer = setTimeout(() => {
    if (!String(state.paused ?? "").startsWith("usage limit")) return;
    state.paused = null; state.resumeAt = null; event("usage limit window passed; resumed");
  }, Math.max(60_000, until - Date.now()));
  event(`usage limit hit (${where}); resumes ${new Date(until).toLocaleTimeString()}`);
}

// Per-tier ROI: spend and outcomes by tier, so a tier that burns money without landing rows is visible.
function tierRoi(entry) {
  const t = state.tierStats[entry.tier] ??= { rows: 0, landed: 0, escalated: 0, blocked: 0, cost: 0, tokens: 0 };
  t.rows++; t.cost += entry.cost ?? 0; t.tokens += entry.tokens ?? 0;
  if (entry.status === "escalated") t.escalated++; else if (entry.status === "blocked") t.blocked++;
  else if (/^(playable|building|already-implemented|proven)$/.test(entry.status)) t.landed++;
}
// Landed rows whose own summary says nothing reaches them are scaffolding; the board reports cost per wired row.
const UNWIRED = /\bnot (?:yet )?wired\b|\bunwired\b|\bno callers?\b|\bnot wired to\b|\bunused\b|\bunplayable\b/i;
function record(entry) {
  rev++;
  tierRoi(entry);
  state.results.unshift(entry); if (state.results.length > 200) state.results.pop();
  appendFileSync(resultsFile, JSON.stringify(entry) + "\n");
  // An already-implemented verdict is a landing too: without this the ledger kept such rows "open",
  // the board showed a full queue while every agent idled, and a fresh swarm paid to re-scout them.
  if (entry.final && entry.status === "already-implemented") markImplemented(entry);
}
function markImplemented(entry) {
  try {
    const ledger = loadLedger(), row = ledger.rows.find(r => r.id === entry.id);
    if (!row || row.status !== "open") return;
    writeStatus(entry.id, { status: "building", evidence: (entry.files ?? []).slice(0, 6), note: "swarm (already implemented): " + String(entry.summary ?? "").slice(0, 150) }, ledger.revision, new Set(ledger.rows.map(r => r.id)));
    ledgerChanged();
  } catch (error) { entry.ledger = "not recorded: " + String(error.message).slice(0, 80); }
}

// Build-health gate: new rows start only while the shared tree compiles. A red tree means every agent's
// final `cargo check` fails and they burn budget "fixing" someone else's break. Checked at most every
// 3 minutes (single flight); rows already running continue, and agents are told nothing about it.
const useHealthGate = !flag("no-health");
const health = { ok: true, at: 0, errors: [], checking: null, sig: "" };
// What the compiler reads, as one cheap value (about 2,500 stats, under 0.1 s): file count and newest mtime of
// the Rust, RON, WGSL and manifest files. A red result carries the signature of the tree it was taken on.
function treeSignature() {
  let n = 0, newest = 0;
  const walk = d => {
    let entries; try { entries = readdirSync(d, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (e.name === "target" || e.name.startsWith(".")) continue;
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (/\.(rs|ron|wgsl|toml)$/.test(e.name)) { n++; try { const m = statSync(p).mtimeMs; if (m > newest) newest = m; } catch { /* vanished */ } }
    }
  };
  walk(path.join(root, "crates")); walk(path.join(root, "assets"));
  return `${n}:${newest}`;
}
// The one place health changes, so the page's copy (state.health) can never disagree with the gate.
const breakerRed = () => /^circuit breaker\b.*does not compile/.test(String(state.paused ?? ""));
function setHealth(ok, errors = [], at = Date.now(), sig = health.sig) {
  const was = health.ok;
  Object.assign(health, { ok, at, errors: errors.slice(0, 6).map(l => String(l).slice(0, 200)), sig });
  state.health = { ok: health.ok, at: health.at, errors: health.errors };
  if (ok === was) return;
  rev++;
  // The breaker paused because the tree was red and the repair failed. Someone (a peer session, the owner) has
  // fixed it since: that was the only thing the pause waited for, so it ends here instead of on a click.
  if (ok && breakerRed()) { state.paused = null; event("circuit breaker: the tree compiles again; resumed"); }
}
// Stale red self-heals. Health was refreshed only by a builder about to start a row or by a checkpoint after
// 8 more landings, so with every worker idle or paused a red result outlived its fix: the page said "builders
// wait for green", no builder was there to recheck, and it took a click on Checkpoint (2026-11-19). While the
// result is red, recheck as soon as the sources differ from the tree that failed, and every 15 minutes anyway
// (a fix can also be a deleted file or a peer's finished build). Nothing runs while green.
if (useHealthGate && !flag("dry")) setInterval(() => {
  if (state.stop || health.ok || health.checking || state.checkpointing || (state.paused && !breakerRed())) return;
  const age = Date.now() - health.at;
  if (age < 120_000) return;
  const changed = treeSignature() !== health.sig;
  if (!changed && age < 15 * 60_000) return;
  event(changed ? "health: the sources changed since the failing check; rechecking" : "health: red for 15 minutes; rechecking");
  checkHealth();
}, 60_000).unref();
function checkHealth() {
  if (health.checking) return health.checking;
  health.checking = sharedCheck(APP).then(() => { health.checking = null; });
  return health.checking;
}
// Shared compile check. Every caller (agents through swarm-check.mjs, the health gate) gets the result of a
// cargo run that STARTED after it asked, so its edits are covered; callers that arrive while a run is in
// flight share the single follow-up run. Before, five agents queued five 3-minute checks behind the lock.
const checks = new Map(); // pkg -> { running, next }
// Runtime table check, re-run only when a table or the content crate changed (mtime signature).
let tablesMemo = { sig: "", result: { ok: true, failures: [] } };
function contentSignature() {
  const parts = [];
  const walk = d => { try { for (const e of readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p); else if (/\.(ron|rs)$/.test(e.name)) parts.push(e.name + statSync(p).mtimeMs); } } catch { /* vanished */ } };
  walk(path.join(root, "assets")); walk(path.join(root, "crates", "content", "src"));
  return createHash("sha1").update(parts.join("|")).digest("hex");
}
async function tablesCheck() {
  const sig = contentSignature();
  if (sig === tablesMemo.sig) return tablesMemo.result;
  const result = await runTables({ root, env: childEnv() });
  if (result.ran) tablesMemo = { sig, result };
  if (!result.ok) event(`content tables fail at runtime: ${result.failures.join(" ; ").slice(0, 200)}`);
  return result;
}
// The workspace globs `crates/*`, so a builder that creates crates/<x>/src before its Cargo.toml makes
// every cargo command fail with "failed to load manifest" (ENG-META-10's crates/server held all builders,
// itself included, since the error names a directory no worker's file list matches). Write a dependency-free
// stub manifest instead: the tree loads, and anything missing becomes an ordinary error inside that crate's
// own files, which the half-done-edit rule attributes to its author. The builder may rewrite the stub.
function stubMissingManifests() {
  const crates = path.join(root, "crates");
  let dirs = [];
  try { dirs = readdirSync(crates, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name); } catch { return; }
  for (const dir of dirs) {
    const base = path.join(crates, dir);
    if (existsSync(path.join(base, "Cargo.toml")) || !existsSync(path.join(base, "src"))) continue;
    // Prefer the name the crate's own binaries already use (`engine_server::paths`), else the house prefix.
    let name = `engine_${dir.replace(/[^A-Za-z0-9_]/g, "_")}`;
    try {
      const bins = path.join(base, "src", "bin");
      for (const f of existsSync(bins) ? readdirSync(bins) : []) {
        const m = /\b(engine_\w+)::/.exec(readFileSync(path.join(bins, f), "utf8"));
        if (m) { name = m[1]; break; }
      }
    } catch { /* keep the default name */ }
    try {
      writeFileSync(path.join(base, "Cargo.toml"), `# Stub written by the swarm health gate: src/ existed without a manifest.\n# Replace with the real manifest.\n[package]\nname = "${name}"\nversion.workspace = true\nedition.workspace = true\n\n[dependencies]\n`, { flag: "wx" });
      event(`stub Cargo.toml written for crates/${dir} (${name}): src/ had no manifest and broke the workspace`);
    } catch { /* written meanwhile by its builder */ }
  }
}
function sharedCheck(pkg) {
  let s = checks.get(pkg);
  if (!s) checks.set(pkg, s = { running: null, next: null });
  const start = () => {
    stubMissingManifests();
    const sig = pkg === APP ? treeSignature() : "";
    s.running = runCheck(pkg, { root, env: childEnv() }).then(async r => {
      if (pkg === APP && r.ok) {
        const t = await tablesCheck();
        if (!t.ok) r = { ...r, ok: false, errors: t.failures, text: `FAILED: the code compiles but a content table does not load at runtime (the game would panic at start):\n${t.failures.join("\n")}\nFix the row you added or changed; ids must exist in the tables they reference.` };
      }
      if (pkg === APP) {
        // A spawn/lock failure with no compiler error is not "red".
        setHealth(r.ok || !r.errors.length, r.errors, r.at, sig);
      }
      return r;
    }).catch(e => ({ ok: false, errors: [], warnings: 0, text: "FAILED (check did not run): " + String(e?.message ?? e).slice(0, 200), at: Date.now() }))
      .finally(() => { s.running = null; });
    return s.running;
  };
  if (s.next) return s.next;
  if (!s.running) return start();
  s.next = s.running.then(() => { s.next = null; return start(); });
  return s.next;
}
// Never wait forever: a tree broken by a peer session (not the swarm) used to park every worker indefinitely.
// After two red checks a checkpoint repair runs; after 20 minutes the builder starts anyway and POLICY
// RED TREE tells it to report foreign errors instead of fixing them.
async function waitForGreen(worker) {
  if (!useHealthGate) return;
  // A stale green result refreshes in the background; awaiting it held every builder ~3 min before its
  // first turn. Only a known-red tree blocks.
  if (!health.ok) await checkHealth();
  else if (Date.now() - health.at > 180_000) checkHealth();
  const since = Date.now();
  // Errors inside files a running agent is editing are its half-done edit (ENG-MESH-02's flag_table held
  // every builder): only orphaned errors block. The checkpoint applies the same rule.
  const blocking = () => {
    const editing = state.workers.filter(w => w.child && w !== worker).flatMap(w => w.files ?? []);
    return health.errors.filter(e => !editing.some(f => e.replaceAll("\\", "/").includes(f)));
  };
  for (let red = 1; !health.ok && blocking().length && !state.stop; red++) {
    if (Date.now() - since > 8 * 60_000) { logTail(worker, "build still red after 8 min; starting anyway"); return; }
    worker.status = "waiting for green build";
    logTail(worker, "build red: " + (health.errors[0] ?? "").slice(0, 100));
    if (red === 2 && !state.checkpointing) checkpoint("build red; repair");
    await new Promise(r => setTimeout(r, 45_000));
    await checkHealth(); // shared: joins a checkpoint's run instead of starting another
  }
}

// A throw anywhere in one row (a bad ledger row, a JSON parse, a file race) used to end that worker's loop
// for good while its rail still said "working". Restart the loop, release the row and retry it later.
async function work(worker) {
  for (let crashes = 0; !state.stop && !worker.retire; crashes++) {
    try { await workLoop(worker); return; }
    catch (e) {
      const id = worker.row;
      event(`worker ${worker.id} recovered from: ${String(e?.message ?? e).slice(0, 140)}${id ? ` (row ${id} requeued)` : ""}`);
      if (worker.child) treeKill(worker.child);
      if (id) { active.delete(id); if (crashes < 3) retry.set(id, retry.get(id) ?? 0); else parked.add(id); }
      Object.assign(worker, { status: "idle", row: null, child: null, phase: "" });
      await new Promise(r => setTimeout(r, 5000));
    }
  }
}
// Self-feeding. When no row is pickable the queue used to sit empty (design-blocked rows, parked owner systems,
// epics, spent continue passes). Two feeders keep the workers busy. A refill starts when a worker finds nothing
// and also when the queue runs low, so it rarely runs dry; one refill at a time, at most every 30 s:
//  1. designStage: a read-only design lead designs what waits on a decision (see swarm-design.mjs for the
//     order and the rules it designs within) and the item goes back to the queue with that design as its note.
//     Up to cfg.designParallel at once and cfg.decideMax an hour.
//  2. feedChores: improve-cli rule hits (unwrap, stale TODO, wall-clock) become ENG-ARCH-C* rows in a generated
//     matrix, which the normal pipeline then works.
// --no-decide / --no-chores turn them off.
const designing = new Map(); // key -> { kind, since, slot }
const designTries = new Map(); // key -> replies that could not be read as a design
let designTimes = []; // when designs were made (hourly cap)
let designWaiting = { owner: 0, row: 0, epic: 0 };
let refilling = null, lastRefill = 0;
const designsThisHour = () => { const from = Date.now() - 3_600_000; designTimes = designTimes.filter(t => t > from); return designTimes.length; };
async function designOne(item, ledger) {
  if (state.restartPending) return;
  const slot = { id: "design", activity: [], turns: 0, files: [] };
  designing.set(item.key, { kind: item.kind, since: Date.now(), slot }); rev++;
  try {
    const briefText = item.row ? JSON.stringify(briefOf(item.row, ledger)).slice(0, 2200) : "";
    const out = await runAgent(slot, { id: item.key.replace(/[^\w.-]/g, "-"), label: "design" },
      { ...designTier, prompt: designPrompt({ ...item, brief: briefText }) }, "", "");
    const tokens = (out.usage?.input_tokens ?? 0) + (out.usage?.cache_read_input_tokens ?? 0) + (out.usage?.cache_creation_input_tokens ?? 0);
    state.totals.in += tokens; state.totals.out += out.usage?.output_tokens ?? 0; addCost(out.cost);
    state.totals.cached += out.usage?.cache_read_input_tokens ?? 0;
    if (state.stop) return 0;
    if (limit.test((out.error || "") + " " + (out.text || ""))) { pauseForLimit(out.error || out.text || "", "design"); return 0; }
    const parsed = parseDesign(out.text);
    tierRoi({ tier: designTier.name, status: parsed?.verdict === "decide" ? "building" : parsed ? "blocked" : "escalated", cost: out.cost ?? 0, tokens });
    const base = { id: item.key, kind: item.kind, at: Date.now(), v: DESIGN_VERSION, cost: out.cost ?? 0, log: slot.log };
    if (!parsed) {
      // An unreadable reply used to be retried on every refill with no limit. Two tries, then say so and stop.
      const tries = (designTries.get(item.key) ?? 0) + 1; designTries.set(item.key, tries);
      if (tries < 2) { event(`design ${item.key}: no usable answer; one more try later`); return 0; }
      designs.save({ ...base, verdict: "defer", text: "The design lead gave no usable design in two attempts. Read its log, or press Redesign to try again." });
      event(`design ${item.key}: no usable design in two attempts; left for the owner`);
      return 0;
    }
    designTimes.push(Date.now());
    if (parsed.verdict === "defer") {
      designs.save({ ...base, verdict: "defer", text: parsed.text });
      event(`design ${item.key}: left for the owner (${parsed.text.slice(0, 100)})`);
      return 0;
    }
    const earlier = designs.get(item.key);
    const round = earlier?.verdict === "decide" && (earlier.v ?? 1) >= DESIGN_VERSION ? (earlier.round ?? 1) + 1 : 1;
    const rel = path.relative(root, designs.pathOf(item.key)).replaceAll("\\", "/");
    const note = designNote(item.kind, item.key, parsed.parts, rel);
    const title = item.kind === "owner" ? `Owner system ${item.owner} ${item.system?.name ?? ""}` : `${item.key} ${item.row?.label ?? ""}`;
    const text = parsed.parts.DESIGN.replace(/\s+/g, " ").slice(0, 600);
    designs.save({ ...base, verdict: "decide", round, text, note },
      `# ${title.trim()}\n\nWritten by the swarm design lead (${designTier.name}) at ${new Date().toISOString().slice(0, 16)}Z. The owner has not reviewed it.\n\n${parsed.body}\n`);
    if (item.kind === "owner") {
      // Rows that stopped on this owner go back with its design. Rows never tried pick it up from the owner
      // note the worker adds to every row that needs this system.
      const byId = new Map(ledger.rows.map(r => [r.id, r]));
      let back = 0;
      for (const f of [...finished.values()]) {
        if (f.status !== "blocked" || parked.has(f.id) || active.has(f.id) || !byId.get(f.id)?.readiness.missing.includes(item.owner)) continue;
        requeue(f.id, 0, note); back++;
      }
      event(`design ${item.key}: ${text.slice(0, 90)} (${back} blocked row(s) requeued, ${item.rows.length} were waiting)`);
    } else {
      requeue(item.key, 0, note);
      event(`design ${item.key}: ${text.slice(0, 110)}`);
    }
    return 1;
  } catch (e) { event(`design ${item.key} failed: ${String(e?.message ?? e).slice(0, 120)}`); return 0; }
  finally { designing.delete(item.key); rev++; peekCache.at = 0; persist(); }
}
// What the design lead would take next (`limit` items) and how much waits in all.
function findDesignTargets(ledger, limit) {
  eligible(ledger); // refreshes deadOwners
  const args = { ledger, finished, store: designs, skip: id => parked.has(id) || active.has(id) || designing.has(id) || retry.has(id),
    order: rows => nextRows(rows, rows.length), isDesignBlock, dead: deadOwners, epics: cfg.designEpics, limit: Math.max(0, limit) };
  const found = designTargets({ ...args, scope: rows => filterRows(rows, { family: familyOf() }) });
  // Same rule as the row queue: a family with nothing left to design takes from the rest of the ledger.
  if (found.counts.owner + found.counts.row + found.counts.epic || !cfg.familyFallback) return found;
  return designTargets({ ...args, scope: rows => rows });
}
// The page shows the waiting counts while paused and before the first refill too; recount at most every 10 s.
let designCountedAt = 0;
function countDesignWaiting() {
  if (Date.now() - designCountedAt < 10_000) return;
  designCountedAt = Date.now();
  try { designWaiting = findDesignTargets(ledgerNow(), 0).counts; } catch { /* keep the last count */ }
}
async function designStage() {
  if (!cfg.decide || state.stop || state.paused || overBudget()) return 0;
  const room = Math.min(cfg.designParallel, cfg.decideMax - designsThisHour()) - designing.size;
  const ledger = ledgerNow();
  const found = findDesignTargets(ledger, room);
  designWaiting = found.counts; designCountedAt = Date.now();
  if (room <= 0 || !found.items.length) return 0;
  const made = await Promise.all(found.items.map(item => designOne(item, ledger)));
  return made.reduce((a, b) => a + b, 0);
}
// Measured perf rows: build, bench, write ENG-REN-PF* rows for the worst stages. Runs in the background (a
// release-fast build plus bench takes many minutes) so designs and chores are not held behind it.
let perfFeeding = null;
function startPerfFeed() {
  // A stage waits while its last row is running or not started; once it lands or blocks, the next bench judges it.
  const isOpen = id => active.has(id) || !finished.has(id);
  perfFeeding = feedPerf({ root, isOpen, log: m => event(m) })
    .then(added => { if (added) { ledgerChanged(); event(`perf feeder: ${added} measured row(s) added`); } })
    .catch(e => event("perf feeder error: " + String(e?.message ?? e).slice(0, 120)))
    .finally(() => { perfFeeding = null; });
}
// The chore scan is a full-tree ripgrep (improve-cli scan). An idle swarm ran it every 30 s and it found nothing
// for hours (2026-11-21), so an empty scan waits CHORE_EMPTY_MS before the next. `choreScan` feeds the idle message.
const CHORE_EMPTY_MS = 15 * 60_000;
let choreScan = { at: 0, added: 0 };
function refill() {
  if (refilling) return refilling;
  if (Date.now() - lastRefill < 30_000) return Promise.resolve();
  refilling = (async () => {
    try {
      lastRefill = Date.now();
      let added = await designStage();
      if (!added && cfg.perfFeed && !perfFeeding && !designing.size && !pick(true)) startPerfFeed();
      const choreDue = choreScan.added > 0 || Date.now() - choreScan.at >= CHORE_EMPTY_MS;
      if (!added && cfg.chores && choreDue && !designing.size && !pick(true)) {
        added = await feedChores({ root, limit: 8, log: m => event(m) });
        choreScan = { at: Date.now(), added };
        if (added) { ledgerChanged(); event(`feeder: ${added} chore row(s) added from improve-cli`); }
      }
    } catch (e) { event("feeder error: " + String(e?.message ?? e).slice(0, 120)); }
    finally { refilling = null; }
  })();
  return refilling;
}
// Why nothing is running, in the owner's terms. The page used to say "Queue empty for this family" beside
// "516 open rows", which read as a hang; the real state is almost always "everything left needs a design".
function idleReason() {
  const ledger = ledgerNow();
  const n = { landed: 0, design: 0, owner: 0, blocked: 0, parked: 0 };
  for (const r of filterRows(ledger.rows, { family: familyOf() })) {
    if (r.status === "verified") continue;
    const f = finished.get(r.id);
    if (parked.has(r.id) || r.status === "parked") n.parked++;
    else if (r.status === "design" && !designs.decided(r.id)) n.design++;
    else if (f?.status === "blocked") { if (isDesignBlock(f)) n.design++; else n.blocked++; }
    else if (f) n.landed++;
    else if (r.readiness.missing.some(o => deadOwners.has(o) && !designs.decided(ownerKey(o)))) n.owner++;
  }
  const parts = [n.landed && `${n.landed} landed and wait for proof or review`, n.design && `${n.design} wait on a design`,
    n.owner && `${n.owner} wait on a parked owner system`, n.blocked && `${n.blocked} blocked`, n.parked && `${n.parked} parked`].filter(Boolean);
  // Every source that can still bring work back, with its time, so a quiet swarm never reads as a hang
  // (2026-11-21: the page said only "design stage is off" while the perf bench sat on its 2 h cooldown).
  const waiting = designWaiting.owner + designWaiting.row + designWaiting.epic;
  const clock = t => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
  const design = !cfg.decide ? `The design stage is off${waiting ? ` (${waiting} item${waiting === 1 ? "" : "s"} wait for it)` : ""}; turn it on in Settings.`
    : designing.size ? `Designing now: ${[...designing.keys()].join(", ")}.`
    : designsThisHour() >= cfg.decideMax ? `The design stage reached its cap of ${cfg.decideMax} an hour and continues as the hour rolls.`
    : waiting || !lastRefill ? "The design stage starts within 30 s."
    : "Nothing is left to design.";
  const nextBench = perfNextAt(root);
  const perf = !cfg.perfFeed ? "" : perfFeeding ? "The perf feeder is building and benchmarking to write measured rows."
    : nextBench > Date.now() ? `Next perf bench at ${clock(nextBench)}.` : "The perf bench starts within 30 s.";
  const chores = cfg.chores && choreScan.at && !choreScan.added ? `The chore scan found nothing new at ${clock(choreScan.at)}; next scan ${clock(choreScan.at + CHORE_EMPTY_MS)}.` : "";
  const stage = state.paused ? "" : [design, perf, chores].filter(Boolean).join(" ");
  return { text: `No row can start: ${parts.join(", ") || "every row in this family is verified"}. ${stage}`.trim(), counts: n };
}
async function workLoop(worker) {
  while (!state.stop && !worker.retire) {
    const disk = brain.diskGuard();
    if (disk.low) { worker.status = "paused"; worker.phase = `low disk ${disk.gb.toFixed(1)} GB`; await new Promise(r => setTimeout(r, 15000)); continue; }
    // Checkpoints and proof runs no longer pause new rows: agents edit while the engine builds and their
    // compile checks simply queue behind it. Pausing idled every worker for up to the whole engine build.
    if (state.paused) { worker.status = "paused"; await new Promise(r => setTimeout(r, 5000)); continue; }
    rollSpend();
    if (overBudget()) {
      const win = budgetWindowMs();
      state.paused = win ? `budget cap $${cfg.budget} per ${cfg.budgetWindowH} h reached ($${spend.total.toFixed(2)} spent); resumes by itself ${new Date(spend.since + win).toLocaleString()}`
        : `budget cap $${cfg.budget} reached ($${spend.total.toFixed(2)} spent since ${new Date(spend.since).toLocaleDateString()}); raise it in Settings`;
      continue;
    }
    if (cfg.maxRows && state.totals.done + state.totals.blocked + active.size >= cfg.maxRows && !retry.size) break;
    // Green-build gate moved to just before the builder: read-only scouts run while the tree is red.
    if (state.stop || state.paused) continue;
    const next = pick();
    // Not awaited. A design takes a minute or two and puts its rows back as it lands; waiting for the whole
    // refill held every idle worker until the slowest design finished.
    if (!next && (cfg.decide || cfg.chores || cfg.perfFeed) && !state.stop) refill();
    if (!next && cfg.prove && !active.size && !idleProved) { idleProved = true; if (proofPending().size) checkpoint("idle: proof of landed rows"); }
    if (!next) {
      // An idle rail shows why it is idle, not the tier and files of the row it finished an hour ago.
      Object.assign(worker, { status: "idle", row: null, label: "", tier: "", files: [], likely: [],
        phase: designing.size ? `waiting on ${designing.size} design${designing.size === 1 ? "" : "s"}` : "nothing to start" });
      await new Promise(r => setTimeout(r, 3000)); continue;
    }
    // Low water: start designing before the queue is empty, not after it.
    if ((cfg.decide || cfg.chores || cfg.perfFeed) && peekCached().length < Math.max(2, state.workers.filter(w => !w.retire && w.status !== "stopped").length)) refill();
    const { row, files, order } = next;
    if (next.proof) {
      retry.delete(row.id); active.set(row.id, { files: [] });
      Object.assign(worker, { status: "working", row: row.id, label: row.label });
      await proofPass(worker, row, next, finished.get(row.id));
      continue;
    }
    const wiring = !!next.wire, continuing = !!next.cont, prior = wiring || continuing ? finished.get(row.id) : null;
    const design = designs.get(row.id), designed = !wiring && !continuing && design?.verdict === "decide";
    const isRetry = retry.has(row.id) || wiring || continuing || designed; // these skip the scout: they have their files or a design
    const tierIndex = wiring || continuing ? 0 : Math.min(tiers.length - 1, retry.get(row.id) ?? startTier(row));
    let tier = wiring && cfg.wireModel === "haiku" ? wireTier : tiers[tierIndex];
    // Always a string: a row requeued after a worker crash has no note, and `undefined` reached the prompt as
    // text and then threw on note.startsWith, which crashed the worker again until the row was parked.
    let note = String((wiring || continuing ? next.note : retry.has(row.id) ? retry.get(row.id + ":note") : "") ?? "");
    if (designed && !/design lead|Owner decision \(auto/.test(note)) note = `${note} ${noteOf(design)}`.trim();
    // Owner systems this row needs that now have a design: the row builds the part it needs from that design.
    const ownerNotes = (row.readiness?.missing ?? []).map(o => designs.get(ownerKey(o)))
      .filter(d => d?.verdict === "decide" && !note.includes(noteOf(d).slice(0, 60))).map(noteOf);
    if (ownerNotes.length) note = `${note} ${ownerNotes.join(" ")}`.trim().slice(0, 2600);
    retry.delete(row.id);
    active.set(row.id, { files });
    Object.assign(worker, { status: "working", row: row.id, label: row.label, phase: "context", activity: [], turns: 0, denied: 0, files: [] });
    const ctx = await rowContext(row, files);
    prewarm();
    let rowCost = 0;
    // Owner rows skip the haiku scout: their design says "the owner has not reviewed it", which the scout read
    // as needs-design and would settle the row without building it.
    if (cfg.scout && !isRetry && !isOwnerRow(row.id)) {
      Object.assign(worker, { status: "working", row: row.id, label: row.label, tier: scoutTier.name, startedAt: Date.now(), activity: [], turns: 0, files: [], likely: files });
      logTail(worker, `scout ${row.id}`);
      const s = await runAgent(worker, row, scoutTier, "", order, ctx);
      rowCost += addCost(s.cost);
      state.totals.in += (s.usage?.input_tokens ?? 0) + (s.usage?.cache_read_input_tokens ?? 0) + (s.usage?.cache_creation_input_tokens ?? 0);
      state.totals.out += s.usage?.output_tokens ?? 0; state.totals.cached += s.usage?.cache_read_input_tokens ?? 0;
      // A scout killed by Stop/Park fell through into waitForGreen's cargo check before noticing the stop.
      if (state.stop || parked.has(row.id)) { active.delete(row.id); worker.child = null; logTail(worker, "stopped after scout; not recorded"); if (state.stop) break; continue; }
      const verdict = parseScout(s.text);
      if (limit.test((s.error || "") + " " + (s.text || ""))) { active.delete(row.id); retry.set(row.id, tierIndex); retry.set(row.id + ":note", note); pauseForLimit((s.error || "") + " " + (s.text || ""), "scout"); continue; }
      // Cheap models over-block: a needs-design verdict must cite an explicit design marker, else build.
      if (verdict?.verdict === "needs-design" && !/proposal|proposed|undecided|open question|tbd|design decision|owner decision|protocol|lore|proper noun|['"“]/i.test(verdict.evidence ?? "")) verdict.verdict = "build";
      if (verdict?.verdict === "already-implemented" || verdict?.verdict === "needs-design") {
        active.delete(row.id); worker.child = null;
        const entry = { id: row.id, label: row.label, tier: scoutTier.name, at: Date.now(), ms: Date.now() - worker.startedAt, files: verdict.files ?? [],
          summary: `scout: ${verdict.evidence || verdict.plan || verdict.verdict}`.slice(0, 240), status: verdict.verdict === "needs-design" ? "blocked" : "already-implemented",
          log: worker.log, tokens: s.usage?.input_tokens ?? 0, cost: rowCost, final: true, why: verdict.verdict === "needs-design" ? "design" : undefined };
        finished.set(row.id, entry);
        if (entry.status === "blocked") state.totals.blocked++; else state.totals.done++;
        record(entry); persist(); logTail(worker, `scout settled: ${entry.status}`);
        continue;
      }
      if (verdict?.verdict === "build") note = `Scout (read-only) says edit: ${(verdict.files ?? []).join(", ")}. Plan: ${String(verdict.plan ?? "").slice(0, 500)} Start from these files; skip broad exploration.`;
      if (verdict?.verdict === "build" && cfg.haikuData && tierIndex === 0 && dataOnly(verdict.files)) tier = dataTier;
    }
    // Fable rows build (and continue) on Fable; wire passes stay on the cheap wire tier.
    if (!wiring && isFableRow(row.id)) tier = isOwnerRow(row.id) ? ownerTier : fableTier;
    await waitForGreen(worker);
    if (state.stop) { active.delete(row.id); break; }
    // Live coordination context: what peers are editing now and what already landed in these files.
    const mine = new Set([...(files ?? []), ...(parseFilesFromNote(note))]);
    const peers = state.workers.filter(w => w !== worker && w.status === "working" && w.row).map(w => `${w.row}: ${(w.files?.length ? w.files : w.likely ?? []).slice(0, 4).join(", ")}`).filter(s => !s.endsWith(": "));
    const related = state.results.filter(r => r.final !== false && /building|playable/.test(r.status) && (r.files ?? []).some(f => mine.has(f))).slice(0, 4)
      .map(r => `${r.id} (${(r.files ?? []).filter(f => mine.has(f)).join(", ")}): ${String(r.summary).slice(0, 140)}`);
    if (peers.length) note += ` Peers editing now (do not touch their hunks): ${peers.join(" ; ")}.`;
    if (related.length) note += ` Recently landed in these files (build on it, do not redo): ${related.join(" ; ")}.`;
    Object.assign(worker, { status: "working", row: row.id, label: row.label, tier: tier.name, startedAt: Date.now(), activity: [], turns: 0, files: [], likely: files });
    logTail(worker, `start ${row.id} on ${tier.name}`);
    let out;
    for (let attempt = 0; ; attempt++) {
      out = await runAgent(worker, row, tier, note, order, ctx);
      const text = (out.error || "") + " " + (out.err || "");
      const transient = !out.ok && !limit.test(text) && (out.stalled || out.noResult || /overloaded|529|5\d\d|ECONN|ETIMEDOUT|ENOTFOUND|socket|network|fetch failed|connection|timed out|stream/i.test(text));
      if (!transient || attempt >= 3 || state.stop) break;
      const wait = [5, 15, 45][attempt] * 1000;
      worker.phase = "retrying";
      logTail(worker, `connection problem (${(text.trim().slice(0, 60) || (out.stalled ? "stalled" : "no result"))}); retry ${attempt + 1}/3 in ${wait / 1000}s`);
      await new Promise(r => setTimeout(r, wait));
    }
    active.delete(row.id);
    worker.child = null;
    if (state.stop && !out.ok) { logTail(worker, "stopped mid-row; not recorded"); break; }
    const tokens = (out.usage?.input_tokens ?? 0) + (out.usage?.cache_read_input_tokens ?? 0) + (out.usage?.cache_creation_input_tokens ?? 0);
    state.totals.in += tokens; state.totals.out += out.usage?.output_tokens ?? 0; rowCost += addCost(out.cost);
    state.totals.cached += out.usage?.cache_read_input_tokens ?? 0;
    const parsed = parseResult(out.text);
    const entry = { id: row.id, label: row.label, tier: tier.name, at: Date.now(), ms: Date.now() - worker.startedAt, files: parsed?.files?.length ? parsed.files : out.files,
      summary: parsed?.summary ?? (out.error || out.text || "no result").toString().slice(0, 240), status: parsed?.status ?? (out.ok ? "building" : "failed"), log: worker.log, tokens, cost: rowCost,
      turns: worker.turns, cached: out.usage?.cache_read_input_tokens ?? 0, out: out.usage?.output_tokens ?? 0,
      wired: parsed?.wired, entry: parsed?.entry, why: parsed?.why, tests: parsed?.tests, estimated: out.estimated || undefined, denied: out.denied || undefined };
    if (limit.test(out.error || out.text || "")) {
      // Sleep until the provider's stated reset instead of relaunching failing agents every 10 minutes. The row
      // keeps its note (scout plan, design): it used to come back with none and pay for a second scout.
      retry.set(row.id, tierIndex); retry.set(row.id + ":note", note.split(" Peers editing now")[0]);
      // A Fable limit (weekly) is that model's alone: park Fable rows until its reset and keep the cheap
      // tiers (sonnet proof, haiku wire) working. 2026-11-20 one Fable limit idled all five workers for days.
      if (tier.model === cfg.fable && cfg.fable !== cfg.sonnet) {
        state.fableUntil = limitResetAt(out.error || out.text || "");
        event(`fable limit; Fable rows wait until ${new Date(state.fableUntil).toLocaleString()}, other tiers continue`);
      } else pauseForLimit(out.error || out.text || "", "builder");
      logTail(worker, "paused: usage limit");
      continue;
    }
    if (/CLI not found/.test(out.error ?? "")) { state.stop = true; state.paused = out.error; break; }
    if (retryPassIds.has(row.id)) entry.retryPass = true;
    if (entry.status === "already-implemented" && (wiring || continuing)) {
      // A follow-up pass found the slice already reachable (wire) or nothing left to add (continue): keep the
      // landed result, mark it wired, and stop further passes of that kind.
      const kept = { ...prior, wired: true, entry: entry.entry ?? prior?.entry, wirePass: true, cont: continuing ? cfg.maxCont : prior?.cont, contDone: continuing || prior?.contDone, final: true };
      record({ ...entry, final: false, summary: (continuing ? "continue pass: " : "wire pass: ") + entry.summary });
      finished.set(row.id, kept); appendFileSync(resultsFile, JSON.stringify(kept) + "\n"); persist(); continue;
    }
    if (entry.status === "already-implemented") {
      entry.final = true; finished.set(row.id, entry); state.totals.done++; record(entry); persist(); continue;
    }
    const good = (entry.status === "building" || entry.status === "playable") && entry.files.length > 0;
    if (tier === dataTier && !good) {
      // Haiku did not land the data row: hand its report and any partial edits to a normal sonnet run.
      retry.set(row.id, 0); retry.set(row.id + ":note", (`haiku tried first and reported: ${entry.summary}`.slice(0, 300) + (entry.files.length ? ` It already edited ${entry.files.join(", ")}; finish or correct that.` : "") + " " + note.split(" Peers editing now")[0]).slice(0, 1200));
      record({ ...entry, status: "escalated", final: false });
      continue;
    }
    if (wiring) {
      // One wire pass per row. A failed pass keeps the earlier landed slice as the row's result.
      entry.wirePass = true;
      // An explicit wired:true wins over summary keywords ("no unused default" matched UNWIRED). Many
      // candidates honestly have no possible caller and fail on any model, so haiku (~$0.004 a pass) is
      // never swapped for sonnet; only sonnet wire passes switch off when they stop paying.
      wireStats.n++; if (good && (entry.wired === true || (entry.wired === undefined && !UNWIRED.test(entry.summary ?? "")))) wireStats.wired++;
      if (cfg.wire && cfg.wireModel === "sonnet" && wireStats.n >= 6 && wireStats.wired / wireStats.n < 1 / 3) {
        cfg.wire = false; event(`wire pass off: ${wireStats.wired}/${wireStats.n} wired on sonnet; turn it on in Settings`);
      }
      if (!good) {
        record({ ...entry, status: "blocked", summary: "wire pass: " + entry.summary, final: false });
        finished.set(row.id, { ...prior, wirePass: true });
        appendFileSync(resultsFile, JSON.stringify({ ...prior, wirePass: true, final: true }) + "\n");
        persist(); continue;
      }
      entry.files = [...new Set([...(prior?.files ?? []), ...entry.files])];
    }
    if (continuing) {
      // A failed continuation keeps the earlier result and stops further continues of that row.
      entry.cont = (prior?.cont ?? 0) + 1; entry.wirePass = prior?.wirePass;
      if (!good) {
        record({ ...entry, status: "blocked", summary: `continue pass ${entry.cont}: ` + entry.summary, final: false });
        // A pass that edited nothing because a peer held the file is not the end of the row (ENG-VOX-10 lost
        // its remaining slices to an ENG-MESH-10 run). It waits 15 minutes and tries again, three times at most.
        const peerBusy = !entry.files.length && (prior?.contSkips ?? 0) < 3 && /\bpeers?\b|being edited|editing (?:it |this |that |them )?(?:right )?now|another agent|concurrent/i.test(entry.summary ?? "");
        const kept = peerBusy ? { ...prior, contAfter: Date.now() + 15 * 60_000, contSkips: (prior?.contSkips ?? 0) + 1 } : { ...prior, cont: cfg.maxCont, contDone: true };
        finished.set(row.id, kept);
        appendFileSync(resultsFile, JSON.stringify({ ...kept, final: true }) + "\n");
        persist(); continue;
      }
      entry.files = [...new Set([...(prior?.files ?? []), ...entry.files])];
      if (entry.wired === undefined) entry.wired = prior?.wired;
    }
    // A stronger model cannot invent a missing spec or a save-format bump: stop those at once.
    // Cost: a stronger model re-explores the same missing owner and fails the same way (11 of 17 opus runs).
    const noOwner = !entry.files.length && (entry.why === "no_owner" || /explored \d+ turns without an edit|owner code not located/i.test(entry.summary ?? ""));
    const hardBlock = entry.why === "design" || designBlock.test(entry.summary ?? "") || noOwner;
    const repeat = !good && lastFail.has(row.id) && sameFailure(lastFail.get(row.id), entry.summary);
    if (!good) lastFail.set(row.id, entry.summary ?? "");
    // Zero-edit failures ("too wide", "out of budget") never landed on opus: it re-explores at medium effort and
    // hits the no-edit cap (16 of 22 before the fix, 2 of 2 after, ~$0.35 each). They get ONE cheap sonnet retry
    // forced to the narrowest slice instead; opus is kept for rows that edited code and then failed.
    if (!good && !hardBlock && !repeat && !entry.files.length && !forced.has(row.id)) {
      forced.add(row.id);
      const plan = note.startsWith("Scout") ? " " + note.split(" Peers editing now")[0] : "";
      retry.set(row.id, 0); retry.set(row.id + ":note", (`Previous attempt made NO edit and said: ${entry.summary}`.slice(0, 300)
        + " That scope is too wide on purpose: land ONLY the narrowest slice that compiles (the RON rows + a typed loader/struct + one real caller, or one mode on an existing kind). Do not re-explore: start from the files below, make your first Edit within 6 tool calls." + plan).slice(0, 1200));
      record({ ...entry, status: "escalated", final: false, summary: "retrying narrower on sonnet: " + entry.summary });
      continue;
    }
    if (!good && !hardBlock && !repeat && entry.files.length && tier !== fableTier && tierIndex < tiers.length - 1) {
      const plan = note.startsWith("Scout") ? " " + note.split(" Peers editing now")[0] : "";
      retry.set(row.id, tierIndex + 1); retry.set(row.id + ":note", (`${tier.name} reported: ${entry.summary}`.slice(0, 300) + plan).slice(0, 1200));
      record({ ...entry, status: "escalated", final: false });
      continue;
    }
    entry.final = true; entry.status = good ? entry.status : "blocked";
    finished.set(row.id, entry);
    // Circuit breaker: a run of rows that do not land means something systemic (red tree, bad prompt,
    // provider trouble). Pause and say so instead of spending the budget on it.
    failStreak = good ? 0 : failStreak + 1;
    if (good) breakerTrips = 0;
    if (cfg.breaker > 0 && failStreak >= cfg.breaker && !state.paused) {
      const why = `circuit breaker: ${failStreak} rows in a row did not land`;
      state.paused = why + ". Checking the tree";
      event(state.paused); failStreak = 0;
      // Self-repair. The usual systemic cause is a tree that stopped compiling: if the checkpoint finds that and
      // repairs it, the swarm resumes by itself. If the tree compiled all along the cause is elsewhere (prompt,
      // provider), and spending on it without the owner is what the breaker exists to prevent.
      checkpoint("circuit breaker").then(r => {
        if (!String(state.paused ?? "").startsWith("circuit breaker")) return;
        if (r?.repaired && r.ok) { state.paused = null; event("circuit breaker: the tree was red and is repaired; resumed"); return; }
        if (r && !r.ok) { state.paused = why + ". The tree does not compile and the repair did not fix it. Fix it, then press Resume"; event(state.paused); return; } // the health gate resumes once it compiles
        // Tree compiles: the cause is elsewhere (prompt, provider, a run of design blocks). An unattended swarm used
        // to stay paused for good here. It now resumes after a cooldown that doubles per trip (30 min .. 8 h) and
        // resets when a row lands, so a lasting fault costs at most `breaker` rows per cooldown.
        const wait = Math.min(8 * 3600_000, 30 * 60_000 * 2 ** breakerTrips++);
        const at = new Date(Date.now() + wait).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
        state.paused = why + `. The tree compiles, so the cause is elsewhere; read the supervisor log and lessons. Resumes by itself at ${at}, or press Resume`;
        event(state.paused);
        clearTimeout(breakerTimer);
        breakerTimer = setTimeout(() => {
          if (!String(state.paused ?? "").startsWith("circuit breaker") || breakerRed()) return;
          state.paused = null; event("circuit breaker: cooldown over; resumed");
        }, wait);
      });
    }
    if (good) {
      if (wiring) state.totals.wired = (state.totals.wired ?? 0) + 1;
      else if (continuing) state.totals.cont = (state.totals.cont ?? 0) + 1;
      else state.totals.done++;
      sinceCheckpoint++;
      const flat = v => String(v ?? "").replace(/\s+/g, " ").trim();
      try { appendFileSync(path.join(dir, "changes.md"), `${row.id} | ${entry.files.join(", ")} | ${flat(entry.summary)} | ${flat(entry.tests) || "-"}\n`); } catch { /* best effort */ }
      if (entry.wired === false || (entry.wired === undefined && UNWIRED.test(entry.summary ?? ""))) { state.totals.scaffold = (state.totals.scaffold ?? 0) + 1; entry.unwired = true; }
      if (!parsed?.scenario && prior?.scenario) entry.scenario = prior.scenario; // a wire/continue pass keeps the row's proof
      let proofWhy = parsed?.scenario ? null : entry.scenario || /no observable effect/i.test(entry.summary ?? "") ? null : "no scenario returned";
      if (parsed?.scenario) {
        const bad = validateScenario(parsed.scenario, ENGINE_OPS);
        if (bad) { proofWhy = bad; event(`${row.id}: scenario not stored (${bad})`); }
        else try {
          const f = scenarioFile(row.id, parsed.scenario, entry.summary);
          mkdirSync(scenarioPath, { recursive: true });
          writeFileSync(path.join(scenarioPath, f.name), f.body);
          entry.scenario = "tools/axiom-dev/scenarios/" + f.name; idleProved = false;
        } catch (error) { proofWhy = "not stored"; event(`${row.id}: scenario not stored (${String(error.message).slice(0, 80)})`); }
      }
      // Send back once: a read-only proof pass writes the scenario before new work starts (proofCandidates).
      if (proofWhy && cfg.prove) Object.assign(entry, { needsProof: true, proofWhy });
      if (entry.scenario) state.totals.scenarios = (state.totals.scenarios ?? 0) + 1;
      try {
        const ledger = loadLedger();
        writeStatus(row.id, { status: "building", evidence: entry.files.slice(0, 6), note: "swarm: " + entry.summary.slice(0, 160) }, ledger.revision, new Set(ledger.rows.map(r => r.id)));
        ledgerChanged();
      } catch (error) { entry.ledger = "not recorded: " + error.message.slice(0, 80); }
    } else state.totals.blocked++;
    record(entry);
    persist();
    brain.learn(entry).catch(() => {});
    if (good && cfg.checkpointEvery > 0 && sinceCheckpoint >= cfg.checkpointEvery && !state.checkpointing) sinceCheckpoint = 0, checkpoint(`auto: ${cfg.checkpointEvery} rows landed`);
  }
  worker.status = "stopped";
}

// Self-correction: quiesce, compile the whole tree once, and send any compiler errors to a repair agent.
async function checkpoint(why) {
  if (state.checkpointing || state.stop) return;
  state.checkpointing = why;
  event(`checkpoint: ${why}`);
  try {
    // No drain and no pause: the shared check compiles the whole tree with every agent's edits (and loads the
    // content tables), so a quiet tree is not needed. Draining used to idle all five workers for the length of
    // the slowest run on every checkpoint. Errors inside files a running agent is editing are its work in
    // progress and stay with it; only orphaned errors go to the repair agent.
    let result = await sharedCheck(APP), repaired = false;
    for (let round = 1; !result.ok && result.errors.length && round <= 2 && !state.stop; round++) {
      const editing = state.workers.filter(w => w.child).flatMap(w => w.files ?? []);
      const orphaned = result.errors.filter(e => !editing.some(f => e.replaceAll("\\", "/").includes(f)));
      if (!orphaned.length) { event(`checkpoint: ${result.errors.length} error(s), all in files agents are editing now; left to them`); break; }
      // A red tree stops every builder, so the second round is the one place the strongest model is the cheap
      // choice: round 1 stays on the builder tier (most breaks are a missed call site), and only what it could
      // not fix goes to Fable, once, under the same cost cap as a Fable row. --no-fable-repair keeps both cheap.
      const repairTier = round === 1 || !cfg.fableRepair ? tiers[0] : { ...fableTier, name: "fable·repair" };
      event(`checkpoint: ${orphaned.length} orphaned error(s); repair agent round ${round} on ${repairTier.name}`);
      const fixer = { id: "fix", activity: [], turns: 0, files: [] };
      const out = await runAgent(fixer, { id: "CHECKPOINT", label: "Fix compile errors" }, repairTier, "NOT a feature row. The tree no longer compiles or a content table no longer loads; fix ONLY these errors with the smallest edits, add no behaviour, never revert another agent's work. Report status building with the files you changed.", orphaned.join("\n").slice(0, 3500));
      addCost(out.cost);
      repaired = true;
      result = await sharedCheck(APP);
    }
    event(result.ok ? "checkpoint: tree compiles and tables load" : `checkpoint: still failing (${result.errors.length} errors); see the shared check`);
    state.lastCheckpoint = { at: Date.now(), ok: result.ok, errors: result.errors.slice(0, 5) };
    // The checkpoint just compiled the same package the health gate checks; share it instead of re-checking.
    if (result.ok || result.errors.length) setHealth(result.ok, result.errors);
    if (result.ok) await prove(/proof|supervisor/.test(why), /idle/.test(why));
    // The caller learns whether the tree is green and whether a repair agent had to run (circuit breaker).
    return { ok: result.ok, repaired };
  } finally { state.checkpointing = null; }
}

// ---- live engine proof (no model cost) ----
// Landed rows that carry a tagged scenario are run headless in a freshly built engine. A pass is recorded
// as `proven`; a failure goes back to the row once, with the observed values, so the agent refines its
// own work against real engine behaviour instead of a compile check alone.
const proven = new Set(), proofRetried = new Set();
const scenarioPath = path.join(root, "tools/axiom-dev/scenarios");
// Proof builds use their own target dir: in the shared target/ the debug build + link held cargo's build
// lock for many minutes and every agent's compile check queued behind it (7 of 7 agents stalled 4-6 min).
// sccache stays on for this build (the repo's own config), so a cold build reuses cached dependencies.
const proofTarget = path.join(root, "target", "proof");
const debugExe = path.join(proofTarget, "debug", "minecraft_style_voxel_prototype.exe");
let proofChild = null;
const killProof = () => { if (proofChild?.pid && proofChild.exitCode === null) { if (process.platform === "win32") spawnSync("taskkill", ["/T", "/F", "/PID", String(proofChild.pid)], { windowsHide: true, stdio: "ignore" }); else proofChild.kill(); } };
process.on("exit", killProof);
const proofEnv = () => { const env = { ...childEnv(), CARGO_TARGET_DIR: proofTarget }; for (const k of ["RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) delete env[k]; return env; };
const proofPending = () => pendingProof(finished.values(), tolerantScenarios(scenarioPath).scenarios, proven);
function buildEngine(timeoutMs = 40 * 60_000) {
  return new Promise(resolve => {
    const [cmd, ...args] = buildCommand.split(" ");
    const child = spawn(cmd, args, { cwd: root, windowsHide: true, env: proofEnv(), stdio: ["ignore", "ignore", "pipe"] });
    // Stop all and supervisor exit end the build too; on Windows an orphaned cargo kept running.
    proofChild = child; child.once("close", () => { if (proofChild === child) proofChild = null; });
    let err = "";
    child.stderr.setEncoding("utf8").on("data", c => { err = (err + c).slice(-4000); });
    // Tree-kill on timeout and settle on exit: a plain kill left rustc holding the pipe and proof hung.
    settleOnce(child, code => resolve({ ok: code === 0, note: err.split(/\r?\n/).filter(l => /error/.test(l)).slice(0, 2).join(" ") || (code === null ? "build timed out" : "") }), timeoutMs);
  });
}
function sendBack(id, entry, summary, note, files = []) {
  const base = { id, label: entry.label, tier: "engine·proof", at: Date.now(), ms: 0, files, log: entry.log, tokens: 0, cost: 0 };
  if (proofRetried.has(id)) {
    // Blocked in memory too, as a restart would load it: the landed entry stayed pending, so every later proof
    // run re-ran the same failing scenario and recorded the row as blocked again.
    const blocked = { ...base, status: "blocked", summary: summary.replace("sent back", "twice"), final: true };
    record(blocked); finished.set(id, blocked); state.totals.blocked++; return;
  }
  proofRetried.add(id);
  record({ ...base, status: "escalated", summary, final: false });
  brain.learn({ id, label: entry.label, status: "failed", summary: "engine mismatch: " + summary }, "proof").catch(() => {});
  requeue(id, 0, note);
}
// The scenario asked the engine the wrong question (a field the reply does not have, a request the engine
// refused). The landed code is not at fault, so no builder is paid to "fix" it and the row is not blocked:
// the scenario file is dropped and the read-only proof pass writes a new one with the engine's answer in hand.
// One rewrite per row; after a second mismatch the row stays landed, marked as having no working proof.
function rescript(id, entry, action) {
  for (const f of action.files) { try { rmSync(path.join(root, f), { force: true }); } catch { /* already gone */ } }
  taggedCache.at = 0;
  const detail = action.summary.replace(/^live proof failed[^:]*:\s*/, "").slice(0, 260);
  const again = !entry.proofRescripted;
  const kept = { ...entry, scenario: undefined, proofPass: again ? undefined : true, proofRetried: undefined, final: true,
    ...(again ? { needsProof: true, proofRescripted: true, proofWhy: "it ran in the built engine and the reply did not have what it expected: " + detail }
      : { needsProof: undefined, noProof: ("two scenarios did not match the engine's replies: " + detail).slice(0, 200) }) };
  finished.set(id, kept);
  appendFileSync(resultsFile, JSON.stringify(kept) + "\n");
  record({ id, label: entry.label, tier: "engine·proof", at: Date.now(), ms: 0, files: action.files, log: entry.log, tokens: 0, cost: 0, status: "escalated", final: false,
    summary: (again ? "scenario did not match the engine; rewriting it: " : "no working scenario after a rewrite; row stays landed: ") + detail });
  if (again) brain.learn({ id, label: entry.label, status: "failed", summary: "engine mismatch: " + detail }, "proof").catch(() => {});
}
async function prove(manual = false, idle = false) {
  if (!cfg.prove || state.stop) return;
  const { scenarios, invalid } = tolerantScenarios(scenarioPath);
  for (const bad of invalid) event(`proof: skipped unreadable scenario ${bad.file} (${bad.error})`);
  const pending = pendingProof(finished.values(), scenarios, proven);
  if (!shouldProve({ pending: pending.size, batch: cfg.proveBatch, manual, idle })) {
    if (pending.size) event(`proof: ${pending.size} row(s) waiting; runs at ${cfg.proveBatch}, when idle, or on Prove now`);
    return;
  }
  // A debug engine build writes gigabytes; never start one on a nearly full disk.
  // Reclaim stale incremental caches first: the guard only reclaimed below 3.5 GB, so 3.5-8 GB free skipped every proof.
  const free = brain.ensureFree(8);
  if (free < 8) { const gb = (Math.floor(free * 10) / 10).toFixed(1); state.lastProof = { at: Date.now(), ok: false, note: `skipped: ${gb} GB free after reclaiming caches, the engine build needs 8` }; event(`proof: skipped, only ${gb} GB free after reclaim (needs 8 for the engine build)`); return; }
  const run = [...new Map([...pending.values()].flat().map(s => [s.id, s])).values()];
  state.checkpointing = `building the engine to prove ${pending.size} row(s)`;
  event("proof: " + state.checkpointing);
  let built = await buildEngine();
  // `undefined symbol: anon.<hash>.llvm.<n>` at link is a stale incremental codegen cache for the app (several
  // builds and checks share target/). Drop the app's incremental sessions not touched in 2 minutes and retry once.
  if (!built.ok && /undefined symbol|LNK20(01|19)/.test(built.note ?? "")) {
    const inc = path.join(proofTarget, "debug", "incremental");
    let dropped = 0;
    try {
      for (const d of readdirSync(inc).filter(d => d.startsWith("minecraft_style_voxel_prototype-"))) {
        try { const p = path.join(inc, d); if (Date.now() - statSync(p).mtimeMs > 120_000) { rmSync(p, { recursive: true, force: true }); dropped++; } } catch { /* in use */ }
      }
    } catch { /* no incremental dir */ }
    event(`proof: link failed on stale incremental code; dropped ${dropped} app cache dir(s), rebuilding once`);
    built = await buildEngine();
  }
  if (!built.ok) { state.lastProof = { at: Date.now(), ok: false, note: "engine build failed" }; event("proof: engine build failed; rows stay unproven. " + (built.note ?? "")); return; }
  state.checkpointing = `running ${run.length} scenario(s) in the engine`;
  let results;
  try { results = await runScenarios(run, { timeoutMs: 180_000, exe: existsSync(debugExe) ? debugExe : undefined }); }
  catch (e) {
    // The engine refused to start or died: usually a content table that no longer validates.
    const message = String(e.message).slice(0, 240), blame = blameContent(message, finished.values(), proven);
    state.lastProof = { at: Date.now(), ok: false, note: message };
    if (!blame.ids.length) { event(`proof: the engine rejected the run and no landed row edited ${blame.table ? "assets/" + blame.table + ".ron" : "a named table"}: ${message}`); return; }
    for (const id of blame.ids) sendBack(id, finished.get(id), `content check failed, sent back: ${message}`,
      `Your change to assets/${blame.table}.ron no longer loads in the engine. Fix the table (ids must exist in the tables they reference). Engine said: ${message}`, [`assets/${blame.table}.ron`]);
    event(`proof: assets/${blame.table}.ron does not load; sent back ${blame.ids.join(", ")}`);
    persist(); return;
  }
  let pass = 0, fail = 0, rewrite = 0;
  for (const action of settleProof(pending, results, proofRetried)) {
    const entry = finished.get(action.id);
    if (action.kind === "proven") {
      proven.add(action.id); pass++; state.totals.proven++; entry.proven = true;
      record({ id: action.id, label: entry.label, tier: "engine·proof", at: Date.now(), ms: 0, files: action.files, log: entry.log, tokens: 0, cost: 0, status: "proven", summary: action.summary, final: true });
      // A passing scenario in the built engine is what "playable" means on the board. Proof only ever wrote the
      // swarm's own result, so 62 proven rows still read "building" and the board showed 0 playable.
      promotePlayable(action.id, action.files, action.summary);
    } else if (action.fault === "scenario" && action.files.every(f => path.basename(f).startsWith("swarm-"))) { rewrite++; rescript(action.id, entry, action); }
    else { fail++; sendBack(action.id, entry, action.summary, action.note ?? action.summary, action.files); }
  }
  state.lastProof = { at: Date.now(), ok: fail === 0, pass, fail };
  event(`proof: ${pass} row(s) proven, ${fail} sent back${rewrite ? `, ${rewrite} scenario(s) to rewrite` : ""}`);
  persist();
}

function promotePlayable(id, files, summary) {
  try {
    const ledger = loadLedger();
    const row = ledger.rows.find(r => r.id === id);
    if (!row || /^(playable|verified|parked)$/.test(row.status)) return;
    writeStatus(id, { status: "playable", evidence: (files ?? []).filter(f => existsSync(path.join(root, String(f).split("#")[0]))).slice(0, 6), note: "swarm proof: " + String(summary ?? "").slice(0, 160) }, ledger.revision, new Set(ledger.rows.map(r => r.id)));
    ledgerChanged();
  } catch (error) { event(`${id}: proven but not marked playable (${String(error.message).slice(0, 80)})`); }
}
// Rows proven before promotion existed: settle them once at start-up.
setTimeout(() => {
  for (const [id, r] of finished) if (r.status === "proven" || r.proven) { proven.add(id); promotePlayable(id, r.files, "proven earlier"); }
}, 2000).unref?.();

// ---- supervisor read models (bounded, read-only) ----
const resultFor = id => state.results.find(r => r.id === id) ?? finished.get(id);
// Tail of a stream-json agent log, turned into short readable lines.
function readableLog(rel, maxBytes = 400_000) {
  const file = path.join(root, rel ?? "");
  if (!rel || !path.resolve(file).startsWith(path.resolve(dir)) || !existsSync(file)) return [];
  const fd = openSync(file, "r");
  let text = "";
  try {
    const size = fstatSync(fd).size, start = Math.max(0, size - maxBytes), buf = Buffer.alloc(size - start);
    readSync(fd, buf, 0, buf.length, start); text = buf.toString("utf8");
  } finally { closeSync(fd); }
  const lines = [];
  for (const line of text.split("\n")) {
    let ev; try { ev = JSON.parse(line); } catch { continue; }
    if (ev.type === "assistant") for (const b of ev.message?.content ?? []) {
      if (b.type === "text" && b.text.trim()) lines.push({ k: "say", t: b.text.trim().slice(0, 1200) });
      else if (b.type === "tool_use") {
        const i = b.input ?? {};
        lines.push({ k: /^(Edit|Write)$/.test(b.name) ? "edit" : b.name === "Bash" ? "run" : "look",
          t: `${b.name} ${String(i.command ?? i.file_path ?? i.pattern ?? i.path ?? "").slice(0, 220)}` });
      }
    } else if (ev.type === "user") for (const b of ev.message?.content ?? []) {
      if (b.type === "tool_result") {
        const c = Array.isArray(b.content) ? b.content.map(x => x.text ?? "").join(" ") : String(b.content ?? "");
        if (b.is_error || /error(\[E\d+\])?:/.test(c)) lines.push({ k: "err", t: c.slice(0, 400) });
      }
    } else if (ev.type === "result") lines.push({ k: "done", t: `result · ${ev.num_turns ?? "?"} turns · $${(ev.total_cost_usd ?? 0).toFixed(3)}` });
  }
  return lines.slice(-300);
}
// Working-tree diff for the files a result reported (other sessions' hunks may show too).
function diffFor(files) {
  const safe = (files ?? []).filter(f => /^[\w./-]+$/.test(f) && !f.includes("..")).slice(0, 12);
  if (!safe.length) return "";
  const out = spawnSync("git", ["diff", "--no-color", "--stat", "--patch", "--", ...safe], { cwd: root, encoding: "utf8", maxBuffer: 8 * 1024 * 1024 });
  const untracked = spawnSync("git", ["ls-files", "--others", "--exclude-standard", "--", ...safe], { cwd: root, encoding: "utf8" }).stdout.trim();
  return (out.stdout || "").slice(0, 120_000) + (untracked ? `\n# untracked (new) files:\n${untracked}\n` : "");
}
function setWorkers(n) {
  const target = Math.min(16, Math.max(1, n));
  let live = state.workers.filter(w => !w.retire && w.status !== "stopped");
  while (live.length < target) {
    if (state.stop && !state.userStop) state.stop = false;
    const w = { id: "w" + (state.workers.length + 1), status: "idle", row: null, activity: [], turns: 0, files: [] };
    state.workers.push(w); work(w); live.push(w);
  }
  for (const w of [...live].reverse()) { if (live.length <= target) break; w.retire = true; live = live.filter(x => x !== w); }
}
function readBody(req) {
  return new Promise(resolve => { let b = ""; req.on("data", c => { b += c; if (b.length > 8192) req.destroy(); }); req.on("end", () => { try { resolve(JSON.parse(b || "{}")); } catch { resolve({}); } }); });
}
const ROW_ID = /^[\w.-]{1,40}$/;
const DESIGN_KEY = /^(?:owner:)?[\w.-]{1,40}$/;

const pageUrl = new URL("./swarm.html", import.meta.url); // the previous dashboard stays at /classic
// peek() reads the whole ledger, so the push loop shares one result for ten seconds.
let peekCache = { at: 0, rows: [] };
const peekCached = () => { if (Date.now() - peekCache.at > 10_000) peekCache = { at: Date.now(), rows: peek() }; return peekCache.rows; };
// The queue is computed first: it also refreshes state.deadOwners, which the spread then carries.
// The design stage as the page shows it: what runs now, what waits, the hourly cap and the latest designs.
const designView = () => (countDesignWaiting(), { on: !!cfg.decide, cap: cfg.decideMax, hour: designsThisHour(), waiting: designWaiting, tier: designTier.name,
  running: [...designing].map(([key, d]) => ({ key, kind: d.kind, since: d.since, doing: d.slot?.activity?.at(-1)?.text ?? "" })),
  recent: designs.all().filter(d => d.at).sort((a, b) => b.at - a.at).slice(0, 6)
    .map(d => ({ id: d.id, kind: d.kind ?? "row", verdict: d.verdict, text: String(d.text ?? "").slice(0, 200), at: d.at })) });
// `idle` is set only when nothing is queued and no agent works, so the page can say why instead of "queue empty".
function idleView(queue) {
  if (queue.length || state.workers.some(w => w.status === "working" || w.status === "waiting for green build")) return null;
  try { return idleReason(); } catch { return null; }
}
const stateView = (queue = peekCached()) => ({ ...state, rev, now: Date.now(), workers: state.workers.map(({ child, ...w }) => w), queue,
  parked: [...parked], lessons: brain.lessons().slice(0, 12), families: Object.keys(FAMILIES), design: designView(), idle: idleView(queue) });
// One broadcaster for every open page. The signature is rev.act.workers: pages fetch the full state
// when rev moves and the light view otherwise. Nothing is serialised to decide whether to send.
const listeners = new Set();
const signature = () => `${rev}.${act}.${state.workers.map(w => String(w.status ?? "-")[0] + (w.row ?? "") + (w.turns ?? 0)).join("")}`;
let sentSig = "", quiet = 0;
setInterval(() => {
  if (!listeners.size) return;
  const sig = signature();
  let msg = "";
  if (sig !== sentSig) { sentSig = sig; quiet = 0; msg = `data: ${sig}\n\n`; } else if (++quiet >= 30) { quiet = 0; msg = ": beat\n\n"; }
  if (msg) for (const res of listeners) res.write(msg);
}, 500).unref();
let engineCache = { at: 0, body: null };
// Implementation prompt for a row: its section in the engine review, else its matrix note.
function promptFor(id) {
  try {
    const review = readFileSync(path.join(root, "docs", "research", "ENGINE_OVERHAUL_REVIEW.md"), "utf8");
    const at = review.indexOf("### " + id + "\n") >= 0 ? review.indexOf("### " + id + "\n") : review.indexOf("### " + id + "\r\n");
    if (at >= 0) { const end = review.slice(at + 4).search(/\n(?:### |# |---)/); return review.slice(at, end < 0 ? undefined : at + 4 + end).trim(); }
    const row = loadLedger().rows.find(r => r.id === id);
    return row ? `${row.id} ${row.label}\n\n${row.mapping}\n\n${row.notes}` : "unknown row";
  } catch (error) { return "prompt unavailable: " + String(error.message ?? error); }
}
// Latest panics from the game's crash log (saves/crash.log): time, site and first message line.
function recentCrashes(count) {
  try {
    const file = path.join(root, "saves", "crash.log");
    if (!existsSync(file)) return [];
    const text = readFileSync(file, "utf8").slice(-400_000);
    const out = [];
    for (const m of text.matchAll(/unix_time=(\d+)\r?\nthread=[^\r\n]*\r?\npanicked at ([^\r\n]+?):\r?\n([^\r\n]*)/g)) {
      out.push({ at: Number(m[1]) * 1000, site: m[2].replaceAll("\\", "/").replace(/^.*?crates\//, "crates/"), message: m[3].slice(0, 200) });
    }
    return out.slice(-count).reverse();
  } catch { return []; }
}

const server = createServer((req, res) => {
  const url = req.url.split("?")[0];
  // Localhost-only, same-origin only: blocks DNS rebinding (Host) and cross-site POSTs (Origin + custom header).
  const self = [`127.0.0.1:${cfg.port}`, `localhost:${cfg.port}`];
  if (!self.includes(req.headers.host ?? "")) { res.writeHead(403); return res.end(); }
  if (req.method === "POST" && (req.headers["x-swarm-ctl"] !== "1" || (req.headers.origin && !self.some(h => req.headers.origin === `http://${h}`)))) { res.writeHead(403); return res.end(); }
  if (url === "/state") {
    // ?lite=1 skips results and events: pages ask for it when only worker activity changed.
    const view = stateView();
    if (req.url.includes("lite=1")) { delete view.results; delete view.events; }
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(JSON.stringify(view));
  } else if (url === "/events") {
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store", "x-accel-buffering": "no" });
    if (!sentSig) sentSig = signature();
    res.write(`data: ${signature()}\n\n`);
    listeners.add(res);
    req.on("close", () => listeners.delete(res));
    return;
  } else if (url === "/plan") {
    // Buildout plan: rows being built, the next rows and why they rank there, due supervisor actions, milestone tally.
    let body;
    try { const queue = peekCached(); body = planView({ ledger: ledgerNow(), state, queue, family: cfg.family, design: designView(), idle: idleView(queue) }); }
    catch (error) { body = { error: String(error.message ?? error).slice(0, 200) }; }
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(JSON.stringify(body));
  } else if (url === "/check") {
    // Agents' shared compile check (swarm-check.mjs). Localhost + Host check above; the package is validated.
    const pkg = validPackage(new URL(req.url, "http://x").searchParams.get("pkg"));
    sharedCheck(pkg).then(r => { res.writeHead(200, { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" }); res.end(r.text); },
      e => { res.writeHead(500); res.end(String(e?.message ?? e)); });
    return;
  } else if (url === "/engine") {
    if (!engineCache.body || Date.now() - engineCache.at > 3000) {
      let body;
      try {
        const ledger = loadLedger();
        const rows = ledger.rows.filter(isEngineRow).map(r => {
          const f = finished.get(r.id);
          return { id: r.id, label: r.label, status: r.status, epic: /^Epics\b/i.test(r.section), ms: r.milestone, missing: r.readiness.missing,
            swarm: active.has(r.id) ? "running" : parked.has(r.id) ? "parked" : (f?.status ?? ""), files: (brief(r, ledger.systems).files ?? []).slice(0, 2) };
        });
        body = { rows, crashes: recentCrashes(8), deadOwners: state.deadOwners ?? [] };
      } catch (error) { body = { error: String(error.message ?? error).slice(0, 200), rows: [], crashes: recentCrashes(8), deadOwners: [] }; }
      engineCache = { at: Date.now(), body: JSON.stringify(body) };
    }
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" }); res.end(engineCache.body);
  } else if (url === "/designs") {
    // Every design and deferral so far, newest first, for the owner to read and overrule.
    const list = designs.all().filter(d => d.at).sort((a, b) => b.at - a.at).map(d => ({ id: d.id, kind: d.kind ?? "row", verdict: d.verdict, at: d.at,
      text: String(d.text ?? "").slice(0, 600), cost: d.cost ?? 0, version: d.v ?? 1, round: d.round ?? 1, full: !!designs.read(d.id),
      state: active.has(d.id) ? "building" : retry.has(d.id) ? "queued" : finished.get(d.id)?.status ?? "" }));
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(JSON.stringify({ designs: list, view: designView() }));
  } else if (url.startsWith("/design/")) {
    const key = decodeURIComponent(url.slice(8)), d = DESIGN_KEY.test(key) ? designs.get(key) : null;
    res.writeHead(200, { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" });
    res.end(d ? designs.read(key) || `${d.verdict === "defer" ? "Left for the owner" : "Decision"}: ${d.text ?? ""}` : "unknown design");
  } else if (/^\/prompt\/[\w.-]{1,40}$/.test(url)) {
    res.writeHead(200, { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" }); res.end(promptFor(url.slice(8)));
  } else if (url.startsWith("/log/")) {
    // /log/<rowId> (latest attempt) or /log/w:<workerId> (live).
    const key = decodeURIComponent(url.slice(5));
    const w = key.startsWith("w:") ? state.workers.find(x => x.id === key.slice(2)) : null;
    const rel = w ? w.log : resultFor(key)?.log;
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(JSON.stringify({ key, log: rel ?? null, lines: readableLog(rel) }));
  } else if (url.startsWith("/diff/")) {
    const id = decodeURIComponent(url.slice(6));
    const r = ROW_ID.test(id) ? resultFor(id) : null;
    res.writeHead(200, { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" });
    res.end(r ? diffFor(r.files) || "(no diff: files unchanged or already committed)" : "unknown row");
  } else if (req.method === "POST" && url === "/ctl/set") {
    // Live settings: { workers, family, budget, scout, readyOnly, exploreTurns, checkpointEvery, context }
    readBody(req).then(b => {
      if (Number.isFinite(b.workers)) setWorkers(b.workers);
      if (typeof b.family === "string" && (b.family === "all" || b.family in FAMILIES)) cfg.family = b.family;
      if (Number.isFinite(b.budget) && b.budget >= 0) { cfg.budget = b.budget; if (state.paused?.startsWith("budget cap") && !overBudget()) state.paused = null; }
      if (b.resetSpend === true) { spend.total = 0; spend.since = Date.now(); saveSpend(); if (state.paused?.startsWith("budget cap")) state.paused = null; }
      if (Number.isFinite(b.budgetWindowH) && b.budgetWindowH >= 0 && b.budgetWindowH <= 720) { cfg.budgetWindowH = b.budgetWindowH; rollSpend(); }
      for (const k of ["scout", "readyOnly", "context", "prove", "wire", "decide", "designEpics"]) if (typeof b[k] === "boolean") cfg[k] = b[k];
      if (Number.isFinite(b.decideMax) && b.decideMax >= 0 && b.decideMax <= 200) cfg.decideMax = b.decideMax;
      peekCache.at = 0; lastRefill = 0; // a changed family or toggle shows in the queue at once
      if (Number.isFinite(b.exploreTurns) && b.exploreTurns >= 8 && b.exploreTurns <= 90) cfg.exploreTurns = b.exploreTurns;
      if (Number.isFinite(b.checkpointEvery) && b.checkpointEvery >= 0 && b.checkpointEvery <= 100) cfg.checkpointEvery = b.checkpointEvery;
      if (Number.isFinite(b.proveBatch) && b.proveBatch >= 1 && b.proveBatch <= 50) cfg.proveBatch = b.proveBatch;
      if (Number.isFinite(b.breaker) && b.breaker >= 0 && b.breaker <= 50) cfg.breaker = b.breaker;
      if (Number.isFinite(b.proofPerHour) && b.proofPerHour >= 0 && b.proofPerHour <= 120) cfg.proofPerHour = b.proofPerHour;
      event(`settings: ${JSON.stringify(b).slice(0, 160)}`); saveSettings(); persist();
      res.writeHead(204); res.end();
    });
    return;
  } else if (req.method === "POST" && /^\/ctl\/(retry|park|unpark|pin|unpin)\/[\w.-]{1,40}$/.test(url)) {
    const [, , verb, id] = url.split("/");
    if (verb === "retry") { parked.delete(id); saveParked(); requeue(id, 0, "Retried by supervisor. " + String(resultFor(id)?.summary ?? "").slice(0, 200)); if (!priority.includes(id)) priority.unshift(id); }
    else if (verb === "park") { parked.add(id); saveParked(); retry.delete(id); for (const w of state.workers) if (w.row === id) w.child?.kill(); }
    else if (verb === "unpark") { parked.delete(id); saveParked(); }
    else if (verb === "pin") { if (!priority.includes(id)) priority.unshift(id); }
    else if (verb === "unpin") { const at = priority.indexOf(id); if (at >= 0) priority.splice(at, 1); }
    event(`${verb} ${id}`);
    peekCache.at = 0;
    res.writeHead(204); return res.end();
  } else if (req.method === "POST" && url.startsWith("/ctl/redesign/")) {
    // The owner overruled a design or wants a deferral looked at again: forget it so the lead designs it afresh.
    const key = decodeURIComponent(url.slice(14));
    if (!DESIGN_KEY.test(key)) { res.writeHead(400); return res.end(); }
    designs.forget(key); designTries.delete(key); retry.delete(key); lastRefill = 0;
    event(`redesign ${key}`);
    res.writeHead(204); return res.end();
  } else if (req.method === "POST" && url.startsWith("/ctl/")) {
    const action = url.slice(5);
    if (action === "pause") { state.paused = `paused from the dashboard at ${new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`; event(state.paused); }
    else if (action === "resume") { if (state.paused) event("resumed from the dashboard"); state.paused = null; state.resumeAt = null; clearTimeout(resumeTimer); }
    else if (action === "checkpoint") checkpoint("requested by supervisor");
    else if (action === "prove") { idleProved = false; checkpoint("proof requested by supervisor"); }
    else if (action === "requeue") { for (const [id, r] of [...finished]) if ((r.status === "blocked" || r.status === "failed") && !isDesignBlock(r)) requeue(id, 0, `Previously blocked: ${String(r.summary).slice(0, 200)}`); }
    else if (action === "stop") { state.stop = true; state.userStop = true; for (const w of state.workers) w.child?.kill(); for (const d of designing.values()) d.slot?.child?.kill(); killProof(); killChecks(); }
    else if (action === "add" && state.workers.length < 16) {
      if (state.stop && !state.userStop) state.stop = false; // revive after an automatic stop
      const w = { id: "w" + (state.workers.length + 1), status: "idle", row: null, activity: [], turns: 0, files: [] };
      state.workers.push(w); work(w);
    } else if (action === "drop") { const w = [...state.workers].reverse().find(x => x.status === "idle" || x.status === "working"); if (w && state.workers.length > 1) w.retire = true; }
    else if (action.startsWith("kill-")) state.workers.find(w => w.id === action.slice(5))?.child?.kill();
    else if (action === "restart" && !state.restartPending) {
      // Graceful restart to load edited supervisor code: nothing new starts, running rows and designs finish,
      // state is saved, then exit 75 so AxiomSwarm.cmd's loop starts the new code and results resume.
      state.restartPending = Date.now(); event("restart requested: running rows finish first, then the supervisor reloads"); persist();
      const timer = setInterval(() => {
        const busy = state.workers.some(w => w.status === "working" || w.status === "waiting for green build") || designing.size || state.checkpointing;
        if (busy) return;
        clearInterval(timer); event("restarting now"); persist();
        if (state.paused) try { writeFileSync(keepPausedFile, JSON.stringify({ paused: state.paused })); } catch { /* restarts unpaused */ }
        setTimeout(() => process.exit(75), 300);
      }, 2000);
    } else if (action === "restart-cancel") { state.restartPending = null; event("restart cancelled"); }
    res.writeHead(204); res.end();
  } else if (url === "/classic") { res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" }); res.end(readFileSync(new URL("./swarm-classic.html", import.meta.url), "utf8"));
  } else { res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" }); res.end(readFileSync(pageUrl, "utf8")); }
});
function peek() {
  const own = peekIn();
  if (own.length || !cfg.familyFallback || cfg.family === "all") return own;
  scopeAll = true;
  try { return peekIn().map(q => ({ ...q, label: "other family: " + q.label })); } finally { scopeAll = false; }
}
function peekIn() {
  try {
    const ledger = ledgerNow();
    const { rows, pushed } = eligible(ledger);
    const fresh = [...pushed, ...nextRows(rows, 12).filter(r => !priority.includes(r.id))].slice(0, 12)
      .map(r => ({ id: r.id, label: (designs.decided(r.id) ? "designed: " : "") + r.label, tier: cfg.scout && !retry.has(r.id) && !designs.decided(r.id) && !isOwnerRow(r.id) ? scoutTier.name : isOwnerRow(r.id) ? ownerTier.name : isFableRow(r.id) ? fableTier.name : tiers[Math.min(tiers.length - 1, retry.get(r.id) ?? 0)].name, pinned: priority.includes(r.id) }));
    // When new rows run short the workers take wire passes; show them so the queue is not reported empty.
    const wires = fresh.length < 12 ? wireCandidates(ledger).slice(0, 12 - fresh.length).map(w => ({ id: w.row.id, label: "wire: " + w.row.label, tier: cfg.wireModel === "haiku" ? wireTier.name : tiers[0].name + "·wire", pinned: false })) : [];
    const conts = fresh.length + wires.length < 12 ? contCandidates(ledger).slice(0, 12 - fresh.length - wires.length).map(c => ({ id: c.row.id, label: "continue: " + c.row.label, tier: tiers[0].name + "·cont", pinned: false })) : [];
    // Same order pick() uses: fresh proof passes first, backfill after wire passes.
    const asItem = p => ({ id: p.row.id, label: "proof: " + p.row.label, tier: proofTier.name, pinned: false });
    const freshProofs = proofCandidates(ledger, { fresh: true }).slice(0, 3).map(asItem);
    const backfill = proofCandidates(ledger, { fresh: false, idle: !fresh.length && !wires.length && !conts.length }).slice(0, 3).map(asItem);
    // A worker paused mid-row (usage limit) still shows that row as its own; listing it again here read as a loop.
    const held = new Set(state.workers.filter(w => w.row && w.status !== "idle").map(w => w.row));
    return [...freshProofs, ...fresh, ...wires, ...backfill, ...conts].filter(r => !held.has(r.id)).slice(0, 14);
  } catch { return []; }
}

if (flag("dry")) {
  // No model, no lock, no server: the queue, what the design stage would take next and why nothing runs.
  const queue = peek(), found = findDesignTargets(ledgerNow(), 8);
  designWaiting = found.counts;
  console.log(JSON.stringify({ cfg, queue, requeuedAtStart: [...retry.keys()].filter(k => !k.endsWith(":note")),
    design: { waiting: found.counts, next: found.items.map(i => ({ key: i.key, kind: i.kind, rows: i.rows?.length, label: i.row?.label ?? i.system?.name })) },
    idle: queue.length ? null : idleReason().text }, null, 1));
  process.exit(0);
}
server.listen(cfg.port, "127.0.0.1", () => console.log(`swarm ui http://127.0.0.1:${cfg.port}  workers=${cfg.workers} family=${cfg.family}`));
for (let i = 1; i <= cfg.workers; i++) state.workers.push({ id: "w" + i, status: "idle", row: null, activity: [], turns: 0, files: [] });
setInterval(persist, 5000);
// Workers sleep while paused and never reach rollSpend(), so the window also rolls (and unpauses) from here.
setInterval(rollSpend, 60_000);
await Promise.all(state.workers.map((w, i) => new Promise(r => setTimeout(r, i * 1500)).then(() => work(w))));
persist();
console.log("swarm workers finished", JSON.stringify(state.totals));
// Stay up after an automatic stop so the UI can report why and "+ agent" can revive the run;
// exit only when the supervisor pressed Stop all.
if (state.userStop) { server.close(); setTimeout(() => process.exit(0), 500); }
