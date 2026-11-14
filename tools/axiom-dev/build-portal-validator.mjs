import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { portalDirectory, portalBinary, portalFingerprint, portalHash } from "./portal-validator.mjs";
const root = fs.realpathSync(fileURLToPath(new URL("../../", import.meta.url)));
const engineLock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
const checkerLock = fs.readFileSync(path.join(root, "tools/portal-native-check/Cargo.lock"), "utf8");
for (const dependency of ["serde", "ron"]) {
  const pattern = new RegExp('\\[\\[package\\]\\]\\s+name = "' + dependency + '"\\s+version = "([^"]+)"');
  if (!engineLock.match(pattern)?.[1] || engineLock.match(pattern)?.[1] !== checkerLock.match(pattern)?.[1])
    throw new Error("Align the portal checker's " + dependency + " version with the engine Cargo.lock before building.");
}
const source = portalFingerprint(root);
const target = path.join(root, "target", "portal-validator-build");
await new Promise((resolve, reject) => {
  const child = spawn("cargo", ["build", "--locked", "--offline", "--manifest-path", "tools/portal-native-check/Cargo.toml", "--target-dir", target],
    { cwd: root, windowsHide: true, stdio: "inherit" });
  child.once("error", reject); child.once("close", code => code === 0 ? resolve() : reject(new Error("Portal validator build failed: " + code)));
});
if (portalFingerprint(root) !== source) throw new Error("Schema changed during build. Rebuild the validator.");
const directory = portalDirectory(root);
fs.mkdirSync(directory, { recursive: true });
const bytes = fs.readFileSync(path.join(target, "debug", portalBinary));
fs.writeFileSync(path.join(directory, portalBinary), bytes);
fs.writeFileSync(path.join(directory, "receipt.json"), JSON.stringify({ source, binary: portalHash(bytes) }, null, 2));
console.log("Portal schema validator ready.");
