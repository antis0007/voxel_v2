import test from "node:test";
import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

async function fixture(fn) {
  const root = mkdtempSync(path.join(tmpdir(), "forge-gate-test-"));
  try {
    const tools = path.join(root, "tools", "axiom-dev");
    mkdirSync(tools, { recursive: true });
    for (const name of readdirSync(fileURLToPath(new URL(".", import.meta.url)))) {
      if (name.endsWith(".mjs")) copyFileSync(new URL(name, import.meta.url), path.join(tools, name));
    }
    const file = "tools/axiom-dev/agent-context.mjs";
    const fixed = readFileSync(path.join(root, file), "utf8");
    const before = "if (!focusLines.has(hit.i + 1) && picked.some(line => Math.abs(line - hit.i) < 15)) continue;";
    const after = "if (!hit.test && !focusLines.has(hit.i + 1) && picked.some(line => Math.abs(line - hit.i) < 15)) continue;";
    assert(fixed.includes(after));
    writeFileSync(path.join(root, file), fixed.replace(after, before));
    const { read } = await import(pathToFileURL(path.join(tools, "workspace.mjs")).href);
    const { assertRegressionFresh, verifyRegression } = await import(pathToFileURL(path.join(tools, "agent-checks.mjs")).href);
    const doc = read(file);
    const run = replacement => ({ id: "gate-fixture", sources: [{ id: "S1", path: file, text: before,
      anchor: { start: doc.text.indexOf(before), version: doc.version } }],
      proposal: { summary: "Fixture repair", checks: ["Frozen check"],
        edits: [{ source: "S1", before, after: replacement, reason: "Frozen regression fixture" }] } });
    await fn({ assertRegressionFresh, verifyRegression, run, before, after, root, file });
  } finally {
    assert(path.resolve(root).startsWith(path.resolve(tmpdir()) + path.sep));
    rmSync(root, { recursive: true, force: true });
  }
}

test("registered check reproduces failure and verifies a candidate with an unchanged harness", () => fixture(async ({ verifyRegression, run, after }) => {
  const receipt = await verifyRegression(run(after), "context-relevance");
  assert.equal(receipt.status, "Regression checked");
  assert.equal(receipt.baseline.fail, 1);
  assert.equal(receipt.candidate.pass, 1);
  assert(receipt.harnessUnchanged && receipt.toolsUnchanged && receipt.sourceUnchanged);
  assert.equal(receipt.mergeAuthorized, false);
}));

test("syntax-only edits cannot manufacture a successful regression", () => fixture(async ({ verifyRegression, run, before }) => {
  const receipt = await verifyRegression(run(before.replace("< 15", "< 16")), "context-relevance");
  assert.equal(receipt.status, "Unverified");
  assert.equal(receipt.candidate.fail, 1);
}));

test("early process exit cannot count as a successful test run", () => fixture(async ({ verifyRegression, run }) => {
  const receipt = await verifyRegression(run("process.exit(0);"), "context-relevance");
  assert.equal(receipt.status, "Unverified");
}));

test("only registered checks and fresh saved baselines can execute", () => fixture(async ({ verifyRegression, run, after, root, file }) => {
  await assert.rejects(verifyRegression(run(after), "model-supplied-shell-command"), /registered/);
  writeFileSync(path.join(root, file), "// concurrent edit\n", { flag: "a" });
  await assert.rejects(verifyRegression(run(after), "context-relevance"), /saved-source/);
}));

test("cached regression rejects changed harness, saved output, proposal and source", () => fixture(async ({ assertRegressionFresh, verifyRegression, run, after, root, file }) => {
  const candidate = run(after), receipt = await verifyRegression(candidate, "context-relevance");
  assert.equal(assertRegressionFresh(receipt, candidate), receipt);
  const harness = path.join(root, "tools/axiom-dev/context-relevance.test.mjs");
  const bytes = readFileSync(harness);
  writeFileSync(harness, "// changed harness\n", { flag: "a" });
  assert.throws(() => assertRegressionFresh(receipt, candidate), /input changed/);
  writeFileSync(harness, bytes);
  assert.throws(() => assertRegressionFresh(receipt, { ...candidate, proposal: { ...candidate.proposal, edits: [] } }), /evidence changed/);
  const output = receipt.candidate.stdout.path, original = readFileSync(output);
  writeFileSync(output, "invented success");
  assert.throws(() => assertRegressionFresh(receipt, candidate), /output changed/);
  writeFileSync(output, original);
  writeFileSync(path.join(root, file), "// concurrent source edit\n", { flag: "a" });
  assert.throws(() => assertRegressionFresh(receipt, candidate), /input changed|source changed/);
}));
