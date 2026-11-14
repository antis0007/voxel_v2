import { copyNativeFixture } from "./workshop-test-fixture.mjs";
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, copyFileSync, readFileSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { workshopService } from "./workshop-service.mjs";
import { workshopPath, workshopSources, reviseProcesses } from "./workshop-process.mjs";
import { fabricationRows } from "./fabrication-data.mjs";
import { nativeTuning, processSummary, productionPlan, resourceCatalog, selectProcess } from "./workshop-planning.mjs";

const source = fileURLToPath(new URL("../../", import.meta.url));
const original = readFileSync(path.join(source, workshopPath), "utf8");
const rows = fabricationRows(original), coils = rows.find(row => row.name === "Coils");
const names = resourceCatalog();
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), "axiom-production-test-"));
  t.after(() => { assert.equal(path.dirname(root), tmpdir()); rmSync(root, { recursive: true, force: true }); });
  for (const file of workshopSources) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    copyFileSync(path.join(source, file), path.join(root, file));
  }
  copyNativeFixture(source, root);
  return { root, service: workshopService(root) };
}
const design = { process: "Coils", changes: { secondsPerBatch: 4, powerKw: 12 } };

test("inspect presents native recipe quantities, time and current resource labels", t => {
  const { service } = fixture(t), report = service.inspect();
  const recipe = report.recipes.find(row => row.id === coils.id);
  assert.equal(recipe.secondsPerBatch, 5);
  assert.equal(recipe.output.items, 2);
  assert.equal(recipe.inputs.find(row => row.resource === 6).items, 2);
  assert.equal(recipe.inputs.find(row => row.resource === 6).name, "Copper ingots");
  assert.equal(recipe.energyKjPerBatch, 50);
  assert.equal(recipe.energyKjPerItem, 25);
  assert.equal(recipe.itemsPerMinute, 24);
  assert.equal(report.resources.length, 25);
  assert.equal(report.resources.find(row => row.id === 24).name, "Feathers");
  assert.equal(report.evidence.playable, false);
});

test("planner rounds machine count up and conserves per-output demand", () => {
  const plan = productionPlan(coils, 120, names);
  assert.equal(plan.machines, 5);
  assert.equal(plan.capacityItemsPerMinute, 120);
  assert.equal(plan.utilization, 1);
  assert.equal(plan.averagePowerKw, 50);
  assert.equal(plan.peakPowerKw, 50);
  assert.deepEqual(plan.inputs.map(row => [row.resource, row.itemsPerMinute]), [[6, 120], [4, 60]]);
  const partial = productionPlan(coils, 25, names);
  assert.equal(partial.machines, 2);
  assert.equal(partial.capacityItemsPerMinute, 48);
  assert.equal(partial.peakPowerKw, 20);
  assert.equal(partial.averagePowerKw, 25 * 25 / 60);
  assert.equal(partial.utilization, 25 / 48);
  assert.match(partial.scope, /not a native playtest/);
});

test("fractional batches use exact subunits without silently rounding", () => {
  const converted = nativeTuning({ secondsPerBatch: 0.25, powerKw: 1, inputs: [[6, 1 / 64]], output: [16, 1 / 64] });
  assert.deepEqual(converted, { ticks: 1, power: 1, inputs: [[6, 1]], output: [16, 1] });
  const result = reviseProcesses(original, [{ op: "tune-process", id: coils.id, changes: { inputs: [[6, 0.5]], output: [16, 1.5] } }]);
  assert.deepEqual(result.after[coils.id].inputs, [[6, 32]]);
  assert.deepEqual(result.after[coils.id].output, [16, 96]);
  assert.equal(processSummary(result.after[coils.id]).output.items, 1.5);
});

test("human units reject out-of-range, unknown, partial and imprecise values", () => {
  for (const changes of [
    {}, null, [], { secondsPerBatch: 0 }, { secondsPerBatch: 0.3 }, { secondsPerBatch: 3600.25 },
    { secondsPerBatch: "4" }, { powerKw: 65 }, { powerKw: 1.5 }, { ticks: 20 },
    { inputs: [] }, { inputs: [[6, 0.1]] }, { inputs: [[6, 0]] }, { inputs: [[6, 65]] },
    { inputs: [[25, 1]] }, { output: [6, 1, 2] }, { output: [6, Infinity] }, { output: [6.5, 1] },
  ]) assert.throws(() => nativeTuning(changes));
  assert.throws(() => reviseProcesses(original, [{ op: "tune-process", id: 0, changes: { inputs: [[6, 1], [6, 2]] } }]), /repeats/);
  assert.throws(() => reviseProcesses(original, [{ op: "tune-process", id: 0, changes: { name: "" } }]), /name/);
  assert.throws(() => reviseProcesses(original, [{ op: "tune-process", id: 0, changes: { powerKw: 12 }, extra: true }]), /fields/);
});

test("planner rejects invalid targets and resolves only full names or integer IDs", () => {
  for (const value of [0, -1, NaN, Infinity, "10", 1000001, null]) assert.throws(() => productionPlan(coils, value));
  assert.equal(selectProcess(rows, " coils "), coils);
  assert.equal(selectProcess(rows, coils.id), coils);
  for (const value of ["Co", "", "0", 0.5, {}, null, 999]) assert.throws(() => selectProcess(rows, value));
  assert.throws(() => selectProcess([coils, { ...coils, id: 100 }], "Coils"));
});

test("resource discovery uses all generated native mappings", () => {
  assert.equal(resourceCatalog().length, 25);
  assert.deepEqual(resourceCatalog().at(-1), { id: 24, name: "Feathers" });
  assert.deepEqual(nativeTuning({ output: [24, 1] }), { output: [24, 64] });
});

test("design returns one exact stage request and before/after production", t => {
  const { root, service } = fixture(t), result = service.design(design);
  assert.equal(result.status, "ready-to-stage-content");
  assert.equal(result.production[0].before.itemsPerMinute, 24);
  assert.equal(result.production[0].after.itemsPerMinute, 30);
  assert.equal(result.production[0].after.energyKjPerItem, 24);
  assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
  const staged = service.stage({ ...result.request, approval: result.approval });
  assert.equal(readFileSync(path.join(root, workshopPath), "utf8"), original);
  assert.equal(staged.manifest.originalModified, false);
  assert.equal(staged.manifest.evidence.playable, false);
  assert.deepEqual(staged.manifest.production, result.production);
  assert.match(staged.manifest.planningScope, /estimate/);
});

test("copy design appends an ID and leaves all original definitions intact", t => {
  const { service } = fixture(t), result = service.design({ ...design, copyName: "Coils trial" });
  const candidate = fabricationRows(result.candidate);
  assert.deepEqual(candidate.slice(0, rows.length), rows);
  assert.equal(candidate.at(-1).id, rows.length);
  assert.equal(candidate.at(-1).name, "Coils trial");
  assert.equal(candidate.at(-1).ticks, 16);
  assert.equal(result.production[0].before, null);
  assert.throws(() => service.design({ ...design, copyName: "Coils" }), /unique name/);
});

test("design rejects unknown intent and cannot use an old approval for a later edit", t => {
  const { service } = fixture(t), first = service.design(design);
  assert.throws(() => service.design({ ...design, publish: true }), /fields/);
  assert.throws(() => service.stage({ ...first.request, approval: "wrong" }), /exact supported/);
  const changed = structuredClone(first.request);
  changed.operations[0].changes.powerKw = 14;
  assert.throws(() => service.stage({ ...changed, approval: first.approval }), /exact supported/);
  const unchanged = service.design({ process: "Coils", changes: { secondsPerBatch: 5 } });
  assert.equal(unchanged.status, "no-change");
  assert.equal(unchanged.approval, null);
});

test("planning and CLI implementation changes invalidate prior design reviews", t => {
  for (const file of ["tools/axiom-dev/workshop-planning.mjs", "tools/axiom-dev/workshop-cli.mjs", "crates/app/src/world/expedition.rs"]) {
    const { root, service } = fixture(t), result = service.design(design);
    writeFileSync(path.join(root, file), readFileSync(path.join(root, file), "utf8") + "\n");
    assert.throws(() => service.stage({ ...result.request, approval: result.approval }), /dependencies changed|missing or stale/);
    assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
  }
});

test("plan and design CLI match service results and do not write content", t => {
  const { root, service } = fixture(t), cli = path.join(root, "tools/axiom-dev/workshop-cli.mjs");
  const run = (args, input) => spawnSync(process.execPath, [cli, ...args], { input, encoding: "utf8", windowsHide: true, timeout: 10000 });
  for (const selector of ["Coils", String(coils.id)]) {
    const result = run(["plan", selector, "120"]);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(result.stdout), service.plan({ process: "Coils", itemsPerMinute: 120 }));
  }
  const result = run(["design"], JSON.stringify(design));
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), service.design(design));
  for (const args of [["plan"], ["plan", "Coils", "0"], ["plan", "Coils", "120", "extra"], ["inspect", "extra"]]) {
    assert.equal(run(args).status, 1);
  }
  assert.equal(existsSync(path.join(root, "target/axiom-dev/workshop")), false);
  assert.equal(readFileSync(path.join(root, workshopPath), "utf8"), original);
});

test("exact capacity boundaries do not add a phantom machine", () => {
  const row = { ...coils, ticks: 7, output: [16, 5] };
  assert.equal(productionPlan(row, 18.75).machines, 7);
  assert.equal(productionPlan(row, 18.750000000000004).machines, 8);
  assert.equal(productionPlan(row, Number.MIN_VALUE).machines, 1);
  const slow = { ...coils, ticks: 14400, output: [16, 1] };
  assert.equal(productionPlan(slow, 1e6).machines, 3840000000);
});
