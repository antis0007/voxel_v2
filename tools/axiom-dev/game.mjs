import { spawn } from "node:child_process";
import { existsSync, statSync, writeFileSync, appendFileSync, renameSync, unlinkSync, realpathSync, readFileSync } from "node:fs";
import { randomBytes, createHash } from "node:crypto";
import path from "node:path";
import { createInterface } from "node:readline";
import { root, state, ensureDirectory, maxText } from "./workspace.mjs";
import { parse } from "./syntax.mjs";
import { zeroRecipeIngredient } from "./idea-data.mjs";
import { parseVoxel } from "./voxel-model.mjs";

const name = process.platform === "win32" ? "axiom-playtest.exe" : "axiom-playtest";
// AXIOM_DEV_EXE points the workbench at any built executable, e.g. target/debug for fast iteration.
const binary = process.env.AXIOM_DEV_EXE ? path.resolve(process.env.AXIOM_DEV_EXE) : path.join(state, name);
const fallback = path.join(root, "target", "release-fast",
  process.platform === "win32" ? "minecraft_style_voxel_prototype.exe" : "minecraft_style_voxel_prototype");
const logs = [];
const pending = new Map();
const saveDirectory = path.join(state, "worlds");
const recipeDirectory = path.join(state, "recipe-preview");
let activeDirectory = saveDirectory;
let build;
let game;
let ready = false;
let sparkReady = false;
let recipeReady = false;
let built = false;
let sequence = 0;
let logSequence = 0;
let runIdentity;

function log(message) {
  logs.push({ id: ++logSequence, message: message.slice(0, 900) });
  if (logs.length > 120) logs.shift();
}

function lines(stream, onLine) {
  createInterface({ input: stream }).on("line", onLine);
}

function launch(program, args, env = process.env) {
  return spawn(program, args, { cwd: root, env, stdio: "pipe", windowsHide: true });
}

export function status() {
  return { running: Boolean(game), ready, sparkReady, recipeReady, building: Boolean(build), built: built || existsSync(binary),
    fallback: existsSync(fallback), timestamp: existsSync(binary) ? statSync(binary).mtimeMs : null,
    logs, saveDirectory: activeDirectory, run: runIdentity };
}

export function rebuild(onBuilt) {
  if (build || game) throw new Error("Close the playtest and wait for any active build");
  ensureDirectory(state);
  const candidate = path.join(state, "building-" + name);
  log("Building Axiom in release-fast…");
  const child = build = launch("cargo", ["rustc", "--locked", "--offline", "-p",
    "minecraft_style_voxel_prototype", "--profile", "release-fast", "--bin",
    "minecraft_style_voxel_prototype", "--", "-o", candidate]);
  child.stdin.end();
  lines(child.stdout, log);
  lines(child.stderr, log);
  child.once("error", error => log(error.message));
  child.once("close", async code => {
    try {
      if (code !== 0) return log("Build failed (" + code + ")");
      const { rename } = await import("node:fs/promises");
      await rename(candidate, binary);
      built = true;
      log("Build complete");
      build = undefined;
      onBuilt?.();
    } catch (error) {
      log(error.message);
    } finally {
      build = undefined;
    }
  });
}

function stageRecipes(text, saveRoot = recipeDirectory) {
  if (typeof text !== "string" || Buffer.byteLength(text) > maxText) throw new Error("Recipe draft exceeds the size limit");
  const root = parse(text, "ron");
  const stations = new Set(root.children?.filter(child => child.opener === "[").map(child => child.key));
  if (!["hand", "bench", "crystal_tech"].every(station => stations.has(station))) throw new Error("Expected Axiom's three recipe tables");
  const invalid = zeroRecipeIngredient(root);
  if (invalid) throw new Error(invalid.name + " · Set ingredient " + (invalid.index + 1) + " to a positive count in Recipes.");
  const directory = ensureDirectory(path.join(saveRoot, "datapack"));
  const target = path.join(directory, "recipes.ron");
  if (existsSync(target) && (realpathSync(target) !== target || !statSync(target).isFile())) throw new Error("Recipe preview file was redirected");
  const temp = target + "." + randomBytes(5).toString("hex");
  try {
    writeFileSync(temp, text, { flag: "wx" });
    renameSync(temp, target);
  } finally {
    if (existsSync(temp)) unlinkSync(temp);
  }
  return saveRoot;
}

export function buildRecipePreview(text, recipe) {
  if (build || game) throw new Error("Close the playtest and wait for any active build");
  if (recipe !== undefined && (typeof recipe !== "string" || !recipe.trim() || recipe.length > 160 || /[\x00-\x1f]/.test(recipe))) {
    throw new Error("Invalid recipe for the native preview");
  }
  const directory = stageRecipes(text, path.join(recipeDirectory, randomBytes(8).toString("hex")));
  rebuild(() => start(false, undefined, undefined, { directory, recipe }));
}

export function startModelPreview(asset, text) {
  if (game || build) throw new Error("Close the playtest and wait for any active build");
  if (typeof asset !== "string" || !/^[a-z0-9_-]{1,80}$/.test(asset) ||
      !existsSync(path.join(root, "assets", "models", "factory", asset + ".vox"))) {
    throw new Error("Choose an existing factory model slot");
  }
  if (typeof text !== "string" || Buffer.byteLength(text) > 16_384 || text.split(/\r?\n/, 1)[0] !== "size 8 8 8") {
    throw new Error("Expected a native 8³ factory model");
  }
  const model = parseVoxel(text);
  if (!model.cells.size) throw new Error("The model is empty");
  const directory = ensureDirectory(path.join(state, "model-preview", randomBytes(8).toString("hex")));
  const file = path.join(directory, "preview.vox");
  writeFileSync(file, text, { flag: "wx" });
  start(false, undefined, undefined, { directory, model: { asset, file } });
}

export function start(existing, recipeText, spark, preview) {
  const sparkSpeed = spark?.speed;
  if (game || build) throw new Error("A build or playtest is active");
  const candidate = existing ? fallback : binary;
  if (!existsSync(candidate)) throw new Error(existing ? "Existing release-fast executable unavailable" : "Build the playtest executable first");
  activeDirectory = sparkSpeed !== undefined
    ? ensureDirectory(path.join(state, "spell-preview", randomBytes(8).toString("hex")))
    : preview?.directory ?? (recipeText === undefined ? ensureDirectory(saveDirectory) : stageRecipes(recipeText));
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("VOXEL_")));
  env.VOXEL_SAVE_DIR = activeDirectory;
  // Presentation-only override: a live HUD placement table (docs/design/HUD_LAYOUT.md).
  if (process.env.VOXEL_HUD_LAYOUT) env.VOXEL_HUD_LAYOUT = process.env.VOXEL_HUD_LAYOUT;
  if (sparkSpeed !== undefined) env.VOXEL_FORGE_SPARK_SPEED = String(sparkSpeed);
  if (preview?.recipe) env.VOXEL_FORGE_RECIPE = preview.recipe;
  if (preview?.model) {
    env.VOXEL_FORGE_MODEL = preview.model.asset;
    env.VOXEL_FORGE_MODEL_FILE = preview.model.file;
  }
  ready = false;
  recipeReady = false;
  log(sparkSpeed !== undefined ? "Spark fixture requested at " + sparkSpeed + " m/s · " + activeDirectory
    : preview?.model ? "Opening model draft in its own playtest folder"
    : preview?.session ? "Opening isolated " + preview.session.world + " world · seed " + preview.session.seed
    : recipeText !== undefined || preview ? "Opening recipe draft in its own playtest folder"
    : existing ? "Opening existing build (clipboard commands)" : "Opening playtest");
  const trace = spark && path.join(activeDirectory, "forge-spark.jsonl");
  function record(entry) {
    if (!trace) return;
    try { appendFileSync(trace, JSON.stringify({ time: new Date().toISOString(), ...entry }) + "\n"); }
    catch (error) { log("Could not write Spark evidence: " + error.message); }
  }
  if (spark) record({ event: "launch", ...spark, binaryModifiedAt: statSync(candidate).mtime.toISOString() });
  const args = existing ? [] : ["--axiom-dev-stdin"];
  if (preview?.session) {
    const { world, seed, scale, generator, scene = "exploration", view } = preview.session;
    if (existing || !["superflat","hills","planet","nether","manyfold"].includes(world)
      || !Number.isInteger(seed) || seed < 0 || seed > 0xffffffff || ![1,0.5,0.25].includes(scale)
      || !["exploration","factory"].includes(scene)
      || (view !== undefined && (!Number.isInteger(view) || view < 4 || view > 64))
      || (generator !== undefined && (!Number.isInteger(generator) || generator < 1 || generator > 10000))) {
      throw new Error("Invalid engine session; choose a supported world, uint32 seed and voxel scale");
    }
    args.push("--benchmark-world=" + world, "--benchmark-seed=" + seed, "--benchmark-scale=" + scale,
      "--benchmark-physics", "--benchmark-motion=static", "--benchmark-warmup=3600", "--benchmark-duration=3600");
    if (generator !== undefined) args.push("--benchmark-generator-version=" + generator);
    // Optional view distance in chunks: small values load fast and fit debug builds.
    if (view !== undefined) args.push("--benchmark-view-chunks=" + view);
    if (scene === "factory") {
      // The fixture flag installs short review defaults; restore the workbench
      // lifetime afterwards so it cannot close while a model is being edited.
      args.push("--benchmark-torch-review", "--benchmark-warmup=3600", "--benchmark-duration=3600");
      env.VOXEL_TORCH_REVIEW_MODE = "factory";
      env.VOXEL_CAPTURE_DIR = path.join(activeDirectory,"captures","review");
    }
  }
  runIdentity = { id: randomBytes(8).toString("hex"), binary: candidate,
    sha256: createHash("sha256").update(readFileSync(candidate)).digest("hex"), started: new Date().toISOString(), session: preview?.session };
  const child = game = launch(candidate, args, env);
  child.stdin.on("error", error => log(error.message));
  lines(child.stderr, log);
  lines(child.stdout, line => {
    const reply = /^AXIOM_DEV\t(\d+)\t(.*)$/.exec(line);
    if (!reply) return log(line);
    const id = Number(reply[1]);
    if (!id) {
      ready = true;
      if (preview?.recipe && reply[2].startsWith("Forge recipe fixture ready")) recipeReady = true;
      if (spark && reply[2].startsWith("Forge Spark fixture ready")) {
        sparkReady = true;
        record({ event: "ready", message: reply[2] });
      }
      return log(reply[2]);
    }
    if (spark && reply[2].startsWith("Forge Spark")) {
      record({ event: reply[2].startsWith("Forge Spark #0") ? "emitted" : "fixture", message: reply[2] });
    }
    const request = pending.get(id);
    if (request) {
      clearTimeout(request.timeout);
      pending.delete(id);
      request.resolve(reply[2]);
    }
    log(reply[2]);
  });
  child.once("error", error => log(error.message));
  child.once("close", code => {
    game = undefined;
    activeDirectory = saveDirectory;
    ready = false;
    sparkReady = false;
    recipeReady = false;
    for (const { timeout, reject } of pending.values()) {
      clearTimeout(timeout);
      reject(new Error("Playtest closed before the reply"));
    }
    pending.clear();
    log("Playtest closed (" + code + ")");
  });
}

// Only this panel's disposable engine sessions may skip the save/quit flow.
export async function closeExperiment() {
  if (!game || !runIdentity?.session || !activeDirectory.startsWith(path.join(state,"engine-worlds") + path.sep)) {
    throw new Error("Only an active disposable Engine workbench world can be closed here");
  }
  const directory = activeDirectory;
  const child = game;
  await new Promise((resolve,reject)=>{
    const closed=()=>{clearTimeout(timeout);resolve();};
    const timeout=setTimeout(()=>{child.off("close",closed);reject(new Error("Experiment is still closing; inspect status before starting another"));},5000);
    child.once("close",closed);
    if(!child.kill()){clearTimeout(timeout);child.off("close",closed);reject(new Error("Could not close the experiment"));}
  });
  return {closed:true,saveDirectory:directory,scope:"Disposable session closed without requesting a save; captures and presets remain"};
}

export function command(line) {
  if (!game || !ready) throw new Error("Launch a connected playtest first");
  if (typeof line !== "string" || !/^\/[\w-]+(?: [^\r\n\t]*)?$/.test(line)
      || Buffer.byteLength(line) > (line.startsWith("/engine ") ? 32000 : 960) || pending.size >= 4) throw new Error("Invalid or busy command queue");
  const id = ++sequence;
  log("> " + (line.startsWith("/engine ") ? line.slice(0, 180) + (line.length > 180 ? "…" : "") : line));
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error("No game reply yet. Check the log before repeating the command."));
    }, 10000);
    pending.set(id, { resolve, reject, timeout });
    game.stdin.write(id + "\t" + line + "\n", error => {
      if (!error || !pending.has(id)) return;
      clearTimeout(timeout);
      pending.delete(id);
      reject(error);
    });
  });
}

