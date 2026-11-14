import { checkedValidator, nativeSourcePaths, binaryName, validateNativeContent } from "./native-content-validation.mjs";
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { workshopPath, workshopSources, fields, capabilityReport, firstCampaign, reviseProcesses } from "./workshop-process.mjs";
import { fabricationRows } from "./fabrication-data.mjs";
import { resourceCatalog, processSummary, selectProcess, productionPlan, planningScope } from "./workshop-planning.mjs";
import { stageContent } from "./workshop-stage.mjs";

export const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const defaultRoot = fileURLToPath(new URL("../../", import.meta.url));
const evidence = () => ({ authoring: "native-schema-checked", correctness: "runtime-execution-pending",
  performance: "not-measured", craft: "needs-playtest", playable: false, promotion: "not-authorized" });

export function workshopService(directory = defaultRoot) {
  const root = realpathSync(directory);
  function snapshot() {
    checkedValidator(root, workshopPath);
    const dependencies = [...new Set([...workshopSources, ...nativeSourcePaths(root),
      "target/axiom-dev/native-validation/receipt.json",
      "target/axiom-dev/native-validation/" + binaryName("check_fabrication")])];
    const files = dependencies.map(relative => {
      const file = path.join(root, relative), stat = lstatSync(file);
      if (!stat.isFile() || stat.isSymbolicLink() || realpathSync(file) !== file || stat.size > (relative.endsWith(binaryName("check_fabrication")) ? 32 : 4) * 1024 * 1024) {
        throw new Error("Workshop dependency is redirected or exceeds its limit: " + relative);
      }
      const bytes = readFileSync(file);
      return { path: relative, revision: digest(bytes), ...(relative === workshopPath ? { text: bytes.toString("utf8") } : {}) };
    });
    const revisions = files.map(({ path, revision }) => ({ path, revision }));
    return { version: digest(JSON.stringify(revisions)), dependencies: revisions, text: files.find(row => row.path === workshopPath).text,
      resources: resourceCatalog() };
  }
  function preview(request) {
    fields(request, ["version", "requirements", "operations", "approval"], ["version", "requirements", "operations"]);
    const source = snapshot();
    if (request.version !== source.version) throw new Error("Workshop source or runtime dependencies changed. Inspect and preview again.");
    const capabilities = capabilityReport(request.requirements);
    if (capabilities.gaps.length) return { status: "authority-required", version: source.version, capabilities, evidence: evidence() };
    const candidate = reviseProcesses(source.text, request.operations);
    validateNativeContent(root, workshopPath, candidate.text);
    const approval = candidate.changed ? digest(JSON.stringify({ version: source.version, requirements: request.requirements,
      operations: request.operations, text: candidate.text })) : null;
    return { status: candidate.changed ? "ready-to-stage-content" : "no-change", version: source.version, capabilities,
      changes: candidate.changes, production: candidate.changes.map(change => ({ id: change.id,
        before: change.before ? processSummary(change.before, source.resources) : null,
        after: processSummary(change.after, source.resources) })), planningScope, units: candidate.units, approval, evidence: evidence(),
      dependencies: source.dependencies, baseline: source.text, candidate: candidate.text };
  }
  return {
    snapshot,
    inspect() {
      const source = snapshot();
      return { path: workshopPath, version: source.version, dependencies: source.dependencies,
        capabilities: capabilityReport(), firstCampaign, evidence: evidence(),
        resources: source.resources, recipes: fabricationRows(source.text).map(row => processSummary(row, source.resources)),
        planningScope, operations: ["derive-process {sourceId, name}", "set-process {id, field, value}",
          "tune-process {id, changes: {name?, secondsPerBatch?, powerKw?, inputs?: [[resourceId, items]], output?: [resourceId, items]}}"],
        activation: "Staging writes content snapshots only. Native build, disposable world integration, and playtest remain pending." };
    },
    plan(request) {
      fields(request, ["process", "itemsPerMinute"]);
      const source = snapshot();
      return { version: source.version, ...productionPlan(selectProcess(fabricationRows(source.text), request.process), request.itemsPerMinute, source.resources) };
    },
    design(request) {
      fields(request, ["process", "changes", "copyName"], ["process", "changes"]);
      const source = snapshot(), rows = fabricationRows(source.text), row = selectProcess(rows, request.process);
      const copy = Object.hasOwn(request, "copyName");
      const operations = copy ? [{ op: "derive-process", sourceId: row.id, name: request.copyName }] : [];
      operations.push({ op: "tune-process", id: copy ? rows.length : row.id, changes: request.changes });
      const command = { version: source.version, requirements: ["fabricator-process", "electric-power"], operations };
      return { request: command, ...preview(command) };
    },
    preview,
    stage(request) {
      const plan = preview(request);
      if (plan.status !== "ready-to-stage-content" || !plan.approval || request.approval !== plan.approval) {
        throw new Error("Stage requires the exact supported preview approval.");
      }
      return stageContent(root, plan, snapshot);
    },
  };
}
