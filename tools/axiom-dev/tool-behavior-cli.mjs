import { toolBehaviorCommand } from "./tool-behavior.mjs";
try {
  const [command = "help", id, ...extra] = process.argv.slice(2);
  if (extra.length) throw new Error("Unexpected arguments.");
  let request = {};
  if (command === "help") {
    console.log(JSON.stringify({ commands: ["inspect [schema ID]", "preview < JSON", "apply < JSON", "revert <bundle ID>"],
      request: "Use schemaId, version and a complete capabilities object returned by inspect. Apply also needs the exact preview approval.",
      activation: "Rebuild and relaunch; matching assembled tools share the authored behavior." }, null, 2));
  } else {
    if (command === "inspect") request = id === undefined ? {} : { schemaId: id };
    else if (command === "revert") request = { bundle: id };
    else if (["preview", "apply"].includes(command) && id === undefined) {
      let text = "";
      for await (const chunk of process.stdin) { text += chunk; if (Buffer.byteLength(text) > 32768) throw new Error("Request exceeds 32 KiB."); }
      request = JSON.parse(text.replace(/^\uFEFF/, ""));
    } else throw new Error("Run help for usage.");
    console.log(JSON.stringify(toolBehaviorCommand(command, request), null, 2));
  }
} catch (error) { console.error(JSON.stringify({ error: error.message })); process.exitCode = 1; }
