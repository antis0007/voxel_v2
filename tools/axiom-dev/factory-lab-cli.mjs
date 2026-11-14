// Headless machine tests: no window, no running game. Output is minimal JSON.
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { buildCommand, labExe, loadScenarios, root, runScenarios, scenarioDir, send, staleContent } from "./factory-lab.mjs";

const usage = {
  "run [id|PARITY-ID ...]": "run matching scenarios (all when none named); prints failures only",
  "list": "scenario ids with the parity rows they cover",
  "recipes [Kind]": "process table of one machine kind, or every kind that has processes",
  "kinds": "machine kinds as [id, name, flags]",
  "test x y z Kind [ticks]": "place Kind on a fresh world, apply set JSON from stdin, step; prints end state + changes",
  "eval": "request lines (JSON) on stdin against one fresh world; prints each reply's data or error",
  "build": "compile the debug executable the lab runs",
};

const stdin = () => process.stdin.isTTY ? "" : readFileSync(0, "utf8");
const unwrap = reply => reply.ok ? reply.data : { error: reply.error };

async function run() {
  const [command = "help", ...rest] = process.argv.slice(2);
  if (command === "help") return { commands: usage, scenarios: scenarioDir + "/*.json",
    expect: "path: literal | {eq,ne,lt,le,gt,ge,has,absent}; absent numbers read as 0; sum:a+b adds paths; paths index the reply data, e.g. end.out.Copper dust, changes.0 40 0.progress.1",
    steps: "any engine op (machine_test, place_machine, place_shell, set_machine, step_factory, machine, machines, fill, probe, command) + recipes; 120 steps = 1 s, max 7200 per step",
    docs: "tools/axiom-dev/FACTORY_LAB.md" };
  if (command === "build") { execSync(buildCommand, { cwd: root, stdio: ["ignore", "ignore", "inherit"] }); return { exe: labExe() }; }
  if (command === "list") return { cols: "id parity steps", rows: loadScenarios().map(s => [s.id, (s.parity ?? []).join(","), s.steps.length]) };
  if (command === "kinds") return unwrap((await send([{ op: "machine_kinds" }]))[0]);
  if (command === "recipes") return unwrap((await send([rest[0] ? { op: "recipes", kind: rest.join(" ") } : { op: "recipes" }]))[0]);
  if (command === "test") {
    const [x, y, z, kind, ticks = "0"] = rest;
    if (!kind) throw new Error("test x y z Kind [ticks]");
    const text = stdin().trim();
    return unwrap((await send([{ op: "machine_test", position: [x, y, z].map(Number), kind, ticks: Number(ticks), set: text ? JSON.parse(text) : {} }]))[0]);
  }
  if (command === "eval") {
    const requests = stdin().split("\n").map(line => line.trim()).filter(Boolean).map(line => JSON.parse(line));
    if (!requests.length) throw new Error("eval: request lines on stdin");
    return (await send(requests)).map(unwrap);
  }
  if (command === "run") {
    const wanted = new Set(rest.map(name => name.toUpperCase()));
    const scenarios = loadScenarios(s => !wanted.size || wanted.has(s.id.toUpperCase()) || (s.parity ?? []).some(id => wanted.has(id.toUpperCase())));
    if (!scenarios.length) throw new Error("No matching scenario; use list");
    const results = await runScenarios(scenarios);
    const failed = results.filter(result => !result.pass);
    const out = { pass: results.length - failed.length, fail: failed.length };
    if (failed.length) { out.cols = "step path test want got"; out.failures = Object.fromEntries(failed.map(result => [result.id, result.fail])); process.exitCode = 1; }
    const stale = staleContent();
    if (stale.length) out.stale = `exe older than ${stale.join(", ")}; run build`;
    return out;
  }
  throw new Error("Unknown command; use help");
}

run().then(out => console.log(JSON.stringify(out))).catch(error => { console.error(error.message); process.exitCode = 1; });
