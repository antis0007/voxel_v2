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
const suite = { id: "context-relevance", file: "context-relevance.test.mjs", tests: 1,
  owners: ["tools/axiom-dev/agent-context.mjs"] };
export function availableCheck(paths) {
  return paths.length && paths.every(file => suite.owners.includes(file)) ? suite.id : undefined;
}
function execute(root, output) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--test", "--test-reporter=tap", "tools/axiom-dev/" + suite.file], {
      cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
      env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot,
        TEMP: tmpdir(), TMP: tmpdir(), NODE_OPTIONS: "" },
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
      const one = name => { const values = [...stdout.matchAll(new RegExp("^# " + name + " (\\d+)$", "gm"))];
        return values.length === 1 ? Number(values[0][1]) : undefined; };
      const names = [...stdout.matchAll(/^(?:not )?ok \d+ - (.+)$/gm)].map(match => match[1]);
      resolve({ code, stopped, tests: one("tests"), pass: one("pass"), fail: one("fail"),
        skipped: one("skipped"), cancelled: one("cancelled"), todo: one("todo"), names,
        stdout: { path: output + ".stdout", sha256: sha(stdout) }, stderr: { path: output + ".stderr", sha256: sha(stderr) } });
    });
  });
}

// Reusing a receipt requires the same proposal, implementation and check inputs.
// A changed harness/verifier or altered saved output cannot retain a passing badge.
export function assertRegressionFresh(receipt, run) {
  if (receipt.checkId !== suite.id || receipt.proposalHash !== sha(JSON.stringify(run.proposal))
    || !Array.isArray(receipt.toolSources) || !receipt.toolSources.length) {
    throw new Error("Regression evidence changed; run the registered check again");
  }
  const toolRoot = fileURLToPath(new URL(".", import.meta.url));
  for (const { file, sha256 } of receipt.toolSources) {
    if (!/^[\w.-]+\.mjs$/.test(file) || sha(readFileSync(path.join(toolRoot, file))) !== sha256) {
      throw new Error("Regression check input changed: " + file);
    }
  }
  for (const file of receipt.files) {
    if (read(file.path).version !== file.before) throw new Error("Regression source changed: " + file.path);
  }
  for (const result of [receipt.baseline, receipt.candidate]) {
    for (const output of [result.stdout, result.stderr]) {
      if (sha(readFileSync(output.path)) !== output.sha256) throw new Error("Regression output changed");
    }
  }
  return receipt;
}

// A fixed server-owned command and frozen harness; model-proposed check strings are never executed.
export async function verifyRegression(run, checkId) {
  if (checkId !== suite.id || availableCheck(run.sources.map(row => row.path)) !== checkId) {
    throw new Error("No registered independent check for this candidate scope");
  }
  const validity = verifyCandidate(run.proposal, run.sources, { protectTests: true });
  if (validity.status !== "Syntax checked") throw new Error("Candidate must pass saved-source syntax verification first");
  const id = randomUUID();
  const output = ensureDirectory(path.join(state, "verification", id));
  const scratch = mkdtempSync(path.join(tmpdir(), "forge-check-"));
  const toolRoot = fileURLToPath(new URL(".", import.meta.url));
  const snapshots = new Map(readdirSync(toolRoot).filter(file => file.endsWith(".mjs"))
    .map(file => [file, readFileSync(path.join(toolRoot, file))]));
  if (sha(snapshots.get("agent-checks.mjs")) !== verifierAtLoad) {
    throw new Error("Verifier changed since startup; restart Forge before checking");
  }
  const versions = new Map(run.sources.map(source => [source.path, read(source.path).version]));
  const harnessHash = sha(snapshots.get(suite.file));
  const changes = prepareProposal(run.proposal, run.sources.map(source => {
    const doc = read(source.path);
    return { ...source, doc, original: doc.text, start: source.anchor.start };
  }));
  const roots = ["baseline", "candidate"].map(name => path.join(scratch, name));
  for (const root of roots) {
    mkdirSync(path.join(root, "tools", "axiom-dev"), { recursive: true });
    for (const [file, bytes] of snapshots) writeFileSync(path.join(root, "tools", "axiom-dev", file), bytes);
  }
  for (const change of changes) {
    const file = run.sources.find(source => read(source.path).version === change.doc.version)?.path;
    if (!file || !suite.owners.includes(file)) throw new Error("Unexpected candidate file");
    writeFileSync(path.join(roots[1], file), change.text);
  }
  const baseline = await execute(roots[0], path.join(output, "baseline"));
  const candidate = await execute(roots[1], path.join(output, "candidate"));
  const harnessUnchanged = roots.every(root => sha(readFileSync(path.join(root, "tools", "axiom-dev", suite.file))) === harnessHash)
    && sha(readFileSync(path.join(toolRoot, suite.file))) === harnessHash;
  const toolsUnchanged = [...snapshots].every(([file, bytes]) => sha(readFileSync(path.join(toolRoot, file))) === sha(bytes));
  const sourceUnchanged = [...versions].every(([file, version]) => read(file).version === version);
  const complete = result => !result.stopped && result.tests === suite.tests && result.names.length === suite.tests
    && result.skipped === 0 && result.cancelled === 0 && result.todo === 0;
  const reproduced = complete(baseline) && baseline.code !== 0 && baseline.fail > 0;
  const passed = complete(candidate) && candidate.code === 0 && candidate.pass === suite.tests && candidate.fail === 0;
  const matched = JSON.stringify(baseline.names) === JSON.stringify(candidate.names);
  const receipt = { schema: "axiom.forge.regression.v1", id, runId: run.id, checkId,
    status: reproduced && passed && matched && harnessUnchanged && toolsUnchanged && sourceUnchanged ? "Regression checked" : "Unverified",
    baseline, candidate, harness: { file: suite.file, sha256: harnessHash },
    verifier: { file: "agent-checks.mjs", sha256: sha(snapshots.get("agent-checks.mjs")) },
    files: validity.files, proposalHash: sha(JSON.stringify(run.proposal)),
    toolSources: [...snapshots].map(([file, bytes]) => ({ file, sha256: sha(bytes) })),
    harnessUnchanged, toolsUnchanged, sourceUnchanged, reproduced, matched,
    scratch, mergeAuthorized: false, remaining: "Only the registered regression was executed. Broader correctness, performance and visual quality remain pending review." };
  writeFileSync(path.join(output, "receipt.json"), JSON.stringify(receipt, null, 2));
  return receipt;
}
