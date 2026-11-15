import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const [command, output = "target/regional-review"] = process.argv.slice(2);
if (!command || command === "help") {
  console.log("review [output directory]  Render matching Hills, Ridged and Planet civilization maps for the old defaults, 321 and 322.");
  console.log("live                       Inspect stocks, jobs and settlement development in the active dev world.");
} else {
  const [program, args] = command === "review"
    ? ["cargo", ["run", "-p", "engine_worldgen", "--example", "regional_review", "--", output]]
    : command === "live"
      ? [process.execPath, ["tools/axiom-dev/engine-cli.mjs", "inspect", "world", "civilization"]]
      : [];
  if (!program) {
    console.error("Use review or live.");
    process.exitCode = 1;
  } else {
    const result = spawnSync(program, args, { cwd: root, stdio: "inherit" });
    if (result.error) console.error(result.error.message);
    process.exitCode = result.status ?? 1;
  }
}
