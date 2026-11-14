import { workshopService } from "./workshop-service.mjs";
import { runWorkshopCampaign } from "./workshop-campaign.mjs";
try {
  const [command = "help", ...extra] = process.argv.slice(2);
  if (command !== "plan" && extra.length) throw new Error("Unexpected arguments.");
  if (command === "help") {
    console.log(JSON.stringify({ commands: ["inspect", "plan <recipe ID or full name> <items/minute>", "design < JSON", "preview < JSON", "stage < JSON", "campaign < JSON"],
      design: "{process: recipe ID or full name, changes: {secondsPerBatch, powerKw, inputs?, output?}, copyName?: new recipe name}. Read-only; returns the exact request and preview approval for stage.",
      request: "Use version from inspect, requirements, and typed operations. Stage also needs approval from preview.",
      scope: "Content snapshots only; no runtime edits, save migration, build, or live installation.",
      guide: "tools/axiom-dev/WORKSHOP.md" }, null, 2));
  } else {
    const service = workshopService();
    let request = {};
    if (command === "plan") {
      if (extra.length !== 2) throw new Error("Use plan <recipe ID or full name> <items/minute>.");
      request = { process: /^\d+$/.test(extra[0]) ? Number(extra[0]) : extra[0], itemsPerMinute: Number(extra[1]) };
    } else if (command !== "inspect") {
      let text = "";
      for await (const chunk of process.stdin) { text += chunk; if (Buffer.byteLength(text) > 32768) throw new Error("Request exceeds 32 KiB."); }
      request = JSON.parse(text.replace(/^\uFEFF/, ""));
    }
    if (!["inspect", "plan", "design", "preview", "stage", "campaign"].includes(command)) throw new Error("Run help for usage.");
    const result = command === "campaign" ? runWorkshopCampaign(service, request) : service[command](request);
    console.log(JSON.stringify(result, null, 2));
    if (result.stop === "candidate-failed") process.exitCode = 1;
  }
} catch (error) { console.error(JSON.stringify({ error: error.message })); process.exitCode = 1; }

