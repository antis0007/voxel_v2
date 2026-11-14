import { portalCommand } from "./portal-service.mjs";
try {
  const command = process.argv[2] || "help";
  let request = {};
  if (["preview", "apply", "revert"].includes(command)) {
    let input = "";
    for await (const chunk of process.stdin) { input += chunk; if (Buffer.byteLength(input) > 262144) throw new Error("Command exceeds 256 KiB."); }
    request = JSON.parse(input);
  }
  console.log(JSON.stringify(portalCommand(command, request), null, 2));
} catch (error) { console.error(error.message); process.exitCode = 1; }
