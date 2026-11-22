import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ensureDirectory, read, state } from "./workspace.mjs";
import { prepareProposal } from "./agent-contract.mjs";
import { verifyCandidate } from "./agent-evidence.mjs";

const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const verifierAtLoad = sha(readFileSync(fileURLToPath(import.meta.url)));
const suites = [
  { id: "context-relevance", file: "context-relevance.test.mjs", tests: 1,
    owner: "tools/axiom-dev/agent-context.mjs" },
  { id: "mesh-budget", file: "mesh-budget-regression.rs", tests: 4, native: true,
    owner: "crates/app/src/render/streaming/mesh_budget.rs" },
];
export function availableCheck(paths) {
  return paths.length ? suites.find(suite => paths.every(file => file === suite.owner))?.id : undefined;
}
export function toolingCheckOwners() {
  return suites.filter(suite => suite.owner.startsWith("tools/axiom-dev/")).map(suite => suite.owner);
}
export function candidateCheck(run) {
  const edited = new Set(run.proposal?.edits.map(edit => edit.source));
  return availableCheck(run.sources.filter(source => edited.has(source.id)).map(source => source.path));
}
function execute(program, args, root, output, parse, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, {
      cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"], env,
    });
    let stdout = "", stderr = "", stopped = false;
    const timer = setTimeout(() => { stopped = true; child.kill(); }, 30000);
    const collect = (key, chunk) => {
      if (key === "stdout") stdout += chunk; else stderr += chunk;
      if (stdout.length + stderr.length > 512000) { stopped = true; child.kill(); }
    };
    child.stdout.on("data", data => collect("stdout", data));
    child.stderr.on("data", data => collect("stderr", data));
    child.once("error", error => { clearTimeout(timer); reject(error); });
    child.once("close", code => {
      clearTimeout(timer);
      writeFileSync(output + ".stdout", stdout); writeFileSync(output + ".stderr", stderr);
      resolve({ code, stopped, ...parse(stdout),
        stdout: { path: output + ".stdout", sha256: sha(stdout) }, stderr: { path: output + ".stderr", sha256: sha(stderr) } });
    });
  });
}

function tap(stdout) {
  const one = name => { const values = [...stdout.matchAll(new RegExp("^# " + name + " (\\d+)$", "gm"))];
    return values.length === 1 ? Number(values[0][1]) : undefined; };
  return { tests: one("tests"), pass: one("pass"), fail: one("fail"), skipped: one("skipped"),
    cancelled: one("cancelled"), todo: one("todo"),
    names: [...stdout.matchAll(/^(?:not )?ok \d+ - (.+)$/gm)].map(match => match[1]) };
}

const growthTest = "mesh_budget::fifo_regression::growth_can_finish_when_fifo_head_cannot_fit";
function rustTests(stdout) {
  const summary = /(\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out/.exec(stdout);
  const rows = [...stdout.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)\r?$/gm)];
  return { tests: rows.length, names: rows.map(row => row[1]), pass: summary && Number(summary[1]),
    fail: summary && Number(summary[2]), skipped: summary && Number(summary[3]) + Number(summary[4]),
    cancelled: 0, todo: 0, reproduced: rows.filter(row => row[2] === "FAILED").map(row => row[1]).join() === growthTest
      && stdout.includes("FIFO blocked the only request that could release planning bytes") };
}

async function runSuite(suite, root, output) {
  const env = { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot, TEMP: tmpdir(), TMP: tmpdir() };
  if (!suite.native) {
    return execute(process.execPath, ["--test", "--test-reporter=tap", "tools/axiom-dev/" + suite.file],
      root, output, tap, { ...env, NODE_OPTIONS: "" });
  }
  const owner = readFileSync(path.join(root, suite.owner), "utf8");
  const harness = readFileSync(path.join(root, "tools", "axiom-dev", suite.file), "utf8");
  writeFileSync(path.join(root, "mesh_budget.rs"), owner + "\n" + harness);
  writeFileSync(path.join(root, "harness.rs"),
    'mod mesh { pub const PLANNING_BYTES: usize = 24 * 1024 * 1024; }\nmod mesh_budget;\n');
  const binary = path.join(root, process.platform === "win32" ? "mesh-budget.exe" : "mesh-budget");
  const compiler = await execute("rustc", ["--edition=2024", "--test", "harness.rs", "-o", binary], root, output + ".compile",
    () => ({}), { ...env, USERPROFILE: process.env.USERPROFILE, HOME: process.env.HOME, RUSTUP_HOME: process.env.RUSTUP_HOME });
  if (compiler.stopped || compiler.code !== 0) return { ...compiler, names: [], tests: 0, compileFailed: true };
  const result = await execute(binary, ["--test-threads=1"], root, output, rustTests, env);
  return { ...result, compiler };
}

// Reusing a receipt requires the same proposal, implementation and check inputs.
// A changed harness/verifier or altered saved output cannot retain a passing badge.
export function assertRegressionFresh(receipt, run) {
  if (receipt.checkId !== candidateCheck(run) || receipt.proposalHash !== sha(JSON.stringify(run.proposal))
    || !Array.isArray(receipt.toolSources) || !receipt.toolSources.length) {
    throw new Error("Regression evidence changed; run the registered check again");
  }
  const toolRoot = fileURLToPath(new URL(".", import.meta.url));
  for (const { file, sha256 } of receipt.toolSources) {
    if (!(file === "mesh-budget-regression.rs" || /^[\w.-]+\.mjs$/.test(file))
      || sha(readFileSync(path.join(toolRoot, file))) !== sha256) {
      throw new Error("Regression check input changed: " + file);
    }
  }
  for (const file of receipt.files) {
    if (read(file.path).version !== file.before) throw new Error("Regression source changed: " + file.path);
  }
  for (const result of [receipt.baseline, receipt.candidate]) {
    for (const output of [result.stdout, result.stderr, result.compiler?.stdout, result.compiler?.stderr].filter(Boolean)) {
      if (sha(readFileSync(output.path)) !== output.sha256) throw new Error("Regression output changed");
    }
  }
  return receipt;
}

// A fixed server-owned command and frozen harness; model-proposed check strings are never executed.
export async function verifyRegression(run, checkId) {
  const suite = suites.find(row => row.id === checkId);
  if (!suite || candidateCheck(run) !== checkId) {
    throw new Error("No registered independent check for this candidate scope");
  }
  const validity = verifyCandidate(run.proposal, run.sources, { protectTests: true });
  if (validity.status !== (suite.native ? "Checks pending" : "Syntax checked")) {
    throw new Error("Candidate must pass saved-source verification first");
  }
  const id = randomUUID();
  const output = ensureDirectory(path.join(state, "verification", id));
  const scratch = mkdtempSync(path.join(tmpdir(), "forge-check-"));
  const toolRoot = fileURLToPath(new URL(".", import.meta.url));
  const snapshots = new Map(readdirSync(toolRoot).filter(file => suite.native
    ? file === suite.file || file === "agent-checks.mjs" : file.endsWith(".mjs"))
    .map(file => [file, readFileSync(path.join(toolRoot, file))]));
  if (sha(snapshots.get("agent-checks.mjs")) !== verifierAtLoad) {
    throw new Error("Verifier changed since startup; restart Forge before checking");
  }
  const versions = new Map(run.sources.map(source => [source.path, read(source.path).version]));
  const harnessHash = sha(snapshots.get(suite.file));
  const sources = run.sources.filter(source => source.path === suite.owner);
  const changes = prepareProposal(run.proposal, sources.map(source => {
    const doc = read(source.path);
    return { ...source, doc, original: doc.text, start: source.anchor.start };
  }));
  const roots = ["baseline", "candidate"].map(name => path.join(scratch, name));
  for (const root of roots) {
    mkdirSync(path.join(root, "tools", "axiom-dev"), { recursive: true });
    for (const [file, bytes] of snapshots) writeFileSync(path.join(root, "tools", "axiom-dev", file), bytes);
    if (suite.native) {
      mkdirSync(path.dirname(path.join(root, suite.owner)), { recursive: true });
      writeFileSync(path.join(root, suite.owner), read(suite.owner).text);
    }
  }
  for (const change of changes) {
    if (change.doc.version !== versions.get(suite.owner)) throw new Error("Unexpected candidate file");
    writeFileSync(path.join(roots[1], suite.owner), change.text);
  }
  const baseline = await runSuite(suite, roots[0], path.join(output, "baseline"));
  const candidate = await runSuite(suite, roots[1], path.join(output, "candidate"));
  const harnessUnchanged = roots.every(root => sha(readFileSync(path.join(root, "tools", "axiom-dev", suite.file))) === harnessHash)
    && sha(readFileSync(path.join(toolRoot, suite.file))) === harnessHash;
  const toolsUnchanged = [...snapshots].every(([file, bytes]) => sha(readFileSync(path.join(toolRoot, file))) === sha(bytes));
  const sourceUnchanged = [...versions].every(([file, version]) => read(file).version === version);
  const complete = result => !result.stopped && result.tests === suite.tests && result.names.length === suite.tests
    && result.skipped === 0 && result.cancelled === 0 && result.todo === 0;
  const reproduced = complete(baseline) && baseline.code !== 0 && baseline.fail > 0 && (!suite.native || baseline.reproduced);
  const passed = result => complete(result) && result.code === 0 && result.pass === suite.tests && result.fail === 0;
  const matched = JSON.stringify(baseline.names) === JSON.stringify(candidate.names);
  const valid = passed(candidate) && matched && harnessUnchanged && toolsUnchanged && sourceUnchanged;
  const receipt = { schema: "axiom.forge.regression.v1", id, runId: run.id, checkId,
    status: valid && reproduced ? "Regression checked" : valid && suite.native && passed(baseline) ? "Native checks passed" : "Unverified",
    baseline, candidate, harness: { file: suite.file, sha256: harnessHash },
    verifier: { file: "agent-checks.mjs", sha256: sha(snapshots.get("agent-checks.mjs")) },
    files: validity.files, proposalHash: sha(JSON.stringify(run.proposal)),
    toolSources: [...snapshots].map(([file, bytes]) => ({ file, sha256: sha(bytes) })),
    harnessUnchanged, toolsUnchanged, sourceUnchanged, reproduced, matched,
    scratch, mergeAuthorized: false, remaining: "Only the registered regression was executed. Broader correctness, performance and visual quality remain pending review." };
  writeFileSync(path.join(output, "receipt.json"), JSON.stringify(receipt, null, 2));
  return receipt;
}
