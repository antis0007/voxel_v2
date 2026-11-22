import { createHash, randomUUID } from "node:crypto";
import { createReadStream, existsSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, statfsSync, statSync, watch, writeFileSync } from "node:fs";
import path from "node:path";
import { createAgentLane, agentLane as legacyLane } from "./agent-lane.mjs";
import { inspectBundle } from "./agent-bundles.mjs";
import { editable, executeParity, hasAssertions, scenariosFor } from "./parity-execute.mjs";
import { loadScenarios } from "./factory-lab.mjs";
import { nativeCheckInput, sharedSourceRevision, sourceConflict } from "./parity-snapshot.mjs";
import { brief, loadLedger, writeStatus } from "./parity-data.mjs";
import { ensureDirectory, inside, root, state } from "./workspace.mjs";

const folder = path.join(state, "parity-work");
const { acquireAgentLane, agentLane, recoverAgentLane, releaseAgentLane } = createAgentLane("parity-lane.lock");
const nativeValidationLane = createAgentLane("published-validation-lane.lock").agentLane;
const tables = ["assets/processes.ron", "assets/fabrication.ron"];
const validId = /^[A-Z][A-Z0-9-]{0,47}$/;
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const copy = value => structuredClone(value);
const evidenceHashes = new Map();
let active, controller;
let refreshTimer;
let nextDiskCheck = 0;
const backoff = (count, base, max) => Math.min(max, base * 2 ** Math.min(count - 1, 8));

const partialNativeCoverage = new Set(["L026", "A09-01", "A12-03", "B-GT-04", "B-GT-06"]);

function verificationChange(row, receipt) {
  const evidence = [receipt];
  if (row.id === "L026" && ["open", "design", "building"].includes(row.status)) {
    return { status: "building", evidence,
      note: "Native Crusher and Gold Furnace checks passed. Enrichment and combiner stages remain unverified." };
  }
  return partialNativeCoverage.has(row.id) ? { evidence } : { status: "verified", evidence };
}

function recordFile(id) {
  if (typeof id !== "string" || !validId.test(id)) throw Error("Invalid parity feature ID.");
  return path.join(folder, id + ".json");
}
function save(job) {
  ensureDirectory(folder);
  const file = recordFile(job.id), temporary = file + "." + job.run + ".tmp";
  writeFileSync(temporary, JSON.stringify(job, null, 2));
  renameSync(temporary, file);
}
const stopFile = job => path.join(folder, job.run + ".stop");

function phase(job, value, message) {
  job.phase = value;
  job.message = message;
  save(job);
}
export function parityWorkStatus(id, lane = agentLane()) {
  if (active?.id === id) return copy(active);
  const file = recordFile(id);
  if (!existsSync(file)) return { id, phase: "Idle" };
  const result = JSON.parse(readFileSync(file, "utf8"));
  if (!result.running) return result;
  if (result.phase === "Queued") return { ...result, stop: existsSync(stopFile(result)) };
  const owner = lane?.id === result.lease ? lane : legacyLane();
  if (owner?.alive && owner.id === result.lease) return { ...result, stop: existsSync(stopFile(result)) };
  return { ...result, running: false, phase: "Interrupted", interruptedPhase: result.phase, stop: existsSync(stopFile(result)),
    message: "The worker stopped unexpectedly. Its snapshot and publication journal are available in the run directory." };
}

function recoverInterrupted(job) {
  if (job.phase !== "Interrupted") return;
  if (job.stop) {
    phase(job, "Stopped", "Recovered the recorded stop request.");
    rmSync(stopFile(job), { force: true });
    return;
  }
  if (job.receipt) return;
  if (job.snapshotRun && existsSync(path.join(folder, job.run, "checks.json"))) {
    job.receipt = path.relative(root, path.join(folder, job.run, "checks.json")).split(path.sep).join("/");
    phase(job, "Checks passed", "Recovered the saved checks. The worker will verify the receipt before recording it.");
    return;
  }
  if (job.preparedCommit || job.restartedFrom) {
    if (job.automatic) {
      job.retryable = true;
      job.recoveryAttempts = (job.recoveryAttempts ?? 0) + 1;
      job.retryAt = Date.now() + backoff(job.recoveryAttempts, 60_000, 15 * 60_000);
      phase(job, "Verification deferred", "The worker stopped again. A fresh verification will run after a delay.");
    }
    return;
  }
  if (!job.snapshotRun && !["Building", "Checking", "Snapshotting"].includes(job.interruptedPhase)) return;
  const { id, owner, line, provider, model, feedback, attempts, recordOnPass, reviewWithBoth, reasoning, refreshOf, refreshKey, automatic, recoveryAttempts } = job;
  save({ id, owner, line, provider, model, feedback, attempts, recordOnPass, reviewWithBoth, reasoning, refreshOf, refreshKey, automatic, recoveryAttempts,
    run: randomUUID(), restartedFrom: job.run, queuedAt: Date.now(), phase: "Queued", running: true, snapshotRun: true,
    usage: job.usage ?? { input: 0, output: 0, promptBytes: 0 }, providers: [], bundles: [],
    message: "Recovered the interrupted run. A fresh snapshot is queued." });
}

function parityDiskReady() {
  const enough = () => {
    const disk = statfsSync(folder);
    return disk.bavail * disk.bsize >= 1536 * 1024 * 1024;
  };
  if (enough()) return true;
  // Private, regenerable Cargo intermediates. Run only while holding the parity lane.
  const scratch = path.join(folder, "cargo-cache", "release-fast", "incremental");
  if (existsSync(scratch) && realpathSync(scratch) === scratch && inside(folder, scratch)) {
    rmSync(scratch, { recursive: true, force: true, maxRetries: 2 });
  }
  return enough();
}

export function parityWorkProgress() {
  if (!existsSync(folder)) return new Map();
  const lane = agentLane();
  return new Map(readdirSync(folder).filter(file => file.endsWith(".json") && validId.test(file.slice(0, -5)))
    .map(file => {
      const id = file.slice(0, -5);
      const { run, phase, attempt, buildRetries, stop, running, automatic, retryAt, retryable } = parityWorkStatus(id, lane);
      return [id, { run, phase, attempt, buildRetries, stop, running, automatic, retryAt, retryable }];
    }));
}

export function parityWorkChecks(id) {
  return scenariosFor(id);
}
function pin(files) {
  return files.map(file => sha(readFileSync(path.resolve(root, file))));
}
async function hashFile(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

async function evidenceHash(file) {
  const stamp = () => {
    const stat = statSync(file);
    return `${stat.size}:${stat.mtimeMs}:${stat.ctimeMs}`;
  };
  const before = stamp();
  const cached = evidenceHashes.get(file);
  if (cached?.stamp === before) return cached.hash;
  const hash = await hashFile(file);
  if (stamp() !== before) return undefined;
  if (evidenceHashes.size >= 16) evidenceHashes.delete(evidenceHashes.keys().next().value);
  evidenceHashes.set(file, { stamp: before, hash });
  return hash;
}

function receiptInputs(receipt, revisions, scenarios) {
  const configFiles = [...tables, ...scenarios.map(scenario => scenario.file)];
  const hashes = configFiles.map(file => existsSync(path.resolve(root, file)) ? pin([file])[0] : null);
  const inputs = receipt.inputs.map(({ file }) => {
    const current = statSync(file, { throwIfNoEntry: false });
    return [current?.mtimeMs, current?.size];
  });
  const current = receipt.revisions.matrix === revisions.matrix && receipt.revisions.owners === revisions.owners
    && Object.keys(receipt.scenarios).length === configFiles.length
    && configFiles.every((file, index) => hashes[index] !== null && receipt.scenarios[file] === hashes[index])
    && receipt.inputs.every(({ modified, size }, index) => inputs[index][0] === modified && inputs[index][1] === size);
  return { current, key: sha(JSON.stringify([revisions.matrix, revisions.owners, configFiles, hashes, inputs])) };
}

async function receiptState(entry, id, revisions, snapshotChecks = new Map()) {
  const file = path.resolve(root, entry);
  if (!inside(folder, file) || path.basename(file) !== "checks.json") return;
  const receipt = JSON.parse(readFileSync(file, "utf8"));
  if (receipt.id !== id || !inside(folder, receipt.executable.path)) return;
  const scenarios = parityWorkChecks(id);
  const passed = new Set(receipt.checks.filter(check => check.pass === true).map(check => check.id));
  if (!scenarios.length || receipt.checks.length !== scenarios.length
      || !scenarios.every(scenario => passed.has(scenario.id))) {
    return { current: false, coverageChanged: true, key: sha(JSON.stringify(scenarios)) };
  }
  const executable = statSync(receipt.executable.path, { throwIfNoEntry: false });
  const executableMatches = async () => !!executable && receipt.executable.sha256 === await evidenceHash(receipt.executable.path);
  if (receipt.snapshotInputs) {
    const scope = receipt.inputScope === "native" ? nativeCheckInput : undefined;
    const pin = sha(JSON.stringify([receipt.inputScope, receipt.snapshotInputs]));
    if (!snapshotChecks.has(pin)) snapshotChecks.set(pin, sourceConflict(receipt.snapshotInputs, scope));
    const conflict = await snapshotChecks.get(pin);
    const planning = receipt.revisions.matrix === revisions.matrix && receipt.revisions.owners === revisions.owners;
    const modified = conflict && statSync(path.join(root, conflict), { throwIfNoEntry: false })?.mtimeMs;
    return { current: planning && !conflict && await executableMatches(),
      key: sha(JSON.stringify([conflict, modified, executable?.mtimeMs, revisions.matrix, revisions.owners])) };
  }
  const { current, key } = receiptInputs(receipt, revisions, scenarios);
  return {
    current: current && await executableMatches(),
    key: sha(JSON.stringify([key, executable?.size, executable?.mtimeMs, executable?.ctimeMs])),
  };
}

function claimScenarios() {
  const indexed = new Map();
  for (const scenario of loadScenarios()) {
    for (const id of new Set((scenario.parity ?? []).map(tag => tag.toUpperCase()))) {
      if (!indexed.has(id)) indexed.set(id, []);
      indexed.get(id).push(scenario);
    }
  }
  for (const [id, scenarios] of indexed) {
    if (!scenarios.every(hasAssertions) || new Set(scenarios.map(scenario => scenario.id)).size !== scenarios.length) indexed.delete(id);
  }
  return indexed;
}

async function currentReceipt(row, revisions, snapshotChecks) {
  for (const entry of [...row.evidence].reverse()) {
    const proof = await receiptState(entry, row.id, revisions, snapshotChecks).catch(() => undefined);
    if (proof?.current) return entry;
  }
}

async function queueVerificationGap(ledger, registered, { only, nativeOnly, shouldStart }) {
  let sourceRevision;
  const attempts = nativeOnly ? 0 : 3;
  const snapshotChecks = new Map();
  for (const row of ledger.rows) {
    if (row.status !== "verified" || !registered.has(row.id) || (only && only !== row.id)) continue;
    const previous = parityWorkStatus(row.id);
    if (previous.running || previous.stop || previous.phase === "Stopped" || (previous.retryAt ?? 0) > Date.now()) continue;
    if (await currentReceipt(row, ledger.planningRevisions, snapshotChecks)) continue;
    sourceRevision ??= await sharedSourceRevision(nativeCheckInput);
    const sameInputs = previous.automatic && previous.refreshKey === sourceRevision;
    if (sameInputs && !previous.retryable && previous.attempts >= attempts && !previous.receipt) continue;
    if (!shouldStart() || agentLane()) return false;
    const receipt = row.evidence.findLast(entry => inside(folder, path.resolve(root, entry)) && path.basename(entry) === "checks.json");
    if (!queueParityWork({ id: row.id, recordOnPass: true, attempts }, {
      automatic: true, receipt, key: sourceRevision, previousRun: previous.run,
      recoveryAttempts: previous.retryable && !previous.receipt ? previous.recoveryAttempts : 0,
    })) continue;
    return true;
  }
  return false;
}

async function recordPendingReceipt(ledger, id, shouldStart) {
  const row = ledger.rows.find(row => row.id === id);
  if (!row) return false;
  const job = parityWorkStatus(id);
  if (!job.receipt || job.running || job.phase === "Stopped" || (!job.recordOnPass && !job.commit)
      || row.evidence.includes(job.receipt)) return false;
  const proof = await receiptState(job.receipt, id, ledger.planningRevisions).catch(() => undefined);
  if (!proof?.current || !shouldStart() || agentLane()) return false;
  const lease = acquireAgentLane("Publishing parity receipt " + id);
  try {
    if (parityWorkStatus(id).run !== job.run) return false;
    const evidence = [job.receipt, ...(job.commit ? ["commit:" + job.commit] : [])];
    const change = job.recordOnPass && !job.automatic ? verificationChange(row, job.receipt) : {};
    writeStatus(id, { ...change, evidence }, ledger.revision, new Set([id]));
    job.running = false;
    phase(job, (change.status ?? row.status) === "verified" ? "Verified" : "Checks passed",
      "The native receipt was recorded on the board.");
    return true;
  } finally { releaseAgentLane(lease); }
}

async function recordRelatedReceipts(job, directory, proof, results, only, signal) {
  const initial = loadLedger();
  const passed = new Map(results.filter(check => check.pass).map(check => [check.id, check]));
  const candidates = [];
  const snapshotChecks = new Map();
  for (const [id, scenarios] of claimScenarios()) {
    if (id === job.id || (only && only !== id)) continue;
    const checks = scenarios.map(scenario => passed.get(scenario.id));
    if (!checks.every((check, index) => check?.file === scenarios[index].file)) continue;
    const row = initial.rows.find(row => row.id === id);
    if (row?.status !== "verified") continue;
    const previous = parityWorkStatus(id);
    if (previous.running || previous.phase === "Stopped") continue;
    if (!await currentReceipt(row, initial.planningRevisions, snapshotChecks)) candidates.push({ id, checks });
  }
  if (!candidates.length) return;
  const conflict = await sourceConflict(proof.snapshotInputs, nativeCheckInput);
  signal.throwIfAborted();
  if (conflict) return conflict;
  for (const { id, checks } of candidates) {
    const previous = parityWorkStatus(id);
    if (previous.running || previous.phase === "Stopped") continue;
    try {
      const latest = loadLedger();
      if (latest.rows.find(row => row.id === id)?.status !== "verified") continue;
      if (latest.planningRevisions.matrix !== proof.revisions.matrix || latest.planningRevisions.owners !== proof.revisions.owners) break;
      const receipt = path.join(ensureDirectory(path.join(directory, "related", id)), "checks.json");
      const entry = path.relative(root, receipt).split(path.sep).join("/");
      writeFileSync(receipt, JSON.stringify({ ...proof, id, checks, regressions: [] }, null, 2), { flag: "wx" });
      writeStatus(id, { evidence: [entry] }, latest.revision, new Set([id]));
    } catch (error) {
      // The dedicated recovery pass can still run this row if its ledger write raced.
      console.error("Recording shared parity checks for " + id + ":", error.message);
    }
  }
}

let allowParityStart = () => true;

export function startParityRefreshScheduler({ intervalMs = 10000, only, nativeOnly = false, keepAlive = false, shouldStart = () => true } = {}) {
  if (refreshTimer) return;
  allowParityStart = shouldStart;
  let scanning = false;
  const refresh = async () => {
    if (scanning || active || !shouldStart()) return;
    scanning = true;
    try {
      const lane = agentLane();
      if (lane?.alive) return;
      if (lane) recoverAgentLane(lane.id);
      for (const [id, entry] of parityWorkProgress()) {
        if (entry.phase !== "Interrupted" || (only && only !== id)) continue;
        const job = parityWorkStatus(id);
        if (nativeOnly && job.attempts !== 0 && !job.stop) continue;
        // Serialize orphan recovery with normal job claims across panels/watchers.
        const lease = acquireAgentLane("Recovering parity " + id);
        try { recoverInterrupted(parityWorkStatus(id)); }
        finally { releaseAgentLane(lease); }
      }
      if (startQueued({ only, nativeOnly })) return;
      const ledger = loadLedger();
      const registered = claimScenarios();
      const ids = only ? [only] : [...parityWorkProgress().keys()];
      for (const id of ids) {
        if (await recordPendingReceipt(ledger, id, shouldStart)) return;
      }
      const autoQueued = ids.some(id => {
        const job = parityWorkStatus(id);
        return job.automatic && job.running && (!nativeOnly || job.attempts === 0);
      });
      if (!autoQueued && await queueVerificationGap(ledger, registered, { only, nativeOnly, shouldStart })) {
        startQueued({ only, nativeOnly });
        return;
      }
      for (const id of ids) {
        const row = ledger.rows.find(row => row.id === id);
        if (!row || (row.status === "verified" && registered.has(id))) continue;
        const previous = parityWorkStatus(id);
        if (previous.running || !previous.recordOnPass || previous.automatic || previous.phase === "Stopped") continue;
        const receipt = previous.receipt ?? previous.refreshOf;
        if (!receipt) continue;
        const recorded = row.evidence.includes(receipt);
        let proof;
        try { proof = await receiptState(receipt, row.id, ledger.planningRevisions); }
        catch (error) {
          if (error.code !== "ENOENT") {
            console.error("Cannot read parity receipt for " + id + ":", error.message);
            continue;
          }
          proof = { current: false, key: "missing:" + receipt };
        }
        if (!proof) continue;
        if (!shouldStart() || agentLane() || parityWorkStatus(row.id).run !== previous.run) return;
        if (proof.current || (!recorded && !previous.receipt)
            || (previous.refreshKey === proof.key && previous.phase !== "Interrupted")) continue;
        const attempts = nativeOnly ? 0 : previous.attempts ?? 0;
        queueParityWork({ id: row.id, recordOnPass: true, attempts, provider: previous.provider ?? "auto",
          model: previous.model ?? "", reviewWithBoth: attempts >= 2 && Boolean(previous.reviewWithBoth) },
        { receipt, key: proof.key });
        startQueued({ only, nativeOnly });
        return;
      }
    } catch (error) {
      console.error("Parity evidence refresh:", error.message);
    } finally {
      scanning = false;
    }
  };
  const tick = () => void refresh().catch(error => console.error("Parity evidence refresh:", error.message));
  const laneWatcher = watch(ensureDirectory(state), { persistent: false }, (_event, name) => {
    if (name === "parity-lane.lock") tick();
  });
  laneWatcher.on("error", error => console.error("Parity lane watch:", error.message));
  refreshTimer = setInterval(tick, intervalMs);
  tick();
  if (!keepAlive) refreshTimer.unref();
}

export async function parityEvidence(row, revisions, snapshotChecks = new Map()) {
  return Promise.all(row.evidence.map(async entry => {
    if (/^(?:https?:|commit:)/.test(entry)) return { entry, status: "Reference" };
    const file = path.resolve(root, entry.split("#")[0]);
    if (!inside(root, file) || !existsSync(file)) return { entry, status: "Missing" };
    try {
      if (path.dirname(file) === path.join(state, "candidate-bundles")) {
        const applied = inspectBundle(path.basename(file, ".json")).files.every(change => change.status === "Applied");
        return { entry, status: applied ? "Current source bundle" : "Historical · source changed" };
      }
      if (inside(folder, file) && path.basename(file) === "checks.json") {
        const receipt = await receiptState(entry, row.id, revisions, snapshotChecks);
        if (!receipt) return { entry, status: "Invalid native receipt" };
        if (receipt.current) return { entry, native: "current", status: "Current native checks" };
        return { entry, native: "historical", status: receipt.coverageChanged
          ? "Historical · registered checks changed" : "Historical · checked inputs or executable changed" };
      }
      return { entry, status: "Present · source revision unpinned" };
    } catch {
      return { entry, status: "Evidence unavailable" };
    }
  }));
}

export async function parityVerification(ledger, id) {
  const rows = ledger.rows.filter(row => id ? row.id === id : row.status === "verified");
  const registered = claimScenarios();
  const snapshotChecks = new Map();
  return Promise.all(rows.map(async row => {
    const entries = await parityEvidence(row, ledger.planningRevisions, snapshotChecks);
    const currentNative = entries.some(entry => entry.native === "current");
    const evidence = currentNative ? "Current native checks"
      : entries.some(entry => entry.native === "historical") ? "Historical native checks"
      : "Current native receipt unavailable";
    return { id: row.id, status: row.status, currentNative, registered: registered.has(row.id), evidence, entries };
  }));
}
function verifyPlanning(order, ledger) {
  const pinned = order.contract.planningRevisions;
  if (ledger.planningRevisions.matrix !== pinned.matrix || ledger.planningRevisions.owners !== pinned.owners) {
    throw Error("The feature specification changed during the run. Queue the current brief.");
  }
}

function startQueued({ only, nativeOnly = false } = {}) {
  // Completion can call this directly, between scheduler ticks. Honor
  // the same drain policy so a hot update can release the worker.
  if (!allowParityStart() || active || agentLane()) return false;
  const nativeBuildActive = nativeValidationLane()?.alive;
  const queued = [...parityWorkProgress().keys()].map(id => parityWorkStatus(id))
    .filter(job => job.phase === "Queued" && (!only || job.id === only) && (job.notBefore ?? 0) <= Date.now()
      && (!nativeBuildActive || job.stop)
      && (job.stop || Date.now() >= nextDiskCheck)
      && (!nativeOnly || job.attempts === 0 || job.stop))
    .sort((a, b) => queuePriority(a) - queuePriority(b) || a.queuedAt - b.queuedAt);
  for (const job of queued) {
    try { job.lease = acquireAgentLane("Factory parity " + job.id); }
    catch (error) { if (agentLane()) return false; throw error; }
    const current = parityWorkStatus(job.id);
    if (current.run !== job.run || current.phase !== "Queued") { releaseAgentLane(job.lease); continue; }
    try {
      if (current.stop) {
        job.running = false;
        phase(job, "Stopped", "Removed from the queue.");
        rmSync(stopFile(job), { force: true });
        releaseAgentLane(job.lease);
        continue;
      }
      if (!parityDiskReady()) {
        nextDiskCheck = Date.now() + 30000;
        const message = "Queued until there is enough disk space for an isolated build.";
        if (job.message !== message) phase(job, "Queued", message);
        releaseAgentLane(job.lease);
        return false;
      }
      const ledger = loadLedger();
      const row = ledger.rows.find(row => row.id === job.id);
      if (!row) throw Error("This feature was removed from the parity matrix.");
      if (job.automatic && row.status !== "verified") {
        job.running = false;
        phase(job, "Superseded", "The recorded delivery status changed before verification started.");
        releaseAgentLane(job.lease);
        continue;
      }
      const order = brief(row, ledger.systems, true);
      const dir = ensureDirectory(path.join(folder, job.run));
      job.snapshotRun = true;
      controller = new AbortController();
      if (existsSync(stopFile(job))) controller.abort();
      active = job;
      const stopTimer = setInterval(() => {
        if (existsSync(stopFile(job))) { job.stop = true; controller.abort(); }
      }, 400);
      const checkPlanning = () => {
        const current = loadLedger();
        verifyPlanning(order, current);
        if (job.automatic && current.rows.find(row => row.id === job.id)?.status !== "verified") {
          throw Error("The recorded delivery status changed during verification.");
        }
      };
      void executeParity(job, order, dir, { signal: controller.signal, save: () => save(job),
        phase: (value, message) => phase(job, value, message), checkPlanning,
        recordRelated: job.automatic ? (proof, results) => recordRelatedReceipts(job, dir, proof, results, only, controller.signal) : undefined,
        record: ({ evidenceOnly = false } = {}) => {
          const latest = loadLedger();
          verifyPlanning(order, latest);
          const evidence = [job.receipt, ...(job.commit ? ["commit:" + job.commit] : [])];
          const feature = latest.rows.find(row => row.id === job.id);
          const change = evidenceOnly || job.automatic ? { evidence } : { ...verificationChange(feature, job.receipt), evidence };
          writeStatus(job.id, change, latest.revision, new Set([job.id]));
          return change.status ?? (job.automatic ? feature.status : undefined);
        },
      }).catch(error => {
        if (job.automatic && !controller.signal.aborted) {
          job.retryable = ["ETIMEDOUT", "ENOSPC", "EBUSY", "EPIPE", "ENOENT"].includes(error.code)
            || /timed out|factory lab closed after|changed on disk|checkout changed|specification changed/i.test(error.message);
          return phase(job, "Verification deferred", error.message);
        }
        if (!controller.signal.aborted && job.attempts === 0 && error.code === "ETIMEDOUT"
            && (job.nativeTimeoutRetries ?? 0) < 2) {
          return phase(job, "Build delayed", error.message);
        }
        phase(job, controller.signal.aborted ? "Stopped" : "Needs input", error.message);
      })
        .finally(() => {
          clearInterval(stopTimer);
          try {
            job.running = false;
            if (job.automatic && !job.receipt && job.phase !== "Stopped" && !controller.signal.aborted && !existsSync(stopFile(job))) {
              if (job.phase === "Source changed") job.retryable = true;
              job.recoveryAttempts = (job.recoveryAttempts ?? 0) + 1;
              const delay = backoff(job.recoveryAttempts, 60_000, 15 * 60_000);
              job.retryAt = Date.now() + delay;
              job.message += job.retryable ? " Automatic retry in " + delay / 1000 + "s." : " Verification resumes when the checked inputs change.";
              save(job);
            } else if (["Source changed", "Build delayed"].includes(job.phase) && !controller.signal.aborted && !existsSync(stopFile(job))) {
              const { id, owner, line, provider, model, feedback, attempts, recordOnPass, refreshOf, refreshKey, reviewWithBoth } = job;
              const sourceChanged = job.phase === "Source changed";
              const sourceRetries = (job.sourceRetries ?? 0) + Number(sourceChanged);
              const nativeTimeoutRetries = (job.nativeTimeoutRetries ?? 0) + Number(!sourceChanged);
              const retryDelay = sourceChanged
                ? backoff(sourceRetries, 5000, 300_000)
                : backoff(nativeTimeoutRetries, 60_000, 300_000);
              save({ id, owner, line, provider, model, feedback, attempts, recordOnPass, refreshOf, refreshKey, reviewWithBoth,
                run: randomUUID(), phase: "Queued", running: true, snapshotRun: true,
                queuedAt: Date.now(), notBefore: Date.now() + retryDelay,
                sourceRetries, nativeTimeoutRetries, retriedFrom: job.run,
                providers: [], bundles: [], usage: job.usage,
                message: job.message + " A fresh snapshot is queued after a " + retryDelay / 1000 + "s delay." });
            } else save(job);
            rmSync(stopFile(job), { force: true });
          } finally {
            active = controller = undefined;
            releaseAgentLane(job.lease);
          }
          startQueued({ only, nativeOnly });
        }).catch(error => console.error("Parity worker:", error.message));
      return true;
    } catch (error) {
      job.running = false;
      phase(job, "Needs input", error.message);
      releaseAgentLane(job.lease);
    }
  }
  return false;
}

const queuePriority = job => job.automatic ? 1 : job.refreshOf ? 2 : 0;

export function startParityWork(options) {
  queueParityWork({ attempts: 3, ...options });
  startQueued();
  return parityWorkStatus(options?.id);
}

export function queueParityWork({ id, owner, line, provider = "auto", model = "", feedback = "", attempts = 0, recordOnPass = false,
  reviewWithBoth = false, reasoning = "medium" } = {}, refresh) {
  recordFile(id);
  if (!loadLedger().rows.some(row => row.id === id)) throw Error("Unknown parity feature ID.");
  if ((owner !== undefined && !editable(owner)) || typeof feedback !== "string" || feedback.length > 420 ||
      !["auto", "codex", "claude", "ollama", "lmstudio"].includes(provider) || typeof model !== "string" ||
      (model && (!/^[a-zA-Z0-9][\w.:/@-]{0,99}$/.test(model) || provider === "auto")) ||
      (["ollama", "lmstudio"].includes(provider) && !model) ||
      (line !== undefined && (!Number.isSafeInteger(line) || line < 1)) ||
      !Number.isInteger(attempts) || attempts < 0 || attempts > 3 || typeof recordOnPass !== "boolean" || typeof reviewWithBoth !== "boolean"
      || (reviewWithBoth && (provider !== "auto" || attempts < 2)) || !["low", "medium", "high"].includes(reasoning)) {
    throw Error("Choose a supported provider and up to three attempts. An explicit model needs a specific provider.");
  }
  // The panel and several CLI processes can submit while another row runs.
  // Serialize the check and write for this row without taking the worker lane.
  const submission = createAgentLane(`parity-submit-${id}.lock`);
  if (submission.agentLane()?.alive === false) submission.recoverAgentLane();
  const lease = submission.acquireAgentLane(`Queue parity ${id}`);
  try {
    const previous = parityWorkStatus(id);
    if (refresh?.automatic && (previous.run !== refresh.previousRun || previous.running || previous.stop || previous.phase === "Stopped")) return;
    if (previous.running) throw Error("This feature already has a queued or active run.");
    const job = { id, run: randomUUID(), phase: "Queued", snapshotRun: true, queuedAt: Date.now(), running: true, owner, line, provider, model,
    feedback, attempts, recordOnPass, reviewWithBoth, reasoning, providers: [], bundles: [], usage: { input: 0, output: 0, promptBytes: 0 },
      refreshOf: refresh?.receipt, refreshKey: refresh?.key, automatic: refresh?.automatic, recoveryAttempts: refresh?.recoveryAttempts };
    save(job);
    return job;
  } finally {
    submission.releaseAgentLane(lease);
  }
}

export function stopParityWork(id, run) {
  const job = parityWorkStatus(id);
  if (!job.running && job.phase !== "Interrupted") return job;
  if (run !== job.run) throw Error("This run was replaced. Refresh its status before stopping.");
  writeFileSync(stopFile(job), "");
  if (active?.run === job.run) {
    active.stop = true;
    controller.abort();
  }
  return parityWorkStatus(id);
}

export async function recordParityWork(id, revision) {
  const result = parityWorkStatus(id);
  if (result.running || result.phase !== "Checks passed" || !result.receipt) throw Error("Finish the tagged checks before recording verification.");
  const ledger = loadLedger();
  const proof = await receiptState(result.receipt, id, ledger.planningRevisions);
  if (!proof?.current || parityWorkStatus(id).run !== result.run) throw Error("The checked source or scenarios changed.");
  return writeStatus(id, { evidence: [result.receipt] }, revision, new Set([id]));
}
