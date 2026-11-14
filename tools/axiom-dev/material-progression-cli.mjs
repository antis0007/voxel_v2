import { materialProgressionCommand as command } from "./material-progression.mjs";
try {
  const [action = "help", ...args] = process.argv.slice(2);
  let request = {};
  if (action === "help") {
    console.log(JSON.stringify({ commands: ["inspect [material ID] [schema ID]", "preview <material ID> <mining level> [schema ID]", "preview < JSON", "apply < JSON", "revert <bundle ID>"],
      workflow: "Preview is read-only. Review gained/lost harvest gates, then send its request plus approval to apply. Revert preserves later edits.",
      guide: "tools/axiom-dev/MATERIAL_PROGRESSION.md" }, null, 2));
  } else {
    if (action === "inspect" && args.length <= 2) request = { ...(args[0] ? { materialId: args[0] } : {}), ...(args[1] ? { schemaId: args[1] } : {}) };
    else if (action === "preview" && args.length >= 2 && args.length <= 3) {
      const inspected = command("inspect", { materialId: args[0], ...(args[2] ? { schemaId: args[2] } : {}) });
      if (!/^\d+$/.test(args[1])) throw new Error("Mining level must be a whole number.");
      request = { materialId: args[0], schemaId: args[2] ?? "pickaxe", version: inspected.version, miningLevel: Number(args[1]) };
    } else if (["preview", "apply"].includes(action) && !args.length) {
      let text = "";
      for await (const chunk of process.stdin) { text += chunk; if (Buffer.byteLength(text) > 32768) throw new Error("Request exceeds 32 KiB."); }
      request = JSON.parse(text.replace(/^\uFEFF/, ""));
    } else if (action === "revert" && args.length === 1) request = { bundle: args[0] };
    else throw new Error("Run help for usage.");
    console.log(JSON.stringify(command(action, request), null, 2));
  }
} catch (error) { console.error(JSON.stringify({ error: error.message })); process.exitCode = 1; }
