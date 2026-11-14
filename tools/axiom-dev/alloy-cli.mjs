import { readFileSync } from "node:fs";
import { inspectAlloy, previewAlloy, applyAlloy } from "./alloy-authoring.mjs";

try {
  const [command = "help", material, ...rest] = process.argv.slice(2);
  if (rest.length) throw new Error("Unexpected arguments. Run help for usage.");
  let result;
  if (command === "help") result = {
    commands: ["inspect <tool material ID>", "preview < JSON", "apply < JSON"],
    request: { material: "bronze", changes: { durability: 480 }, batchCounts: [3, 1] },
    apply: "Repeat the preview request with its versions and approval. Both files use Forge's native-validated, revision-checked bundle publisher.",
    scope: "Existing block-linked tool materials with one existing molten alloy recipe. Rebuild and relaunch after saving.",
  };
  else if (command === "inspect") result = inspectAlloy(material);
  else if ((command === "preview" || command === "apply") && material === undefined) {
    const request = JSON.parse(readFileSync(0, "utf8").replace(/^\uFEFF/, ""));
    result = command === "preview" ? previewAlloy(request) : applyAlloy(request);
  } else throw new Error("Unknown command. Run help for usage.");
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
} catch (error) {
  process.stderr.write(JSON.stringify({ error: error.message }) + "\n");
  process.exitCode = 1;
}
