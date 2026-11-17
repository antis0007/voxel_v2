import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { blameContent, pendingProof, scenarioFile, settleProof, shouldProve, tolerantScenarios, validateScenario } from "./swarm-proof.mjs";

const step = { op: "machine_test", position: [0, 40, 0], kind: "Crusher", ticks: 600, expect: { "end.out.Tin dust": 64 } };

test("validateScenario accepts a small asserting scenario and rejects unsafe shapes", () => {
  assert.equal(validateScenario({ steps: [step] }), null);
  assert.match(validateScenario(null), /object/);
  assert.match(validateScenario({ steps: [] }), /at least one step/);
  assert.match(validateScenario({ steps: [{ op: "machine_test" }] }), /asserts nothing/);
  assert.match(validateScenario({ steps: [{ op: "../x", expect: { a: 1 } }] }), /valid op/);
  assert.match(validateScenario({ steps: [{ ...step, expect: {} }] }), /non-empty/);
  assert.match(validateScenario({ steps: [{ ...step, ticks: 1e9 }] }), /ticks/);
  assert.match(validateScenario({ steps: Array(13).fill(step) }), /more than 12/);
  assert.match(validateScenario({ steps: [{ ...step, pad: "x".repeat(7000) }] }), /larger/);
});

test("scenarioFile names by row, tags the row and cannot escape the directory", () => {
  const f = scenarioFile("../B-GT/06", { steps: [step] }, "note");
  assert.equal(f.name, "swarm-b-gt-06.json");
  const body = JSON.parse(f.body);
  assert.deepEqual(body.parity, ["../B-GT/06"]);
  assert.equal(body.steps.length, 1);
});

test("tolerantScenarios reports a broken file without losing the good ones", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "swarm-proof-"));
  writeFileSync(path.join(dir, "a.json"), JSON.stringify({ parity: ["L1"], steps: [step] }));
  writeFileSync(path.join(dir, "b.json"), "{ not json");
  writeFileSync(path.join(dir, "c.json"), JSON.stringify({ steps: [] }));
  const { scenarios, invalid } = tolerantScenarios(dir);
  assert.deepEqual(scenarios.map(s => s.id), ["a"]);
  assert.deepEqual(invalid.map(i => i.file.split("/").pop()), ["b.json", "c.json"]);
});

const landed = [
  { id: "L1", status: "playable", files: ["assets/processes.ron"] },
  { id: "L2", status: "building", files: ["crates/app/src/x.rs"] },
  { id: "L3", status: "blocked", files: ["assets/processes.ron"] },
  { id: "L4", status: "playable", files: ["assets\\processes.ron"] },
];
const scenarios = [{ id: "s1", parity: ["L1"], file: "f1" }, { id: "s1b", parity: ["L1"], file: "f1b" }, { id: "s2", parity: ["L2"], file: "f2" }, { id: "s3", parity: ["L3"], file: "f3" }];

test("pendingProof takes landed, unproven rows that have a scenario", () => {
  const p = pendingProof(landed, scenarios, new Set(["L2"]));
  assert.deepEqual([...p.keys()], ["L1"]);
  assert.equal(p.get("L1").length, 2);
});

test("settleProof: all pass proves, first failure retries with observed values, second blocks, missing result fails", () => {
  const pending = pendingProof(landed, scenarios, new Set());
  const results = [{ id: "s1", pass: true, fail: [] }, { id: "s1b", pass: true, fail: [] }, { id: "s2", pass: false, fail: [[0, "end.out.Tin dust", "eq", 64, 32]] }];
  let actions = settleProof(pending, results, new Set());
  assert.deepEqual(actions.map(a => [a.id, a.kind]), [["L1", "proven"], ["L2", "retry"]]);
  assert.match(actions[1].note, /want 64, engine gave 32/);
  actions = settleProof(pending, results, new Set(["L2"]));
  assert.equal(actions[1].kind, "blocked");
  // one of L1's two scenarios produced no result: never a pass
  actions = settleProof(pending, results.filter(r => r.id !== "s1b"), new Set());
  assert.equal(actions[0].kind, "retry");
  assert.match(actions[0].summary, /no result/);
});

test("blameContent names the table and only the unproven landed rows that edited it", () => {
  const msg = "processes: CokeOven recipe 2: resource 34 x 192 is outside the resource table or empty";
  assert.deepEqual(blameContent(msg, landed, new Set()), { table: "processes", ids: ["L1", "L4"] });
  assert.deepEqual(blameContent(msg, landed, new Set(["L1", "L4"])), { table: "processes", ids: [] });
  assert.deepEqual(blameContent("factory lab timed out after 0/3 replies", landed, new Set()), { table: null, ids: [] });
});

test("shouldProve waits for a batch unless asked or idle", () => {
  assert.equal(shouldProve({ pending: 0, batch: 3, manual: true, idle: true }), false);
  assert.equal(shouldProve({ pending: 1, batch: 3, manual: false, idle: false }), false);
  assert.equal(shouldProve({ pending: 3, batch: 3, manual: false, idle: false }), true);
  assert.equal(shouldProve({ pending: 1, batch: 3, manual: true, idle: false }), true);
  assert.equal(shouldProve({ pending: 1, batch: 3, manual: false, idle: true }), true);
});
