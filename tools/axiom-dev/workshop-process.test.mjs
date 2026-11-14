import { copyNativeFixture } from "./workshop-test-fixture.mjs";
import { test } from "node:test";
import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync, readdirSync, rmSync, existsSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { workshopService } from "./workshop-service.mjs";
import { workshopPath, workshopSources, reviseProcesses, capabilityReport, firstCampaign } from "./workshop-process.mjs";
import { stageContent } from "./workshop-stage.mjs";
import { runWorkshopCampaign } from "./workshop-campaign.mjs";
import { fabricationRows } from "./fabrication-data.mjs";
import { parse } from "./syntax.mjs";

const source = fileURLToPath(new URL("../../", import.meta.url));
const original = readFileSync(path.join(source, workshopPath), "utf8");
const initialRows = fabricationRows(original);
const nextId = initialRows.length;
const operations = [
  { op: "derive-process", sourceId: 0, name: "Workshop coil prototype" },
  { op: "set-process", id: nextId, field: "ticks", value: 24 },
];
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "axiom-workshop-test-"));
  t.after(() => { assert.equal(path.dirname(root), tmpdir()); rmSync(root, { recursive: true, force: true }); });
  for (const file of [...workshopSources, "tools/axiom-dev/workshop-cli.mjs"]) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    copyFileSync(path.join(source, file), path.join(root, file));
  }
  copyNativeFixture(source, root);
  const service = workshopService(root);
  const request = { version: service.inspect().version, requirements: ["fabricator-process", "electric-power"], operations };
  return { root, service, request };
}

test("deriving a process preserves every original row byte-for-byte", () => {
  const result = reviseProcesses(original, operations);
  const before = parse(original, "ron"), after = parse(result.text, "ron");
  for (let i = 0; i < before.children.length; i++) {
    assert.equal(result.text.slice(after.children[i].start, after.children[i].end), original.slice(before.children[i].start, before.children[i].end));
  }
  assert.equal(result.after.length, nextId + 1);
  assert.equal(result.after[nextId].ticks, 24);
  assert.equal(result.changes.length, 1);
  assert.equal(result.changes[0].before, null);
  assert.equal(result.units.ticks, "0.25 seconds");
});

test("derive works when the final row has no trailing comma", () => {
  const root = parse(original, "ron"), comma = root.children.at(-1).comma;
  const text = original.slice(0, comma) + original.slice(comma + 1);
  assert.equal(reviseProcesses(text, operations).after.length, nextId + 1);
});

test("process fields use the existing native-shaped catalog validation", () => {
  const edits = [
    { field: "inputs", value: [[6, 64], [4, 128]] },
    { field: "output", value: [16, 64] },
    { field: "power", value: 8 },
    { field: "name", value: 'Coils "prototype"' },
  ].map(edit => ({ op: "set-process", id: nextId, ...edit }));
  const result = reviseProcesses(original, [...operations, ...edits]);
  assert.deepEqual(result.after[nextId].inputs, [[6, 64], [4, 128]]);
  assert.equal(result.after[nextId].name, 'Coils "prototype"');
});

test("invalid operations cannot rename IDs or add unregistered behavior", () => {
  for (const edit of [
    { op: "delete-process", id: 0, field: "ticks", value: 10 },
    { op: "set-process", id: 0, field: "id", value: 1 },
    { op: "set-process", id: 0, field: "ticks", value: 0 },
    { op: "set-process", id: 0, field: "ticks", value: 1.5 },
    { op: "set-process", id: 0, field: "power", value: 0 },
    { op: "set-process", id: 0, field: "output", value: [999, 64] },
    { op: "set-process", id: 0, field: "inputs", value: [[6, 64], [6, 64]] },
    { op: "set-process", id: 0, field: "outputs", value: [[6, 64], [7, 64]] },
    { op: "derive-process", sourceId: 0, name: "Coils" },
    { op: "derive-process", sourceId: 0, name: "Line\nbreak" },
    { op: "derive-process", sourceId: 0, name: "Prototype", authority: true },
  ]) assert.throws(() => reviseProcesses(original, [edit]));
  assert.throws(() => reviseProcesses(original, []));
  assert.throws(() => reviseProcesses(original, Array(17).fill(operations[0])));
});

test("separator and mill report the missing primitives without pretending they exist", () => {
  for (const candidate of firstCampaign) {
    const report = capabilityReport(candidate.requirements);
    assert(report.gaps.some(row => row.id === "mechanical-drive"));
    assert(report.gaps.some(row => row.id === "playable-preview"));
  }
  assert(capabilityReport(firstCampaign[0].requirements).gaps.some(row => row.id === "multiple-outputs"));
  assert.throws(() => capabilityReport(["imagined-capability"]));
  assert.throws(() => capabilityReport(["electric-power", "electric-power"]));
});

test("preview is read-only and staging preserves originals and saved worlds", t => {
  const { root, service, request } = fixture(t);
  mkdirSync(path.join(root, "saves"));
  writeFileSync(path.join(root, "saves/world.bin"), "live save sentinel");
  const preview = service.preview(request);
  assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
  assert.equal(preview.status, "ready-to-stage-content");
  const staged = service.stage({ ...request, approval: preview.approval });
  assert.equal(readFileSync(path.join(root, workshopPath), "utf8"), original);
  assert.equal(readFileSync(path.join(root, "saves/world.bin"), "utf8"), "live save sentinel");
  assert.equal(readFileSync(path.join(staged.directory, "baseline/assets/fabrication.ron"), "utf8"), original);
  assert.equal(fabricationRows(readFileSync(path.join(staged.directory, "candidate/assets/fabrication.ron"), "utf8")).length, nextId + 1);
  assert.equal(staged.manifest.evidence.playable, false);
  assert.equal(staged.manifest.evidence.craft, "needs-playtest");
  assert.equal(staged.manifest.evidence.correctness, "runtime-execution-pending");
  assert.equal(staged.manifest.worldCopied, false);
  assert.equal(staged.manifest.originalModified, false);
});

test("wrong preview approval and modified intent cannot stage", t => {
  const { service, request } = fixture(t), preview = service.preview(request);
  assert.throws(() => service.stage({ ...request, approval: "wrong" }), /exact supported preview/);
  assert.throws(() => service.stage({ ...request, operations: [operations[0]], approval: preview.approval }), /exact supported preview/);
  assert.throws(() => service.stage({ ...request, live: true, approval: preview.approval }), /fields/);
});

test("content and native dependency revisions invalidate old previews", t => {
  for (const file of [workshopPath, "crates/app/src/world/factory/recipes.rs", "tools/axiom-dev/workshop-stage.mjs"]) {
    const { root, service, request } = fixture(t), preview = service.preview(request);
    writeFileSync(path.join(root, file), readFileSync(path.join(root, file), "utf8") + "\n");
    assert.throws(() => service.stage({ ...request, approval: preview.approval }), /dependencies changed|missing or stale/);
    assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
  }
});

test("a dependency change during staging discards only its private partial folder", t => {
  const { root, service, request } = fixture(t), preview = service.preview(request);
  assert.throws(() => stageContent(root, preview, () => ({ version: "changed" })), /during staging/);
  assert.deepEqual(readdirSync(path.join(root, "target/axiom-dev/workshop")), []);
  assert.equal(readFileSync(path.join(root, workshopPath), "utf8"), original);
});

test("a redirected staging directory is rejected", t => {
  const { root, service, request } = fixture(t), preview = service.preview(request);
  const outside = mkdtempSync(path.join(tmpdir(), "axiom-workshop-outside-"));
  t.after(() => { assert.equal(path.dirname(outside), tmpdir()); rmSync(outside, { recursive: true, force: true }); });
  symlinkSync(outside, path.join(root, "target/axiom-dev/workshop"), process.platform === "win32" ? "junction" : "dir");
  assert.throws(() => service.stage({ ...request, approval: preview.approval }), /redirect/);
  assert.deepEqual(readdirSync(outside), []);
});

test("campaign stages no more than its explicit budget without model calls", t => {
  const { service, request } = fixture(t);
  const report = runWorkshopCampaign(service, { version: request.version, budget: { maxCandidates: 3, maxStages: 1 },
    candidates: ["first", "second"].map(id => ({ id, requirements: request.requirements, operations })) });
  assert.equal(report.stop, "budget-exhausted");
  assert.equal(report.staged, 1);
  assert.equal(report.attempted, 1);
  assert.equal(report.modelCalls, 0);
  assert.equal(report.tokens, 0);
});

test("campaign stops at the first authority boundary, before staging later candidates", t => {
  const { root, service, request } = fixture(t);
  const report = runWorkshopCampaign(service, { version: request.version, budget: { maxCandidates: 3, maxStages: 2 }, candidates: [
    { ...firstCampaign[0], operations: [] },
    { id: "fallback", requirements: request.requirements, operations },
  ] });
  assert.equal(report.stop, "authority-required");
  assert.equal(report.staged, 0);
  assert.equal(report.attempted, 1);
  assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
});

test("campaign rejects unbounded budgets and duplicate candidate identity", t => {
  const { service, request } = fixture(t);
  for (const budget of [{ maxCandidates: 4, maxStages: 1 }, { maxCandidates: 1, maxStages: 2 }, { maxCandidates: 0, maxStages: 0 }]) {
    assert.throws(() => runWorkshopCampaign(service, { version: request.version, budget, candidates: [] }));
  }
  const candidate = { id: "duplicate", requirements: request.requirements, operations };
  assert.throws(() => runWorkshopCampaign(service, { version: request.version, budget: { maxCandidates: 2, maxStages: 2 }, candidates: [candidate, candidate] }));
});

test("campaign suppresses duplicate content and reports no remaining candidate", t => {
  const { service, request } = fixture(t);
  const report = runWorkshopCampaign(service, { version: request.version, budget: { maxCandidates: 3, maxStages: 3 },
    candidates: ["first", "duplicate"].map(id => ({ id, requirements: request.requirements, operations })) });
  assert.equal(report.stop, "no-remaining-candidate");
  assert.equal(report.staged, 1);
  assert.equal(report.outcomes[1].status, "no-change");
});

test("unchanged content does not produce a stageable approval", t => {
  const { service, request } = fixture(t);
  const result = service.preview({ ...request, operations: [{ op: "set-process", id: 0, field: "ticks", value: initialRows[0].ticks }] });
  assert.equal(result.status, "no-change");
  assert.equal(result.approval, null);
});

test("CLI inspects from its own checkout and rejects unsupported commands", t => {
  const { root } = fixture(t);
  const cli = path.join(root, "tools/axiom-dev/workshop-cli.mjs");
  const result = spawnSync(process.execPath, [cli, "inspect"], { encoding: "utf8", windowsHide: true, timeout: 5000 });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).evidence.playable, false);
  const rejected = spawnSync(process.execPath, [cli, "publish"], { input: "{}", encoding: "utf8", windowsHide: true, timeout: 5000 });
  assert.equal(rejected.status, 1);
  assert.match(rejected.stderr, /help/);
});

test("campaign failure preserves earlier completed stages and stops later candidates", t => {
  const { service, request } = fixture(t);
  const report = runWorkshopCampaign(service, { version: request.version, budget: { maxCandidates: 3, maxStages: 3 }, candidates: [
    { id: "first", requirements: request.requirements, operations },
    { id: "invalid", requirements: request.requirements, operations: [{ op: "set-process", id: 0, field: "ticks", value: 0 }] },
    { id: "later", requirements: request.requirements, operations },
  ] });
  assert.equal(report.stop, "candidate-failed");
  assert.equal(report.staged, 1);
  assert.equal(report.attempted, 2);
  assert(existsSync(path.join(report.outcomes[0].directory, "manifest.json")));
  assert.equal(report.outcomes[1].id, "invalid");
  assert.match(report.outcomes[1].error, /ticks/);
});

test("campaign stale-dependency failure retains its existing stage receipt", t => {
  const { root, service, request } = fixture(t);
  let previews = 0;
  const changingService = { ...service, preview(command) {
    if (++previews === 2) writeFileSync(path.join(root, workshopPath), original + "\n");
    return service.preview(command);
  } };
  const report = runWorkshopCampaign(changingService, { version: request.version, budget: { maxCandidates: 2, maxStages: 2 },
    candidates: ["first", "stale"].map(id => ({ id, requirements: request.requirements, operations })) });
  assert.equal(report.stop, "candidate-failed");
  assert.equal(report.staged, 1);
  assert.equal(report.outcomes[0].status, "content-staged");
  assert.match(report.outcomes[1].error, /dependencies changed|missing or stale/);
});
