import contract from "./fabrication-contract.mjs";

export const planningScope = "Ideal base-drive estimate: continuous inputs, sufficient power and empty output capacity. Transport, warm-up, upgrades and interruptions are excluded; this is not a native playtest.";
const object = value => value && typeof value === "object" && !Array.isArray(value);
function exactUnits(value, scale, low, high, label) {
  const units = value * scale;
  if (typeof value !== "number" || !Number.isFinite(value) || !Number.isSafeInteger(units) || units < low || units > high) {
    throw new Error(label + " must be from " + low / scale + " to " + high / scale + " in steps of " + 1 / scale + ".");
  }
  return units;
}

export function nativeTuning(changes) {
  const keys = ["name", "secondsPerBatch", "powerKw", "inputs", "output"];
  if (!object(changes) || !Object.keys(changes).length || Object.keys(changes).some(key => !keys.includes(key))) {
    throw new Error("Tune name, secondsPerBatch, powerKw, inputs or output.");
  }
  const result = {};
  const stack = pair => {
    if (!Array.isArray(pair) || pair.length !== 2 || !Number.isSafeInteger(pair[0]) || pair[0] < 0 || pair[0] >= contract.resourceCount) {
      throw new Error("Use [registered resource ID, items] pairs.");
    }
    return [pair[0], exactUnits(pair[1], contract.unitsPerItem, 1, contract.capacity, "Items per batch")];
  };
  for (const [key, value] of Object.entries(changes)) {
    if (key === "name") result.name = value;
    if (key === "secondsPerBatch") result.ticks = exactUnits(value, contract.ticksPerSecond, 1, contract.maxTicks, "Seconds per batch");
    if (key === "powerKw") result.power = exactUnits(value, 1, 1, contract.maxPower, "Power in kW");
    if (key === "output") result.output = stack(value);
    if (key === "inputs") {
      if (!Array.isArray(value) || value.length < 1 || value.length > contract.maxInputs) throw new Error("Use one to three input stacks.");
      result.inputs = value.map(stack);
    }
  }
  return result;
}

export function resourceCatalog() {
  return contract.resources.map((name, id) => ({ id, name }));
}

export function processSummary(row, resources = []) {
  const stack = ([resource, units]) => ({ resource, name: resources.find(item => item.id === resource)?.name ?? "Resource " + resource, items: units / contract.unitsPerItem });
  const secondsPerBatch = row.ticks / contract.ticksPerSecond;
  const output = stack(row.output);
  return { id: row.id, name: row.name, inputs: row.inputs.map(stack), output, secondsPerBatch,
    powerKw: row.power, energyKjPerBatch: row.power * secondsPerBatch,
    energyKjPerItem: row.power * secondsPerBatch / output.items,
    itemsPerMinute: output.items * 60 / secondsPerBatch };
}

export function selectProcess(rows, process) {
  if (typeof process !== "string" && !Number.isSafeInteger(process)) throw new Error("Choose a recipe ID or its full name from inspect.");
  const matches = typeof process === "number" ? rows.filter(row => row.id === process)
    : rows.filter(row => row.name.toLowerCase() === process.trim().toLowerCase());
  if (matches.length !== 1) throw new Error("Choose one recipe ID or full name from inspect.");
  return matches[0];
}

function requiredMachines(row, target) {
  // Decimal cross-products avoid adding a machine at an exact capacity boundary.
  const [mantissa, exponent = "0"] = String(target).split("e");
  const places = (mantissa.split(".")[1] ?? "").length - Number(exponent);
  let numerator = BigInt(mantissa.replace(".", "")) * BigInt(contract.unitsPerItem * row.ticks);
  let denominator = BigInt(60 * contract.ticksPerSecond * row.output[1]);
  if (places > 0) denominator *= 10n ** BigInt(places);
  else numerator *= 10n ** BigInt(-places);
  return Number((numerator + denominator - 1n) / denominator);
}

export function productionPlan(row, itemsPerMinute, resources = []) {
  if (typeof itemsPerMinute !== "number" || !Number.isFinite(itemsPerMinute) || itemsPerMinute <= 0 || itemsPerMinute > 1000000) {
    throw new Error("Target must be above 0 and no more than 1000000 items per minute.");
  }
  const recipe = processSummary(row, resources);
  const machines = requiredMachines(row, itemsPerMinute);
  const capacity = machines * recipe.itemsPerMinute;
  return { recipe, targetItemsPerMinute: itemsPerMinute, machines,
    capacityItemsPerMinute: capacity, utilization: Math.min(1, itemsPerMinute / capacity),
    averagePowerKw: itemsPerMinute * recipe.energyKjPerItem / 60,
    peakPowerKw: machines * recipe.powerKw,
    inputs: recipe.inputs.map(input => ({ ...input, itemsPerMinute: input.items * itemsPerMinute / recipe.output.items })),
    scope: planningScope };
}
