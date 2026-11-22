import { randomUUID } from "node:crypto";
import { existsSync, statfsSync, readFileSync, writeFileSync, renameSync } from "node:fs";
import path from "node:path";
import { canAssist, maxTaskLength, maxExcerpt, prepareProposal, verificationSource } from "./agent-contract.mjs";
import { collectContext } from "./agent-context.mjs";
import { centeredExcerpt } from "./context-targets.mjs";
import { files, read, root, state, ensureDirectory } from "./workspace.mjs";
import { agentStatus, cancelAgent, planWorkflowSteps, runWorkflowAgent, providerResetAt } from "./agent-runner.mjs";
import { acquireAgentLane, agentLane, recoverAgentLane, releaseAgentLane } from "./agent-lane.mjs";
import { archivedWorkflows, archiveWorkflow, loadWorkflows, loadLegacyWorkflows, makeWorkflowRoom, readArchivedWorkflow, workflowStorageVersion, mutateWorkflows, workflowEvent } from "./workflow-store.mjs";
import { verifyCandidate } from "./agent-evidence.mjs";
import { candidateCheck, verifyRegression, assertRegressionFresh, toolingCheckOwners } from "./agent-checks.mjs";
import { decisionBundles, inspectBundle, publishBundle } from "./agent-bundles.mjs";
import { invalidateKnowledge } from "./project-knowledge.mjs";
import { status as gameStatus } from "./game.mjs";
import { brief, comparePriority, loadLedger, nextRows, parityTask, parityTaskRevision, writeStatus } from "./parity-data.mjs";
import { parityWorkProgress } from "./parity-work.mjs";
import { containsRecoveryWindows, missingSourceStep, recoverWorkflowSourceCandidate } from "./workflow-source.mjs";
import { assertMissionCommit, publishMissionCommit, sharedSourceRevision, sourceConflict } from "./parity-snapshot.mjs";
import { checkWorkflowCompile, assertWorkflowCompileMatches } from "./workflow-compile.mjs";
import { providerLimitText } from "./parity-provider.mjs";

// Opt-in persistent board intake. Existing mission checks and publication remain authoritative.
const autonomyFile = path.join(state, "autonomous-board.json");
function autonomyPolicy() {
  try { return JSON.parse(readFileSync(autonomyFile, "utf8")); }
  catch (error) { if (error.code === "ENOENT") return { enabled: false, admitted: [] }; throw error; }
}
function saveAutonomy(policy) {
  ensureDirectory(state);
  const temporary = autonomyFile + "." + randomUUID() + ".tmp";
  writeFileSync(temporary, JSON.stringify(policy));
  renameSync(temporary, autonomyFile);
}
let active;
let scheduler;
const terminalSteps = new Set(["Saved", "Accepted", "Reported"]);
const runningPhases = new Set(["Running", "Planning"]);
const fail = message => { throw Object.assign(new Error(message), { status: 409 }); };
const required = (value, label, limit) => {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${label} is empty. Enter the intended outcome.`);
  if (value.trim().length > limit) throw new Error(`${label} has ${value.trim().length} characters; the limit is ${limit}.`);
  return value.trim();
};
const find = (rows, id) => rows.find(row => row.id === id) ?? fail("Mission not found.");
const now = () => new Date().toISOString();
const transientFailure = /\b(?:EAI_AGAIN|ECONNRESET|ETIMEDOUT|ECONNREFUSED|EBUSY|ENOSPC|rate limit(?:ed)?|temporarily unavailable|service unavailable|exceeded its time limit|workbench build lane is occupied|less than 512 MB free|no space left on device|os error 112)\b/i;
const providerQuota = providerLimitText;
const invalidPlanScope = /^Choose 1–5 distinct indexed implementation files/;
const automaticParitySource = file => /^crates\/[^/]+\/(?:src\/.*\.(?:rs|wgsl)|build\.rs)$/.test(file)
  || /^assets\/.*\.(?:ron|json)$/.test(file);

// `failedAt` is when the provider reported the limit. "resets 5:30am" names the next 5:30 after that moment,
// so an old failure read back later must not be pushed a further day out.
function quotaRetryAt(message, failedAt = Date.now()) {
  const resetText = message.match(/try again at ([A-Z][a-z]+ \d{1,2})(?:st|nd|rd|th)?, (\d{4} \d{1,2}:\d{2} [AP]M)/i);
  const reset = resetText ? Date.parse(resetText[1] + ", " + resetText[2]) : NaN;
  const from = Number.isFinite(failedAt) ? failedAt : Date.now();
  return Math.max(Date.now() + 60_000, Number.isFinite(reset) ? reset : providerResetAt(message, from));
}
const canRetry = mission => mission.attempts < mission.settings.maxAttempts
  && mission.elapsedMs < mission.settings.maxMinutes * 60000;
const stepAttempts = step => step.attempts - (step.retryBase ?? 0);
const sourceDiagnostic = /^(?:Context changed(?::| on disk)|Source changed during recovery:|Saved sources changed|Attached excerpt changed|No current source context for |Missing source context\s*:|Owner gap\s*:|Candidate validation is Stale)/i;

function missionParityRow(objective, ledger) {
  const words = new Set(objective.toUpperCase().match(/[A-Z0-9]+(?:-[A-Z0-9]+)*/g));
  const matches = ledger.rows.filter(row => words.has(row.id));
  return matches.length === 1 ? matches[0] : undefined;
}

const parityIdOf = (mission, ledger) => mission.parityId ?? missionParityRow(mission.objective, ledger)?.id;
const scheduledParity = mission => runningPhases.has(mission.phase) || mission.phase === "Queued"
  || (mission.phase === "Review" && mission.settings?.publication === "automatic");
const heldParity = mission => ["Paused", "Cancelled", "Review", "Draft", "Ready"].includes(mission.phase);

function autonomyCandidates(ledger, current, legacy, archive, work) {
  const latest = new Map(), occupied = new Set();
  for (const mission of [...archive, ...legacy, ...current]) {
    const id = parityIdOf(mission, ledger);
    if (!id) continue;
    if (scheduledParity(mission) || mission.waitingForSource || mission.stopping) occupied.add(id);
    const previous = latest.get(id);
    if (!previous || (mission.updatedAt ?? mission.createdAt ?? "") >= (previous.updatedAt ?? previous.createdAt ?? "")) latest.set(id, mission);
  }
  for (const [id, run] of work) if (run.running && !run.stop) occupied.add(id);
  return nextRows(ledger.rows, ledger.rows.length).flatMap(row => {
    const previous = latest.get(row.id);
    if (row.generated || occupied.has(row.id) || (previous && heldParity(previous))) return [];
    const revision = parityTaskRevision(row);
    if (previous?.autonomousRevision === revision) return [];
    return [{ row, revision, previous }];
  });
}

function feedAutonomousBoard(ledger, rows, work) {
  if (!autonomyPolicy().enabled || rows.filter(row => row.autonomousRevision && scheduledParity(row)).length >= 3) return;
  const [next] = autonomyCandidates(ledger, rows, loadLegacyWorkflows(true), archivedWorkflows(), work);
  if (!next) return;
  const { row, revision, previous } = next;
  const progress = previous && (previous.error || previous.steps?.filter(step => terminalSteps.has(step.status))
    .map(step => step.candidate?.proposal?.summary ?? step.title).join(" ") || previous.phase);
  workflowCommand({ action: "create", parityId: row.id,
    objective: parityTask(row, { previous: progress }),
    settings: { publication: "automatic", checks: "compile", provider: "auto", maxAttempts: 12, maxMinutes: 30 } },
  [], { revision, previousId: previous?.id });
}

// "auto" tries Claude first, then Codex. A long wait recorded from one provider's limit must not
// block the mission while the other provider can still run it; the runner skips limited providers itself.
function autoHasOtherProvider(row) {
  return row.settings?.provider === "auto" && /^(?:codex|claude) exited with/i.test(String(row.error ?? ""));
}

function nextQueuedMission(rows, ledger, work) {
  const byId = new Map(ledger.rows.map(row => [row.id, row]));
  return rows.filter(row => row.phase === "Queued" && ((row.retryAt ?? 0) <= Date.now() || autoHasOtherProvider(row)))
    .map(mission => ({ mission, parity: byId.get(parityIdOf(mission, ledger)) }))
    .filter(({ mission, parity }) => !parity || (!work.get(parity.id)?.running
      && !(mission.autonomousRevision && ["verified", "parked"].includes(parity.status))))
    .sort((a, b) => a.parity && b.parity ? comparePriority(a.parity, b.parity)
      : Number(Boolean(a.parity)) - Number(Boolean(b.parity)))[0]?.mission;
}

function assertUniqueParity(mission, rows, legacy, claiming = false) {
  const ledger = loadLedger();
  const rowId = row => parityIdOf(row, ledger);
  const id = rowId(mission);
  if (!id) return;
  const earlier = rows.slice(0, rows.findIndex(row => row.id === mission.id));
  if (legacy.some(row => scheduledParity(row) && rowId(row) === id)
      || rows.some(row => row.id !== mission.id && scheduledParity(row) && rowId(row) === id &&
        (!claiming || runningPhases.has(row.phase) || earlier.includes(row)))) {
    fail(id + " already has a mission in progress in the current or legacy queue.");
  }
}

function assertFollowUpCurrent(mission, rows, legacy) {
  if (rows.some(row => row.followUpOf?.id === mission.id && !["Complete", "Cancelled"].includes(row.phase))) {
    fail("This mission has an unfinished follow-up. Continue the follow-up.");
  }
  const link = mission.followUpOf;
  if (!link) return;
  const original = [...rows, ...legacy].find(row => row.id === link.id);
  if (!original || original.revision !== link.revision || original.phase !== link.phase) {
    fail("The original mission changed after the follow-up was prepared. Review its current state before continuing.");
  }
}

function parityPrototype(id, scope) {
  const ledger = loadLedger();
  const row = ledger.rows.find(entry => entry.id === id);
  if (!row) throw new Error("Unknown parity feature.");
  const order = brief(row, ledger.systems, true);
  const hints = order.hints.files.filter(file => /\.(rs|ron)$/.test(file) && canAssist(file) && !verificationSource(file));
  if (!scope && !hints.length) throw new Error("Choose an existing source file under Provider and source scope to start this prototype.");
  const paths = cleanPaths(scope ?? hints.slice(0, 2));
  const objective = order.task;
  return { objective, paths, parityId: id, steps: [{
    title: order.label.slice(0, 120),
    task: [id + " · " + order.label, order.mapping, order.contract.outcome,
      order.system && "Honor the world system switch for " + order.system + ".",
      "Implement one usable slice within the selected scope. Reuse the existing mechanism and preserve stable IDs."].filter(Boolean).join("\n").slice(0, 1600),
    acceptance: order.contract.checks.join(" ").slice(0, 700), paths, mode: "refine",
  }] };
}

function toolingImprovement(input) {
  if (!toolingCheckOwners().includes(input.path) || verificationSource(input.path)) {
    throw new Error("Choose a tool with a registered regression check.");
  }
  const outcome = required(input.objective, "Observed problem", 1350);
  const paths = [input.path];
  return { objective: "Improve the dev tools\n" + outcome, paths,
    improvementVersion: read(input.path).version, steps: [{
      title: "Fix one observed tool problem",
      task: outcome + "\nMake one focused change supported by the source. Preserve provenance and existing controls.",
      acceptance: "Reproduce the problem with the frozen registered check and pass the same check after the change. Preserve verification and publication boundaries.",
      paths, mode: "refine",
    }],
    settings: { provider: input.settings?.provider, model: input.settings?.model,
      checks: "registered", publication: "automatic", maxAttempts: 2, maxMinutes: 15 },
  };
}

export function parityMissionProgress(ledger, evidenceFor) {
  const progress = new Map();
  const owner = agentLane();
  for (const mission of [...loadLegacyWorkflows().map(row => ({ ...row, legacy: true })), ...loadWorkflows()]) {
    const id = parityIdOf(mission, ledger);
    if (!id) continue;
    const completed = mission.steps.filter(step => terminalSteps.has(step.status));
    const phase = runningPhases.has(mission.phase) && (!owner?.alive || owner.id !== mission.lease)
      ? "Interrupted" : mission.phase;
    const live = id === evidenceFor && active?.id === mission.id && active.runId ? agentStatus(active.runId) : undefined;
    const pending = mission.steps.find(step => !terminalSteps.has(step.status));
    progress.set(id, { id: mission.id, revision: mission.revision, phase, error: mission.error, legacy: mission.legacy === true, readOnly: mission.legacy === true,
      stopping: mission.stopping, activity: live?.phase === "Running" ? live.step : pending?.status,
      usage: mission.usage, attempts: mission.attempts, maxAttempts: mission.settings.maxAttempts,
      done: completed.length, total: mission.steps.length, evidence: id === evidenceFor ? completed.map(step => ({
        title: step.title, status: step.status, summary: step.candidate?.proposal?.summary,
        freshness: step.candidate ? evidenceState(step, step.candidate, step.pins) : "Source snapshot unavailable",
      })) : undefined });
  }
  return progress;
}

function syncMissionParity(mission, ledger) {
  const row = mission.parityId ? ledger.rows.find(row => row.id === mission.parityId) : missionParityRow(mission.objective, ledger);
  if (!row) return false;
  const evidence = mission.steps
    .filter(step => step.status === "Saved" && /^\d{13}-[a-f0-9-]{36}$/.test(step.bundleId))
    .map(step => path.join(state, "candidate-bundles", step.bundleId + ".json"))
    .filter(file => existsSync(file))
    .map(file => path.relative(root, file).replaceAll(path.sep, "/"));
  if (!evidence.some(entry => !row.evidence.includes(entry))) return false;
  writeStatus(row.id, { evidence, status: ["open", "design"].includes(row.status) ? "building" : row.status },
    ledger.revision, new Set([row.id]));
  return true;
}

function cleanPaths(paths, allowEmpty = false) {
  const indexed = new Set(files().map(row => row.path));
  if (!Array.isArray(paths) || (!allowEmpty && !paths.length) || paths.length > 5
      || paths.some(file => !canAssist(file) || verificationSource(file) || !indexed.has(file))
      || new Set(paths).size !== paths.length) throw new Error("Choose 1–5 distinct indexed implementation files; verification code is protected.");
  return paths;
}

function cleanStepContext(step, paths, pinned = false) {
  const query = step.query === undefined || step.query === "" ? undefined : required(step.query, "Retrieval query", 600);
  const targets = step.targets ?? [];
  if (!Array.isArray(targets) || targets.length > paths.length
      || targets.some(target => !paths.includes(target?.path) || target.path.length > 512 || !Number.isSafeInteger(target.line) || target.line < 1)
      || new Set(targets.map(target => target.path)).size !== targets.length) {
    throw new Error("Choose at most one source-line target per scoped source file; target paths allow at most 512 characters.");
  }
  const resolved = targets.map(target => {
    if ((pinned || target.version !== undefined) && !/^[a-f0-9]{64}$/.test(target.version ?? "")) {
      throw new Error("Source target needs its saved revision: " + target.path);
    }
    const doc = read(target.path);
    if (target.version !== undefined && target.version !== doc.version) {
      throw new Error("Source target changed: " + target.path + ". Inspect the source and explicitly re-pin the plan, or create a follow-up mission.");
    }
    if (target.line > doc.text.split(/\r?\n/).length) throw new Error("Source target line is unavailable: " + target.path + ":" + target.line);
    return { path: target.path, line: target.line, version: doc.version };
  });
  return { ...(query ? { query } : {}), ...(resolved.length ? { targets: resolved } : {}) };
}

function cleanSteps(steps, { discover = false, allowed } = {}) {
  if (!Array.isArray(steps) || !steps.length || steps.length > 12) throw new Error("A mission needs 1–12 ordered steps.");
  return steps.map((step, index) => {
    if (!["refine", "repair", "audit"].includes(step?.mode)) throw new Error("Choose implementation, repair or read-only audit for each step.");
    const paths = cleanPaths(step.paths, discover && step.mode === "refine");
    if (allowed && paths.some(file => !allowed.has(file))) throw new Error("Plan selected a path outside the checked owner candidates.");
    const context = cleanStepContext(step, paths);
    return { id: randomUUID(), title: required(step.title, "Step title", 120),
      task: required(step.task, "Step task", 1600), acceptance: required(step.acceptance, "Acceptance", 700),
      paths, ...context, mode: step.mode, status: "Queued", attempts: 0, order: index + 1,
      ...(!paths.length ? { sourceRecovery: { reason: "Find a native implementation owner for this planned step.", toolAssisted: true } } : {}) };
  });
}

function cleanSettings(input) {
  const { provider = input.model ? "codex" : "auto", model = "", publication = "automatic",
    checks = publication === "automatic" ? "compile" : "syntax",
    maxAttempts = 12, maxMinutes = 30 } = input;
  if (!["auto", "codex", "claude", "ollama", "lmstudio"].includes(provider) || typeof model !== "string"
      || (model && !/^[a-zA-Z0-9][\w.:/@-]{0,99}$/.test(model)) || (provider === "auto" && model)
      || (["ollama", "lmstudio"].includes(provider) && !model)
      || !["review", "automatic"].includes(publication) || !["syntax", "compile", "build", "registered"].includes(checks)
      || !Number.isInteger(maxAttempts) || maxAttempts < 1 || maxAttempts > 36
      || !Number.isInteger(maxMinutes) || maxMinutes < 1 || maxMinutes > 180) throw new Error("Invalid mission provider, publication policy or resource budget.");
  return { provider, model, publication, checks, maxAttempts, maxMinutes };
}

function update(id, kind, detail, change, stepId, expectedRevision) {
  return mutateWorkflows(rows => {
    const mission = find(rows, id);
    if (expectedRevision !== undefined && mission.revision !== expectedRevision) fail("Mission changed. Refresh before retrying.");
    change?.(mission, rows);
    workflowEvent(mission, kind, detail, stepId);
    return mission;
  });
}

function view(mission, owner, detail = false, summary = false) {
  const interrupted = runningPhases.has(mission.phase) && (!owner?.alive || owner.id !== mission.lease);
  const current = active?.id === mission.id && active.runId ? agentStatus(active.runId) : undefined;
  const phase = interrupted ? "Interrupted" : mission.phase;
  const progress = { finished: mission.steps.filter(step => terminalSteps.has(step.status)).length, total: mission.steps.length };
  const live = current?.phase === "Running" ? { step: current.step, pass: current.pass, passes: current.passes } : undefined;
  if (summary) {
    const { id, revision, objective, paths, error, retryAt, stopping } = mission;
    return { id, revision, objective, paths, error, retryAt, stopping, phase, progress, live,
      settings: { provider: mission.settings.provider },
      nextStep: mission.steps.find(step => !terminalSteps.has(step.status))?.title,
      outcome: mission.steps.some(step => step.status === "Saved") ? "Slices saved"
        : mission.steps.length && mission.steps.every(step => step.status === "Reported") ? "Report finished" : "Plan finished",
      lastEvent: mission.events.at(-1) };
  }
  return { ...mission, sourceContextVersion: 1, workflowStorageVersion, recordStorageVersion: 2, phase, progress, live,
    events: detail ? mission.events : mission.events.slice(-8),
    steps: mission.steps.map(({ candidate, pins, ...step }) => ({ ...step,
      evidenceState: detail && candidate ? evidenceState(step, candidate, pins) : undefined,
      candidate: candidate ? (detail ? candidate : { id: candidate.id, summary: candidate.proposal?.summary,
        edits: candidate.proposal?.edits.length, evidence: candidate.evidence, regression: candidate.regression,
        compile: candidate.compile && { ok: candidate.compile.ok, message: candidate.compile.message } }) : undefined,
    })),
  };
}

function evidenceState(step, candidate, pins) {
  const expected = new Map((pins ?? []).map(pin => [pin.path, pin.version]));
  if (step.status === "Saved") for (const file of candidate.evidence?.files ?? []) expected.set(file.path, file.after);
  if (!expected.size) return "Source snapshot unavailable";
  try {
    return [...expected].every(([file, version]) => read(file).version === version)
      ? "Source snapshot still current" : "Historical evidence · sources have changed";
  } catch { return "Historical evidence · source unavailable"; }
}

export function workflowStatus(id, summary = false) {
  const rows = loadWorkflows(true);
  const owner = agentLane();
  if (id) {
    const current = rows.find(row => row.id === id);
    if (current) return view(current, owner, true);
    const archived = readArchivedWorkflow(id);
    if (archived) return { ...view(archived, owner, true), archived: true, readOnly: true };
  }
  let legacy = [], legacyHistoryError;
  try { legacy = loadLegacyWorkflows(true); }
  catch (error) {
    if (id) throw error;
    legacyHistoryError = "Legacy history unavailable; retained on disk: " + error.message;
  }
  const legacyView = (row, detail = false) => ({ ...view(row, owner, detail, summary), recordStorageVersion: 1, legacy: true, readOnly: true,
    recovery: "Preserved legacy history. Create a fresh scoped follow-up in the current queue; old workers retain their proposals." });
  if (id) return legacyView(find(legacy, id), true);
  const archived = archivedWorkflows();
  const policy = autonomyPolicy();
  const ledger = loadLedger();
  const candidates = autonomyCandidates(ledger, rows, legacy, archived, parityWorkProgress());
  const autonomy = { version: 2, enabled: policy.enabled, eligible: candidates.length, next: candidates.slice(0, 3)
    .map(({ row }) => ({ id: row.id, label: row.label, score: row.priority.score })) };
  return { sourceContextVersion: 1, workflowStorageVersion, legacyHistoryError, autonomy, lane: owner,
    archivedCount: archived.length, recentArchive: archived.slice(0, 12),
    missions: [...rows.slice().reverse().map(row => view(row, owner, false, summary)), ...legacy.slice().reverse().map(row => legacyView(row))],
    toolingCheckOwners: toolingCheckOwners() };

}

function diskAdmission() {
  ensureDirectory(state);
  const disk = statfsSync(state);
  if (Number(disk.bavail) * Number(disk.bsize) < 512 * 1024 * 1024) throw new Error("Less than 512 MB free. Free disk space before continuing this mission.");
}

function checkBuildLane() {
  if (gameStatus().building) throw new Error("The workbench build lane is occupied. Resume after the current build finishes.");
}

function pinChanged(pin) {
  try {
    const doc = read(pin.path);
    return doc.version !== pin.version && !containsRecoveryWindows(doc.text, pin.windows);
  } catch { return true; }
}

function assertPins(step) {
  const pin = (step.pins ?? []).find(pinChanged);
  if (pin) throw new Error("Context changed: " + pin.path + ". Retry this step with fresh evidence.");
}

function sourceVersion(file) {
  try { return read(file).version; } catch { return null; }
}

function prepared(step) {
  assertPins(step);
  const candidate = step.candidate;
  if (!candidate?.proposal?.edits.length) throw new Error("This step has no changes to publish.");
  const evidence = verifyCandidate(candidate.proposal, candidate.autoSources, { protectTests: true });
  if (["Failed", "Stale"].includes(evidence.status)) throw new Error("Candidate validation is " + evidence.status + ". Retry with fresh evidence.");
  const sources = candidate.autoSources.map(source => {
    const doc = read(source.path);
    return { ...source, doc: { ...doc, path: source.path }, original: doc.text, start: source.anchor.start };
  });
  return { evidence, changes: prepareProposal(candidate.proposal, sources)
    .map(({ doc, text }) => ({ path: doc.path, version: doc.version, text })) };
}

function reconcilePublication(mission, step) {
  if (!step.candidate?.id) return false;
  const bundle = decisionBundles().find(row => row.runId === step.candidate.id);
  if (!bundle) return false;
  const inspected = inspectBundle(bundle.id);
  const expected = step.candidate.evidence?.files ?? [];
  if (inspected.files.some(row => row.status !== "Applied") || inspected.files.length !== expected.length
      || inspected.files.some(row => !expected.some(pin => pin.path === row.path && pin.after === row.after))) {
    throw new Error("A previous publication exists but its files changed. Inspect bundle " + bundle.id + " before retrying.");
  }
  if (step.preparedCommit) assertMissionCommit(step.preparedCommit, inspected.files.map(file => file.path));
  update(mission.id, "publication-reconciled", step.preparedCommit
    ? "Recovered commit " + step.preparedCommit : "Recovered saved bundle " + bundle.id, draft => {
    const row = draft.steps.find(row => row.id === step.id);
    row.status = "Saved"; row.bundleId = bundle.id; row.commit = row.preparedCommit;
    delete row.preparedCommit;
  }, step.id);
  return true;
}

function publishStep(mission, step, reviewed = false) {
  diskAdmission();
  checkBuildLane();
  if (reconcilePublication(mission, step)) return;
  const { evidence, changes } = prepared(step);
  const candidate = step.candidate;
  if (!reviewed && candidate.error) throw new Error("The model reported an unresolved diagnostic. Review this candidate.");
  if (mission.improvementVersion || (!reviewed && mission.settings.checks === "registered")) {
    const status = candidate.regression?.status;
    if (mission.improvementVersion && status !== "Regression checked") {
      throw new Error("The registered check must reproduce the baseline failure and pass on the candidate before saving this improvement.");
    }
    if (!["Regression checked", "Native checks passed"].includes(status)) throw new Error("No registered behavioral check passed. Review is required.");
    assertRegressionFresh(candidate.regression, { ...candidate, sources: candidate.autoSources });
  } else if (!reviewed && evidence.status !== "Syntax checked" && !(["compile", "build"].includes(mission.settings.checks) &&
      candidate.compile?.ok && evidence.files.every(file => file.status === "Passed" ||
        candidate.compile.files.some(checked => file.path === checked.path && file.after === checked.after)))) {
    throw new Error("Native or manual checks are pending. Review this candidate before saving.");
  }
  update(mission.id, "publication-started", "Saving reversible candidate; gameplay remains unverified", draft => {
    draft.steps.find(row => row.id === step.id).status = "Publishing";
  }, step.id);
  let bundleId, commit, reason;
  if (!reviewed && mission.settings.publication === "automatic") {
    ({ commit, reason } = publishMissionCommit(step.title.replace(/\s+/g, " ").trim(), candidate.id, changes, prepared => {
      bundleId = prepared.bundle;
      update(mission.id, "commit-prepared", "Prepared commit " + prepared.commit, draft => {
        draft.steps.find(row => row.id === step.id).preparedCommit = prepared.commit;
      }, step.id);
    }));
  }
  if (!bundleId) bundleId = publishBundle(changes, candidate.id).id;
  invalidateKnowledge();
  update(mission.id, "publication-saved", commit ? "Committed " + commit
    : "Saved bundle " + bundleId + (reason ? " · " + reason : ""), draft => {
    const row = draft.steps.find(row => row.id === step.id);
    row.status = "Saved"; row.bundleId = bundleId; row.savedAt = now();
    row.commit = commit; row.commitNote = reason;
    delete row.preparedCommit;
  }, step.id);
}

function guard(work) {
  const rows = loadWorkflows();
  const mission = find(rows, work.id);
  if (mission.lease !== work.lease) throw new Error("Mission ownership changed; stop before continuing.");
  if (mission.stopping) work.stop = mission.stopping;
  if (work.stop) throw new Error(work.stop === "Cancelled" ? "Mission cancelled"
    : work.stop === "Deferred" ? "Mission time budget exhausted" : "Mission paused");
  if (Date.now() > work.deadline) { work.stop = mission.settings.publication === "automatic" ? "Deferred" : "Paused"; throw new Error("Mission time budget exhausted"); }
  if (mission.followUpOf) assertFollowUpCurrent(mission, rows, loadLegacyWorkflows());
}

function retryAutomaticStep(mission, step, reason) {
  if (mission.settings.publication !== "automatic" || stepAttempts(step) >= 3
      || mission.attempts >= mission.settings.maxAttempts) return false;
  const feedback = reason.slice(0, 500);
  if (step.retryFeedback === feedback && mission.settings.provider !== "auto") return false;
  update(mission.id, "automatic-retry", feedback, draft => {
    const row = draft.steps.find(entry => entry.id === step.id);
    if (row.sourceRecovery) {
      row.sourceRecovery.priorLocations = row.pins?.map(pin => pin.path) ?? row.sourceRecovery.priorLocations;
      row.sourceRecovery.reason = feedback;
      row.sourceRecovery.used = false;
    }
    resetForFreshSource(row, reason);
    row.retryFeedback = feedback;
  }, step.id);
  return true;
}

async function drive(work, planning, logs) {
  const startedAt = Date.now();
  const timer = setTimeout(() => {
    work.stop = work.automatic ? "Deferred" : "Paused";
    work.discovery?.abort();
    work.compile?.abort();
    if (work.runId) cancelAgent(work.runId);
  }, Math.max(1, work.deadline - Date.now()));
  // Another open panel can request pause. The owner observes the durable request
  // while awaiting a model, rather than requiring both requests on one server.
  const controlTimer = setInterval(() => {
    try {
      const mission = find(loadWorkflows(), work.id);
      if (mission.stopping || mission.lease !== work.lease) {
        work.stop = mission.stopping ?? "Paused";
        work.discovery?.abort();
        work.compile?.abort();
        if (work.runId) cancelAgent(work.runId);
      }
    } catch { /* The normal transition path reports persistence errors. */ }
  }, 250);
  try {
    let mission = find(loadWorkflows(), work.id);
    if (planning) {
      guard(work);
      if (mission.attempts >= mission.settings.maxAttempts) throw new Error("Mission attempt budget exhausted.");
      const parity = mission.parityId && loadLedger().rows.find(row => row.id === mission.parityId);
      const context = await collectContext({ task: mission.objective, paths: mission.paths,
        query: parity ? `${parity.label} ${parity.mapping} ${parity.id}` : undefined }, logs);
      const discover = mission.settings.publication === "automatic" && Boolean(mission.parityId);
      const indexed = new Set(files().map(row => row.path));
      const allowed = new Set([...mission.paths, ...context.evidence.map(row => row.path)]
        .filter(file => indexed.has(file) && canAssist(file) && !verificationSource(file)
          && (!discover || automaticParitySource(file))));
      let cleaned;
      let feedback;
      let quotaMessage;
      for (let pass = 0; pass < 2; pass++) {
        guard(work);
        const { attempts, settings } = find(loadWorkflows(), work.id);
        update(work.id, "planning-attempt", pass ? "Retrying the plan after the first attempt failed"
          : "Planning counts against the mission budget", draft => { draft.attempts++; });
        work.runId = undefined;
        try {
          const proposal = await planWorkflowSteps({ task: mission.objective, ...settings, context,
            alternate: pass === 1 && !feedback, editablePaths: [...allowed], discover, feedback },
          work.lease, id => { work.runId = id; });
          try {
            cleaned = cleanSteps(proposal, { discover, allowed });
            if (mission.autonomousRevision && parity?.status !== "playable" && cleaned.every(step => step.mode === "audit")) {
              throw new Error("Include an implementation step for a concrete missing capability. If its source path is unknown, leave paths empty so the worker can trace it.");
            }
          }
          catch (error) { feedback = error.message; throw error; }
          break;
        } catch (error) {
          guard(work);
          if (providerQuota.test(error.message)) quotaMessage = error.message;
          if (pass || attempts + 1 >= settings.maxAttempts || sourceDiagnostic.test(error.message)
              || (!feedback && settings.provider !== "auto")) {
            throw quotaMessage && quotaMessage !== error.message
              ? new Error(quotaMessage + " · Other planning attempt: " + error.message) : error;
          }
        } finally {
          const run = work.runId && agentStatus(work.runId);
          if (run) update(work.id, "planning-usage", run.providerPasses?.at(-1)?.provider ?? run.phase,
            draft => recordUsage(draft, { ...run.usage, promptBytes: run.promptBytes }));
        }
      }
      guard(work);
      const automatic = mission.settings.publication === "automatic";
      update(work.id, "plan-ready", automatic ? "Queued the generated plan" : "Review and edit the ordered plan before starting", draft => {
        draft.steps = cleaned;
        draft.phase = automatic ? "Queued" : "Ready";
        draft.planningRequested = false;
      });
      return;
    }
    while (true) {
      guard(work);
      mission = find(loadWorkflows(), work.id);
      const step = mission.steps.find(row => !terminalSteps.has(row.status));
      if (!step) {
        update(work.id, "complete", "All steps saved, accepted or reported", draft => { draft.phase = "Complete"; });
        return;
      }
      if (step.status === "Publishing" && reconcilePublication(mission, step)) continue;
      if (step.candidate) {
        if (step.candidate.evidence?.status === "Failed") {
          const reason = step.candidate.evidence.files.filter(file => file.status === "Failed")
            .map(file => file.path + " · " + file.detail).join("\n");
          if (retryAutomaticStep(mission, step, reason)) continue;
          throw new Error(reason);
        }
        if (!step.candidate.proposal?.edits.length) {
          const prerequisite = step.candidate.proposal?.checks.find(check => /^blocked prerequisite\s*:/i.test(check));
          if (prerequisite && step.mode !== "audit") throw new Error(prerequisite);
          const diagnostic = step.candidate.proposal?.checks.find(check => /^(?:missing source context|owner gap)\s*:/i.test(check));
          if (step.mode === "audit" && !step.candidate.error) {
            update(work.id, "report-recorded", "Read-only report produced; findings are not runtime verification", draft => {
              draft.steps.find(row => row.id === step.id).status = "Reported";
            }, step.id);
            continue;
          }
          if (mission.settings.publication === "automatic") {
            const reason = step.candidate.error ?? diagnostic ?? step.candidate.proposal?.summary ?? "No publishable result";
            if (missingSourceStep(mission, reason) || sourceDiagnostic.test(reason)) throw new Error(reason);
            if (retryAutomaticStep(mission, step, reason)) continue;
            throw new Error(reason);
          }
          update(work.id, "review-needed", "No changes proposed. Explicitly accept the outcome or retry with a narrower scope.", draft => { draft.phase = "Review"; });
          return;
        }
        if (mission.settings.publication === "automatic" && mission.settings.checks === "registered"
            && step.candidate.regression && (step.checkRefreshes ?? 0) < 2) {
          assertPins(step);
          try { assertRegressionFresh(step.candidate.regression, { ...step.candidate, sources: step.candidate.autoSources }); }
          catch (error) {
            update(work.id, "check-refresh-queued", error.message, draft => {
              const row = draft.steps.find(entry => entry.id === step.id);
              row.checkRefreshes = (row.checkRefreshes ?? 0) + 1;
              delete row.candidate.regression;
              delete row.checkUnavailable;
            }, step.id);
            continue;
          }
        }
        if (mission.settings.checks === "registered" && !step.candidate.regression && !step.checkUnavailable) {
          checkBuildLane();
          guard(work);
          assertPins(step);
          const run = { ...step.candidate, sources: step.candidate.autoSources, protectTests: true };
          const checkId = candidateCheck(run);
          if (checkId) {
            // Fixed read-only checks may be repeated after interruption; source publication is reconciled separately.
            update(work.id, "check-started", checkId, draft => { draft.steps.find(row => row.id === step.id).status = "Checking"; }, step.id);
            const regression = await verifyRegression(run, checkId);
            assertRegressionFresh(regression, run);
            update(work.id, "check-recorded", regression.status, draft => {
              const row = draft.steps.find(row => row.id === step.id);
              row.candidate.regression = regression; row.status = "Candidate";
            }, step.id);
          } else update(work.id, "check-unavailable", "No registered behavioral check for this scope", draft => {
            draft.steps.find(row => row.id === step.id).checkUnavailable = true;
          }, step.id);
          continue;
        }
        guard(work);
        try {
          const { checks, publication } = mission.settings;
          const build = checks === "build";
          if ((build || (publication === "automatic" && checks === "compile" &&
              step.candidate.evidence?.status === "Checks pending")) && !step.candidate.error) {
            const { changes, evidence } = prepared(step);
            work.compile = new AbortController();
            try {
              let receipt = step.candidate.compile;
              if (!receipt || (build && receipt.mode !== "build")) {
                diskAdmission();
                checkBuildLane();
                const sourceRevision = await sharedSourceRevision();
                guard(work);
                update(work.id, "compile-started", build ? "Building Axiom in an isolated workspace" : "Checking an isolated Rust workspace", draft => {
                  draft.steps.find(row => row.id === step.id).status = "Checking";
                }, step.id);
                receipt = await checkWorkflowCompile(changes, evidence, work.compile.signal, checks);
                guard(work);
                if (!receipt.ok && transientFailure.test(receipt.message)) throw new Error(receipt.message);
                if (receipt.baselineBlocked) {
                  update(work.id, "compile-baseline-blocked", receipt.message, draft => {
                    draft.phase = "Deferred";
                    draft.error = receipt.message;
                    const row = draft.steps.find(entry => entry.id === step.id);
                    row.status = "Candidate";
                    draft.waitingForSource = { step: row.id, revision: sourceRevision, recheckCandidate: true };
                  }, step.id);
                  return;
                }
                update(work.id, "compile-recorded", receipt.message, draft => {
                  const row = draft.steps.find(row => row.id === step.id);
                  row.candidate.compile = receipt; row.status = "Candidate";
                }, step.id);
                continue;
              }
              if (publication === "automatic") {
                if (!receipt.ok) throw new Error(receipt.message);
                assertWorkflowCompileMatches(receipt, changes);
                const conflict = build && await sourceConflict(receipt.inputs);
                if (conflict) throw new Error("Context changed: " + conflict + " since the isolated game build.");
              }
            } finally { work.compile = undefined; }
          }
          if (publication !== "automatic") {
            update(work.id, "review-needed", "Candidate is ready for review", draft => { draft.phase = "Review"; });
            return;
          }
          guard(work);
          publishStep(mission, step);
        }
        catch (error) {
          const diagnostics = step.candidate.regression?.status === "Unverified"
            ? "Registered check " + step.candidate.regression.checkId + " failed. Candidate counts: "
              + JSON.stringify({ pass: step.candidate.regression.candidate.pass, fail: step.candidate.regression.candidate.fail,
                compileFailed: step.candidate.regression.candidate.compileFailed })
            : step.candidate.error ?? (step.candidate.compile?.ok === false ? step.candidate.compile.message : undefined);
          if (!sourceDiagnostic.test(error.message) && diagnostics && !transientFailure.test(diagnostics)
              && retryAutomaticStep(mission, step, diagnostics)) continue;
          throw error;
        }
        continue;
      }
      if (mission.attempts >= mission.settings.maxAttempts) {
        throw new Error("Mission attempt budget exhausted. Adjust the budget before resuming.");
      }
      const recovery = step.sourceRecovery;
      if (recovery && (!recovery.used || !recovery.toolAssisted)) {
        diskAdmission();
        const runId = randomUUID();
        const sourceRevision = await sharedSourceRevision();
        guard(work);
        update(work.id, "source-discovery-started", recovery.reason, draft => {
          draft.attempts++;
          const row = draft.steps.find(row => row.id === step.id);
          row.attempts++;
          row.sourceRecovery.used = true;
          row.sourceRecovery.toolAssisted = true;
          row.sourceRecovery.sourceRevision = sourceRevision;
          row.status = "Discovering sources";
          row.runId = runId;
        }, step.id);
        work.discovery = new AbortController();
        let usage;
        try {
          const { candidate, pins, lookups } = await recoverWorkflowSourceCandidate(mission, step, work.discovery.signal, runId,
            counts => { usage ??= { input: 0, output: 0, promptBytes: 0 }; recordUsage({ usage }, counts); });
          guard(work);
          update(work.id, "source-recovery-candidate", candidate.evidence.status, draft => {
            const row = draft.steps.find(row => row.id === step.id);
            row.sourceRecovery.lookups = lookups;
            row.candidate = candidate; row.pins = pins; row.status = "Candidate";
          }, step.id);
        } catch (error) {
          guard(work);
          const latest = find(loadWorkflows(), work.id);
          const attempt = latest.steps.find(row => row.id === step.id);
          if (!transientFailure.test(error.message) && retryAutomaticStep(latest, attempt, error.message)) continue;
          throw error;
        } finally {
          work.discovery = undefined;
          if (usage) update(work.id, "source-recovery-usage", "Recorded provider usage", draft => recordUsage(draft, usage), step.id);
        }
        continue;
      }
      if (mission.improvementVersion && read(step.paths[0]).version !== mission.improvementVersion) {
        throw new Error("The tool changed since the improvement was queued. Create a new improvement on the current source.");
      }

      const task = [step.task, "Acceptance: " + step.acceptance,
        step.retryFeedback && "Previous attempt: " + step.retryFeedback].filter(Boolean).join("\n");
      const contextHints = cleanStepContext(step, step.paths, true);
      const context = await collectContext({ task, ...contextHints, paths: step.paths, runtime: true }, logs);
      for (const target of contextHints.targets ?? []) {
        if (!context.evidence.some(row => row.path === target.path && row.line === target.line
            && row.version === target.version && row.kind !== "Existing test source")) {
          throw new Error("Requested source target is missing from implementation evidence: " + target.path + ":" + target.line);
        }
      }
      guard(work);
      const sources = step.paths.map((file, index) => {
        const doc = read(file);
        const target = contextHints.targets?.find(row => row.path === file);
        const hit = context.evidence.find(row => row.path === file && row.kind !== "Existing test source"
          && (!target || row.line === target.line));
        if (!hit || hit.version !== doc.version) throw new Error("No current source context for " + file + ". Refine the task or scope.");
        const excerpt = doc.text.length <= maxExcerpt ? { start: 0, text: doc.text } : centeredExcerpt(doc.text, hit.line, maxExcerpt);
        return { id: "S" + (index + 1), path: file, label: file.split("/").at(-1), text: excerpt.text,
          anchor: { start: excerpt.start, version: doc.version } };
      });
      update(work.id, "step-started", step.title, draft => {
        draft.attempts++;
        const row = draft.steps.find(row => row.id === step.id);
        row.attempts++; row.status = "Gathering context"; row.attemptId = randomUUID();
      }, step.id);
      update(work.id, "context-pinned", context.id, draft => {
        const row = draft.steps.find(row => row.id === step.id);
        row.pins = [...new Map([...(context.boardVersions ?? []), ...context.evidence.map(item => [item.path, item.version])])]
          .map(([path, version]) => ({ path, version }));
        row.contextId = context.id; row.status = "Generating";
        row.runtime = context.runtime;
      }, step.id);
      const candidate = await runWorkflowAgent({ task, ...mission.settings, mode: step.mode,
        alternate: step.attempts % 2 === 1,
        passes: step.mode === "repair" ? 4 : step.mode === "audit" || mission.quickParity || mission.improvementVersion
          || mission.settings.provider === "auto" ? 1 : 3,
        sources, contextId: context.id,
        reference: JSON.stringify({ objective: mission.quickParity ? undefined : mission.objective,
          completed: mission.steps.filter(row => terminalSteps.has(row.status)).map(row => ({ title: row.title,
            status: row.status, bundleId: row.bundleId, summary: row.candidate?.proposal?.summary.slice(0, 600) })) }) }, work.lease,
      id => { work.runId = id; update(work.id, "run-started", id, draft => { draft.steps.find(row => row.id === step.id).runId = id; }, step.id); });
      update(work.id, "usage-recorded", "Recorded provider usage", draft => {
        recordUsage(draft, { ...candidate.usage, promptBytes: candidate.promptBytes });
      }, step.id);
      if (candidate.phase !== "Ready") {
        guard(work);
        const latest = find(loadWorkflows(), work.id);
        const attempt = latest.steps.find(row => row.id === step.id);
        const reason = candidate.error ?? candidate.phase;
        if (missingSourceStep(latest, reason) || sourceDiagnostic.test(reason)) throw new Error(reason);
        if (providerQuota.test(attempt.retryFeedback ?? "")) {
          throw new Error(attempt.retryFeedback + " · Alternate attempt: " + reason);
        }
        if (providerQuota.test(reason) && latest.settings.provider !== "auto") throw new Error(reason);
        if (!transientFailure.test(reason) && retryAutomaticStep(latest, attempt, reason)) continue;
        throw new Error(reason);
      }
      // Persist before checks or publication so a stopped server never loses the candidate.
      update(work.id, "candidate-recorded", candidate.evidence?.status ?? "Unverified", draft => {
        const row = draft.steps.find(row => row.id === step.id);
        row.candidate = candidate; row.status = "Candidate";
      }, step.id);
      guard(work);
    }
  } catch (error) {
    const message = work.stop === "Deferred" ? "Mission time budget exhausted"
      : work.stop === "Paused" && error.message === "Cancelled" ? "Mission paused" : error.message;
    update(work.id, "stopped", message, draft => {
      const automatic = draft.settings.publication === "automatic";
      draft.phase = work.stop ?? (automatic ? "Deferred" : "Blocked");
      draft.error = message;
      const step = draft.steps.find(row => !terminalSteps.has(row.status));
      if (work.stop || !automatic) return;
      if (step?.status !== "Publishing" && providerQuota.test(message) && canRetry(draft)) {
        if (step?.status === "Discovering sources" && step.sourceRecovery) step.sourceRecovery.used = false;
        draft.retryAt = quotaRetryAt(message);
        draft.phase = "Queued";
        return;
      }
      if (step?.status !== "Publishing" && transientFailure.test(message) && canRetry(draft)) {
        if (step?.status === "Discovering sources" && step.sourceRecovery) step.sourceRecovery.used = false;
        draft.retryAt = Date.now() + [15000, 60000, 240000][Math.min(draft.transientRetries ?? 0, 2)];
        draft.transientRetries = (draft.transientRetries ?? 0) + 1;
        draft.phase = "Queued";
        return;
      }
      // The model's "Missing source context:" names the exact file, symbol and line range it needs. It used to
      // park the row as Deferred until someone pressed Gather Context; on 2026-11-21 six rows sat that way,
      // each re-planned and re-asked for the same excerpt. Feed the request into tool-assisted recovery, twice.
      if (step && step.status !== "Publishing" && /^Missing source context\s*:/i.test(message) && canRetry(draft)
          && (step.contextRecoveries ?? 0) < 2) {
        step.contextRecoveries = (step.contextRecoveries ?? 0) + 1;
        queueSourceRecovery(draft, step);
        draft.retryAt = Date.now() + 5000;
        return;
      }
      if (draft.improvementVersion || !sourceDiagnostic.test(message)) return;
      if (!step || step.targets?.length || step.status === "Publishing" || missingSourceStep(draft)
          || /^(?:Missing source context|Owner gap)\s*:/i.test(message)) return;
      if (error.changedSource && canRetry(draft)) {
        draft.waitingForSource = { step: step.id, pins: [error.changedSource] };
        return;
      }
      if (stepAttempts(step) >= 3) return;
      const freshPins = step.pins?.some(pinChanged)
        || /^(?:Context changed: .* (?:since compilation|before compilation)\.)$/.test(message);
      if (freshPins && canRetry(draft)) {
        if (step.sourceRecovery) step.sourceRecovery.used = false;
        resetForFreshSource(step, message);
        draft.phase = "Queued";
        draft.retryAt = Date.now() + 5000;
        return;
      }
      const revision = step.sourceRecovery?.sourceRevision;
      if (revision || step.pins?.length) draft.waitingForSource = { step: step.id, revision, pins: revision ? undefined : step.pins };
    });
  } finally {
    clearTimeout(timer);
    clearInterval(controlTimer);
    try { update(work.id, "lane-released", "AI lane released", draft => { draft.elapsedMs += Date.now() - startedAt; delete draft.stopping; }); }
    finally { releaseAgentLane(work.lease); active = undefined; }
  }
}

function recordUsage(mission, usage) {
  mission.usage ??= { input: 0, output: 0, promptBytes: 0 };
  mission.usage.input += usage?.input ?? 0;
  mission.usage.output += usage?.output ?? 0;
  mission.usage.promptBytes += usage?.promptBytes ?? 0;
}

function begin(mission, planning, logs) {
  if (active) fail("A mission is already running.");
  if (mission.elapsedMs >= mission.settings.maxMinutes * 60000) fail("Time budget exhausted. Increase the budget before resuming.");
  diskAdmission();
  checkBuildLane();
  const lease = acquireAgentLane("Mission: " + mission.objective.slice(0, 100));
  try {
    update(mission.id, planning ? "planning" : "resumed", planning ? "Creating an ordered plan" : "Continuing from the first unfinished step", (draft, rows) => {
      const legacy = loadLegacyWorkflows();
      assertFollowUpCurrent(draft, rows, legacy);
      assertUniqueParity(draft, rows, legacy, true);
      draft.phase = planning ? "Planning" : "Running"; draft.lease = lease; delete draft.error; delete draft.stopping;
      delete draft.retryAt;
      delete draft.waitingForSource;
    }, undefined, mission.revision);
  } catch (error) { releaseAgentLane(lease); throw error; }
  const work = active = { id: mission.id, lease, deadline: Date.now() + mission.settings.maxMinutes * 60000 - mission.elapsedMs,
    automatic: mission.settings.publication === "automatic" };
  void drive(work, planning, logs).catch(error => { process.stderr.write("Mission persistence failure: " + error.message + "\n"); });
}

function enqueue(mission, planning) {
  update(mission.id, "queued", "Waiting for the shared AI lane and resource admission", (draft, rows) => {
    const legacy = loadLegacyWorkflows();
    assertFollowUpCurrent(draft, rows, legacy);
    assertUniqueParity(draft, rows, legacy);
    draft.phase = "Queued"; draft.planningRequested = planning; delete draft.error;
    delete draft.waitingForSource;
    delete draft.retryAt;
  }, undefined, mission.revision);
}

function queueSourceRecovery(mission, step) {
  if (step.candidate) {
    step.history ??= [];
    step.history.push({ runId: step.candidate.id, summary: step.candidate.proposal?.summary,
      evidence: step.candidate.evidence, at: now(), reason: mission.error });
  }
  step.sourceRecovery = { reason: mission.error, toolAssisted: true,
    priorLocations: step.sourceRecovery?.targets };
  delete step.candidate;
  delete step.pins;
  delete step.checkUnavailable;
  step.status = "Queued";
  mission.phase = "Queued";
  mission.planningRequested = false;
  delete mission.error;
}

function resetForFreshSource(step, reason) {
  if (step.candidate) {
    step.history ??= [];
    step.history.push({ runId: step.candidate.id, summary: step.candidate.proposal?.summary,
      evidence: step.candidate.evidence, at: now(), reason });
  }
  delete step.candidate;
  delete step.pins;
  delete step.checkUnavailable;
  delete step.checkRefreshes;
  step.status = "Queued";
}

function requeueMissingSources(rows) {
  const legacy = loadLegacyWorkflows();
  for (const mission of rows) {
    if (!["Blocked", "Deferred"].includes(mission.phase) || mission.elapsedMs >= mission.settings.maxMinutes * 60000) continue;
    const step = missingSourceStep(mission);
    if (!step) continue;
    try {
      update(mission.id, "source-recovery-queued", mission.error, (draft, current) => {
        assertFollowUpCurrent(draft, current, legacy);
        assertUniqueParity(draft, current, legacy);
        queueSourceRecovery(draft, draft.steps.find(row => row.id === step.id));
      }, step.id, mission.revision);
    } catch (error) { if (error.status !== 409) throw error; }
  }
}

let nextSourceScan = 0;
async function requeueChangedSources(rows) {
  const waiting = rows.filter(row => ["Blocked", "Deferred"].includes(row.phase) && row.waitingForSource
    && row.settings.publication === "automatic" && (canRetry(row) ||
      (row.waitingForSource.recheckCandidate && row.elapsedMs < row.settings.maxMinutes * 60_000)));
  if (!waiting.length || Date.now() < nextSourceScan) return;
  nextSourceScan = Date.now() + 15_000;
  const revision = waiting.some(row => row.waitingForSource.revision) ? await sharedSourceRevision() : undefined;
  const versions = new Map();
  for (const mission of waiting) {
    const wait = mission.waitingForSource;
    const changed = wait.revision ? wait.revision !== revision : wait.pins.some(pin => {
      if (!versions.has(pin.path)) versions.set(pin.path, sourceVersion(pin.path));
      return versions.get(pin.path) !== pin.version;
    });
    if (!changed) continue;
    try {
      update(mission.id, "source-change-queued", mission.error, (draft, current) => {
        const legacy = loadLegacyWorkflows();
        assertFollowUpCurrent(draft, current, legacy);
        assertUniqueParity(draft, current, legacy);
        const step = draft.steps.find(row => row.id === wait.step);
        if (wait.recheckCandidate && step.candidate && step.pins?.every(pin => !pinChanged(pin))) {
          delete step.candidate.compile;
          delete step.checkRefreshes;
          draft.phase = "Queued"; draft.planningRequested = false; delete draft.error;
        } else if (!canRetry(draft)) {
          draft.error = "Candidate sources changed while native validation was waiting. Renew the attempt budget to generate a fresh candidate.";
        } else if (step.sourceRecovery) queueSourceRecovery(draft, step);
        else {
          resetForFreshSource(step, draft.error);
          draft.phase = "Queued"; draft.planningRequested = false; delete draft.error;
        }
        delete draft.waitingForSource;
      }, wait.step, mission.revision);
    } catch (error) { if (error.status !== 409) throw error; }
  }
}

function queueLegacySourceRecovery(requests) {
  if (!Array.isArray(requests) || !requests.length || requests.length > 100 ||
      new Set(requests.map(row => row?.id)).size !== requests.length) throw new Error("Select distinct legacy missions to recover.");
  const recovered = mutateWorkflows(rows => {
    const legacy = loadLegacyWorkflows();
    const result = [];
    for (const request of requests) {
      const previous = find(legacy, request.id);
      if (previous.revision !== request.revision) fail("Legacy mission changed. Refresh recovery selection.");
      const existing = rows.find(row => row.followUpOf?.id === previous.id);
      if (existing) { result.push(existing.id); continue; }
      const step = missingSourceStep(previous);
      if (previous.phase !== "Blocked" || previous.steps.length !== 1 || !step ||
          previous.elapsedMs >= previous.settings.maxMinutes * 60000) fail("Legacy mission is outside automatic source recovery: " + previous.id);
      makeWorkflowRoom(rows, { reserve: reserveForRecovery });
      const mission = structuredClone(previous);
      mission.id = randomUUID();
      mission.revision = 0;
      mission.events = [];
      delete mission.lease;
      delete mission.stopping;
      mission.followUpOf = { id: previous.id, revision: previous.revision, phase: previous.phase,
        recordStorageVersion: 1, reason: "Automatic recovery of missing source context" };
      assertFollowUpCurrent(mission, rows, legacy);
      assertUniqueParity(mission, rows, legacy);
      queueSourceRecovery(mission, mission.steps[0]);
      workflowEvent(mission, "source-recovery-queued", previous.error, step.id);
      rows.push(mission);
      result.push(mission.id);
    }
    return result;
  });
  return { recovered };
}

function recoverAutomaticMission(rows) {
  const lane = agentLane();
  if (lane?.alive) return;
  const mission = rows.find(row => runningPhases.has(row.phase)
    && row.settings.publication === "automatic" && (!lane || lane.id === row.lease));
  if (!mission) {
    if (lane) recoverAgentLane(lane.id);
    return;
  }
  mutateWorkflows(current => {
    const row = find(current, mission.id);
    if (row.revision !== mission.revision) return;
    const currentLane = agentLane();
    if (currentLane && (currentLane.alive || currentLane.id !== row.lease)) return;
    if (currentLane) recoverAgentLane(currentLane.id);
    row.planningRequested = row.phase === "Planning";
    const step = row.steps.find(entry => entry.status === "Discovering sources");
    if (step?.sourceRecovery && canRetry(row)) step.sourceRecovery.used = false;
    row.phase = row.stopping ?? "Queued";
    delete row.stopping;
    workflowEvent(row, "recovered", "Interrupted automatic mission recovered");
  });
}

function resumeAutomaticReview(rows) {
  const mission = rows.find(row => row.phase === "Review" && row.settings.publication === "automatic");
  if (mission) {
    try { enqueue(mission, false); } catch (error) { if (error.status !== 409) throw error; }
  }
}

function deferOldBlocks(rows) {
  const blocked = rows.filter(row => row.phase === "Blocked" && row.settings.publication === "automatic");
  if (!blocked.length) return;
  mutateWorkflows(current => {
    for (const previous of blocked) {
      const row = current.find(entry => entry.id === previous.id && entry.revision === previous.revision);
      if (!row) continue;
      row.phase = "Deferred";
      workflowEvent(row, "deferred", row.error ?? "No publishable result");
    }
  });
}

// Rows in the swarm's generated matrix (rule-scan chores, performance targets) belong to the swarm supervisor,
// which records its own result on them. A mission for one only repeats that work and spends provider quota,
// so the board does not feed them (autonomyCandidates) and closes the missions it fed earlier.
function retireGeneratedRowMissions(ledger, rows) {
  const generated = new Set(ledger.rows.filter(row => row.generated).map(row => row.id));
  const stale = rows.filter(row => row.autonomousRevision && generated.has(row.parityId)
    && ["Queued", "Deferred", "Blocked"].includes(row.phase) && !row.stopping);
  if (!stale.length) return;
  mutateWorkflows(current => {
    for (const previous of stale) {
      const row = current.find(entry => entry.id === previous.id && entry.revision === previous.revision);
      if (!row) continue;
      row.phase = "Cancelled";
      row.error = "The swarm owns this generated row and records its own result. No mission is needed.";
      delete row.retryAt;
      delete row.waitingForSource;
      workflowEvent(row, "generated-row-retired", row.error);
    }
  });
}

function recoverHistoricalAutonomy(rows) {
  const mission = rows.find(row => ["Deferred", "Blocked"].includes(row.phase) && historicalRecovery(row));
  if (!mission) return;
  try {
    update(mission.id, "automatic-recovery-scheduled", mission.error, (draft, current) => {
      const legacy = loadLegacyWorkflows();
      assertFollowUpCurrent(draft, current, legacy);
      assertUniqueParity(draft, current, legacy);
      if (notesOnlyCandidate(draft) || reportedAuditGate(draft)) {
        draft.error = notesOnlyCandidate(draft)
          ? "Missing source context: the previous proposal only changed documentation. Trace the playable implementation owner."
          : "A prior audit is recorded as Reported. Resolve its concrete source and specification findings. Require sign-off only if the original objective or owning policy requires it.";
        queueSourceRecovery(draft, draft.steps.find(step => !terminalSteps.has(step.status)));
        return;
      }
      const quota = providerQuota.test(draft.error ?? "");
      // A Codex-pinned mission with no model choice moves to "auto" (Claude first) on a Codex limit instead of
      // waiting for the reset: 2026-11-21 the whole queue sat a day behind a Codex limit while Claude was free.
      const useAlternate = quota && draft.settings.provider === "codex" && !draft.settings.model;
      if (useAlternate) draft.settings.provider = "auto";
      else if (quota) draft.retryAt = quotaRetryAt(draft.error, Date.parse(draft.updatedAt));
      draft.phase = "Queued";
    }, undefined, mission.revision);
  } catch (error) { if (error.status !== 409) throw error; }
}

function notesOnlyCandidate(mission) {
  const step = mission.steps.find(row => !terminalSteps.has(row.status));
  const files = step?.candidate?.evidence?.files;
  return Boolean(mission.parityId && step?.mode === "refine" && !step.targets?.length
    && /^Compile validation supports/.test(mission.error ?? "") && files?.length
    && files.every(file => /^docs\/.*\.md$/i.test(file.path)));
}

function reportedAuditGate(mission) {
  const step = mission.steps.find(row => !terminalSteps.has(row.status));
  return Boolean(mission.parityId && step?.mode === "refine" && !step.targets?.length
    && mission.events.some(event => event.kind === "plan-ready")
    && mission.steps.some(row => row.mode === "audit" && row.status === "Reported")
    && !/\b(?:human review|manual review|explicit approval|explicit acceptance|sign[- ]off)\b/i.test(mission.objective)
    && /^Blocked prerequisite:.*\b(?:explicit(?:ly)? accept(?:ance|ed)?|accepted|sign[- ]off)\b/i.test(mission.error ?? ""));
}

function historicalRecovery(row) {
  return row.settings.publication === "automatic" && canRetry(row) && !row.waitingForSource
    && !row.events.some(event => event.kind === "automatic-recovery-scheduled")
    && ((row.planningRequested && !row.steps.length && invalidPlanScope.test(row.error ?? ""))
      || providerQuota.test(row.error ?? "") || notesOnlyCandidate(row) || reportedAuditGate(row));
}

function reserveForRecovery(mission) {
  return historicalRecovery(mission) || (canRetry(mission) && Boolean(missingSourceStep(mission)));
}

// Started only by the serving process, never by imports, discovery or static checks.
export function startWorkflowScheduler(logs = () => [], shouldStart = () => true) {
  if (scheduler) return;
  let scanning = false;
  scheduler = setInterval(async () => {
    if (!shouldStart()) return;
    let rows;
    try {
      rows = loadWorkflows();
      const saved = rows.filter(row => row.steps.some(step => step.status === "Saved"));
      if (saved.length) {
        let ledger = loadLedger();
        for (const mission of saved) {
          if (syncMissionParity(mission, ledger)) ledger = loadLedger();
        }
      }
    } catch (error) {
      if (error.status !== 409) process.stderr.write("Mission parity sync: " + error.message + "\n");
    }
    if (active || scanning || !rows) return;
    scanning = true;
    try {
      recoverAutomaticMission(rows);
      resumeAutomaticReview(rows);
      requeueMissingSources(rows);
      deferOldBlocks(rows);
      recoverHistoricalAutonomy(loadWorkflows());
      if (!agentLane()) await requeueChangedSources(rows);
      if (!shouldStart()) return;
      const ledger = loadLedger();
      const work = parityWorkProgress();
      retireGeneratedRowMissions(ledger, loadWorkflows(true));
      feedAutonomousBoard(ledger, loadWorkflows(true), work);
      let queue = loadWorkflows(true);
      for (const mission of queue.filter(row => row.phase === "Queued" && row.autonomousRevision)) {
        const row = ledger.rows.find(entry => entry.id === mission.parityId);
        if (row && !["verified", "parked"].includes(row.status) && mission.autonomousRevision === parityTaskRevision(row)) continue;
        update(mission.id, "parity-contract-changed", "Current parity scope or delivery status changed", draft => {
          draft.phase = "Deferred";
          draft.error = "Parity contract changed or closed. Retained the previous plan and its evidence.";
        }, undefined, mission.revision);
      }
      queue = loadWorkflows(true);
      const mission = nextQueuedMission(queue, ledger, work);
      if (!mission) return;
      const lane = agentLane();
      let reason = lane ? (lane.alive ? "Waiting for " + lane.label : "An interrupted AI worker needs lane recovery") : undefined;
      if (!reason) {
        try { diskAdmission(); checkBuildLane(); } catch (error) { reason = error.message; }
      }
      if (reason) {
        if (mission.error !== reason) update(mission.id, "waiting", reason, draft => { draft.error = reason; }, undefined, mission.revision);
        return;
      }
      try { begin(mission, mission.planningRequested === true, logs()); }
      catch (error) {
        // A competing server may claim the lane between observation and admission.
        if (!agentLane()) update(mission.id, "admission-deferred", error.message, draft => {
          draft.phase = draft.settings.publication === "automatic" ? "Deferred" : "Blocked";
          draft.error = error.message;
        }, undefined, mission.revision);
      }
    } catch (error) {
      if (error.status !== 409) process.stderr.write("Mission scheduler: " + error.message + "\n");
    } finally { scanning = false; }
  }, 2500);
  scheduler.unref();
}

export function workflowCommand(input, logs = [], intake) {
  if (!input || typeof input !== "object") throw new Error("Expected a mission command.");
  if (input.action === "autonomy") {
    if (typeof input.enabled !== "boolean") throw new Error("Specify whether autonomous board work is enabled.");
    saveAutonomy({ ...autonomyPolicy(), enabled: input.enabled });
    return workflowStatus();
  }
  if (input.action === "recover-legacy-source") return queueLegacySourceRecovery(input.missions);
  if (input.action === "recover-lane") {
    mutateWorkflows(() => recoverAgentLane());
    return workflowStatus();
  }
  if (["create", "create-parity", "create-improvement"].includes(input.action)) {
    const quickParity = input.action === "create-parity";
    const improving = input.action === "create-improvement";
    if (improving) input = toolingImprovement(input);
    if (quickParity) input = { ...input, ...parityPrototype(input.parityId, input.paths),
      settings: { maxAttempts: 3, ...input.settings } };
    let followUpOf;
    if (input.followUpOf !== undefined) {
      const currentHistory = loadWorkflows();
      const previous = find([...currentHistory, ...loadLegacyWorkflows()], input.followUpOf?.id);
      if (previous.revision !== input.followUpOf?.revision) fail("Original mission changed. Review its latest evidence before creating a follow-up.");
      if (!["Blocked", "Paused", "Complete", "Cancelled"].includes(previous.phase)) fail("Pause or resolve the original mission before creating its follow-up; review work may resume automatically.");
      if (!input.steps?.length) throw new Error("A recovery follow-up needs a concrete scoped plan, not a blind retry.");
      followUpOf = { id: previous.id, revision: previous.revision, phase: previous.phase, recordStorageVersion: currentHistory.some(row => row.id === previous.id) ? 2 : 1,
        reason: required(input.followUpOf.reason, "What changed to unblock the follow-up", 500) };
    }
    const mission = { id: randomUUID(), revision: 0, followUpOf, objective: required(input.objective, "Mission outcome", maxTaskLength),
      paths: cleanPaths(input.paths ?? [], true), settings: cleanSettings(input.settings ?? {}),
      steps: input.steps ? cleanSteps(input.steps) : [], phase: input.steps ? "Ready" : "Draft",
      attempts: 0, elapsedMs: 0, events: [], createdAt: now(), quickParity: quickParity || undefined,
      autonomousRevision: intake?.revision, previousParityMission: intake?.previousId,
      improvementVersion: improving ? input.improvementVersion : undefined };
    if (input.parityId !== undefined) {
      if (!loadLedger().rows.some(row => row.id === input.parityId)) throw new Error("Unknown parity feature.");
      mission.parityId = input.parityId;
    }
    if (mission.settings.publication === "automatic") {
      mission.phase = "Queued";
      mission.planningRequested = !mission.steps.length;
    }
    mutateWorkflows(rows => {
      const legacy = loadLegacyWorkflows();
      if (followUpOf && rows.some(row => row.followUpOf?.id === followUpOf.id && !["Complete", "Cancelled"].includes(row.phase))) {
        fail("This mission already has an unfinished follow-up. Review that work instead of duplicating it.");
      }
      assertFollowUpCurrent(mission, rows, legacy);
      if (mission.improvementVersion && rows.some(row => row.improvementVersion === mission.improvementVersion
          && row.paths[0] === mission.paths[0] && row.objective === mission.objective && row.phase !== "Cancelled")) {
        fail("This improvement is already recorded for the current source. Open its mission to review or resume it.");
      }
      assertUniqueParity(mission, rows, legacy);
      if (intake) {
        const [next] = autonomyCandidates(loadLedger(), rows, legacy, archivedWorkflows(), parityWorkProgress());
        if (!autonomyPolicy().enabled || rows.filter(row => row.autonomousRevision && scheduledParity(row)).length >= 3
            || next?.row.id !== mission.parityId || next.revision !== intake.revision || next.previous?.id !== intake.previousId) {
          fail("Parity intake changed before admission. The scheduler will refresh its priorities.");
        }
      }
      makeWorkflowRoom(rows, { followUpOf: followUpOf?.id, reserve: reserveForRecovery });
      workflowEvent(mission, "created", "Mission contract recorded");
      if (followUpOf) workflowEvent(mission, "follow-up", `Preserved ${followUpOf.id} at revision ${followUpOf.revision}: ${followUpOf.reason}`);
      rows.push(mission);
    });
    return workflowStatus(mission.id);
  }
  if (loadLegacyWorkflows().some(row => row.id === input.id)) fail("Legacy mission history is read-only here. Create a scoped follow-up; no original history or retained proposal was changed.");
  let mission = find(loadWorkflows(), input.id);
  if (mission.revision !== input.revision) fail("Mission changed. Refresh before retrying.");
  const local = active?.id === mission.id;
  if (["pause", "cancel"].includes(input.action) && runningPhases.has(mission.phase)) {
    const stop = input.action === "cancel" ? "Cancelled" : "Paused";
    update(mission.id, "stop-requested", stop, draft => { draft.stopping = stop; }, undefined, input.revision);
    if (local) {
      active.stop = stop;
      active.discovery?.abort();
      active.compile?.abort();
      if (active.runId) cancelAgent(active.runId);
    }
    return workflowStatus(mission.id);
  }
  if (local || (runningPhases.has(mission.phase) && input.action !== "recover")) fail("Pause the mission, or recover its interrupted worker first.");
  if (input.action === "archive") {
    if (mission.steps.some(step => step.status === "Saved")) syncMissionParity(mission, loadLedger());
    return archiveWorkflow(mission.id, input.revision);
  }
  if (input.action === "recover") {
    if (!runningPhases.has(mission.phase)) fail("Only an interrupted running mission can be recovered. Resume paused work instead.");
    // Serialize recovery with mission commands and never evict a live worker.
    mutateWorkflows(rows => {
      const row = find(rows, mission.id);
      if (row.revision !== input.revision) fail("Mission changed. Refresh before recovery.");
      const owner = agentLane();
      if (owner && owner.id !== row.lease) fail("A different mission owns the AI lane. Recover that mission first.");
      recoverAgentLane();
      row.phase = "Paused";
      workflowEvent(row, "recovered", "Interrupted state retained; publication will be reconciled before retrying");
    });
  } else if (input.action === "update") {
    if (input.steps && mission.steps.some(step => step.query || step.targets?.length) && input.sourceContextVersion !== 1) {
      throw new Error("This plan has focused source context. Refresh the editor, or include sourceContextVersion: 1 when editing the complete plan.");
    }
    const steps = input.steps ? cleanSteps(input.steps) : undefined;
    update(mission.id, "contract-updated", "Mission settings or pending plan updated", draft => {
      if (steps && draft.improvementVersion) throw new Error("The registered tool improvement scope is fixed.");
      if (steps && draft.steps.some(row => row.attempts || row.candidate)) throw new Error("The plan is frozen after execution begins. Retry a step or create a follow-up mission.");
      if (steps) { draft.steps = steps; draft.phase = "Ready"; }
      if (input.settings) {
        const settings = cleanSettings(input.settings);
        if (draft.improvementVersion && settings.checks !== "registered") throw new Error("Tool improvements require the registered regression check.");
        draft.settings = settings;
      }
    }, undefined, input.revision);
  } else if (["plan", "start", "resume"].includes(input.action)) {
    if (input.action === "plan" && mission.steps.length) throw new Error("Planning is available for an empty mission only.");
    if (input.action !== "plan" && !mission.steps.length) throw new Error("Create or enter the plan first.");
    if (["Complete", "Cancelled"].includes(mission.phase)) throw new Error("Create a follow-up mission for completed or cancelled work.");
    enqueue(mission, input.action === "plan");
  } else if (input.action === "requeue") {
    if (mission.settings.publication !== "automatic" || !["Paused", "Deferred", "Blocked"].includes(mission.phase)) {
      fail("Only an unfinished automatic mission can be requeued.");
    }
    const settings = cleanSettings({ ...mission.settings, ...input.settings, publication: "automatic" });
    if (mission.improvementVersion && settings.checks !== "registered") throw new Error("Tool improvements require the registered regression check.");
    if (mission.attempts >= settings.maxAttempts || mission.elapsedMs >= settings.maxMinutes * 60000) {
      throw new Error("The renewed budget must allow more work.");
    }
    update(mission.id, "automatic-work-requeued", "Resumed the original acceptance contract with a renewed budget", (draft, rows) => {
      const legacy = loadLegacyWorkflows();
      assertFollowUpCurrent(draft, rows, legacy);
      assertUniqueParity(draft, rows, legacy);
      const step = draft.steps.find(row => !terminalSteps.has(row.status));
      if (step && step.status !== "Publishing") {
        const error = draft.error;
        step.retryBase = step.attempts;
        step.retryFeedback = error?.slice(0, 500);
        const candidate = step.candidate;
        let keep = false;
        if (candidate?.proposal?.edits.length && !candidate.error && step.pins?.length
            && step.pins.every(pin => !pinChanged(pin))) {
          try {
            const evidence = verifyCandidate(candidate.proposal, candidate.autoSources, { protectTests: true });
            keep = ["Syntax checked", "Checks pending"].includes(evidence.status);
            if (keep) {
              candidate.evidence = evidence;
              if (!candidate.compile?.ok || (settings.checks === "build" && candidate.compile.mode !== "build")) delete candidate.compile;
            }
          } catch { /* Refresh proposals that no longer apply cleanly. */ }
        }
        if (!keep) {
          if (step.sourceRecovery) {
            step.sourceRecovery.priorLocations = step.pins?.map(pin => pin.path) ?? step.sourceRecovery.priorLocations;
            step.sourceRecovery.reason = step.retryFeedback ?? step.sourceRecovery.reason;
            step.sourceRecovery.used = false;
          }
          resetForFreshSource(step, error);
        }
      }
      draft.settings = settings;
      draft.phase = "Queued";
      draft.planningRequested = !draft.steps.length;
      delete draft.retryAt;
      delete draft.waitingForSource;
      delete draft.transientRetries;
      delete draft.error;
    }, undefined, input.revision);
  } else if (input.action === "retry") {
    update(mission.id, "retry-requested", required(input.note, "Retry reason", 500), draft => {
      const step = draft.steps.find(row => !terminalSteps.has(row.status));
      if (!step) throw new Error("There is no unfinished step.");
      if (step.status === "Publishing") throw new Error("Resume first to reconcile an interrupted publication.");
      if (step.candidate?.id && decisionBundles().some(row => row.runId === step.candidate.id)) throw new Error("This candidate already has a saved bundle. Resume to reconcile it.");
      if (step.candidate) {
        step.history ??= [];
        step.history.push({ runId: step.candidate.id, summary: step.candidate.proposal?.summary,
          evidence: step.candidate.evidence, at: now(), reason: input.note });
      }
      if (input.task) step.task = required(input.task, "Refined task", 1600);
      step.retryFeedback = input.note.trim();
      if (step.sourceRecovery) {
        step.sourceRecovery.reason = step.retryFeedback;
        step.sourceRecovery.used = false;
      }
      delete draft.waitingForSource;
      delete draft.error;
      delete step.candidate; delete step.pins; delete step.checkUnavailable; step.status = "Queued"; draft.phase = "Paused";
    }, undefined, input.revision);
  } else if (["approve", "accept"].includes(input.action)) {
    const step = mission.steps.find(row => !terminalSteps.has(row.status));
    if (!step?.candidate) throw new Error("Wait for a saved candidate to review.");
    const note = required(input.note, "Review evidence or remaining limitations", 1000);
    const lease = acquireAgentLane("Reviewing mission candidate");
    let reviewStarted = false;
    try {
      update(mission.id, "review-started", "Reviewing candidate against current sources", (draft, rows) => {
        const legacy = loadLegacyWorkflows();
        assertFollowUpCurrent(draft, rows, legacy);
        assertUniqueParity(draft, rows, legacy);
        draft.phase = "Running"; draft.lease = lease;
      }, step.id, input.revision);
      reviewStarted = true;
      if (input.action === "approve") publishStep(mission, step, true);
      else {
        if (step.candidate.proposal?.edits.length) throw new Error("Publish the candidate or retry it; accepting cannot discard proposed edits.");
        assertPins(step);
      }
      update(mission.id, "review-recorded", note, draft => {
        const row = draft.steps.find(row => row.id === step.id);
        if (input.action === "accept") row.status = "Accepted";
        row.review = { at: now(), note }; draft.phase = "Paused"; delete draft.error;
      }, step.id);
    } catch (error) {
      if (reviewStarted) update(mission.id, "review-blocked", error.message, draft => { draft.phase = "Review"; draft.error = error.message; });
      throw error;
    } finally { releaseAgentLane(lease); }
    mission = find(loadWorkflows(), mission.id);
    if (mission.steps.every(row => terminalSteps.has(row.status))) {
      update(mission.id, "complete", "All steps saved or accepted; gameplay verification remains separate", draft => { draft.phase = "Complete"; });
    } else if (mission.elapsedMs < mission.settings.maxMinutes * 60000) enqueue(mission, false);
  } else if (input.action === "pause" && mission.phase === "Queued") {
    update(mission.id, "paused", "Removed from automatic scheduling", draft => { draft.phase = "Paused"; }, undefined, input.revision);
  } else if (input.action === "cancel") {
    update(mission.id, "cancelled", "Stopped by user; saved bundles retained", draft => { draft.phase = "Cancelled"; }, undefined, input.revision);
  } else throw new Error("Unknown mission action.");
  return workflowStatus(mission.id);
}
