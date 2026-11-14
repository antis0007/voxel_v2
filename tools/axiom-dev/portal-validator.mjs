import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";

export const portalSources = ["crates/content/src/portal_layout.rs", "crates/app/src/world/portals.rs", "crates/app/src/world/portals/authored.rs", "Cargo.lock",
  "tools/portal-native-check/Cargo.toml", "tools/portal-native-check/Cargo.lock", "tools/portal-native-check/build.rs", "tools/portal-native-check/src/main.rs"];
export const portalHash = bytes => createHash("sha256").update(bytes).digest("hex");
export const portalDirectory = root => path.join(root, "target", "axiom-dev", "portal-validation");
export const portalBinary = "check_portal_layout" + (process.platform === "win32" ? ".exe" : "");
export function portalFingerprint(root) {
  return portalHash(portalSources.map(file => file + "\0" + portalHash(fs.readFileSync(path.join(root, file)))).join("\0"));
}
export function checkPortalLayout(root, text) {
  if (typeof text !== "string" || Buffer.byteLength(text) > 262144) throw new Error("Portal layout exceeds 256 KiB.");
  const directory = portalDirectory(root), binary = path.join(directory, portalBinary);
  try {
    const receipt = JSON.parse(fs.readFileSync(path.join(directory, "receipt.json"), "utf8"));
    if (fs.realpathSync(binary) !== binary || receipt.source !== portalFingerprint(root)
      || receipt.binary !== portalHash(fs.readFileSync(binary))) throw new Error();
  } catch { throw new Error("Build the portal validator: node tools/axiom-dev/build-portal-validator.mjs"); }
  const result = spawnSync(binary, [], { input: text, encoding: "utf8", timeout: 10000, windowsHide: true, maxBuffer: 1024 * 1024 });
  let data;
  try { data = JSON.parse(result.stdout); } catch { throw new Error("Portal validator failed. Rebuild it and retry."); }
  if (result.status !== 0 || data.valid !== true) throw new Error(data.diagnostics?.map(issue => issue.target + ": " + issue.message).join("\n") || data.error || "Invalid portal layout.");
  return data;
}
