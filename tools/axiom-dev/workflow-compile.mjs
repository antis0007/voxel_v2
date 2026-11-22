import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtemp, readFile, realpath, rm, statfs, writeFile } from "node:fs/promises";
import path from "node:path";
import { captureSnapshot, checkedInputs, digest } from "./parity-snapshot.mjs";
import { ensureDirectory, state } from "./workspace.mjs";

const rustSource = /^crates\/[^/]+\/(?:src\/.+\.rs|build\.rs)$/;
const shaderSource = /^crates\/app\/src\/[\w/-]+\.wgsl$/;
const shaderRegistry = "crates/app/src/shader_sources.rs";
const gamePackage = "minecraft_style_voxel_prototype";

async function runCargo(directory, target, signal, mode) {
  const build = mode === "build";
  const command = build ? ["build", "--profile", "release-fast", "-p", gamePackage, "--bin", gamePackage]
    : ["check", "--workspace"];
  const label = build ? "Axiom build" : "Cargo check";
  const timeout = AbortSignal.timeout((build ? 25 : 10) * 60_000);
  const child = spawn("cargo", [...command, "--locked", "--offline",
    "--jobs", "2", "--message-format=json", "--target-dir", target], {
    cwd: directory, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, CARGO_INCREMENTAL: "0" },
    signal: AbortSignal.any([signal, timeout]),
  });
  const errors = [];
  let launchError, stderr = "";
  const lines = createInterface({ input: child.stdout });
  lines.on("line", line => {
    if (errors.length >= 4) return;
    let event;
    try { event = JSON.parse(line); } catch { return; }
    if (event.reason !== "compiler-message" || event.message.level !== "error") return;
    const span = event.message.spans.find(item => item.is_primary);
    errors.push({ path: span?.file_name.replaceAll("\\", "/"), line: span?.line_start, message: event.message.message });
  });
  child.stderr.on("data", data => { stderr = (stderr + data).slice(-4000); });
  child.once("error", error => { launchError = error; });
  const code = await new Promise(resolve => child.once("close", resolve));
  signal.throwIfAborted();
  if (timeout.aborted) throw new Error(label + " exceeded its time limit.");
  if (launchError) throw launchError;
  const diagnostics = errors.map(error => (error.path ? error.path + ":" + error.line + " " : "") + error.message);
  const buildError = stderr.split(/\r?\n\s*--- stderr\s*\r?\n/).at(-1)
    .split(/\r?\n/).filter(line => !/^\s*(?:cargo(?:::|:)rerun-if-|warning: build failed, waiting)/.test(line))
    .join("\n").replace(/(thread '[^']+') \(\d+\) panicked/g, "$1 panicked").trim();
  // Cargo progress and panic trailers can differ even when the isolated
  // candidate and baseline hit the exact same shader validator diagnostic.
  const shaderFailure = buildError.match(/^[ \t]*Invalid WGSL programs:\n[\s\S]+?(?=\n\s*note: run with)/m)?.[0].trimStart();
  return { ok: code === 0, mode, errors, shaderFailure, message: code === 0 ? label + " passed in the isolated workspace."
    : label + " failed: " + (diagnostics.join("\n") || buildError.slice(-1800) || "exit " + code).slice(0, 1800) };
}

export async function checkWorkflowCompile(changes, evidence, signal, mode = "compile") {
  const files = evidence.files.filter(file => file.status === "Pending");
  if (files.some(file => file.path === shaderRegistry || !(rustSource.test(file.path) || shaderSource.test(file.path)))) {
    return { ok: false, message: "Compile validation supports workspace Rust implementation and the app's registered WGSL inputs." };
  }
  const parent = ensureDirectory(path.join(state, "workflow-compile"));
  const directory = await mkdtemp(path.join(parent, "run-"));
  try {
    const snapshot = await captureSnapshot(directory, signal);
    if (files.some(file => shaderSource.test(file.path))) {
      const registry = await readFile(path.join(directory, shaderRegistry), "utf8");
      const list = /const SHADER_INPUTS\b[^;]+?=\s*shader_inputs!\[([\s\S]*?)\];/.exec(registry)?.[1] ?? "";
      const registered = new Set([...list.matchAll(/"([\w/-]+\.wgsl)"/g)]
        .map(match => "crates/app/src/" + match[1]));
      if (files.some(file => shaderSource.test(file.path) && !registered.has(file.path))) {
        return { ok: false, message: "A pending shader is outside the app's compile-time WGSL registry." };
      }
    }
    const originals = new Map();
    for (const change of changes) {
      if (!snapshot.inputs.has(change.path) && !files.some(file => file.path === change.path)) continue;
      if (snapshot.inputs.get(change.path) !== change.version) {
        throw new Error("Context changed: " + change.path + " before compilation.");
      }
      const file = path.join(directory, change.path);
      originals.set(change.path, await readFile(file));
      await writeFile(file, change.text);
    }
    // Keep routine mission checks clear of the game's Cargo lock when there is
    // room for a separate dependency cache. Reuse the shared cache on a full disk.
    const disk = await statfs(parent);
    if (mode === "build" && disk.bavail * disk.bsize < 4 * 1024 ** 3) {
      return { ok: false, mode, message: "The isolated game build needs at least 4 GB of free disk space." };
    }
    const target = disk.bavail * disk.bsize >= 4 * 1024 ** 3
      ? ensureDirectory(path.join(parent, "cargo-cache")) : path.dirname(state);
    const result = await runCargo(directory, target, signal, mode);
    if (!result.ok && (!result.errors.length || result.errors.some(error => error.path && !originals.has(error.path)))) {
      for (const [file, original] of originals) await writeFile(path.join(directory, file), original);
      const baseline = await runCargo(directory, target, signal, mode);
      const sameFailure = result.errors.length ? result.errors.every(error =>
        baseline.errors.some(existing => existing.path === error.path && existing.message === error.message))
        : result.shaderFailure ? result.shaderFailure === baseline.shaderFailure : result.message === baseline.message;
      if (!baseline.ok && sameFailure) {
        return { ok: false, baselineBlocked: true, message: "Native baseline does not compile: " + baseline.message };
      }
    }
    return { ...result, files, inputs: result.ok ? checkedInputs(snapshot) : undefined };
  } finally {
    try {
      const resolved = await realpath(directory);
      if (resolved === directory && path.dirname(resolved) === parent) {
        await rm(resolved, { recursive: true, force: true, maxRetries: 3 });
      }
    } catch (error) {
      process.stderr.write("Compile snapshot cleanup: " + error.message + "\n");
    }
  }
}

export function assertWorkflowCompileMatches(receipt, changes) {
  if (changes.some(change => (Object.hasOwn(receipt.inputs, change.path) && receipt.inputs[change.path] !== change.version) ||
      receipt.files.some(file => file.path === change.path && file.after !== digest(change.text)))) {
    throw new Error("Context changed: candidate contents since compilation.");
  }
}
