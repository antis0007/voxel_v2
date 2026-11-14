import { auditCycles } from "./alchemy-cycles.mjs";

try {
  const [command = "help", ...rest] = process.argv.slice(2);
  if (rest.length) throw new Error("Unexpected arguments. Run help for usage.");
  if (command === "help") {
    console.log(JSON.stringify({
      commands: ["cycles"],
      cycles: "Read-only C-MC1 audit of processes.ron, fabrication.ron and brewing.ron. Exit 1 when a loop gains items.",
      scope: "No alchemy design exists yet; inspect/preview/apply/revert wait for an approved design.",
    }, null, 2));
  } else if (command === "cycles") {
    const report = auditCycles();
    console.log(JSON.stringify(report, null, 2));
    if (!report.ok) process.exitCode = 1;
  } else throw new Error("Unknown command. Run help for usage.");
} catch (error) {
  process.stderr.write(JSON.stringify({ error: error.message }) + "\n");
  process.exitCode = 1;
}
