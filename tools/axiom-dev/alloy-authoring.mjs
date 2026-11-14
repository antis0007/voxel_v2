import { createHash } from "node:crypto";
import { read, root } from "./workspace.mjs";
import { publishBundle } from "./agent-bundles.mjs";
import { parse, patch, decode } from "./syntax.mjs";
import { validateNativeContent } from "./native-content-validation.mjs";

const materialPath = "assets/tools/materials.ron", reactionPath = "assets/reactions.ron";
const field = (node, key) => node?.children?.find(child => child.key === key);
const scalar = node => !node ? "" : node.raw === undefined ? scalar(node.children?.[0]) : decode(node.raw);
const unique = (rows, predicate, message) => {
  const matches = rows.filter(predicate);
  if (matches.length !== 1) throw new Error(message);
  return matches[0];
};
const load = path => ({ ...read(path), path });

function target(material) {
  if (typeof material !== "string" || !material.trim()) throw new Error("Choose a tool material ID.");
  const materials = load(materialPath), reactions = load(reactionPath);
  const row = unique(parse(materials.text, "ron").children, row => scalar(field(row, "id")) === material,
    "Choose an exact, unique tool material ID.");
  const solid = scalar(field(row, "block"));
  if (!solid || solid === "None") throw new Error("This material needs an explicit existing block link before alloy editing.");
  const voxel = parse(read("assets/materials.ron").text, "ron");
  const solidRow = unique(voxel.children, row => scalar(field(row, "id")) === solid, "Material block is missing or ambiguous.");
  const liquid = scalar(field(field(solidRow, "sim"), "melts_into"));
  const reaction = unique(parse(reactions.text, "ron").children,
    row => field(row, "batch_counts")?.prefix === "Some" && scalar(field(row, "a_into")) === liquid,
    "This material needs one existing molten batch recipe; native behavior is not created by this command.");
  const props = field(row, "props");
  const counts = field(reaction, "batch_counts").children[0].children.map(node => Number(node.raw));
  return { materials, reactions, row, props, reaction, solid, liquid, counts,
    properties: Object.fromEntries(props.children.map(node => [node.key,
      /^(true|false)$/.test(node.raw) ? node.raw === "true" : Number(node.raw)])) };
}

export function inspectAlloy(material) {
  const state = target(material);
  return { material, solid: state.solid, liquid: state.liquid, properties: state.properties,
    batchCounts: state.counts, versions: { [materialPath]: state.materials.version, [reactionPath]: state.reactions.version },
    effect: "Saved definitions take effect after rebuilding and relaunching the game." };
}

export function previewAlloy(request) {
  if (!request || typeof request !== "object" || Array.isArray(request)
      || Object.keys(request).some(key => !["material", "changes", "batchCounts", "versions", "approval"].includes(key))) {
    throw new Error("Expected one alloy request with material, changes and/or batchCounts.");
  }
  const state = target(request.material);
  const changes = request.changes ?? {};
  if (!changes || typeof changes !== "object" || Array.isArray(changes)) throw new Error("Expected material property changes.");
  const docs = [state.materials, state.reactions];
  if (request.versions && docs.some(doc => request.versions[doc.path] !== doc.version)) {
    throw new Error("Alloy sources changed on disk. Inspect or preview again.");
  }
  const edits = [];
  for (const [key, value] of Object.entries(changes)) {
    const node = field(state.props, key);
    if (!node || typeof value !== "boolean" && (typeof value !== "number" || !Number.isFinite(value))) {
      throw new Error("Choose an existing numeric or boolean tool-material property: " + key);
    }
    const raw = typeof value === "boolean" ? String(value) : Number.isInteger(value) ? value + ".0" : String(value);
    // Integer properties keep their native representation; the checker owns ranges/types.
    const replacement = /^\d+$/.test(node.raw) && typeof value === "number" ? String(value) : raw;
    edits.push({ start: node.start, end: node.end, raw: replacement });
  }
  let materialText = state.materials.text;
  for (const edit of edits.sort((a, b) => b.start - a.start)) materialText = patch(materialText, edit.start, edit.end, edit.raw);
  let reactionText = state.reactions.text;
  if (request.batchCounts !== undefined) {
    if (!Array.isArray(request.batchCounts) || request.batchCounts.length !== 2
        || request.batchCounts.some(value => !Number.isSafeInteger(value))) throw new Error("Batch counts need two whole cell-volume counts.");
    const node = field(state.reaction, "batch_counts");
    reactionText = patch(reactionText, node.start, node.end, `Some((${request.batchCounts.join(", ")}))`);
  }
  const proposed = [materialText, reactionText];
  docs.forEach((doc, index) => validateNativeContent(root, doc.path, proposed[index]));
  const files = docs.flatMap((doc, index) => doc.text === proposed[index] ? []
    : [{ path: doc.path, version: doc.version, text: proposed[index] }]);
  const approval = files.length ? createHash("sha256").update(JSON.stringify(files)).digest("hex") : null;
  return { material: request.material, changed: Boolean(files.length), approval,
    versions: Object.fromEntries(docs.map(doc => [doc.path, doc.version])),
    before: { properties: state.properties, batchCounts: state.counts },
    after: { properties: { ...state.properties, ...changes }, batchCounts: request.batchCounts ?? state.counts }, files };
}

export function applyAlloy(request) {
  const preview = previewAlloy(request);
  if (!preview.approval || request.approval !== preview.approval) {
    throw new Error("Preview this exact alloy edit again and supply its approval value.");
  }
  const bundle = publishBundle(preview.files);
  return { material: request.material, bundle: bundle.id, saved: bundle.saved.map(file => file.path), after: preview.after };
}
