import { catalogContracts } from "./catalog-contracts.mjs";
const nativeCheckers = new Set(catalogContracts.map(row => row.schema.split("/").slice(0, 2).join("/") + "/examples/" + row.validator.example + ".rs"));
const sourcePath = /^(?:assets|crates|docs|tools)\/(?:[\w .-]+\/)*[\w .-]+\.(?:ron|json|rs|wgsl|toml|md|mjs|css|html|txt)$/i;
export const canAssist = path => typeof path === "string" && sourcePath.test(path) &&
  !path.split("/").some(part => part.startsWith("."));
// String.length/HTML input lengths count UTF-16 code units, not UTF-8 bytes.
export const maxTaskLength = 16_000;
// Mission references JSON-encode the objective, acceptance and prior step summaries.
// Reserve worst-case six-character JSON escapes without trimming user instructions.
export const maxWorkflowReference = 6 * (maxTaskLength + 700 + 12 * (600 + 120)) + 4096;
export function assertTask(task, label = "Task") {
  if (typeof task !== "string" || !task.trim()) throw new Error(label + " needs a non-empty brief.");
  if (task.length > maxTaskLength) throw new Error(label + " allows at most " + maxTaskLength
    + " characters (received " + task.length + "). Shorten it or split it into missions; no text was submitted to the model.");
}
export const maxSources = 5;
export const maxExcerpt = 4800;
export const maxPackReference = 64_000;
export const verificationSource = file => nativeCheckers.has(file) || file === "tools/axiom-dev/mesh-budget-regression.rs"
  || /^tools\/axiom-dev\/(?:workflow(?:-store|-live-review|-source)?|agent-lane|parity-(?:work|provider|snapshot|source-tools)|factory-lab|scenarios\/[^/]+)\.(?:mjs|json)$/.test(file)
  || /^tools\/portal-native-check\//.test(file) || /^tools\/axiom-dev\/(?:agent-(?:evidence|checks|contract|decisions|runner|bundles)|verification-guard|workspace|server|catalog-contracts|fabrication-contract|native-content-validation|build-validators|portal-validator|build-portal-validator)\.mjs$/.test(file) || /(?:^|\/)(?:tests?|benches|fixtures)(?:\/|\.)|(?:[._-](?:tests?|spec|bench))\./i.test(file);
const maxChanges = 12;

const string = { type: "string" };
const object = properties => ({ type: "object", properties, required: Object.keys(properties), additionalProperties: false });
export const proposalSchema = object({
  summary: string,
  checks: { type: "array", items: string },
  edits: { type: "array", items: object({ source: string, before: string, after: string, reason: string }) },
});
export const repairAuditSchema = object({
  summary: string, source: string, quote: string, trigger: string, impact: string, check: string,
});

export function cleanRepairAudit(value, sources) {
  const limits = { summary: 1800, source: 8, quote: 1000, trigger: 450, impact: 450, check: 350 };
  for (const [field, limit] of Object.entries(limits)) {
    if (typeof value?.[field] !== "string" || value[field].length > limit) {
      throw new Error("Defect audit " + field + " must be a string of at most " + limit + " characters");
    }
  }
  if (!value.summary.trim()) throw new Error("Defect audit needs a summary");
  if (!value.source) return { summary: value.summary };
  const source = sources.find(row => row.id === value.source);
  if (!source || ![value.quote, value.trigger, value.impact, value.check].every(text => text.trim())) {
    throw new Error("Defect audit lacks an editable source, trigger, impact, or check");
  }
  const start = source.text.indexOf(value.quote);
  if (start < 0 || source.text.indexOf(value.quote, start + 1) >= 0) {
    throw new Error("Defect quote must occur exactly once in the editable excerpt");
  }
  return { ...value, start };
}

export function constrainRepair(candidate, audit, sources) {
  if (!candidate.edits.length) return;
  const source = sources.find(row => row.id === audit.source);
  const touchesFinding = candidate.edits.some(edit => {
    if (edit.source !== audit.source) return false;
    const offset = source.text.indexOf(edit.before);
    let prefix = 0, suffix = 0;
    const shared = Math.min(edit.before.length, edit.after.length);
    while (prefix < shared && edit.before[prefix] === edit.after[prefix]) prefix++;
    while (prefix + suffix < shared && edit.before.at(-1 - suffix) === edit.after.at(-1 - suffix)) suffix++;
    if (offset < 0 || edit.before === edit.after) return false;
    const start = offset + prefix;
    const end = offset + edit.before.length - suffix;
    const findingEnd = audit.start + audit.quote.length;
    return start === end ? start >= audit.start && start <= findingEnd
      : start < findingEnd && end > audit.start;
  });
  if (!touchesFinding || candidate.edits.length > 4) {
    throw new Error("Repair must touch the audited code and stay within four anchored edits");
  }
}

export function cleanProposal(proposal, sources) {
  const bounded = (text, max, empty = false) => typeof text === "string" && text.length <= max && (empty || text.trim());
  if (!bounded(proposal?.summary, 5_500)) throw new Error("Proposal summary must contain 1 to 5,500 characters");
  if (!Array.isArray(proposal.checks) || proposal.checks.length > 10 ||
      proposal.checks.some(check => !bounded(check, 350))) {
    throw new Error("Return at most ten verification checks, each with 1 to 350 characters");
  }
  if (!Array.isArray(proposal.edits) || proposal.edits.length > maxChanges) {
    throw new Error("Return an edits array with at most twelve entries");
  }
  const changes = new Map();
  for (const edit of proposal.edits) {
    const source = sources.find(row => row.id === edit?.source);
    if (!source || !bounded(edit.before, maxExcerpt) || !bounded(edit.after, maxExcerpt * 2, true) ||
        !bounded(edit.reason, 450) || edit.before === edit.after) throw new Error("Invalid AI edit");
    const start = source.text.indexOf(edit.before);
    if (start < 0 || source.text.indexOf(edit.before, start + 1) >= 0) {
      throw new Error(source.id + " · Edit anchor must occur exactly once in the attached excerpt");
    }
    const end = start + edit.before.length;
    const edits = changes.get(source.id) ?? [];
    if (edits.some(other => start < other.end && other.start < end)) throw new Error(source.id + " · Overlapping edits");
    edits.push({ source: source.id, before: edit.before, after: edit.after, reason: edit.reason, start, end });
    changes.set(source.id, edits);
  }
  return { summary: proposal.summary, checks: proposal.checks,
    edits: [...changes.values()].flat().map(({ source, before, after, reason }) => ({ source, before, after, reason })) };
}

export function prepareProposal(proposal, sources) {
  const clean = cleanProposal(proposal, sources);
  const files = new Map();
  for (const edit of clean.edits) {
    const source = sources.find(row => row.id === edit.source);
    const file = files.get(source.path) ?? { doc: source.doc, original: source.original, edits: [] };
    if (source.doc.text !== source.original || file.original !== source.original) {
      throw new Error("Draft changed · " + source.path + " · Run again on the current draft");
    }
    const start = source.start + source.text.indexOf(edit.before);
    const end = start + edit.before.length;
    if (file.edits.some(other => start < other.end && other.start < end)) throw new Error("Overlapping edits in " + source.path);
    file.edits.push({ start, end, after: edit.after });
    files.set(source.path, file);
  }
  return [...files.values()].map(file => {
    let text = file.original;
    let delta = 0;
    const locations = [...file.edits].sort((a, b) => a.start - b.start).map(edit => {
      const start = edit.start + delta;
      delta += edit.after.length - (edit.end - edit.start);
      return start;
    });
    for (const edit of file.edits.sort((a, b) => b.start - a.start)) {
      text = text.slice(0, edit.start) + edit.after + text.slice(edit.end);
    }
    return { doc: file.doc, text, locations };
  });
}
