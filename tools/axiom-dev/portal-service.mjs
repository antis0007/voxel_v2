import { createHash } from "node:crypto";
import { read, root } from "./workspace.mjs";
import { publishBundle, inspectBundle, revertBundle, recentBundles } from "./agent-bundles.mjs";
import { checkPortalLayout } from "./portal-validator.mjs";
import { applyPortalOperations, encodePortalLayout, portalPath, portalOperations } from "./portal-operations.mjs";

const activation = "Rebuild and relaunch Axiom and all multiplayer peers. Layouts apply to every matching world type and seed. Openings do not carve terrain; blocked exits remain blocked.";
function fields(request, allowed) {
  if (!request || typeof request !== "object" || Array.isArray(request) || Object.keys(request).some(key => !allowed.includes(key))) throw new Error("Unexpected portal command fields.");
}
function source() {
  const doc = read(portalPath);
  return { doc, ...checkPortalLayout(root, doc.text) };
}
function preview(request) {
  fields(request, ["version", "operations", "approval"]);
  const { doc, graph: before } = source();
  if (request.version !== doc.version) throw new Error("Portal layout changed on disk. Reload and review your changes again.");
  const after = applyPortalOperations(before, request.operations);
  const changed = JSON.stringify(before) !== JSON.stringify(after);
  const text = changed ? encodePortalLayout(after) : doc.text;
  const checked = checkPortalLayout(root, text);
  const files = changed ? [{ path: portalPath, version: doc.version, text }] : [];
  const difference = (a, b) => ({
    added: b.filter(row => !a.some(old => old.id === row.id)).map(row => row.id),
    removed: a.filter(row => !b.some(next => next.id === row.id)).map(row => row.id),
    changed: b.filter(row => a.some(old => old.id === row.id && JSON.stringify(old) !== JSON.stringify(row))).map(row => row.id),
  });
  return { path: portalPath, version: doc.version, graph: checked.graph, warnings: checked.warnings, activation, changed,
    changes: { openings: difference(before.nodes, after.nodes), links: difference(before.links, after.links) },
    approval: changed ? createHash("sha256").update(JSON.stringify(files)).digest("hex") : null, files };
}
export function portalCommand(command, request = {}) {
  if (command === "help") return { operations: portalOperations, commands: ["inspect", "preview", "apply", "revert"], activation };
  if (command === "inspect") {
    fields(request, []);
    const { doc, graph, warnings } = source();
    const revisions = recentBundles().filter(bundle => bundle.files.length === 1 && bundle.files[0].path === portalPath)
      .map(bundle => ({ id: bundle.id, at: bundle.at }));
    return { path: portalPath, version: doc.version, graph, warnings, activation, revisions };
  }
  if (command === "preview") return preview(request);
  if (command === "apply") {
    const candidate = preview(request);
    if (!candidate.approval || candidate.approval !== request.approval) throw new Error("Preview the exact portal revision before publishing.");
    const bundle = publishBundle(candidate.files);
    return { bundle: bundle.id, ...portalCommand("inspect") };
  }
  if (command === "revert") {
    fields(request, ["bundle"]);
    const bundle = inspectBundle(request.bundle);
    if (bundle.files.length !== 1 || bundle.files[0].path !== portalPath) throw new Error("Choose a portal-layout revision.");
    revertBundle(request.bundle);
    return { reverted: request.bundle, ...portalCommand("inspect") };
  }
  throw new Error("Unknown portal command.");
}
