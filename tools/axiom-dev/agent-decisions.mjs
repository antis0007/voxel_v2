import { createHash } from "node:crypto";
import { closeSync, existsSync, lstatSync, openSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { canAssist, verificationSource } from "./agent-contract.mjs";
import { collectContext } from "./agent-context.mjs";
import { ensureDirectory, read, state } from "./workspace.mjs";

const hash = value => createHash("sha256").update(JSON.stringify(value)).digest("hex");
const digest = /^[a-f0-9]{64}$/;
export const improvementLanes = {
  engine: {
    path: /^crates\/.+\.(?:rs|wgsl)$/,
    task: "Find one reproducible engine defect. Trace the trigger and affected callers. Preserve behavior outside the failing case and specify a native regression check.",
    acceptance: ["Reproduce the trigger on the pinned baseline and pass the same regression on the candidate.",
      "Check affected callers, save/load and authority invariants; leave unexecuted checks pending."],
  },
  performance: {
    path: /^crates\/.+\.(?:rs|wgsl)$/,
    task: "Find one concrete source of avoidable work, allocation, or contention in the selected path. Preserve output and ordering. Specify a matching baseline and candidate workload and an observable metric. Treat improvement as unmeasured.",
    acceptance: ["Use the same seed, save, workload and executable/source provenance for baseline and candidate.",
      "Measure idle and saturated p95 time plus work/allocation counters; verify unchanged outputs before accepting a gain."],
  },
  art: {
    path: /^assets\/(?:icons\/[^/]+\.txt|runtime\/.+\.ron|models\/.+\.ron)$/,
    task: "Propose one small, optional visual refinement in the selected asset. Preserve authored style, animation, silhouettes, palette identity, and asset IDs. Describe a matched before/after view for visual review. Treat visual quality as pending until viewed.",
    acceptance: ["Compare matched camera, lighting and animation views; preserve silhouette, IDs and readability.",
      "Human visual review remains pending; use original stand-ins unless asset reuse is explicitly licensed."],
  },
  tooling: {
    path: /^tools\/axiom-dev\/[^/]+\.mjs$/,
    task: "Find one reproducible developer-tool defect in context relevance, provenance, stale evidence or validation. Reuse existing tools and specify a regression that fails before the fix. Preserve provider and review boundaries.",
    acceptance: ["Run the same frozen regression against baseline and candidate; require failure before and pass after.",
      "Keep provenance, stale-evidence rejection and review boundaries intact; syntax checks alone do not prove correctness."],
  },
};

const pinsFor = context => [...new Map([...(context.boardVersions ?? []),
  ...context.evidence.map(row => [row.path, row.version])])].sort(([a], [b]) => a.localeCompare(b));
const refsFor = context => (context.modReferences ?? []).map(row => row.digest).sort();

function historyConflict(ownerPath, version, history) {
  if (history.active?.includes(ownerPath)) return { blocked: "Owner already has active work" };
  const historical = (history.receipts ?? []).find(row => row.kind === "run" && (
    row.sources?.some(source => source.path === ownerPath && source.version === version)
    || row.evidence?.files?.some(source => source.path === ownerPath && [source.before, source.after].includes(version))));
  const applied = (history.bundles ?? []).find(row => row.files?.some(source => source.path === ownerPath && source.after === version));
  return historical || applied ? { blocked: "Current owner already has a run or saved candidate; review that evidence first",
    priorRun: historical?.id, priorBundle: applied?.id } : undefined;
}

// The ledger records decisions/ownership; existing run receipts and candidate bundles remain
// the sources of execution/publication evidence. Nothing here marks a game change verified.
export function createDecisionStore({ directory, ensure, readSource = read }) {
  const file = path.join(directory, "decisions.json"), lock = file + ".lock";
  function load() {
    if (!existsSync(file)) return [];
    const stat = lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 16 * 1024 * 1024) throw new Error("Invalid improvement ledger");
    const rows = JSON.parse(readFileSync(file, "utf8"));
    if (!Array.isArray(rows) || rows.length > 1000 || rows.some(row => !digest.test(row?.id)
      || !canAssist(row?.path) || !Array.isArray(row.pins) || !Array.isArray(row.references)
      || !["proposed", "claimed", "review-needed", "no-change", "blocked", "cancelled"].includes(row.status))) {
      throw new Error("Invalid improvement ledger; retain it for inspection");
    }
    return rows;
  }
  function mutate(fn) {
    ensure(directory);
    let descriptor;
    try { descriptor = openSync(lock, "wx"); }
    catch (error) {
      if (error.code === "EEXIST") throw new Error("Another planner owns the ledger lock. Retry after it finishes; inspect an abandoned lock manually.");
      throw error;
    }
    const temporary = file + ".tmp";
    try {
      const rows = load(), result = fn(rows);
      writeFileSync(temporary, JSON.stringify(rows), { flag: "wx" });
      renameSync(temporary, file);
      return structuredClone(result);
    } finally {
      closeSync(descriptor);
      unlinkSync(lock);
      // A failed atomic write leaves its .tmp for inspection; never erase somebody else's file.
    }
  }
  function fresh(record) {
    for (const [file, version] of record.pins) {
      if (readSource(file).version !== version) throw new Error("Decision evidence changed: " + file + ". Propose a fresh move.");
    }
  }
  return {
    list: () => structuredClone(load()),
    get(id) {
      const row = load().find(row => row.id === id);
      if (!row) throw new Error("Unknown improvement decision");
      return structuredClone(row);
    },
    fresh,
    propose(context, request, ownerPath, history = {}) {
      const lane = request.lane, pins = pinsFor(context), references = refsFor(context);
      const primary = pins.find(([file]) => file === ownerPath);
      if (!primary || !improvementLanes[lane]?.path.test(ownerPath) || verificationSource(ownerPath)) {
        throw new Error("Choose an implementation owner supported by this lane");
      }
      const identity = hash([lane, ownerPath, primary[1], references]);
      const id = hash([identity, pins]);
      return mutate(rows => {
        const conflict = rows.find(row => row.status === "claimed" && row.path === ownerPath);
        if (conflict || history.active?.includes(ownerPath)) return { blocked: "Owner already has active work", decision: conflict };
        const previous = rows.find(row => row.identity === identity && row.status !== "proposed"
          && row.pins.every(([file, version]) => { try { return readSource(file).version === version; } catch { return false; } }));
        if (previous) return { blocked: "Already attempted on this evidence; review the prior outcome or gather changed evidence", decision: previous };
        const historical = historyConflict(ownerPath, primary[1], history);
        if (historical) return historical;
        const known = rows.find(row => row.id === id);
        if (known) return { decision: known };
        if (rows.length >= 1000) throw new Error("Decision ledger is full; archive it explicitly before planning more work");
        const decision = { id, identity, lane, path: ownerPath, task: request.task, status: "proposed",
          createdAt: new Date().toISOString(), pins, references,
          contextRequest: { task: request.task, paths: request.paths ?? [], symbols: request.symbols ?? [],
            targets: request.targets ?? [], modDigests: references },
          facts: context.evidence.map(({ path, version, line, first, last, kind }) => ({ path, version, line, first, last, kind })),
          referenceFacts: (context.modReferences ?? []).map(row => ({ digest: row.digest, coverage: row.coverage,
            recipePaths: row.recipes.rows.map(recipe => recipe.path),
            methods: row.behavior.methods.map(method => method.owner + "." + method.method), reuse: row.reuse })),
          hypothesis: "Investigate the requested behavior at this owner; existing guards may already satisfy it. No defect or missing feature is established by retrieval.",
          dependencies: context.evidence.filter(row => row.path !== ownerPath).map(row => row.path).filter((file, i, all) => all.indexOf(file) === i),
          acceptance: [...improvementLanes[lane].acceptance, "Do not weaken or rewrite existing tests, fixtures or acceptance criteria to pass."],
          verification: "pending", adoption: "human review required" };
        fresh(decision);
        rows.push(decision);
        return { decision };
      });
    },
    claim(id, owner, history = {}) {
      if (typeof owner !== "string" || !owner || owner.length > 160) throw new Error("A bounded run owner is required");
      return mutate(rows => {
        const row = rows.find(row => row.id === id);
        if (!row || row.status !== "proposed") throw new Error("Decision is already owned or attempted");
        if (rows.some(other => other.status === "claimed" && other.path === row.path)) throw new Error("Implementation owner already claimed");
        fresh(row);
        const prior = historyConflict(row.path, row.pins.find(([file]) => file === row.path)[1],
          typeof history === "function" ? history() : history);
        if (prior) throw new Error(prior.blocked);
        row.status = "claimed";
        row.owner = owner;
        row.claimedAt = new Date().toISOString();
        return row;
      });
    },
    settle(id, owner, outcome) {
      if (!["review-needed", "no-change", "blocked", "cancelled"].includes(outcome?.status)) throw new Error("No automatic verified/adopted outcome is supported");
      return mutate(rows => {
        const row = rows.find(row => row.id === id);
        if (!row || row.status !== "claimed" || row.owner !== owner) throw new Error("Only the claimed run may record its outcome");
        row.status = outcome.status;
        row.finishedAt = new Date().toISOString();
        row.runId = outcome.runId;
        row.detail = String(outcome.detail ?? "").slice(0, 1000);
        row.validation = String(outcome.validation ?? "Not run").slice(0, 100);
        return row;
      });
    },
  };
}

export const decisions = createDecisionStore({ directory: path.join(state, "improvement-decisions"), ensure: ensureDirectory });

export async function proposeNext(request, logs = [], history = {}) {
  if (!Object.hasOwn(improvementLanes, request?.lane)) throw new Error("Choose engine, performance, art or tooling");
  if (typeof request.task !== "string" || request.task.length > 1800) throw new Error("Use a focused brief of up to 1800 characters");
  const context = await collectContext(request, logs);
  const owned = new Set([...(request.paths ?? []), ...(request.targets ?? []).map(row => row.path)]);
  const candidates = context.evidence.filter(row => row.repairEligible && !verificationSource(row.path)
    && improvementLanes[request.lane].path.test(row.path)
    && (!owned.size || owned.has(row.path) || (row.kind === "System board pin" && owned.has(row.via))));
  const skipped = [], seen = new Set();
  for (const row of candidates) {
    if (seen.has(row.path)) continue;
    seen.add(row.path);
    const result = decisions.propose(context, request, row.path, history);
    if (!result.blocked) return { ...result, contextId: context.id, skipped,
      limits: "One bounded investigation; lexical dependencies are leads. Claims cover this checkout's Forge ledger, not unregistered external agents." };
    skipped.push({ path: row.path, reason: result.blocked, decisionId: result.decision?.id,
      priorRun: result.priorRun, priorBundle: result.priorBundle });
  }
  return { decision: null, contextId: context.id, skipped,
    next: "Gather a missing owner/caller, a new reference, or a measured runtime failure. Review prior outcomes before retrying unchanged work." };
}
