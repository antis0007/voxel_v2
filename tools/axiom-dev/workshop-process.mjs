import contract from "./fabrication-contract.mjs";
import { nativeTuning } from "./workshop-planning.mjs";
import { fabricationRows } from "./fabrication-data.mjs";
import { parse, patch, quoted } from "./syntax.mjs";

export const workshopPath = "assets/fabrication.ron";
export const workshopSources = Object.freeze([
  "tools/axiom-dev/fabrication-contract.mjs", "tools/axiom-dev/native-content-validation.mjs",
  "tools/axiom-dev/catalog-contracts.mjs", "crates/content/src/fabrication.rs",
  "crates/content/src/factory_resources.rs", "crates/content/examples/check_fabrication.rs",
  workshopPath, "assets/machines.ron", "crates/app/src/world/factory.rs",
  "crates/app/src/world/factory/recipes.rs", "crates/app/src/world/factory/power.rs",
  "crates/app/src/world/expedition.rs", "crates/app/src/world/storage.rs",
  "tools/axiom-dev/fabrication-data.mjs", "tools/axiom-dev/syntax.mjs",
  "tools/axiom-dev/workshop-process.mjs", "tools/axiom-dev/workshop-service.mjs",
  "tools/axiom-dev/workshop-stage.mjs", "tools/axiom-dev/workshop-campaign.mjs",
  "tools/axiom-dev/workshop-planning.mjs", "tools/axiom-dev/workshop-cli.mjs",
]);

export function fields(value, names, required = names) {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || Object.keys(value).some(key => !names.includes(key))
    || required.some(key => !Object.hasOwn(value, key))) throw new Error("Unexpected or missing workshop fields.");
}

export function capabilityReport(requirements = []) {
  const supported = ["fabricator-process", "electric-power", "single-resource-output"];
  const gaps = {
    "mechanical-drive": "Shaft torque, speed, connectivity and stall state have no authored Fabricator adapter. A waterwheel currently supplies electrical kW.",
    "multiple-outputs": "The native Process and Fabrication definitions store one output. Atomic coproduct reservation needs a reviewed runtime extension.",
    "fluid-waste": "Fabrication has no authored fluid waste port or tank reservation. Existing scrubber water behavior is machine-specific.",
    "blight-resources": "Blight feed, recovered product and waste have no verified resource mapping. New factory resources affect saves.",
    "grain-resources": "Wheat food is not a verified Fabricator resource mapping. Grain-to-flour logistics needs a reviewed item adapter.",
    "presentation-cues": "Fault cues and animation are not authored by the Fabricator catalog. A generic native state-to-cue adapter needs review.",
    "viewport-binding": "Object, port and position bindings need a revisioned native selection bridge.",
    "playable-preview": "Fabrication is embedded at build time. There is no verified Fabrication draft override or disposable factory-world bridge.",
    "core-runtime": "Core runtime changes require explicit approval.",
    "save-migration": "Save migrations require explicit approval.",
    "live-deployment": "Live-world deployment requires explicit approval.",
  };
  if (!Array.isArray(requirements) || requirements.length > 16 || new Set(requirements).size !== requirements.length
    || requirements.some(key => typeof key !== "string" || ![...supported, ...Object.keys(gaps)].includes(key))) {
    throw new Error("Use distinct registered workshop requirements.");
  }
  return { supported, gaps: requirements.filter(key => key in gaps).map(id => ({ id, reason: gaps[id] })),
    knownRequirements: [...supported, ...Object.keys(gaps)],
    scope: "Fabricator authoring adapter only; no native execution proof." };
}

export const firstCampaign = Object.freeze([
  { id: "blight-separator", requirements: ["fabricator-process", "mechanical-drive", "multiple-outputs", "fluid-waste", "blight-resources", "presentation-cues", "playable-preview"] },
  { id: "kinetic-grain-mill", requirements: ["fabricator-process", "mechanical-drive", "grain-resources", "presentation-cues", "playable-preview"] },
]);

function label(value) {
  if (typeof value !== "string" || value !== value.trim() || !value || value.length > 100 || /[\x00-\x1f\x7f]/.test(value)) {
    throw new Error("Process name needs 1-100 visible characters.");
  }
  return quoted(value, "ron");
}
function encode(field, value) {
  if (field === "name") return label(value);
  if (["ticks", "power"].includes(field)) {
    if (!Number.isSafeInteger(value)) throw new Error("Process values must be integers.");
    return String(value);
  }
  const stack = row => {
    if (!Array.isArray(row) || row.length !== 2 || row.some(n => !Number.isSafeInteger(n))) throw new Error("Use resource and quantity integer pairs.");
    return "(" + row.join(", ") + ")";
  };
  if (field === "output") return stack(value);
  if (field === "inputs" && Array.isArray(value)) return "[" + value.map(stack).join(", ") + "]";
  throw new Error("Unsupported process field.");
}

export function reviseProcesses(original, operations) {
  const before = fabricationRows(original);
  if (!Array.isArray(operations) || operations.length < 1 || operations.length > 16) throw new Error("Use 1-16 typed process operations.");
  const expanded = operations.flatMap(operation => {
    if (operation?.op !== "tune-process") return [operation];
    fields(operation, ["op", "id", "changes"]);
    return Object.entries(nativeTuning(operation.changes)).map(([field, value]) =>
      ({ op: "set-process", id: operation.id, field, value }));
  });
  let text = original;
  for (const operation of expanded) {
    fields(operation, operation?.op === "derive-process" ? ["op", "sourceId", "name"] : ["op", "id", "field", "value"]);
    const rows = fabricationRows(text), tree = parse(text, "ron");
    const id = operation.op === "derive-process" ? operation.sourceId : operation.id;
    if (!Number.isSafeInteger(id) || !rows[id]) throw new Error("Select an existing process ID.");
    const row = tree.children[id];
    if (operation.op === "derive-process") {
      let clone = text.slice(row.start, row.end);
      const edits = row.children.filter(field => ["id", "name"].includes(field.key));
      for (const field of edits.sort((a, b) => b.start - a.start)) {
        clone = patch(clone, field.start - row.start, field.end - row.start,
          field.key === "id" ? String(rows.length) : label(operation.name));
      }
      const last = tree.children.at(-1);
      text = patch(text, tree.close, tree.close, `${last.comma === undefined ? "," : ""}\n    ${clone},\n`);
    } else if (operation.op === "set-process") {
      if (!["name", "inputs", "output", "ticks", "power"].includes(operation.field)) throw new Error("Process IDs cannot be edited.");
      const token = row.children.find(field => field.key === operation.field);
      text = patch(text, token.start, token.end, encode(operation.field, operation.value));
    } else throw new Error("Unknown process operation.");
    fabricationRows(text);
  }
  const after = fabricationRows(text);
  const changes = after.filter(row => JSON.stringify(row) !== JSON.stringify(before[row.id]))
    .map(row => ({ id: row.id, before: before[row.id] ?? null, after: row }));
  return { text, before, after, changes, changed: text !== original,
    units: { ticks: (1 / contract.ticksPerSecond) + " seconds", quantity: contract.unitsPerItem + " subunits per item", power: "kW" } };
}
