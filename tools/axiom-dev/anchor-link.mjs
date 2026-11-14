import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { root } from "./workspace.mjs";

const url = "http://127.0.0.1:4377/";
let launching;

async function ready() {
  try {
    const response = await fetch(url + "api/library", { signal: AbortSignal.timeout(1000) });
    return response.ok && Array.isArray((await response.json()).names);
  } catch { return false; }
}

export async function anchorStudio() {
  if (await ready()) return { url };
  if (launching) return launching;
  const compiler = path.join(root, "target/anchor-studio/compiler/release-fast",
    process.platform === "win32" ? "anchor_studio.exe" : "anchor_studio");
  if (!existsSync(compiler)) throw new Error("Run tools/Open-Anchor-Studio.ps1 once to build the building designer.");
  launching = new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["tools/anchor-studio/server.mjs"], {
      cwd: root, windowsHide: true, env: { ...process.env, ANCHOR_STUDIO_PORT: "4377" }, stdio: ["ignore", "pipe", "pipe"],
    });
    let errors = "";
    const timer = setTimeout(() => reject(new Error("Anchor Studio did not start. Open its launcher for details.")), 8000);
    child.stdout.on("data", chunk => {
      if (chunk.toString().includes("Anchor Studio is open at")) { clearTimeout(timer); resolve({ url }); }
    });
    child.stderr.on("data", chunk => { errors = (errors + chunk.toString()).slice(-700); });
    child.once("error", error => { clearTimeout(timer); reject(error); });
    child.once("close", async () => {
      clearTimeout(timer);
      if (await ready()) resolve({ url });
      else reject(new Error(errors.trim() || "Anchor Studio closed"));
    });
  }).finally(() => { launching = undefined; });
  return launching;
}

