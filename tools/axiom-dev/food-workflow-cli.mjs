import { readFileSync } from "node:fs";
import { foodWorkflowCatalog, previewFoodWorkflow, applyFoodWorkflow, inspectFoodWorkflow,
  previewFoodWithdrawal, withdrawFoodRecipe } from "./food-workflow.mjs";

async function run([command = "help", arg, ...rest]) {
  if (rest.length) throw new Error("Unexpected arguments");
  if (command === "help" && arg === undefined) return {
    commands: ["catalog", "preview < JSON", "apply < JSON", "inspect <bundleId>", "preview-withdraw < JSON", "withdraw < JSON"],
    request: { draft: { name: "Herb Bread", template: "Bread", sprite: 60, hue: 0, hunger: 32, health: 1,
      saturation: 20, copyEffects: false }, recipe: { name: "Herb Bread", station: "hand", count: 1,
      inputs: [{ item: "Food(Bread)", count: 1 }, { item: "Food(Egg)", count: 1 }] } },
    apply: "Repeat the preview request with its approval value. Requests are revalidated against every source revision.",
    withdrawal: { bundleId: "Creation receipt ID", approval: "From preview-withdraw" },
    identity: "Withdrawal removes only the recipe; the native item ID, food definition and icon remain allocated.",
    runtime: "Source authoring only; rebuild and native playtest are required.",
  };
  if (command === "catalog" && arg === undefined) return foodWorkflowCatalog();
  if (command === "inspect" && arg) return inspectFoodWorkflow(arg);
  const commands = { preview: previewFoodWorkflow, apply: applyFoodWorkflow,
    "preview-withdraw": previewFoodWithdrawal, withdraw: withdrawFoodRecipe };
  if (!Object.hasOwn(commands, command) || arg !== undefined) throw new Error("Run help for usage; commands read JSON from standard input");
  const request = JSON.parse(readFileSync(0, "utf8").replace(/^\uFEFF/, ""));
  const result = await commands[command](request);
  return result.saved ? { ...result, saved: result.saved.map(({ path, version }) => ({ path, version })) } : result;
}

try { process.stdout.write(JSON.stringify(await run(process.argv.slice(2)), null, 2) + "\n"); }
catch (error) { process.stderr.write(JSON.stringify({ error: error.message }) + "\n"); process.exitCode = 1; }
