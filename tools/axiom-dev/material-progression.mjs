import { createHash } from "node:crypto";
import { read, root } from "./workspace.mjs";
import { publishBundle, inspectBundle, revertBundle } from "./agent-bundles.mjs";
import { validateNativeContent, nativeSourceFingerprint } from "./native-content-validation.mjs";
import { materialRows, miningSchemas, harvestGates, compareHarvest, reviseMiningLevel, progressionScope } from "./material-progression-data.mjs";

export const progressionPath = "assets/tools/materials.ron";
export const progressionSources = [progressionPath, "assets/tools/parts.ron", "assets/mining.ron", "assets/materials.ron",
  "crates/app/src/world/mining.rs", "crates/tools/src/assembly.rs", "tools/axiom-dev/deposit-studio.mjs",
  "tools/axiom-dev/material-progression-data.mjs", "tools/axiom-dev/material-progression.mjs", "tools/axiom-dev/material-progression-cli.mjs"];
const hash = text => createHash("sha256").update(text).digest("hex");
function fields(value, allowed, required = []) {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !allowed.includes(key))
      || required.some(key => !Object.hasOwn(value, key))) throw new Error("Unexpected or missing material-progression fields.");
}
function snapshot(validate = true) {
  const docs = new Map(progressionSources.map(path => [path, read(path)]));
  const dependencies = [...docs].map(([path, doc]) => ({ path, version: doc.version }));
  const nativeSource = nativeSourceFingerprint(root);
  if (validate) {
    validateNativeContent(root, progressionPath, docs.get(progressionPath).text);
    validateNativeContent(root, "assets/tools/parts.ron", docs.get("assets/tools/parts.ron").text);
  }
  return { docs, dependencies, nativeSource, version: hash(JSON.stringify({ dependencies, nativeSource })),
    materials: materialRows(docs.get(progressionPath).text), schemas: miningSchemas(docs.get("assets/tools/parts.ron").text),
    gates: harvestGates(docs.get("assets/mining.ron").text, docs.get("assets/materials.ron").text) };
}
function choose(rows, id, kind) {
  const row = rows.find(row => row.id === id);
  if (!row) throw new Error("Choose an existing " + kind + " ID from inspect.");
  return row;
}
function preview(request) {
  fields(request, ["materialId", "schemaId", "version", "miningLevel", "approval"], ["materialId", "version", "miningLevel"]);
  const source = snapshot();
  if (request.version !== source.version) throw new Error("Material, harvest rules or tool implementation changed. Inspect and preview again.");
  const schema = choose(source.schemas, request.schemaId ?? "pickaxe", "tool schema");
  const doc = source.docs.get(progressionPath);
  const candidate = reviseMiningLevel(doc.text, request.materialId, request.miningLevel);
  validateNativeContent(root, progressionPath, candidate.text);
  const files = doc.text === candidate.text ? [] : [{ path: progressionPath, version: doc.version, text: candidate.text }];
  const normalized = { materialId: candidate.material.id, schemaId: schema.id, version: source.version, miningLevel: candidate.after };
  return { request: normalized, material: candidate.material, version: source.version, changed: Boolean(files.length),
    impact: compareHarvest(source.gates, schema, candidate.before, candidate.after),
    appliesToSchemas: source.schemas.filter(row => row.miningClass && row.levelParts.length),
    approval: files.length ? hash(JSON.stringify({ request: normalized, files })) : null, files,
    validation: "Native material and tool-schema parsers passed; harvest comparison is a source-derived estimate.",
    activation: "Rebuild and relaunch. Existing assembled tools share material definitions; all multiplayer peers need matching content." };
}
export function materialProgressionCommand(command, request = {}) {
  if (command === "inspect") {
    fields(request, ["materialId", "schemaId"]);
    const source = snapshot(false), schema = choose(source.schemas, request.schemaId ?? "pickaxe", "tool schema");
    const rows = request.materialId === undefined ? source.materials : [choose(source.materials, request.materialId, "material")];
    return { path: progressionPath, version: source.version, schemas: source.schemas,
      validation: "Read-only source inspection. Preview and apply require current native validators.",
      materials: rows.map(({ id, name, miningLevel }) => ({ id, name, miningLevel })),
      ...(rows.length === 1 ? { impact: compareHarvest(source.gates, schema, rows[0].miningLevel) } : {}), scope: progressionScope };
  }
  if (command === "preview") return preview(request);
  if (command === "apply") {
    const candidate = preview(request);
    if (!candidate.approval || request.approval !== candidate.approval) throw new Error("Review this exact material change and supply its preview approval.");
    const bundle = publishBundle(candidate.files);
    return { bundle: bundle.id, material: candidate.material, miningLevel: candidate.request.miningLevel, impact: candidate.impact, activation: candidate.activation };
  }
  if (command === "revert") {
    fields(request, ["bundle"], ["bundle"]);
    const bundle = inspectBundle(request.bundle);
    if (bundle.files.some(file => file.path !== progressionPath)) throw new Error("Choose a single tool-material bundle.");
    revertBundle(request.bundle);
    return { reverted: request.bundle };
  }
  throw new Error("Unknown material-progression command.");
}
