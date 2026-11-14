import { spawn } from "node:child_process";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createHash, randomBytes } from "node:crypto";
import path from "node:path";
import { root, state, ensureDirectory, maxAsset } from "./workspace.mjs";

let busy = false;

export async function renderToolArt() {
  if (busy) throw new Error("A native art render is already running");
  const name = process.platform === "win32" ? "axiom-playtest.exe" : "axiom-playtest";
  const binary = [path.join(state, name), path.join(root, "target", "release-fast",
    process.platform === "win32" ? "minecraft_style_voxel_prototype.exe" : "minecraft_style_voxel_prototype")]
    .find(candidate => existsSync(candidate));
  if (!binary) throw new Error("Build the game before rendering compiled tool art");
  const directory = ensureDirectory(path.join(state, "native-art", randomBytes(8).toString("hex")));
  const output = path.join(directory, "tools.png");
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("VOXEL_")));
  env.VOXEL_TOOL_ART_SHEET = output;
  env.VOXEL_SAVE_DIR = directory;
  busy = true;
  try {
    await new Promise((resolve, reject) => {
      const child = spawn(binary, [], { cwd: root, env, windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
      let errors = "", timedOut = false;
      child.stderr.on("data", chunk => { errors = (errors + chunk).slice(-800); });
      const timeout = setTimeout(() => { timedOut = true; child.kill(); }, 30000);
      child.once("error", error => { clearTimeout(timeout); reject(error); });
      child.once("close", code => {
        clearTimeout(timeout);
        if (timedOut) reject(new Error("This build did not export tool art within 30 seconds"));
        else if (code !== 0) reject(new Error("Native tool art export failed: " + errors));
        else resolve();
      });
    });
    if (!existsSync(output)) throw new Error("This build does not support native tool art export");
    const size = statSync(output).size;
    if (size > maxAsset || size < 33) throw new Error("Native art output exceeds the preview limits");
    const bytes = readFileSync(output);
    if (bytes.readUInt32BE(0) !== 0x89504e47) throw new Error("Native art output is not PNG");
    const evidence = { output, binary, buildTime: statSync(binary).mtime.toISOString(),
      binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
      imageSha256: createHash("sha256").update(bytes).digest("hex"),
      width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20),
      scope: "Compiled tool art. Rebuild to include newer source edits." };
    writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
    return { ...evidence, png: bytes.toString("base64") };
  } finally { busy = false; }
}
