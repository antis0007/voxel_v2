import contract from "./fabrication-contract.mjs";
import { decode, parse } from "./syntax.mjs";

export const fabricationPath = "assets/fabrication.ron";
const keys = ["id", "name", "inputs", "output", "ticks", "power"];

export function fabricationRows(text) {
  const root = parse(text, "ron");
  if (root.opener !== "[" || root.children.length < contract.minRecipes) throw new Error("Keep the original Fabricator recipe IDs 0–3");
  const names = new Set();
  const integer = (node, low, high, label) => {
    const value = Number(node?.raw);
    if (!/^\d+$/.test(node?.raw ?? "") || !Number.isSafeInteger(value) || value < low || value > high) {
      throw new Error(label + " must be an integer from " + low + " to " + high);
    }
    return value;
  };
  const stack = (node, label) => {
    if (node?.opener !== "(" || node.children.length !== 2) throw new Error(label + " needs a resource ID and quantity");
    const [resource, quantity] = node.children;
    return [integer(resource, 0, contract.resourceCount - 1, label + " resource ID"), integer(quantity, 1, contract.capacity, label + " quantity")];
  };
  return root.children.map((row, index) => {
    const fields = new Map(row.children?.map(field => [field.key, field]));
    const label = "Fabricator recipe #" + index;
    if (row.opener !== "(" || row.children.length !== keys.length || keys.some(key => !fields.has(key))) {
      throw new Error(label + " needs exactly " + keys.join(", "));
    }
    const id = integer(fields.get("id"), index, index, label + " ID");
    const rawName = fields.get("name").raw;
    const name = typeof rawName === "string" && rawName.startsWith('"') ? decode(rawName) : "";
    if (!name.trim() || names.has(name.trim().toLowerCase())) throw new Error(label + " needs a unique name");
    names.add(name.trim().toLowerCase());
    const list = fields.get("inputs");
    if (list.opener !== "[" || list.children.length < 1 || list.children.length > contract.maxInputs) {
      throw new Error(label + " needs one to three inputs");
    }
    const inputs = list.children.map(node => stack(node, label + " input"));
    if (new Set(inputs.map(([resource]) => resource)).size !== inputs.length) throw new Error(label + " repeats an ingredient");
    return {
      id, name, inputs, output: stack(fields.get("output"), label + " output"),
      ticks: integer(fields.get("ticks"), 1, contract.maxTicks, label + " ticks"),
      power: integer(fields.get("power"), 1, contract.maxPower, label + " power"),
    };
  });
}
