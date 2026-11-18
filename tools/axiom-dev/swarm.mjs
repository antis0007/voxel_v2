// Parity swarm: N concurrent headless Claude Code agents, each implementing one ledger row
// end to end (investigate, edit, cargo check) in the shared tree, plus a live supervisor UI.
//
//   node tools/axiom-dev/swarm.mjs [--workers 3] [--family f] [--port 4377] [--max-rows 0]
//        [--sonnet sonnet] [--opus opus] [--haiku haiku] [--no-scout] [--explore-turns 15] [--reopen] [--ready-only]
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
// - A usage limit pauses the swarm and retries every 10 minutes. UI: http://127.0.0.1:<port>
import { spawn, spawnSync } from "node:child_process";
import { appendFileSync, closeSync, existsSync, fstatSync, mkdirSync, openSync, readFileSync, readSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import path from "node:path";
import { FAMILIES, brief, filterRows, isEngineRow, loadLedger, nextRows, writeStatus } from "./parity-data.mjs";
import { root, buildCommand, runScenarios } from "./factory-lab.mjs";
import { blameContent, pendingProof, scenarioFile, settleProof, shouldProve, tolerantScenarios, validateScenario } from "./swarm-proof.mjs";
import { createBrain } from "./swarm-brain.mjs";
import { APP, runCheck, validPackage } from "./swarm-check.mjs";

const argv = process.argv.slice(2);
const opt = (name, fallback) => { const i = argv.indexOf("--" + name); return i < 0 ? fallback : argv[i + 1]; };
const flag = name => argv.includes("--" + name);
const family = opt("family");
if (family && !(family in FAMILIES)) { console.error("Unknown family " + family); process.exit(2); }
const cfg = { workers: Math.min(16, Math.max(1, Number(opt("workers", 5)))), port: Number(opt("port", 4377)),
  maxRows: Number(opt("max-rows", 0)), timeoutMs: Number(opt("timeout-min", 25)) * 60_000,
  sonnet: opt("sonnet", "sonnet"), opus: opt("opus", "opus"), haiku: opt("haiku", "haiku"), family: family ?? "all",
  scout: !flag("no-scout"), readyOnly: flag("ready-only"), budget: Number(opt("budget-usd", 0)),
  checkpointEvery: Number(opt("checkpoint-every", 8)), context: !flag("no-context"), prove: !flag("no-prove"), proveBatch: Number(opt("prove-batch", 3)), breaker: Number(opt("breaker", 6)), exploreTurns: Number(opt("explore-turns", 15)), wire: !flag("no-wire") };
// Cost ladder (history: failed runs cost as much as successes, and turns drive cost because every
// turn re-reads context). A haiku scout triages first; the builder starts cheap with a tight turn
// cap; ONE escalation to opus, only for failures a stronger model can fix.
const tiers = [
  // Caps bound the tail only: sonnet runs average 8.8 turns, landed opus runs 21-28.
  { name: "sonnet·low", model: cfg.sonnet, effort: "low", maxTurns: 24, budget: 0.5 },
  { name: "opus·medium", model: cfg.opus, effort: "medium", maxTurns: 40, budget: 1.2 },
];
// Wire passes add one call site to code that already compiles: haiku first (about a third of sonnet's price);
// the wire stats below fall back to sonnet if haiku stops landing them. --wire-model sonnet skips haiku.
const wireTier = { name: "haiku·wire", model: cfg.haiku, effort: "low", maxTurns: 16, budget: 0.2 };
cfg.wireModel = opt("wire-model", "haiku") === "sonnet" ? "sonnet" : "haiku";
const scoutTier = { name: "haiku·scout", model: cfg.haiku, effort: "low", maxTurns: 10, budget: 0.15, tools: "Read,Grep,Glob", scout: true };
// A stronger model cannot invent a missing spec, owner decision or protocol: those stop at once.
const designBlock = /\b(spec|design decision|owner decision|needs? (a |an )?(decision|design|approval)|protocol|lore|proper noun)\b/i;
// Scout needs-design verdicts quote "[design]" owners, which designBlock misses; Requeue used to pay a builder for each.
const isDesignBlock = r => r?.why === "design" || /^scout:/.test(r?.summary ?? "") || designBlock.test(r?.summary ?? "");

// --state-dir lets a test instance run beside the live swarm without sharing its results.
const dir = opt("state-dir") ? path.resolve(opt("state-dir")) : path.join(root, "target", "axiom-dev", "swarm");
mkdirSync(path.join(dir, "logs"), { recursive: true });
const resultsFile = path.join(dir, "results.jsonl");
const stateFile = path.join(dir, "state.json");
const finished = new Map(); // id -> last result (only terminal ones block re-pick)
if (existsSync(resultsFile)) for (const line of readFileSync(resultsFile, "utf8").split("\n").filter(Boolean)) {
  try { const r = JSON.parse(line); if (r.final) finished.set(r.id, r); } catch { /* skip */ }
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
const state = { startedAt: Date.now(), cfg, paused: null, stop: false, workers: [], results: history, totals: { in: 0, out: 0, cost: 0, done: 0, blocked: 0, cached: 0, proven: 0 }, tierStats: {} };
const retry = new Map(); // id -> next tier
const wireStats = { n: 0, wired: 0 };
let sinceCheckpoint = 0; // landed rows + wire passes since the last auto checkpoint
const forced = new Set(); // rows already given their one narrower sonnet retry after a zero-edit failure
let failStreak = 0, idleProved = false;
const lastFail = new Map(); // id -> previous failure summary; an identical repeat is deterministic, so do not pay to retry it
const sameFailure = (a, b) => { const x = new Set(String(a).toLowerCase().match(/[a-z_]{4,}/g) ?? []), y = new Set(String(b).toLowerCase().match(/[a-z_]{4,}/g) ?? []); let h = 0; for (const w of x) if (y.has(w)) h++; return h / Math.max(1, Math.min(x.size, y.size)) > 0.8; };
const parkedFile = path.join(dir, "parked.json");
const parked = new Set((() => { try { return JSON.parse(readFileSync(parkedFile, "utf8")); } catch { return []; } })());
const saveParked = () => { try { writeFileSync(parkedFile, JSON.stringify([...parked])); } catch { /* best effort */ } };
const priority = []; // row ids the supervisor pushed to the front
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
// rev: results, events, settings; act: worker activity. The broadcaster pushes both to open pages.
let rev = 0, act = 0;
const logTail = (w, text) => { act++; w.activity.push({ t: Date.now(), text }); if (w.activity.length > 60) w.activity.shift(); };
// Atomic: a crash mid-write must not leave a truncated state file for the next start.
const persist = () => { rev++; try { writeFileSync(stateFile + ".tmp", JSON.stringify(state)); renameSync(stateFile + ".tmp", stateFile); } catch { /* ui still has memory */ } };
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
const STATIC_TASK = `${"Implement ONE parity row end to end, fast. Never stop on a missing owner."} Steps:
1. Use the work order below; do not re-run describe.
2. Pick the smallest playable slice (<300 changed lines). Data first: rows in assets/*.ron or modes/fields on existing kinds. New resources, machine kinds, save fields, generator gates and shader fields are APPROVED: build them (see OWNER POLICY).
3. Gate behaviour under world.rules.systems.enabled("<system>") where it ticks.
4. Edit with Edit/Write only. Others edit this tree concurrently: never run git checkout/stash/reset/restore/clean, never revert or reformat, keep unrelated hunks. Do not touch wand_xmb.rs, adventure/growth.rs, the parity ledger or any test.
5. Verify once at the end with exactly: node tools/axiom-dev/swarm-check.mjs (fix errors you caused; re-run once after fixing). No tests, no cargo fmt.
6. If the behaviour already exists and works, edit nothing: report already-implemented with the owner files you verified.
7. If source context is absent after ~20 lookups, create the owner yourself per OWNER POLICY; do not stop.
8. Machine or factory rows only: make it provable. Add a "scenario" key to RESULT (keep RESULT on ONE line), e.g. "scenario":{"steps":[{"op":"machine_test","position":[0,40,0],"kind":"Crusher","set":{"energy":5000,"input":{"Tin ingots":64}},"ticks":600,"expect":{"end.out.Tin dust":64}}]}. The supervisor stores it, runs it headless in the real engine after the next build and returns any mismatch to you. Expect only values your change produces; at most 12 steps; omit the key for rows with no machine behaviour. Do not write files under tools/.
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
Last line exactly: RESULT {"status":"building|playable|already-implemented|blocked","files":["path"],"summary":"<=200 chars","wired":true,"entry":"<file:fn or data row that reaches the change>","tests":"<owner checks, or empty>","why":"<only when not landed: no_owner|too_wide|compile|design|other>"}`;

function prompt(row, tier, note, order, ctx = "") {
  return `${/^ENG-/.test(row.id) ? ENGINE_TASK : STATIC_TASK}

${POLICY}

${brain.note()}
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
  try { return JSON.parse(hit[1]); } catch { return null; }
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
const PRICE = { opus: [5, 25], sonnet: [3, 15], haiku: [1, 5] };
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
            if (/Finished/.test(c) && !/\berror(\[E\d+\])?:/.test(c)) Object.assign(health, { ok: true, at: Date.now(), errors: [] });
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
            if (!tier.scout && !explored && turns - denied >= cfg.exploreTurns && !touched.size && !/^(Edit|Write)$/.test(b.name)) {
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
    child.stdin.end(tier.scout ? scoutPrompt(row, order, ctx) : prompt(row, tier, note, order, ctx));
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

function prewarm() {
  try {
    const ledger = loadLedger();
    for (const q of peek().slice(0, 3)) {
      const row = ledger.rows.find(r => r.id === q.id);
      if (row && !contextCache.has(row.id) && !active.has(row.id)) rowContext(row, brief(row, ledger.systems).files ?? []);
    }
  } catch { /* best effort */ }
}
const active = new Map(); // id -> { files }
// Rows a worker may take. A missing owner system that already blocked two rows parks every row that
// needs it, so no model is paid to rediscover the same gap. Pinned rows bypass the family filter.
function eligible(ledger) {
  const open = r => !["verified", "blocked", "parked", "design"].includes(r.status) && !parked.has(r.id) && !active.has(r.id) && (!finished.has(r.id) || retry.has(r.id));
  const blocks = new Map();
  for (const r of ledger.rows) if (finished.get(r.id)?.status === "blocked") for (const o of r.readiness.missing) blocks.set(o, (blocks.get(o) ?? 0) + 1);
  const dead = new Set([...blocks].filter(([, n]) => n >= 2).map(([o]) => o));
  state.deadOwners = [...dead].sort().map(o => `${o}×${blocks.get(o)}`);
  const rows = filterRows(ledger.rows, { family: familyOf(), ready: cfg.readyOnly || undefined })
    .filter(r => open(r) && !r.readiness.missing.some(o => dead.has(o)));
  const pushed = priority.map(id => ledger.rows.find(r => r.id === id)).filter(r => r && open(r));
  return { rows, pushed };
}
function pick() {
  const ledger = loadLedger();
  const { rows, pushed } = eligible(ledger);
  const busy = new Set(state.workers.flatMap(w => w.row ? [...(w.likely ?? []).slice(0, 2), ...(w.files ?? [])] : []));
  for (const row of [...pushed, ...nextRows(rows, 80)]) {
    if (active.has(row.id) || (finished.has(row.id) && !retry.has(row.id))) continue;
    if (bumpRows.has(row.id) && [...active.keys()].filter(id => bumpRows.has(id)).length >= 5) continue; // at most 5 save-format editors at once
    const files = (brief(row, ledger.systems).files ?? []);
    if (files.slice(0, 2).some(f => busy.has(f))) continue;
    const at = priority.indexOf(row.id); if (at >= 0) priority.splice(at, 1);
    return { row, files, order: JSON.stringify(brief(row, ledger.systems)).slice(0, 3500) };
  }
  // Nothing new to build: wire what already landed instead of idling.
  const wire = wireCandidates(ledger, busy)[0];
  if (wire) return wire;
}
// Wire pass. Most open rows wait on design-only owner systems, so the queue runs dry while ~2/3 of landed
// slices are scaffolding nothing calls (cost per wired row is ~4x cost per row). When no new row is free, a
// landed-but-unwired row goes back to sonnet once to add its smallest real caller. One pass per row; the
// result carries wirePass so restarts do not repeat it.
const isUnwired = r => r && /^(building|playable)$/.test(r.status) && !r.wirePass && (r.files ?? []).length > 0
  && (r.unwired || r.wired === false || (r.wired === undefined && UNWIRED.test(r.summary ?? "")));
function wireCandidates(ledger, busy = new Set()) {
  if (!cfg.wire) return [];
  const byId = new Map(ledger.rows.map(r => [r.id, r]));
  const out = [];
  for (const r of [...finished.values()].sort((a, b) => (b.at ?? 0) - (a.at ?? 0))) {
    const row = byId.get(r.id);
    if (!row || !isUnwired(r) || active.has(r.id) || parked.has(r.id) || (cfg.family !== "all" && !filterRows([row], { family: cfg.family }).length)) continue;
    const files = (r.files ?? []).filter(f => /^[\w./-]+$/.test(f)).slice(0, 6);
    if (files.slice(0, 2).some(f => busy.has(f))) continue;
    out.push({ row, files, wire: true, order: JSON.stringify(brief(row, ledger.systems)).slice(0, 2500),
      note: `WIRE PASS. An earlier slice of this row landed in ${files.join(", ")} and reported: ${String(r.summary).slice(0, 260)} Nothing reaches it yet. Do NOT add features: wire the smallest real caller so a player action, a tick or a loaded table reaches the existing code (one call site, one RON row read, or one UI line), behind the row's system switch. Start from those files; first Edit within 6 tool calls. If no caller is possible without an owner decision, edit nothing and report blocked with why "design".` });
    if (out.length >= 12) break;
  }
  return out;
}
let resumeTimer;
// "resets 2pm" or "resets 2:30 pm": next occurrence of that local time; otherwise retry in 30 minutes.
function limitResetAt(message, now = Date.now()) {
  const at = String(message).match(/resets?\s+(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*([ap]m)\b/i);
  if (!at) return now + 30 * 60_000;
  const when = new Date(now);
  when.setHours(Number(at[1]) % 12 + (at[3].toLowerCase() === "pm" ? 12 : 0), Number(at[2] ?? 0), 0, 0);
  if (when.getTime() <= now) when.setDate(when.getDate() + 1);
  return when.getTime();
}
const startTier = () => 0;
const limit = /weekly limit|usage limit|rate limit|out of credits|credit balance|hit your [\w -]{0,20}limit|\d+-hour limit|limit resets/i;

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
}

// Build-health gate: new rows start only while the shared tree compiles. A red tree means every agent's
// final `cargo check` fails and they burn budget "fixing" someone else's break. Checked at most every
// 3 minutes (single flight); rows already running continue, and agents are told nothing about it.
const useHealthGate = !flag("no-health");
const health = { ok: true, at: 0, errors: [], checking: null };
function checkHealth() {
  if (health.checking) return health.checking;
  health.checking = sharedCheck(APP).then(() => { health.checking = null; });
  return health.checking;
}
// Shared compile check. Every caller (agents through swarm-check.mjs, the health gate) gets the result of a
// cargo run that STARTED after it asked, so its edits are covered; callers that arrive while a run is in
// flight share the single follow-up run. Before, five agents queued five 3-minute checks behind the lock.
const checks = new Map(); // pkg -> { running, next }
function sharedCheck(pkg) {
  let s = checks.get(pkg);
  if (!s) checks.set(pkg, s = { running: null, next: null });
  const start = () => {
    s.running = runCheck(pkg, { root, env: childEnv() }).then(r => {
      if (pkg === APP) {
        // A spawn/lock failure with no compiler error is not "red".
        Object.assign(health, { ok: r.ok || !r.errors.length, at: r.at, errors: r.errors.slice(0, 6).map(l => l.slice(0, 200)) });
        state.health = { ok: health.ok, at: health.at, errors: health.errors };
      }
      return r;
    }).finally(() => { s.running = null; });
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
  if (Date.now() - health.at > 180_000 || !health.ok) await checkHealth();
  const since = Date.now();
  for (let red = 1; !health.ok && !state.stop; red++) {
    if (Date.now() - since > 20 * 60_000) { logTail(worker, "build still red after 20 min; starting anyway"); return; }
    worker.status = "waiting for green build";
    logTail(worker, "build red: " + (health.errors[0] ?? "").slice(0, 100));
    if (red === 2 && !state.checkpointing) checkpoint("build red; repair");
    await new Promise(r => setTimeout(r, 45_000));
    if (!state.checkpointing) await checkHealth();
  }
}

async function work(worker) {
  while (!state.stop && !worker.retire) {
    const disk = brain.diskGuard();
    if (disk.low) { worker.status = "paused"; worker.phase = `low disk ${disk.gb.toFixed(1)} GB`; await new Promise(r => setTimeout(r, 15000)); continue; }
    if (state.paused || state.checkpointing) { worker.status = "paused"; await new Promise(r => setTimeout(r, state.checkpointing ? 2000 : 5000)); continue; }
    if (cfg.budget && state.totals.cost >= cfg.budget) { state.paused = `budget cap $${cfg.budget} reached; raise it in Settings`; continue; }
    if (cfg.maxRows && state.totals.done + state.totals.blocked + active.size >= cfg.maxRows && !retry.size) break;
    // Green-build gate moved to just before the builder: read-only scouts run while the tree is red.
    if (state.stop || state.paused) continue;
    const next = pick();
    if (!next && cfg.prove && !active.size && !idleProved) { idleProved = true; if (proofPending().size) checkpoint("idle: proof of landed rows"); }
    if (!next) { worker.status = "idle"; worker.row = null; await new Promise(r => setTimeout(r, 3000)); continue; }
    const { row, files, order } = next;
    const wiring = !!next.wire, prior = wiring ? finished.get(row.id) : null;
    const isRetry = retry.has(row.id) || wiring; // a wire pass skips the scout: it already has its files
    const tierIndex = wiring ? 0 : Math.min(tiers.length - 1, retry.get(row.id) ?? startTier(row));
    const tier = wiring && cfg.wireModel === "haiku" ? wireTier : tiers[tierIndex];
    let note = wiring ? next.note : isRetry ? retry.get(row.id + ":note") : "";
    retry.delete(row.id);
    active.set(row.id, { files });
    Object.assign(worker, { status: "working", row: row.id, label: row.label, phase: "context", activity: [], turns: 0, denied: 0, files: [] });
    const ctx = await rowContext(row, files);
    prewarm();
    let rowCost = 0;
    if (cfg.scout && !isRetry) {
      Object.assign(worker, { status: "working", row: row.id, label: row.label, tier: scoutTier.name, startedAt: Date.now(), activity: [], turns: 0, files: [], likely: files });
      logTail(worker, `scout ${row.id}`);
      const s = await runAgent(worker, row, scoutTier, "", order, ctx);
      state.totals.cost += s.cost ?? 0; rowCost += s.cost ?? 0;
      state.totals.in += (s.usage?.input_tokens ?? 0) + (s.usage?.cache_read_input_tokens ?? 0) + (s.usage?.cache_creation_input_tokens ?? 0);
      state.totals.out += s.usage?.output_tokens ?? 0; state.totals.cached += s.usage?.cache_read_input_tokens ?? 0;
      // A scout killed by Stop/Park fell through into waitForGreen's cargo check before noticing the stop.
      if (state.stop || parked.has(row.id)) { active.delete(row.id); worker.child = null; logTail(worker, "stopped after scout; not recorded"); if (state.stop) break; continue; }
      const verdict = parseScout(s.text);
      if (limit.test((s.error || "") + " " + (s.text || ""))) { active.delete(row.id); retry.set(row.id, tierIndex); retry.set(row.id + ":note", ""); state.paused = "usage limit hit (scout)"; continue; }
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
    }
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
    state.totals.in += tokens; state.totals.out += out.usage?.output_tokens ?? 0; state.totals.cost += out.cost ?? 0; rowCost += out.cost ?? 0;
    state.totals.cached += out.usage?.cache_read_input_tokens ?? 0;
    const parsed = parseResult(out.text);
    const entry = { id: row.id, label: row.label, tier: tier.name, at: Date.now(), ms: Date.now() - worker.startedAt, files: parsed?.files?.length ? parsed.files : out.files,
      summary: parsed?.summary ?? (out.error || out.text || "no result").toString().slice(0, 240), status: parsed?.status ?? (out.ok ? "building" : "failed"), log: worker.log, tokens, cost: rowCost,
      turns: worker.turns, cached: out.usage?.cache_read_input_tokens ?? 0, out: out.usage?.output_tokens ?? 0,
      wired: parsed?.wired, entry: parsed?.entry, why: parsed?.why, tests: parsed?.tests, estimated: out.estimated || undefined, denied: out.denied || undefined };
    if (limit.test(out.error || out.text || "")) {
      // Sleep until the provider's stated reset instead of relaunching failing agents every 10 minutes.
      const until = limitResetAt(out.error || out.text || "");
      state.paused = "usage limit hit; resumes " + new Date(until).toLocaleString();
      state.resumeAt = until;
      retry.set(row.id, tierIndex); retry.set(row.id + ":note", "");
      clearTimeout(resumeTimer);
      resumeTimer = setTimeout(() => { state.paused = null; state.resumeAt = null; }, Math.max(60_000, until - Date.now()));
      logTail(worker, "paused: usage limit");
      continue;
    }
    if (/CLI not found/.test(out.error ?? "")) { state.stop = true; state.paused = out.error; break; }
    if (entry.status === "already-implemented") {
      entry.final = true; finished.set(row.id, entry); state.totals.done++; record(entry); persist(); continue;
    }
    const good = (entry.status === "building" || entry.status === "playable") && entry.files.length > 0;
    if (wiring) {
      // One wire pass per row. A failed pass keeps the earlier landed slice as the row's result.
      entry.wirePass = true;
      // Wire passes must pay for themselves: below 1 in 3 actually wired after 6 tries, stop starting them
      // (re-enable in Settings). Idle workers cost nothing; unproductive passes cost ~$0.09 each.
      wireStats.n++; if (good && entry.wired !== false && !UNWIRED.test(entry.summary ?? "")) wireStats.wired++;
      if (cfg.wire && wireStats.n >= 6 && wireStats.wired / wireStats.n < 1 / 3) {
        if (cfg.wireModel === "haiku") { cfg.wireModel = "sonnet"; event(`wire pass: haiku wired ${wireStats.wired}/${wireStats.n}; switching wire passes to sonnet`); wireStats.n = wireStats.wired = 0; }
        else { cfg.wire = false; event(`wire pass off: ${wireStats.wired}/${wireStats.n} wired; turn it on in Settings`); }
      }
      if (!good) {
        record({ ...entry, status: "blocked", summary: "wire pass: " + entry.summary, final: false });
        finished.set(row.id, { ...prior, wirePass: true });
        appendFileSync(resultsFile, JSON.stringify({ ...prior, wirePass: true, final: true }) + "\n");
        persist(); continue;
      }
      entry.files = [...new Set([...(prior?.files ?? []), ...entry.files])];
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
    if (!good && !hardBlock && !repeat && entry.files.length && tierIndex < tiers.length - 1) {
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
    if (cfg.breaker > 0 && failStreak >= cfg.breaker && !state.paused) {
      state.paused = `circuit breaker: ${failStreak} rows in a row did not land. Read the supervisor log and lessons, then press Resume`;
      event(state.paused); failStreak = 0;
    }
    if (good) {
      if (wiring) state.totals.wired = (state.totals.wired ?? 0) + 1; else state.totals.done++;
      sinceCheckpoint++;
      const flat = v => String(v ?? "").replace(/\s+/g, " ").trim();
      try { appendFileSync(path.join(dir, "changes.md"), `${row.id} | ${entry.files.join(", ")} | ${flat(entry.summary)} | ${flat(entry.tests) || "-"}\n`); } catch { /* best effort */ }
      if (entry.wired === false || (entry.wired === undefined && UNWIRED.test(entry.summary ?? ""))) { state.totals.scaffold = (state.totals.scaffold ?? 0) + 1; entry.unwired = true; }
      if (parsed?.scenario) {
        const bad = validateScenario(parsed.scenario);
        if (bad) event(`${row.id}: scenario not stored (${bad})`);
        else try {
          const f = scenarioFile(row.id, parsed.scenario, entry.summary);
          mkdirSync(scenarioPath, { recursive: true });
          writeFileSync(path.join(scenarioPath, f.name), f.body);
          entry.scenario = "tools/axiom-dev/scenarios/" + f.name; idleProved = false;
        } catch (error) { event(`${row.id}: scenario not stored (${String(error.message).slice(0, 80)})`); }
      }
      try {
        const ledger = loadLedger();
        writeStatus(row.id, { status: "building", evidence: entry.files.slice(0, 6), note: "swarm: " + entry.summary.slice(0, 160) }, ledger.revision, new Set(ledger.rows.map(r => r.id)));
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
  event(`checkpoint: ${why}; draining agents`);
  try {
    // Drain only agents that are actually running: workers parked in waitForGreen hold a row in `active`
    // but no child, and waiting on them stalled every red-tree checkpoint for the full 10 minutes.
    for (let i = 0; state.workers.some(w => w.child) && i < 240 && !state.stop; i++) await new Promise(r => setTimeout(r, 2500));
    let result = await brain.compile();
    for (let round = 1; !result.ok && result.errors.length && round <= 2 && !state.stop; round++) {
      event(`checkpoint: ${result.errors.length} compiler errors; repair agent round ${round}`);
      const fixer = { id: "fix", activity: [], turns: 0, files: [] };
      const order = result.errors.join("\n").slice(0, 3500);
      const out = await runAgent(fixer, { id: "CHECKPOINT", label: "Fix compile errors" }, tiers[0], "NOT a feature row. The tree no longer compiles; fix ONLY these errors with the smallest edits, add no behaviour, never revert another agent's work. Report status building with the files you changed.", order);
      state.totals.cost += out.cost ?? 0;
      result = await brain.compile();
    }
    event(result.ok ? "checkpoint: tree compiles" : `checkpoint: still failing (${result.errors.length} errors${result.note ? ", " + result.note : ""}); see cargo check`);
    state.lastCheckpoint = { at: Date.now(), ok: result.ok, errors: result.errors.slice(0, 5) };
    // The checkpoint just compiled the same package the health gate checks; share it instead of re-checking.
    if (result.ok || result.errors.length) Object.assign(health, { ok: result.ok, at: Date.now(), errors: result.errors.slice(0, 6) }), state.health = { ok: health.ok, at: health.at, errors: health.errors };
    if (result.ok) await prove(/proof|supervisor/.test(why), /idle/.test(why));
  } finally { state.checkpointing = null; }
}

// ---- live engine proof (no model cost) ----
// Landed rows that carry a tagged scenario are run headless in a freshly built engine. A pass is recorded
// as `proven`; a failure goes back to the row once, with the observed values, so the agent refines its
// own work against real engine behaviour instead of a compile check alone.
const proven = new Set(), proofRetried = new Set();
const scenarioPath = path.join(root, "tools/axiom-dev/scenarios");
const debugExe = path.join(root, "target/debug/minecraft_style_voxel_prototype.exe");
const proofPending = () => pendingProof(finished.values(), tolerantScenarios(scenarioPath).scenarios, proven);
function buildEngine(timeoutMs = 40 * 60_000) {
  return new Promise(resolve => {
    const [cmd, ...args] = buildCommand.split(" ");
    const child = spawn(cmd, args, { cwd: root, windowsHide: true, env: childEnv(), stdio: ["ignore", "ignore", "pipe"] });
    let err = ""; const timer = setTimeout(() => child.kill(), timeoutMs);
    child.stderr.setEncoding("utf8").on("data", c => { err = (err + c).slice(-4000); });
    child.on("error", e => { clearTimeout(timer); resolve({ ok: false, note: e.message }); });
    child.on("close", code => { clearTimeout(timer); resolve({ ok: code === 0, note: err.split(/\r?\n/).filter(l => /error/.test(l)).slice(0, 2).join(" ") }); });
  });
}
function sendBack(id, entry, summary, note, files = []) {
  const base = { id, label: entry.label, tier: "engine·proof", at: Date.now(), ms: 0, files, log: entry.log, tokens: 0, cost: 0 };
  if (proofRetried.has(id)) { record({ ...base, status: "blocked", summary: summary.replace("sent back", "twice"), final: true }); state.totals.blocked++; return; }
  proofRetried.add(id);
  record({ ...base, status: "escalated", summary, final: false });
  brain.learn({ id, label: entry.label, status: "failed", summary: "engine mismatch: " + summary }).catch(() => {});
  requeue(id, 0, note);
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
  const free = brain.freeGb();
  if (free < 8) { state.lastProof = { at: Date.now(), ok: false, note: `skipped: ${free.toFixed(1)} GB free, the engine build needs 8` }; event(`proof: skipped, only ${free.toFixed(1)} GB free (needs 8 for the engine build)`); return; }
  const run = [...new Map([...pending.values()].flat().map(s => [s.id, s])).values()];
  state.checkpointing = `building the engine to prove ${pending.size} row(s)`;
  event("proof: " + state.checkpointing);
  const built = await buildEngine();
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
  let pass = 0, fail = 0;
  for (const action of settleProof(pending, results, proofRetried)) {
    const entry = finished.get(action.id);
    if (action.kind === "proven") {
      proven.add(action.id); pass++; state.totals.proven++; entry.proven = true;
      record({ id: action.id, label: entry.label, tier: "engine·proof", at: Date.now(), ms: 0, files: action.files, log: entry.log, tokens: 0, cost: 0, status: "proven", summary: action.summary, final: true });
    } else { fail++; sendBack(action.id, entry, action.summary, action.note ?? action.summary, action.files); }
  }
  state.lastProof = { at: Date.now(), ok: fail === 0, pass, fail };
  event(`proof: ${pass} row(s) proven, ${fail} sent back`);
  persist();
}

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

const pageUrl = new URL("./swarm.html", import.meta.url);
// peek() reads the whole ledger, so the push loop shares one result for ten seconds.
let peekCache = { at: 0, rows: [] };
const peekCached = () => { if (Date.now() - peekCache.at > 10_000) peekCache = { at: Date.now(), rows: peek() }; return peekCache.rows; };
// The queue is computed first: it also refreshes state.deadOwners, which the spread then carries.
const stateView = (queue = peekCached()) => ({ ...state, rev, now: Date.now(), workers: state.workers.map(({ child, ...w }) => w), queue,
  parked: [...parked], lessons: brain.lessons().slice(0, 12), families: Object.keys(FAMILIES) });
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
      if (Number.isFinite(b.budget) && b.budget >= 0) { cfg.budget = b.budget; if (state.paused?.startsWith("budget cap") && (!b.budget || state.totals.cost < b.budget)) state.paused = null; }
      for (const k of ["scout", "readyOnly", "context", "prove", "wire"]) if (typeof b[k] === "boolean") cfg[k] = b[k];
      if (Number.isFinite(b.exploreTurns) && b.exploreTurns >= 8 && b.exploreTurns <= 90) cfg.exploreTurns = b.exploreTurns;
      if (Number.isFinite(b.checkpointEvery) && b.checkpointEvery >= 0 && b.checkpointEvery <= 100) cfg.checkpointEvery = b.checkpointEvery;
      if (Number.isFinite(b.proveBatch) && b.proveBatch >= 1 && b.proveBatch <= 50) cfg.proveBatch = b.proveBatch;
      if (Number.isFinite(b.breaker) && b.breaker >= 0 && b.breaker <= 50) cfg.breaker = b.breaker;
      event(`settings: ${JSON.stringify(b).slice(0, 160)}`); persist();
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
    res.writeHead(204); return res.end();
  } else if (req.method === "POST" && url.startsWith("/ctl/")) {
    const action = url.slice(5);
    if (action === "pause") state.paused = "paused by supervisor";
    else if (action === "resume") state.paused = null;
    else if (action === "checkpoint") checkpoint("requested by supervisor");
    else if (action === "prove") { idleProved = false; checkpoint("proof requested by supervisor"); }
    else if (action === "requeue") { for (const [id, r] of [...finished]) if ((r.status === "blocked" || r.status === "failed") && !isDesignBlock(r)) requeue(id, 0, `Previously blocked: ${String(r.summary).slice(0, 200)}`); }
    else if (action === "stop") { state.stop = true; state.userStop = true; for (const w of state.workers) w.child?.kill(); }
    else if (action === "add" && state.workers.length < 16) {
      if (state.stop && !state.userStop) state.stop = false; // revive after an automatic stop
      const w = { id: "w" + (state.workers.length + 1), status: "idle", row: null, activity: [], turns: 0, files: [] };
      state.workers.push(w); work(w);
    } else if (action === "drop") { const w = [...state.workers].reverse().find(x => x.status === "idle" || x.status === "working"); if (w && state.workers.length > 1) w.retire = true; }
    else if (action.startsWith("kill-")) state.workers.find(w => w.id === action.slice(5))?.child?.kill();
    res.writeHead(204); res.end();
  } else { res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" }); res.end(readFileSync(pageUrl, "utf8")); }
});
function peek() {
  try {
    const ledger = loadLedger();
    const { rows, pushed } = eligible(ledger);
    const fresh = [...pushed, ...nextRows(rows, 12).filter(r => !priority.includes(r.id))].slice(0, 12)
      .map(r => ({ id: r.id, label: r.label, tier: cfg.scout && !retry.has(r.id) ? scoutTier.name : tiers[Math.min(tiers.length - 1, retry.get(r.id) ?? 0)].name, pinned: priority.includes(r.id) }));
    // When new rows run short the workers take wire passes; show them so the queue is not reported empty.
    const wires = fresh.length < 12 ? wireCandidates(ledger).slice(0, 12 - fresh.length).map(w => ({ id: w.row.id, label: "wire: " + w.row.label, tier: cfg.wireModel === "haiku" ? wireTier.name : tiers[0].name + "·wire", pinned: false })) : [];
    return [...fresh, ...wires];
  } catch { return []; }
}

if (flag("dry")) { console.log(JSON.stringify({ cfg, queue: peek() }, null, 1)); process.exit(0); }
server.listen(cfg.port, "127.0.0.1", () => console.log(`swarm ui http://127.0.0.1:${cfg.port}  workers=${cfg.workers} family=${cfg.family}`));
for (let i = 1; i <= cfg.workers; i++) state.workers.push({ id: "w" + i, status: "idle", row: null, activity: [], turns: 0, files: [] });
setInterval(persist, 5000);
await Promise.all(state.workers.map((w, i) => new Promise(r => setTimeout(r, i * 1500)).then(() => work(w))));
persist();
console.log("swarm workers finished", JSON.stringify(state.totals));
// Stay up after an automatic stop so the UI can report why and "+ agent" can revive the run;
// exit only when the supervisor pressed Stop all.
if (state.userStop) { server.close(); setTimeout(() => process.exit(0), 500); }
