import { capabilities, toolsForPath, toolSummary } from "./tool-capabilities.mjs";

try {
  const [command = "list", value, ...extra] = process.argv.slice(2);
  if (extra.length) throw new Error("Too many arguments");
  let result;
  if (command === "list" && value === undefined) result = capabilities.map(toolSummary);
  else if (command === "route" && value) result = toolsForPath(value.replaceAll("\\", "/")).map(toolSummary);
  else if (command === "describe" && value) {
    const tool = capabilities.find(tool => tool.id === value);
    if (!tool) throw new Error("Unknown tool; use list");
    result = { ...toolSummary(tool), flow: tool.flow, docs: "tools/axiom-dev/" + tool.docs };
  } else throw new Error("Usage: tools-cli.mjs list | route <source-path> | describe <tool-id>");
  console.log(JSON.stringify(result));
} catch (error) { console.error(error.message); process.exitCode = 1; }
