import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { read, root } from "./workspace.mjs";
import { publishBundle, inspectBundle, revertBundle } from "./agent-bundles.mjs";
import { parse, decode, patch, addEntry } from "./syntax.mjs";
import { checkedValidator, validateNativeContent } from "./native-content-validation.mjs";

export const toolBehaviorPath = "assets/tools/parts.ron";
const field = (node, key) => node?.children?.find(child => child.key === key);
const value = node => node?.raw === undefined ? undefined : decode(node.raw);
const scalar = node => node?.raw === "true" ? true : node?.raw === "false" ? false : value(node);
const clone = value => structuredClone(value);
const canonical = value => value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value;
const object = value => value && typeof value === "object" && !Array.isArray(value);
function keys(value, allowed) {
  if (!object(value) || Object.keys(value).some(key => !allowed.includes(key))) throw new Error("Unexpected tool-behavior request fields.");
}

function description() {
  const binary = checkedValidator(root, toolBehaviorPath);
  const result = spawnSync(binary, ["--describe"], { cwd: root, encoding: "utf8", windowsHide: true,
    timeout: 10000, maxBuffer: 1024 * 1024 });
  let schema;
  try { schema = JSON.parse(result.stdout); } catch { /* Report the actionable rebuild below. */ }
  if (result.status !== 0 || schema?.valid !== true || !Array.isArray(schema.miningStyles)
      || !schema.miningStyles.length || !object(schema.defaults) || !object(schema.miningDefaults)) {
    throw new Error("Tool capability discovery is unavailable. Run node tools/axiom-dev/build-validators.mjs.");
  }
  return schema;
}

function source() {
  const doc = read(toolBehaviorPath), schema = description();
  validateNativeContent(root, toolBehaviorPath, doc.text);
  const tree = parse(doc.text, "ron"), rows = field(tree, "schemas")?.children ?? [];
  return { doc, schema, rows };
}
function snapshot(row, schema) {
  const caps = field(row, "capabilities"), mining = field(caps, "mining");
  const capabilities = clone(schema.defaults);
  if (caps) for (const entry of caps.children) capabilities[entry.key] = scalar(entry);
  capabilities.mining = mining?.prefix === "Some"
    ? Object.fromEntries(mining.children[0].children.map(child => [child.key, scalar(child)])) : null;
  return { id: value(field(row, "id")), name: value(field(row, "display_name")), capabilities,
    parts: field(row, "parts").children.map(part => value(field(part, "kind"))) };
}
function pick(rows, id) {
  const found = rows.filter(row => value(field(row, "id")) === id);
  if (found.length !== 1) throw new Error("Choose one existing assembled-tool schema ID.");
  return found[0];
}
function capabilities(value, schema) {
  keys(value, Object.keys(schema.defaults));
  if (Object.keys(value).length !== Object.keys(schema.defaults).length) throw new Error("Supply the complete capability object from inspect.");
  for (const [key, model] of Object.entries(schema.defaults)) {
    if (key === "mining") continue;
    if (typeof model !== "boolean" || typeof value[key] !== "boolean") throw new Error(key + " needs a boolean value.");
  }
  if (value.mining !== null) {
    keys(value.mining, Object.keys(schema.miningDefaults));
    for (const [key, model] of Object.entries(schema.miningDefaults)) {
      const valid = key === "class" ? schema.miningStyles.includes(value.mining[key])
        : typeof model === "boolean" && typeof value.mining[key] === "boolean";
      if (!valid) throw new Error("Choose a native mining " + key + " value from inspect.");
    }
  }
  return clone(value);
}
function encode(caps) {
  const mining = caps.mining === null ? "None" : "Some((" + Object.entries(caps.mining)
    .map(([key, value]) => key + ": " + String(value)).join(", ") + "))";
  return "(mining: " + mining + ", " + Object.entries(caps).filter(([key]) => key !== "mining")
    .map(([key, value]) => key + ": " + String(value)).join(", ") + ")";
}
function preview(request) {
  keys(request, ["schemaId", "version", "capabilities", "approval"]);
  const { doc, schema, rows } = source();
  if (request.version !== doc.version) throw new Error("Tool definitions changed on disk. Reload and review the edit again.");
  const row = pick(rows, request.schemaId), before = snapshot(row, schema), after = capabilities(request.capabilities, schema);
  const node = field(row, "capabilities");
  const changed = JSON.stringify(canonical(before.capabilities)) !== JSON.stringify(canonical(after));
  const text = !changed ? doc.text : node ? patch(doc.text, node.start, node.end, encode(after))
    : addEntry(doc.text, row, "capabilities: " + encode(after));
  validateNativeContent(root, toolBehaviorPath, text);
  const files = changed ? [{ path: toolBehaviorPath, version: doc.version, text }] : [];
  return { schemaId: before.id, name: before.name, version: doc.version, before: before.capabilities, after,
    changed, approval: changed ? createHash("sha256").update(JSON.stringify(files)).digest("hex") : null, files };
}

export function toolBehaviorCommand(command, request = {}) {
  if (command === "inspect") {
    keys(request, ["schemaId"]);
    const { doc, schema, rows } = source();
    const selected = request.schemaId === undefined ? rows : [pick(rows, request.schemaId)];
    return { path: toolBehaviorPath, version: doc.version, miningStyles: schema.miningStyles,
      miningDefaults: schema.miningDefaults, schemas: selected.map(row => snapshot(row, schema)),
      activation: "Rebuild and relaunch Axiom. All matching assembled tools use the saved behavior." };
  }
  if (command === "preview") return preview(request);
  if (command === "apply") {
    const candidate = preview(request);
    if (!candidate.approval || candidate.approval !== request.approval) throw new Error("Preview this exact change and supply its approval value.");
    const bundle = publishBundle(candidate.files);
    return { bundle: bundle.id, schemaId: candidate.schemaId, capabilities: candidate.after };
  }
  if (command === "revert") {
    keys(request, ["bundle"]);
    const bundle = inspectBundle(request.bundle);
    if (bundle.files.some(file => file.path !== toolBehaviorPath)) throw new Error("Choose a tool-behavior bundle.");
    revertBundle(request.bundle);
    return { reverted: request.bundle };
  }
  throw new Error("Unknown tool-behavior command.");
}
