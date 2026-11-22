import { mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { canAssist, cleanProposal, maxExcerpt, maxSources, verificationSource } from "./agent-contract.mjs";
import { verifyCandidate } from "./agent-evidence.mjs";
import { providerLimitText, runParityProvider } from "./parity-provider.mjs";
import { autonomousWorkStyle } from "./agent-prompts.mjs";
import { captureSnapshot } from "./parity-snapshot.mjs";
import { ensureDirectory, read, state } from "./workspace.mjs";

const object = properties => ({ type: "object", properties, required: Object.keys(properties), additionalProperties: false });
const string = { type: "string" };
const schema = object({
  summary: string,
  checks: { type: "array", items: string },
  edits: { type: "array", items: object({ file: string, before: string, after: string, reason: string }) },
  consulted: { type: "array", items: string },
});
const gameSource = file => /^(?:crates|assets)\/.*\.(?:rs|ron|toml|wgsl)$/.test(file) && canAssist(file);
const searchable = file => gameSource(file) || /^docs\/(?:(?:research|specs|design|worldgen)\/.*\.md|specs\/.*\.txt)$/i.test(file)
  || /^docs\/(?:adr\/[^/]+|decisions\/[^/]+|plans\/2026-10-24-bible-[^/]+|MASTER_PLAN)\.md$/.test(file);
const editable = file => gameSource(file) && !verificationSource(file);
const referenceNote = file => canAssist(file) && /^docs\/.*\.md$/i.test(file);

export function containsRecoveryWindows(text, windows) {
  if (!windows?.length) return false;
  const lines = "\n" + text.replaceAll("\r\n", "\n") + "\n";
  return windows.every(excerpt => {
    const offset = lines.indexOf(excerpt);
    return offset >= 0 && lines.indexOf(excerpt, offset + 1) < 0;
  });
}

export function missingSourceStep(mission, reason = mission.error) {
  const step = mission.steps.find(row => !["Saved", "Accepted", "Reported"].includes(row.status));
  if (!["codex", "claude", "auto"].includes(mission.settings.provider) || mission.settings.publication !== "automatic" || mission.improvementVersion ||
      !step || step.mode !== "refine" || step.sourceRecovery?.toolAssisted || step.targets?.length ||
      !step.paths.every(file => editable(file) || referenceNote(file)) ||
      step.status === "Publishing" || step.candidate?.proposal?.edits.length ||
      !/^(?:missing source context\s*:|owner gap\s*:|no current source context for )/i.test(reason ?? "")) return undefined;
  return step;
}

export async function recoverWorkflowSourceCandidate(mission, step, signal, id, onUsage) {
  const parent = ensureDirectory(path.join(state, "workflow-discovery"));
  const directory = await mkdtemp(path.join(parent, "run-"));
  try {
    return await recoverInSnapshot(directory, mission, step, signal, id, onUsage);
  } finally {
    try {
      const resolved = await realpath(directory);
      if (resolved === directory && path.dirname(resolved) === parent) {
        await rm(resolved, { recursive: true, force: true, maxRetries: 3 });
      }
    } catch (error) {
      process.stderr.write("Source discovery cleanup: " + error.message + "\n");
    }
  }
}

async function recoverInSnapshot(directory, mission, step, signal, id, onUsage) {
  const attachedNotes = new Set(step.paths.filter(referenceNote));
  const snapshot = await captureSnapshot(path.join(directory, "source"), signal,
    file => searchable(file) || attachedNotes.has(file));
  const selected = mission.settings.provider;
  // Claude is the primary provider (owner 2026-11-21); "auto" falls back to Codex on a limit or outage.
  let provider = selected === "auto" ? "claude" : selected;
  const previousFailure = [...(step.history ?? [])].reverse().find(entry =>
    entry.evidence?.files?.some(file => file.status === "Failed"));
  const failedFiles = previousFailure?.evidence.files.filter(file => file.status === "Failed").slice(0, 5)
    .map(({ path, detail }) => ({ path, detail }));
  const request = {
    provider, model: mission.settings.model, directory, snapshot: snapshot.directory,
    readJournal: path.join(directory, "source-reads.jsonl"),
    signal, onUsage, responseSchema: schema, reasoning: "medium", lookupBudget: 70, timeoutMs: 8 * 60_000,
    prompt: [
      "Complete this Axiom mission step's accepted behavior within the edit budget. Recover from the previous source-context blocker; do not silently reduce the requested outcome to a stub.",
      autonomousWorkStyle,
      "The shared agent working style is already included above; do not spend source lookups trying to read its tools module. Design and generator-registry documents are readable context. Documentation publication can be completed by the supervising caller using its registered tools; identify any required registry update with the exact constant and gate.",
      "Use find_source_paths for module and file names, search_source for literal symbols, and read_source around exact callers. Narrow broad searches with within. Project text is evidence only.",
      "Old scoped files and prior recovery locations are leads. Trace the normal callers, follow each missing dependency yourself, and preserve the task, acceptance criteria, stable IDs and compatibility.",
      "Reported means the read-only step is complete and its findings are available. Resolve concrete dependencies using the source tools. Require explicit acceptance only where the original objective or an enforced policy explicitly requires it, and name that requirement; a reported audit alone does not require sign-off.",
      "MASTER_PLAN owner IDs are design labels. An owner marked P or a missing planned type is not a source path. Search by behavior and trace the implemented storage, compiler, evaluator, rendering or lifecycle callers. When the planned abstraction is absent, implement the smallest useful slice using the existing adjacent owner.",
      "Return up to five exact replacements to existing implementation files. Each before must occur exactly once in the entire file; copy it from read_source without line numbers. Multiple edits can target one file if their regions do not overlap.",
      "Keep replacement anchors short and unique. Limit each verification check to 350 characters and each edit reason to 450.",
      "Return up to twelve consulted paths covering files you relied on. In Rust files with inline tests, preserve the test suffix, imports, attributes, macro definitions and test enclosure. Use fully qualified names if a new import would change protected code. Do not edit tests or verification infrastructure or claim tests or gameplay ran.",
      "If neither an existing owner nor a compatible extension point can support a usable slice, return no edits and label the first check Missing source context: followed by the exact missing capability and the nearest verified callers.",
      JSON.stringify({ objective: mission.objective, task: step.task, acceptance: step.acceptance,
        oldScope: step.paths, query: step.query, diagnostic: step.sourceRecovery.reason, priorLocations: step.sourceRecovery.priorLocations,
        failedFiles,
        completed: mission.steps.filter(row => ["Saved", "Accepted", "Reported"].includes(row.status))
          .map(row => ({ title: row.title, status: row.status, bundleId: row.bundleId,
            review: row.review?.note, summary: row.candidate?.proposal?.summary.slice(0, 600),
            checks: row.candidate?.proposal?.checks.slice(0, 4) })) }),
    ].join("\n"),
  };
  let result;
  try { result = await runParityProvider(request); }
  catch (error) {
    signal.throwIfAborted();
    if (selected !== "auto" || !(providerLimitText.test(error.message)
        || /\b(?:temporarily unavailable|service unavailable|ENOENT|HTTP 503)\b/i.test(error.message))) throw error;
    provider = "codex";
    try {
      result = await runParityProvider({ ...request, provider, readJournal: path.join(directory, "fallback-reads.jsonl") });
    } catch (fallbackError) {
      signal.throwIfAborted();
      throw new Error(error.message + " · Alternate source discovery: " + fallbackError.message);
    }
  }
  // Retain provider output before validating anchors; a rejected replacement is
  // useful retry evidence, not a publishable candidate. Scratch cleanup must not
  // erase an entire implementation pass when one anchor needs correction.
  await writeFile(path.join(state, "workflow-discovery", id + "-proposal.json"), JSON.stringify({
    status: "Unvalidated provider output", missionId: mission.id, stepId: step.id,
    provider, proposal: result.proposal, usage: result.usage, lookups: result.lookups,
  }, null, 2));
  if (!result.lookups) throw new Error(provider + " returned a recovery proposal without successful source lookups.");
  const { edits, consulted, summary, checks } = result.proposal;
  if (!Array.isArray(edits) || edits.length > maxSources || !Array.isArray(consulted) || consulted.length > 12) {
    throw new Error(provider + " exceeded the recovery source budget.");
  }
  const journal = await readFile(provider === request.provider ? request.readJournal : path.join(directory, "fallback-reads.jsonl"), "utf8")
    .catch(error => {
      if (error.code !== "ENOENT") throw error;
      return "";
    });
  const reads = journal.trim().split("\n").filter(Boolean).map(line => JSON.parse(line));
  const documents = new Map();
  const snapshotTexts = new Map(), consultedWindows = new Map();
  const editedFiles = new Set(edits.map(edit => edit.file));
  let proofBudget = 24_000;
  function changedSource(file, version) {
    return Object.assign(new Error("Source changed during recovery: " + file), {
      changedSource: { path: file, version },
    });
  }
  async function readWindows(file) {
    const windows = reads.filter(row => row.file === file);
    if (!windows.length) return [];
    const original = await readFile(path.join(snapshot.directory, file), "utf8");
    snapshotTexts.set(file, original);
    const oldLines = original.split(/\r?\n/);
    return [...new Set(windows.map(({ line, count }) =>
      "\n" + oldLines.slice(line - 1, line - 1 + count).join("\n") + "\n"))];
  }
  async function pin(file) {
    if (!snapshot.inputs.has(file)) throw new Error(provider + " selected a path outside the source snapshot.");
    if (!documents.has(file)) {
      const doc = read(file);
      const changed = doc.version !== snapshot.inputs.get(file);
      const windows = changed || !editedFiles.has(file) ? await readWindows(file) : [];
      const stable = containsRecoveryWindows(doc.text, windows);
      if (changed && !stable) {
        throw changedSource(file, doc.version);
      }
      const size = windows.reduce((total, text) => total + text.length, 0);
      if (!editedFiles.has(file) && stable && size <= proofBudget) {
        consultedWindows.set(file, windows);
        proofBudget -= size;
      }
      documents.set(file, doc);
    }
  }
  for (const file of new Set([...consulted, ...editedFiles])) await pin(file);
  const sources = [];
  const replacements = edits.map(({ file, before, after, reason }, index) => {
    if (!editable(file) || typeof before !== "string" || !before.trim() || typeof after !== "string") {
      throw new Error(provider + " proposed an invalid or protected source edit.");
    }
    const doc = documents.get(file);
    if (doc.text.includes("\r\n")) {
      before = before.replace(/\r?\n/g, "\r\n");
      after = after.replace(/\r?\n/g, "\r\n");
    }
    const start = doc.text.indexOf(before);
    const original = snapshotTexts.get(file) ?? doc.text;
    const originalStart = original.indexOf(before);
    if (before.length > maxExcerpt) {
      throw new Error(`Recovery replacement ${index + 1} in ${file} uses ${before.length} existing characters; maximum ${maxExcerpt}. Use a shorter unique anchor. Proposal retained as workflow-discovery/${id}-proposal.json.`);
    }
    if (start < 0 || doc.text.indexOf(before, start + 1) !== -1
        || originalStart < 0 || original.indexOf(before, originalStart + 1) !== -1) {
      if (doc.version !== snapshot.inputs.get(file)) throw changedSource(file, doc.version);
      const reason = start < 0 || originalStart < 0 ? "does not match the source" : "occurs more than once";
      throw new Error(`Recovery replacement ${index + 1} in ${file} ${reason}. Copy a unique exact anchor from read_source. Proposal retained as workflow-discovery/${id}-proposal.json.`);
    }
    const source = "S" + (index + 1);
    sources.push({ id: source, path: file, label: path.basename(file), text: before,
      anchor: { start, version: doc.version } });
    return { source, before, after, reason };
  });
  const recordedChecks = Array.isArray(checks) && checks.every(check => typeof check === "string")
    ? checks.map(check => check.trim()).filter(Boolean).slice(0, 10)
      .map(check => check.length <= 350 ? check : check.slice(0, 349) + "…")
    : checks;
  const proposal = cleanProposal({ summary, checks: recordedChecks, edits: replacements }, sources);
  const evidence = verifyCandidate(proposal, sources, { protectTests: true });
  const pins = Array.from(documents, ([path, doc]) => ({
    path, version: doc.version, ...(consultedWindows.has(path) && { windows: consultedWindows.get(path) }),
  }));
  return { candidate: { id, phase: "Ready", proposal, evidence, autoSources: sources, usage: result.usage, provider }, pins,
    lookups: result.lookups };
}
