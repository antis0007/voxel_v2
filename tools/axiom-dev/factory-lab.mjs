// Headless factory lab client. Spawns the game with `--factory-lab` (no window,
// GPU or save), sends one JSON request per line and checks scenario expectations.
// Scenario files live in tools/axiom-dev/scenarios/*.json:
//   { "id": "crusher-copper", "parity": ["A09-08"], "steps": [
//       { "op": "machine_test", "position": [0,40,0], "kind": "Crusher",
//         "set": { "energy": 5000, "input": { "Copper ore": 64 } }, "ticks": 600,
//         "expect": { "end.out.Copper dust": 128, "end.energy": { "lt": 5000 } } } ] }
// Expectation values: a literal (deep equal), or {eq,ne,lt,le,gt,ge,has,absent}.
// Each scenario starts from a fresh world. "ok": false marks a step that must fail.
import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const scenarioDir = "tools/axiom-dev/scenarios";
const labSource = path.join(root, "crates/app/src/dev_panel/factory_lab.rs");
const exeName = "minecraft_style_voxel_prototype.exe";
export const buildCommand = "cargo build --offline -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype";

// An exe older than the lab source would ignore --factory-lab and open the game.
export function labExe() {
  const minimum = statSync(labSource).mtimeMs;
  const candidates = [process.env.AXIOM_LAB_EXE, path.join(root, "target/debug", exeName), path.join(root, "target/release-fast", exeName)]
    .filter(Boolean).map(file => path.resolve(root, file)).filter(existsSync)
    .map(file => ({ file, at: statSync(file).mtimeMs })).filter(entry => entry.at >= minimum).sort((a, b) => b.at - a.at);
  if (!candidates.length) throw new Error("No executable newer than factory_lab.rs; run: " + buildCommand);
  return candidates[0].file;
}

/** Content newer than the exe is embedded stale; callers surface this so a pass is not trusted blindly. */
export function staleContent(exe = labExe()) {
  const built = statSync(exe).mtimeMs;
  return ["machines.ron", "reactions.ron", "recipes.ron"]
    .filter(name => { const file = path.join(root, "assets", name); return existsSync(file) && statSync(file).mtimeMs > built; });
}

/** Sends request objects in order; resolves with one reply per request. */
export function send(requests, { timeoutMs = 120_000 } = {}) {
  return new Promise((resolve, reject) => {
    // Tables come from disk, so processes.ron and fabrication.ron edits need no rebuild.
    const drafts = ["processes", "fabrication"].map(name => `--${name}=${path.join(root, "assets", name + ".ron")}`);
    const child = spawn(labExe(), ["--factory-lab", ...drafts], { cwd: root, stdio: ["pipe", "pipe", "ignore"], windowsHide: true });
    let buffer = "";
    const replies = [];
    const timer = setTimeout(() => { child.kill(); reject(new Error(`factory lab timed out after ${replies.length}/${requests.length} replies`)); }, timeoutMs);
    child.stdout.on("data", chunk => {
      buffer += chunk;
      let end;
      while ((end = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, end).trim();
        buffer = buffer.slice(end + 1);
        if (!line.startsWith("{")) continue;
        let reply;
        try { reply = JSON.parse(line); } catch { continue; }
        if (reply.fatal) { clearTimeout(timer); child.kill(); reject(new Error(reply.fatal)); return; }
        replies.push(reply);
      }
    });
    child.on("error", error => { clearTimeout(timer); reject(error); });
    child.on("close", () => {
      clearTimeout(timer);
      replies.length === requests.length ? resolve(replies)
        : reject(new Error(`factory lab closed after ${replies.length}/${requests.length} replies`));
    });
    child.stdin.end(requests.map(request => JSON.stringify(request)).join("\n") + "\n");
  });
}

const at = (value, dotted) => dotted.split(".").reduce((node, key) => node == null ? undefined : node[key], value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const TESTS = {
  eq: (got, want) => same(got, want), ne: (got, want) => !same(got, want),
  lt: (got, want) => got < want, le: (got, want) => got <= want, gt: (got, want) => got > want, ge: (got, want) => got >= want,
  has: (got, want) => typeof got === "string" ? got.includes(want) : Array.isArray(got) && got.some(entry => same(entry, want)),
  absent: (got, want) => (got === undefined || got === null) === want,
};

/** Failed expectations of one reply as [path, test, want, got]. Defaults are omitted by the engine, so absent numbers read as 0. */
export function check(data, expect = {}) {
  const failures = [];
  for (const [key, rule] of Object.entries(expect)) {
    // "sum:a.b+c.d" adds the numbers at several paths, for conservation checks.
    const raw = key.startsWith("sum:") ? key.slice(4).split("+").reduce((total, part) => total + (Number(at(data, part.trim())) || 0), 0) : at(data, key);
    const tests = rule !== null && typeof rule === "object" && !Array.isArray(rule) ? rule : { eq: rule };
    for (const [test, want] of Object.entries(tests)) {
      if (!TESTS[test]) { failures.push([key, test, want, "unknown test"]); continue; }
      const got = raw === undefined && typeof want === "number" ? 0 : raw;
      if (!TESTS[test](got, want)) failures.push([key, test, want, raw ?? null]);
    }
  }
  return failures;
}

export function loadScenarios(filter = () => true) {
  const dir = path.join(root, scenarioDir);
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter(name => name.endsWith(".json")).sort().map(name => {
    const scenario = JSON.parse(readFileSync(path.join(dir, name), "utf8"));
    if (!Array.isArray(scenario.steps) || !scenario.steps.length) throw new Error(name + ": steps required");
    return { ...scenario, id: scenario.id ?? name.replace(/\.json$/, ""), file: `${scenarioDir}/${name}` };
  }).filter(filter);
}

/** Runs scenarios in one process. Result rows: {id, file, parity, pass, fail:[[step, path, test, want, got]]}. */
export async function runScenarios(scenarios) {
  const requests = [];
  const plan = [];
  for (const scenario of scenarios) {
    requests.push({ op: "reset" });
    plan.push(null);
    scenario.steps.forEach((step, index) => {
      const { expect, ok, note, ...request } = step;
      requests.push(request);
      plan.push({ scenario, index, expect, ok: ok ?? true });
    });
  }
  const replies = await send(requests);
  const results = new Map(scenarios.map(s => [s.id, { id: s.id, file: s.file, parity: s.parity ?? [], pass: true, fail: [] }]));
  replies.forEach((reply, i) => {
    const step = plan[i];
    if (!step) return;
    const result = results.get(step.scenario.id);
    if (reply.ok !== step.ok) result.fail.push([step.index, "ok", "eq", step.ok, reply.error ?? reply.ok]);
    else for (const failure of check(reply.data, step.expect)) result.fail.push([step.index, ...failure]);
  });
  for (const result of results.values()) result.pass = result.fail.length === 0;
  return [...results.values()];
}
